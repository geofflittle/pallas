//! Follow one real node with the network2 initiator and count what it sends.
//!
//! The program drives `InitiatorBehavior` and `TcpInterface` with its own loop,
//! the same loop `Manager` runs, so that it can see every interface command the
//! behavior emits and every interface event before the behavior applies it. It
//! counts sends per message type, confirmations per message type, and sends of
//! a type that was already queued to the same peer and not yet confirmed.
//!
//! `--arm natural` issues one `Housekeeping` per tick and one `ContinueSync` or
//! `RequestBlocks` per chain-sync event. `--arm stressed` issues each of those
//! commands twice in a row, which the public command API allows.
//!
//! ```sh
//! cargo run --release -p network2-live-smoke -- --peer 127.0.0.1:3001 \
//!     --magic 164 --arm stressed --intersect <slot>:<hash> --run-secs 600
//! ```

use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

use futures::StreamExt;
use pallas_codec::minicbor::Decoder;
use pallas_crypto::hash::Hasher;
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
        Point,
        handshake::{VersionTable, n2n::VersionData},
        leiosfetch, leiosnotify,
    },
};
use pallas_network2::Behavior;
use tokio::select;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Arm {
    Natural,
    Stressed,
}

#[derive(Debug)]
struct Args {
    peer: String,
    magic: u64,
    tick_ms: u64,
    run_secs: u64,
    arm: Arm,
    intersect: Point,
    label: String,
}

fn parse_args() -> Args {
    let mut args = Args {
        peer: "127.0.0.1:3001".into(),
        magic: 164,
        tick_ms: 1000,
        run_secs: 600,
        arm: Arm::Natural,
        intersect: Point::Origin,
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
            "--label" => args.label = value,
            "--arm" => {
                args.arm = match value.as_str() {
                    "natural" => Arm::Natural,
                    "stressed" => Arm::Stressed,
                    other => panic!("unknown arm {other}"),
                }
            }
            "--intersect" => {
                let (slot, hash) = value.split_once(':').expect("slot:hash");
                args.intersect =
                    Point::Specific(slot.parse().expect("slot"), hex::decode(hash).expect("hash"));
            }
            other => panic!("unknown flag {other}"),
        }
    }

    args
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

/// The protocol of a message.
fn recv_label(msg: &AnyMessage) -> &'static str {
    match msg {
        AnyMessage::Handshake(_) => "Handshake",
        AnyMessage::KeepAlive(_) => "KeepAlive",
        AnyMessage::ChainSync(_) => "ChainSync",
        AnyMessage::PeerSharing(_) => "PeerSharing",
        AnyMessage::BlockFetch(_) => "BlockFetch",
        AnyMessage::TxSubmission(_) => "TxSubmission",
        AnyMessage::LeiosNotify(_) => "LeiosNotify",
        AnyMessage::LeiosFetch(_) => "LeiosFetch",
    }
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

#[derive(Default)]
struct Counts {
    sends: BTreeMap<String, u64>,
    sent: BTreeMap<String, u64>,
    dup_before_sent: BTreeMap<String, u64>,
    recv: BTreeMap<&'static str, u64>,
    outstanding: HashMap<(PeerId, String), u64>,
    connect_cmds: u64,
    disconnect_cmds: u64,
    connected: u64,
    disconnected: u64,
    errors: u64,
    promotion_changes: u64,
    bans: u64,
    initialized: u64,
    intersections: u64,
    headers: u64,
    header_decode_failures: u64,
    rollbacks: u64,
    block_requests: u64,
    blocks: u64,
    block_bytes: u64,
    max_header_slot: u64,
    node_tip_slot: u64,
    eb_offers: u64,
    eb_announcements: u64,
    eb_txs_offers: u64,
    vote_batches: u64,
    eb_bodies: u64,
    eb_tx_batches: u64,
}

fn fmt_map<K: std::fmt::Display>(map: &BTreeMap<K, u64>) -> String {
    let parts: Vec<String> = map.iter().map(|(k, v)| format!("{k}={v}")).collect();
    format!("{{{}}}", parts.join(" "))
}

struct Run {
    args: Args,
    start: Instant,
    counts: Counts,
    eb_tx_counts: HashMap<Point, usize>,
    last_promotion: Option<PromotionTag>,
    banned: bool,
}

impl Run {
    fn ms(&self) -> u128 {
        self.start.elapsed().as_millis()
    }

    fn repeats(&self) -> usize {
        match self.args.arm {
            Arm::Natural => 1,
            Arm::Stressed => 2,
        }
    }

