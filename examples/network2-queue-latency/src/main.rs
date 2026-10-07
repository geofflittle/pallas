//! Queue block ranges and EB fetches to one real node and time how long each
//! queued request waits after the previous reply ends.
//!
//! The program drives `InitiatorBehavior` and `TcpInterface` with its own loop,
//! the same loop `Manager` runs, so that it sees every send the behavior emits
//! and every reply before the behavior applies it. It queues one range before
//! the handshake, collects header points with chain sync, queues all ranges back
//! to back, then collects EB offers and queues a fetch for each of them and
//! for each point given with `--eb-point`.
//!
//! ```sh
//! cargo run --release -p network2-queue-latency -- --peer 127.0.0.1:3001 \
//!     --magic 164 --intersect <slot>:<hash> --tick-ms 3000
//! ```

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use futures::StreamExt;
use pallas_codec::minicbor::Decoder;
use pallas_crypto::hash::Hasher;
use pallas_network2::Behavior;
use pallas_network2::{
    BehaviorOutput, Interface, InterfaceCommand, InterfaceEvent, PeerId,
    behavior::{
        AnyMessage,
        initiator::{
            Config as HandshakeConfig, HandshakeBehavior, InitiatorBehavior, InitiatorCommand,
            InitiatorEvent, PromotionBehavior, PromotionConfig, PromotionTag,
        },
    },
    interface::TcpInterface,
    protocol::{
        Point, blockfetch,
        handshake::{self, VersionTable, n2n::VersionData},
        leiosfetch, leiosnotify,
    },
};
use tokio::select;

#[derive(Debug)]
struct Args {
    peer: String,
    magic: u64,
    tick_ms: u64,
    run_secs: u64,
    intersect: Point,
    ranges: usize,
    range_blocks: usize,
    eb_target: usize,
    eb_window_secs: u64,
    eb_repeat: usize,
    eb_points: Vec<Point>,
    label: String,
}

fn parse_args() -> Args {
    let mut args = Args {
        peer: "127.0.0.1:3001".into(),
        magic: 164,
        tick_ms: 3000,
        run_secs: 900,
        intersect: Point::Origin,
        ranges: 24,
        range_blocks: 2,
        eb_target: 6,
        eb_window_secs: 420,
        eb_repeat: 4,
        eb_points: Vec::new(),
        label: "run".into(),
    };

    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let value = it.next().unwrap_or_else(|| panic!("{flag} needs a value"));
        match flag.as_str() {
            "--peer" => args.peer = value,
            "--magic" => args.magic = value.parse().expect("magic"),
            "--tick-ms" => args.tick_ms = value.parse().expect("tick-ms"),
            "--run-secs" => args.run_secs = value.parse().expect("run-secs"),
            "--ranges" => args.ranges = value.parse().expect("ranges"),
            "--range-blocks" => args.range_blocks = value.parse().expect("range-blocks"),
            "--eb-target" => args.eb_target = value.parse().expect("eb-target"),
            "--eb-window-secs" => args.eb_window_secs = value.parse().expect("eb-window-secs"),
            "--eb-repeat" => args.eb_repeat = value.parse().expect("eb-repeat"),
            "--label" => args.label = value,
            "--intersect" => args.intersect = parse_point(&value),
            "--eb-point" => args.eb_points.push(parse_point(&value)),
            other => panic!("unknown flag {other}"),
        }
    }

    args
}

/// A point written as `slot:hash`.
fn parse_point(value: &str) -> Point {
    let (slot, hash) = value.split_once(':').expect("slot:hash");
    Point::Specific(slot.parse().expect("slot"), hex::decode(hash).expect("hash"))
}

/// The protocol and message variant of a message, such as `KeepAlive.KeepAlive`.
fn send_label(msg: &AnyMessage) -> String {
    let text = format!("{msg:?}");
    let (outer, rest) = text.split_once('(').unwrap_or((text.as_str(), ""));
    let inner: String = rest
        .chars()
        .take_while(|c| !matches!(c, '(' | ')' | ',' | ' ' | '{'))
        .collect();
    format!("{outer}.{inner}")
}

/// The point of a post Byron header, from its slot and its hash.
fn header_point(cbor: &[u8]) -> Option<Point> {
    let mut d = Decoder::new(cbor);
    d.array().ok()?;
    d.array().ok()?;
    let _block_number = d.u64().ok()?;
    let slot = d.u64().ok()?;
    let hash = Hasher::<256>::hash(cbor);
    Some(Point::Specific(slot, hash.as_ref().to_vec()))
}

fn point_slot(point: &Point) -> u64 {
    match point {
        Point::Origin => 0,
        Point::Specific(slot, _) => *slot,
    }
}

/// Count, min, median, p90 and max of a set of durations in milliseconds.
fn distribution(values: &[f64]) -> String {
    if values.is_empty() {
        return "count=0".into();
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    let rank = |q: f64| sorted[((q * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len()) - 1];
    format!(
        "count={} min_ms={:.3} median_ms={:.3} p90_ms={:.3} max_ms={:.3}",
        sorted.len(),
        sorted[0],
        rank(0.5),
        rank(0.9),
        sorted[sorted.len() - 1]
    )
}

/// Send and reply-end times of one protocol, one request outstanding at a time.
#[derive(Default)]
struct Timeline {
    sends: Vec<Instant>,
    ends: Vec<Instant>,
}

impl Timeline {
    /// Each send after the first, minus the end of the reply before it.
    fn gaps_ms(&self) -> Vec<f64> {
        self.sends
            .iter()
            .skip(1)
            .zip(self.ends.iter())
            .map(|(send, end)| send.saturating_duration_since(*end).as_secs_f64() * 1000.0)
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Headers,
    Blocks,
    WaitOffers,
    Ebs,
    Done,
}

struct Run {
    args: Args,
    start: Instant,
    phase: Phase,
    sends: BTreeMap<String, u64>,
    points: Vec<Point>,
    probe_queued: Option<Instant>,
    accept_at: Option<Instant>,
    probe_send: Option<Instant>,
    probe_end: Option<Instant>,
    burst_at: Option<Instant>,
    blocks: Timeline,
    blocks_done_at: Option<Instant>,
    blocks_fetched: u64,
    block_bytes: u64,
    no_blocks: u64,
    offers: Vec<Point>,
    eb_burst_at: Option<Instant>,
    eb_sources: Vec<&'static str>,
    ebs: Timeline,
    ebs_total: usize,
    ebs_fetched: u64,
    ebs_done_at: Option<Instant>,
    connected: u64,
    disconnected: u64,
    errors: u64,
    bans: u64,
    banned: bool,
    last_promotion: Option<PromotionTag>,
}

impl Run {
    fn ms(&self, at: Instant) -> f64 {
        at.saturating_duration_since(self.start).as_secs_f64() * 1000.0
    }

    fn now_ms(&self) -> f64 {
        self.ms(Instant::now())
    }

    fn needed_points(&self) -> usize {
        self.args.ranges * self.args.range_blocks
    }

    fn on_command(&mut self, cmd: &InterfaceCommand<AnyMessage>) {
        let now = Instant::now();
        match cmd {
            InterfaceCommand::Send(_, msg) => {
                *self.sends.entry(send_label(msg)).or_default() += 1;
                match msg {
                    AnyMessage::BlockFetch(blockfetch::Message::RequestRange(_)) => {
                        if self.probe_send.is_none() {
                            self.probe_send = Some(now);
                            println!("PROBE_SEND t_ms={:.3}", self.ms(now));
                        } else if self.burst_at.is_some() {
                            self.blocks.sends.push(now);
                            println!("BF_SEND n={} t_ms={:.3}", self.blocks.sends.len(), self.ms(now));
                        } else {
                            println!("BF_SEND_OUTSIDE t_ms={:.3}", self.ms(now));
                        }
                    }
                    AnyMessage::LeiosFetch(leiosfetch::Message::BlockRequest(_)) => {
                        if self.eb_burst_at.is_some() {
                            self.ebs.sends.push(now);
                            let n = self.ebs.sends.len();
                            let source = self.eb_sources.get(n - 1).copied().unwrap_or("unknown");
                            println!("LF_SEND n={n} t_ms={:.3} src={source}", self.ms(now));
                        } else {
                            println!("LF_SEND_OUTSIDE t_ms={:.3}", self.ms(now));
                        }
                    }
                    _ => {}
                }
            }
            InterfaceCommand::Disconnect(pid) => {
                println!("DISCONNECT_CMD t_ms={:.3} pid={pid}", self.ms(now));
            }
            InterfaceCommand::Connect(_) => {}
        }
    }

    fn on_io(&mut self, event: &InterfaceEvent<AnyMessage>) {
        let now = Instant::now();
        match event {
            InterfaceEvent::Connected(pid) => {
                self.connected += 1;
                println!("CONNECTED t_ms={:.3} pid={pid}", self.ms(now));
            }
            InterfaceEvent::Disconnected(pid) => {
                self.disconnected += 1;
                println!("DISCONNECTED t_ms={:.3} pid={pid}", self.ms(now));
            }
            InterfaceEvent::Error(pid, err) => {
                self.errors += 1;
                println!("ERROR t_ms={:.3} pid={pid} err={err:?}", self.ms(now));
            }
            InterfaceEvent::Recv(_, msgs) => {
                for msg in msgs {
                    match msg {
                        AnyMessage::Handshake(handshake::Message::Accept(version, _)) => {
                            self.accept_at = Some(now);
                            println!("ACCEPT t_ms={:.3} version={version}", self.ms(now));
                        }
                        AnyMessage::BlockFetch(
                            end @ (blockfetch::Message::BatchDone | blockfetch::Message::NoBlocks),
                        ) => {
                            if matches!(end, blockfetch::Message::NoBlocks) {
                                self.no_blocks += 1;
                            }
                            let kind = send_label(msg);
                            if self.probe_end.is_none() {
                                self.probe_end = Some(now);
                                println!("PROBE_END t_ms={:.3} kind={kind}", self.ms(now));
                            } else if self.burst_at.is_some() {
                                self.blocks.ends.push(now);
                                println!(
                                    "BF_END n={} t_ms={:.3} kind={kind}",
                                    self.blocks.ends.len(),
                                    self.ms(now)
                                );
                            }
                        }
                        AnyMessage::LeiosFetch(
                            leiosfetch::Message::Block(_) | leiosfetch::Message::BlockTxs { .. },
                        ) if self.eb_burst_at.is_some() => {
                            self.ebs.ends.push(now);
                            println!("LF_END n={} t_ms={:.3}", self.ebs.ends.len(), self.ms(now));
                        }
                        _ => {}
                    }
                }
            }
            InterfaceEvent::Sent(..) | InterfaceEvent::Idle => {}
        }
    }

    fn on_event(&mut self, behavior: &mut InitiatorBehavior, event: InitiatorEvent) {
        match event {
            InitiatorEvent::PeerInitialized(pid, (version, _)) => {
                println!("INITIALIZED t_ms={:.3} pid={pid} version={version}", self.now_ms());
            }
            InitiatorEvent::IntersectionFound(pid, point, tip) => {
                println!(
                    "INTERSECTION t_ms={:.3} slot={} tip_slot={}",
                    self.now_ms(),
                    point_slot(&point),
                    point_slot(&tip.0)
                );
                behavior.execute(InitiatorCommand::ContinueSync(pid));
            }
            InitiatorEvent::BlockHeaderReceived(pid, header, _) => {
                if self.points.len() >= self.needed_points() {
                    return;
                }
                if let Some(point) = header_point(&header.cbor) {
                    self.points.push(point);
                }
                if self.points.len() < self.needed_points() {
                    behavior.execute(InitiatorCommand::ContinueSync(pid));
                }
            }
            InitiatorEvent::RollbackReceived(pid, point, _) => {
                let slot = point_slot(&point);
                self.points.retain(|p| point_slot(p) <= slot);
                println!("ROLLBACK t_ms={:.3} slot={slot}", self.now_ms());
                if self.points.len() < self.needed_points() {
                    behavior.execute(InitiatorCommand::ContinueSync(pid));
                }
            }
            InitiatorEvent::BlockBodyReceived(_, body) => {
                if self.burst_at.is_some() {
                    self.blocks_fetched += 1;
                    self.block_bytes += body.len() as u64;
                }
            }
            InitiatorEvent::EbNotification(_, notification) => {
                if let leiosnotify::Notification::BlockOffer(eb, _) = notification {
                    println!("EB_OFFER t_ms={:.3} slot={}", self.now_ms(), point_slot(&eb));
                    if !self.offers.contains(&eb) {
                        self.offers.push(eb);
                    }
                }
            }
            InitiatorEvent::EbFetched(_, _, response) => {
                if let leiosfetch::Response::Block(_) = response {
                    if self.eb_burst_at.is_some() {
                        self.ebs_fetched += 1;
                    }
                }
            }
            InitiatorEvent::TxRequested(..) => {}
        }
    }

    fn advance(&mut self, behavior: &mut InitiatorBehavior, pid: &PeerId) {
        let now = Instant::now();
        match self.phase {
            Phase::Headers => {
                if self.points.len() >= self.needed_points() && self.probe_end.is_some() {
                    self.burst_at = Some(now);
                    println!("BLOCK_BURST t_ms={:.3} ranges={}", self.ms(now), self.args.ranges);
                    for chunk in self.points.chunks(self.args.range_blocks) {
                        let first = chunk.first().expect("chunk").clone();
                        let last = chunk.last().expect("chunk").clone();
                        behavior.execute(InitiatorCommand::RequestBlocks((first, last)));
                    }
                    self.phase = Phase::Blocks;
                }
            }
            Phase::Blocks => {
                if self.blocks.ends.len() >= self.args.ranges {
                    self.blocks_done_at = Some(now);
                    println!("BLOCKS_DONE t_ms={:.3}", self.ms(now));
                    self.phase = Phase::WaitOffers;
                }
            }
            Phase::WaitOffers => {
                let window = Duration::from_secs(self.args.eb_window_secs);
                let enough = self.offers.len() >= self.args.eb_target;
                let late = now.saturating_duration_since(self.start) >= window;
                if enough || late {
                    let mut targets: Vec<(Point, &'static str)> =
                        self.offers.iter().map(|eb| (eb.clone(), "offered")).collect();
                    for eb in &self.args.eb_points {
                        if !self.offers.contains(eb) {
                            targets.push((eb.clone(), "seeded"));
                        }
                    }
                    if targets.is_empty() {
                        println!("NO_OFFERS t_ms={:.3}", self.ms(now));
                        self.phase = Phase::Done;
                        return;
                    }
                    self.eb_burst_at = Some(now);
                    self.ebs_total = targets.len() * self.args.eb_repeat;
                    println!(
                        "EB_BURST t_ms={:.3} offered={} seeded={} total={}",
                        self.ms(now),
                        self.offers.len(),
                        targets.len() - self.offers.len(),
                        self.ebs_total
                    );
                    for _ in 0..self.args.eb_repeat {
                        for (eb, source) in &targets {
                            self.eb_sources.push(source);
                            behavior.execute(InitiatorCommand::FetchEb(pid.clone(), eb.clone()));
                        }
                    }
                    self.phase = Phase::Ebs;
                }
            }
            Phase::Ebs => {
                if self.ebs.ends.len() >= self.ebs_total {
                    self.ebs_done_at = Some(now);
                    println!("EBS_DONE t_ms={:.3}", self.ms(now));
                    self.phase = Phase::Done;
                }
            }
            Phase::Done => {}
        }
    }

    fn observe(&mut self, behavior: &InitiatorBehavior, pid: &PeerId) {
        let now = behavior.peers.get(pid).map(|s| s.promotion());
        if now != self.last_promotion {
            println!(
                "PROMOTION t_ms={:.3} from={:?} to={:?}",
                self.now_ms(),
                self.last_promotion,
                now
            );
            self.last_promotion = now;
        }

        let banned = behavior.promotion.banned_peers.contains(pid);
        if banned && !self.banned {
            self.bans += 1;
            println!("BAN t_ms={:.3} pid={pid}", self.now_ms());
        }
        self.banned = banned;
    }

    fn summary(&self) {
        let span = |from: Option<Instant>, to: Option<Instant>| match (from, to) {
            (Some(a), Some(b)) => format!("{:.3}", b.saturating_duration_since(a).as_secs_f64() * 1000.0),
            _ => "none".into(),
        };
        let first = |from: Option<Instant>, sends: &[Instant]| span(from, sends.first().copied());

        println!(
            "SUMMARY label={} t_ms={:.3} phase={:?} tick_ms={} sends={:?}",
            self.args.label,
            self.now_ms(),
            self.phase,
            self.args.tick_ms,
            self.sends
        );
        println!(
            "SUMMARY_PROBE queued_to_send_ms={} accept_to_send_ms={}",
            span(self.probe_queued, self.probe_send),
            span(self.accept_at, self.probe_send)
        );
        println!(
            "SUMMARY_BLOCKS ranges={} range_sends={} batch_ends={} no_blocks={} blocks={} bytes={} \
             burst_to_first_send_ms={} burst_to_done_ms={}",
            self.args.ranges,
            self.blocks.sends.len(),
            self.blocks.ends.len(),
            self.no_blocks,
            self.blocks_fetched,
            self.block_bytes,
            first(self.burst_at, &self.blocks.sends),
            span(self.burst_at, self.blocks_done_at)
        );
        println!("SUMMARY_BLOCK_GAPS {}", distribution(&self.blocks.gaps_ms()));
        println!(
            "SUMMARY_EBS offers={} total={} eb_sends={} eb_ends={} ebs_fetched={} \
             burst_to_first_send_ms={} burst_to_done_ms={}",
            self.offers.len(),
            self.ebs_total,
            self.ebs.sends.len(),
            self.ebs.ends.len(),
            self.ebs_fetched,
            first(self.eb_burst_at, &self.ebs.sends),
            span(self.eb_burst_at, self.ebs_done_at)
        );
        println!("SUMMARY_EB_GAPS {}", distribution(&self.ebs.gaps_ms()));
        println!(
            "SUMMARY_PEER connected={} disconnected={} errors={} bans={} promotion={:?}",
            self.connected, self.disconnected, self.errors, self.bans, self.last_promotion
        );
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let args = parse_args();
    let pid: PeerId = args.peer.parse().expect("peer");
    let intersect = args.intersect.clone();

    // Versions 11 to 15 with peer sharing enabled, the same table as the live smoke.
    let supported_version = VersionTable {
        values: (11..=15)
            .map(|v| (v, VersionData::new(args.magic, true, Some(1), Some(false))))
            .collect(),
    };

    // One peer at most, so every request goes to the node under test.
    let promotion = PromotionBehavior::new(PromotionConfig {
        max_peers: 1,
        max_warm_peers: 1,
        max_hot_peers: 1,
        ..PromotionConfig::default()
    });

    let mut behavior = InitiatorBehavior {
        handshake: HandshakeBehavior::new(HandshakeConfig { supported_version }),
        promotion,
        ..Default::default()
    };
    let mut interface: TcpInterface<AnyMessage> = TcpInterface::new();

    let mut run = Run {
        start: Instant::now(),
        phase: Phase::Headers,
        sends: BTreeMap::new(),
        points: Vec::new(),
        probe_queued: None,
        accept_at: None,
        probe_send: None,
        probe_end: None,
        burst_at: None,
        blocks: Timeline::default(),
        blocks_done_at: None,
        blocks_fetched: 0,
        block_bytes: 0,
        no_blocks: 0,
        offers: Vec::new(),
        eb_burst_at: None,
        eb_sources: Vec::new(),
        ebs: Timeline::default(),
        ebs_total: 0,
        ebs_fetched: 0,
        ebs_done_at: None,
        connected: 0,
        disconnected: 0,
        errors: 0,
        bans: 0,
        banned: false,
        last_promotion: None,
        args,
    };

    println!("START {:?}", run.args);

    behavior.execute(InitiatorCommand::IncludePeer(pid.clone()));
    behavior.execute(InitiatorCommand::StartSync(vec![intersect.clone()]));
    run.probe_queued = Some(Instant::now());
    behavior.execute(InitiatorCommand::RequestBlocks((intersect.clone(), intersect)));

    let mut tick = tokio::time::interval(Duration::from_millis(run.args.tick_ms));
    let deadline = tokio::time::sleep(Duration::from_secs(run.args.run_secs));
    tokio::pin!(deadline);

    loop {
        select! {
            output = behavior.select_next_some() => match output {
                BehaviorOutput::InterfaceCommand(cmd) => {
                    run.on_command(&cmd);
                    interface.dispatch(cmd);
                }
                BehaviorOutput::ExternalEvent(event) => run.on_event(&mut behavior, event),
            },
            event = interface.select_next_some() => {
                run.on_io(&event);
                behavior.handle_io(event);
            }
            _ = tick.tick() => {
                println!("HOUSEKEEPING t_ms={:.3}", run.now_ms());
                behavior.execute(InitiatorCommand::Housekeeping);
            }
            _ = &mut deadline => break,
        }

        run.observe(&behavior, &pid);
        run.advance(&mut behavior, &pid);
        if run.phase == Phase::Done {
            break;
        }
    }

    run.summary();
}