    fn on_command(&mut self, cmd: &InterfaceCommand<AnyMessage>) {
        match cmd {
            InterfaceCommand::Connect(_) => self.counts.connect_cmds += 1,
            InterfaceCommand::Disconnect(pid) => {
                self.counts.disconnect_cmds += 1;
                println!("DISCONNECT_CMD t={} pid={pid}", self.ms());
            }
            InterfaceCommand::Send(pid, msg) => {
                let label = send_label(msg);
                let t = self.ms();
                *self.counts.sends.entry(label.clone()).or_default() += 1;
                let pending = self
                    .counts
                    .outstanding
                    .entry((pid.clone(), label.clone()))
                    .or_default();
                if *pending > 0 {
                    let queued = *pending;
                    *self.counts.dup_before_sent.entry(label.clone()).or_default() += 1;
                    println!("DUP t={t} label={label} unconfirmed={queued}");
                }
                *pending += 1;
            }
        }
    }

    fn clear_outstanding(&mut self, pid: &PeerId) {
        self.counts.outstanding.retain(|(p, _), _| p != pid);
    }

    fn on_io(&mut self, event: &InterfaceEvent<AnyMessage>) {
        match event {
            InterfaceEvent::Connected(pid) => {
                self.counts.connected += 1;
                println!("CONNECTED t={} pid={pid}", self.ms());
            }
            InterfaceEvent::Disconnected(pid) => {
                self.counts.disconnected += 1;
                println!("DISCONNECTED t={} pid={pid}", self.ms());
                self.clear_outstanding(pid);
            }
            InterfaceEvent::Error(pid, err) => {
                self.counts.errors += 1;
                println!("ERROR t={} pid={pid} err={err:?}", self.ms());
                self.clear_outstanding(pid);
            }
            InterfaceEvent::Sent(pid, msg) => {
                let label = send_label(msg);
                *self.counts.sent.entry(label.clone()).or_default() += 1;
                if let Some(pending) = self.counts.outstanding.get_mut(&(pid.clone(), label)) {
                    *pending = pending.saturating_sub(1);
                }
            }
            InterfaceEvent::Recv(_, msgs) => {
                for msg in msgs {
                    *self.counts.recv.entry(recv_label(msg)).or_default() += 1;
                }
            }
            InterfaceEvent::Idle => {}
        }
    }

    fn on_event(&mut self, behavior: &mut InitiatorBehavior, event: InitiatorEvent) {
        let n = self.repeats();
        match event {
            InitiatorEvent::PeerInitialized(pid, (version, _)) => {
                self.counts.initialized += 1;
                println!("INITIALIZED t={} pid={pid} version={version}", self.ms());
            }
            InitiatorEvent::IntersectionFound(pid, point, tip) => {
                self.counts.intersections += 1;
                self.counts.node_tip_slot = point_slot(&tip.0);
                println!(
                    "INTERSECTION t={} slot={} tip_slot={}",
                    self.ms(),
                    point_slot(&point),
                    point_slot(&tip.0)
                );
                for _ in 0..n {
                    behavior.execute(InitiatorCommand::ContinueSync(pid.clone()));
                }
            }
            InitiatorEvent::BlockHeaderReceived(pid, header, tip) => {
                self.counts.headers += 1;
                self.counts.node_tip_slot = point_slot(&tip.0);
                match header_point(&header.cbor) {
                    Some(point) => {
                        self.counts.max_header_slot =
                            self.counts.max_header_slot.max(point_slot(&point));
                        for _ in 0..n {
                            self.counts.block_requests += 1;
                            behavior
                                .execute(InitiatorCommand::RequestBlocks((point.clone(), point.clone())));
                        }
                    }
                    None => self.counts.header_decode_failures += 1,
                }
                for _ in 0..n {
                    behavior.execute(InitiatorCommand::ContinueSync(pid.clone()));
                }
            }
            InitiatorEvent::RollbackReceived(pid, point, tip) => {
                self.counts.rollbacks += 1;
                self.counts.node_tip_slot = point_slot(&tip.0);
                println!(
                    "ROLLBACK t={} slot={} tip_slot={}",
                    self.ms(),
                    point_slot(&point),
                    point_slot(&tip.0)
                );
                for _ in 0..n {
                    behavior.execute(InitiatorCommand::ContinueSync(pid.clone()));
                }
            }
            InitiatorEvent::BlockBodyReceived(_, body) => {
                self.counts.blocks += 1;
                self.counts.block_bytes += body.len() as u64;
            }
            InitiatorEvent::EbNotification(pid, notification) => match notification {
                leiosnotify::Notification::BlockOffer(eb, _) => {
                    self.counts.eb_offers += 1;
                    behavior.execute(InitiatorCommand::FetchEb(pid, eb));
                }
                leiosnotify::Notification::BlockAnnouncement(_) => {
                    self.counts.eb_announcements += 1;
                }
                leiosnotify::Notification::BlockTxsOffer(eb) => {
                    self.counts.eb_txs_offers += 1;
                    if let Some(count) = self.eb_tx_counts.get(&eb).copied().filter(|c| *c > 0) {
                        let bitmaps = leiosfetch::Bitmaps::all(count.min(64));
                        behavior.execute(InitiatorCommand::FetchEbTxs(pid, eb, bitmaps));
                    }
                }
                leiosnotify::Notification::Votes(_) => self.counts.vote_batches += 1,
            },
            InitiatorEvent::EbFetched(_, eb, response) => match response {
                leiosfetch::Response::Block(body) => {
                    self.counts.eb_bodies += 1;
                    let mut d = Decoder::new(body.raw_bytes());
                    let count = d.map().ok().flatten().unwrap_or(0) as usize;
                    self.eb_tx_counts.insert(eb, count);
                }
                leiosfetch::Response::BlockTxs { .. } => self.counts.eb_tx_batches += 1,
            },
            InitiatorEvent::TxRequested(..) => {}
        }
    }

    fn observe(&mut self, behavior: &InitiatorBehavior, pid: &PeerId) {
        let now = behavior.peers.get(pid).map(|s| s.promotion());
        if now != self.last_promotion {
            self.counts.promotion_changes += 1;
            println!(
                "PROMOTION t={} from={:?} to={:?}",
                self.ms(),
                self.last_promotion,
                now
            );
            self.last_promotion = now;
        }

        let banned = behavior.promotion.banned_peers.contains(pid);
        if banned && !self.banned {
            self.counts.bans += 1;
            println!("BAN t={} pid={pid}", self.ms());
        }
        self.banned = banned;
    }

    fn line(&self, tag: &str) -> String {
        let c = &self.counts;
        format!(
            "{tag} label={} arm={:?} t={} sends={} sent={} dup_before_sent={} recv={} \
             connect_cmds={} disconnect_cmds={} connected={} disconnected={} errors={} \
             initialized={} promotion_changes={} bans={} promotion={:?} intersections={} \
             headers={} header_decode_failures={} rollbacks={} block_requests={} blocks={} \
             block_bytes={} max_header_slot={} node_tip_slot={} eb_offers={} \
             eb_announcements={} eb_txs_offers={} vote_batches={} eb_bodies={} eb_tx_batches={}",
            self.args.label,
            self.args.arm,
            self.ms(),
            fmt_map(&c.sends),
            fmt_map(&c.sent),
            fmt_map(&c.dup_before_sent),
            fmt_map(&c.recv),
            c.connect_cmds,
            c.disconnect_cmds,
            c.connected,
            c.disconnected,
            c.errors,
            c.initialized,
            c.promotion_changes,
            c.bans,
            self.last_promotion,
            c.intersections,
            c.headers,
            c.header_decode_failures,
            c.rollbacks,
            c.block_requests,
            c.blocks,
            c.block_bytes,
            c.max_header_slot,
            c.node_tip_slot,
            c.eb_offers,
            c.eb_announcements,
            c.eb_txs_offers,
            c.vote_batches,
            c.eb_bodies,
            c.eb_tx_batches,
        )
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

    // Versions 11 to 15 with peer sharing enabled, so the node runs peer sharing.
    let supported_version = VersionTable {
        values: (11..=15)
            .map(|v| (v, VersionData::new(args.magic, true, Some(1), Some(false))))
            .collect(),
    };

    // One peer at most, so a peer the node shares is connected only once the node leaves the peer set.
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
        counts: Counts::default(),
        eb_tx_counts: HashMap::new(),
        last_promotion: None,
        banned: false,
        args,
    };

    println!("START {:?}", run.args);

    behavior.execute(InitiatorCommand::IncludePeer(pid.clone()));
    behavior.execute(InitiatorCommand::StartSync(vec![intersect]));

    let mut tick = tokio::time::interval(Duration::from_millis(run.args.tick_ms));
    let mut report = tokio::time::interval(Duration::from_secs(60));
    report.tick().await;
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
                for _ in 0..run.repeats() {
                    behavior.execute(InitiatorCommand::Housekeeping);
                }
            }
            _ = report.tick() => println!("{}", run.line("STATS")),
            _ = &mut deadline => break,
        }

        run.observe(&behavior, &pid);
    }

    println!("{}", run.line("SUMMARY"));
}
