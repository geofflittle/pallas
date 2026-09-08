//! Time a chain of `leios-fetch` requests against a Leios relay.
//!
//! `leios-fetch` serves one request per peer at a time, so fetching an endorser
//! block is a chain of round trips: the next request cannot be issued until the
//! previous reply has arrived. This program makes that chain explicit and times
//! each link of it, so the cost of a fetch can be attributed to the peer, to the
//! network, or to the client.
//!
//! Two modes:
//!
//! - `--mode discover` connects, listens for `leios-notify` block offers, and
//!   prints the endorser block ids it was offered, one per line. Use it once to
//!   produce the target list.
//! - `--mode replay` reads that list and issues one `FetchEb` per target,
//!   strictly sequentially, each issued when the previous reply arrives. It
//!   prints one line per request carrying the outcome and the timings, then a
//!   summary.
//!
//! Every request ends in exactly one of three recorded outcomes: a reply for the
//! endorser block that was asked for, a reply for a different one, or no reply
//! before the deadline. A request that was never issued because the chain
//! stopped is counted separately and is never silently absent from the summary.
//!
//! ```sh
//! cargo run --release -p leios-fetch-timing -- --mode discover --count 12
//! cargo run --release -p leios-fetch-timing -- --mode replay --targets ebs.txt
//! ```

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use pallas_codec::minicbor::{Decoder, data::Type};
use pallas_crypto::hash::Hasher;
use pallas_network2::{
    Manager, PeerId,
    behavior::{
        AnyMessage,
        initiator::{
            Config as HandshakeConfig, HandshakeBehavior, InitiatorBehavior, InitiatorCommand,
            InitiatorEvent,
        },
    },
    interface::TcpInterface,
    protocol::{
        EbId, Point,
        handshake::n2n::{LEIOS_MIN_VERSION, VersionTable},
        leiosfetch, leiosnotify,
    },
};
use tokio::select;

const DEFAULT_RELAY: &str = "leios-node.play.dev.cardano.org:3001";
const DEFAULT_MAGIC: u64 = 164;

/// What a client's periodic `Housekeeping` command costs a queued fetch is the
/// whole point of the measurement, so the period is a parameter rather than a
/// constant. One second is what a follower built on this stack was using.
const DEFAULT_TICK_MS: u64 = 1000;

/// How long a single request may wait for its reply before it is recorded as
/// having produced no reply at all.
const DEFAULT_TIMEOUT_MS: u64 = 30_000;

/// How long to wait for the handshake before giving up on the whole run.
const CONNECT_TIMEOUT_MS: u64 = 60_000;

/// An upper bound on the whole run, so a mode that waits for the relay to say
/// something ends on its own rather than being killed from outside.
const DEFAULT_RUN_SECS: u64 = 300;

#[derive(Clone, Debug)]
enum Mode {
    Discover { count: usize },
    Replay { targets: Vec<Point> },
}

#[derive(Clone, Debug)]
struct Args {
    relay: String,
    magic: u64,
    tick_ms: u64,
    timeout_ms: u64,
    run_secs: u64,
    label: String,
    dump: Option<String>,
    mode: Mode,
}

/// The outcome of one issued request. Each arm carries the evidence for itself:
/// there is no arm that means "nothing went wrong", and no timing is reported
/// for a request that produced no reply.
#[derive(Debug)]
enum Outcome {
    /// The relay answered with the endorser block that was asked for.
    Served {
        elapsed_ms: f64,
        bytes: usize,
        txs: usize,
        digest: String,
    },
    /// The relay answered, but for a different endorser block.
    WrongBlock { elapsed_ms: f64, got: String },
    /// The deadline passed with no reply.
    NoReply { waited_ms: f64 },
}

#[derive(Debug)]
struct Row {
    idx: usize,
    target: Point,
    issued_unix_ms: u128,
    outcome: Outcome,
}

struct Run {
    args: Args,
    network: Manager<TcpInterface<AnyMessage>, InitiatorBehavior, AnyMessage>,
    peer: Option<PeerId>,
    targets: Vec<Point>,
    next: usize,
    inflight: Option<(usize, Instant, u128, Point)>,
    rows: Vec<Row>,
    discovered: Vec<Point>,
    stopped: bool,
}

impl Run {
    fn new(args: Args) -> Self {
        let behavior = InitiatorBehavior {
            handshake: HandshakeBehavior::new(HandshakeConfig {
                supported_version: VersionTable::v11_and_above_with_query(args.magic, false),
            }),
            ..Default::default()
        };

        let targets = match &args.mode {
            Mode::Replay { targets } => targets.clone(),
            Mode::Discover { .. } => Vec::new(),
        };

        Self {
            network: Manager::new(TcpInterface::new(), behavior),
            args,
            peer: None,
            targets,
            next: 0,
            inflight: None,
            rows: Vec::new(),
            discovered: Vec::new(),
            stopped: false,
        }
    }

    /// Issues the next queued target, if there is one and nothing is in flight.
    fn issue_next(&mut self) {
        if self.inflight.is_some() || self.stopped {
            return;
        }

        // Discover mode issues nothing, and an empty target list there is the
        // normal state rather than the end of a chain.
        if matches!(self.args.mode, Mode::Discover { .. }) {
            return;
        }

        let Some(pid) = self.peer.clone() else {
            return;
        };

        let Some(target) = self.targets.get(self.next).cloned() else {
            self.stopped = true;
            return;
        };

        let idx = self.next;
        self.next += 1;

        let issued_unix_ms = unix_ms();
        let started = Instant::now();
        self.inflight = Some((idx, started, issued_unix_ms, target.clone()));

        // The timestamp is taken as close to the command as it can be, because
        // the gap this program exists to measure opens on the far side of it.
        println!(
            "ISSUE idx={idx} target={} issued_unix_ms={issued_unix_ms}",
            fmt_eb(&target)
        );
        self.network
            .execute(InitiatorCommand::FetchEb(pid, target.clone()));
    }

    fn on_body(&mut self, eb: EbId, body: &[u8]) {
        let Some((idx, started, issued_unix_ms, target)) = self.inflight.take() else {
            println!("UNSOLICITED eb={} bytes={}", fmt_eb(&eb), body.len());
            return;
        };

        let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;

        let outcome = if eb == target {
            if let Some(dir) = &self.args.dump {
                let path = format!("{dir}/{idx:03}.eb");
                if let Err(e) = std::fs::write(&path, body) {
                    eprintln!("could not write {path}: {e}");
                }
            }
            Outcome::Served {
                elapsed_ms,
                bytes: body.len(),
                txs: eb_tx_count(body),
                digest: hex::encode(Hasher::<256>::hash(body)),
            }
        } else {
            Outcome::WrongBlock {
                elapsed_ms,
                got: fmt_eb(&eb),
            }
        };

        self.record(Row {
            idx,
            target,
            issued_unix_ms,
            outcome,
        });
        self.issue_next();
    }

    fn on_deadline(&mut self) {
        let Some((idx, started, issued_unix_ms, target)) = self.inflight.take() else {
            return;
        };

        self.record(Row {
            idx,
            target,
            issued_unix_ms,
            outcome: Outcome::NoReply {
                waited_ms: started.elapsed().as_secs_f64() * 1000.0,
            },
        });

        // A peer that did not answer leaves the leios-fetch state machine
        // waiting, so the chain cannot continue on this connection. Stopping
        // here keeps the untried targets counted rather than reported as if
        // they had been tried and gone well.
        self.stopped = true;
    }

    fn record(&mut self, row: Row) {
        match &row.outcome {
            Outcome::Served {
                elapsed_ms,
                bytes,
                txs,
                digest,
            } => println!(
                "FETCH idx={} target={} outcome=served issued_unix_ms={} elapsed_ms={:.3} bytes={} txs={} digest={}",
                row.idx,
                fmt_eb(&row.target),
                row.issued_unix_ms,
                elapsed_ms,
                bytes,
                txs,
                digest
            ),
            Outcome::WrongBlock { elapsed_ms, got } => println!(
                "FETCH idx={} target={} outcome=wrong_block issued_unix_ms={} elapsed_ms={:.3} got={}",
                row.idx,
                fmt_eb(&row.target),
                row.issued_unix_ms,
                elapsed_ms,
                got
            ),
            Outcome::NoReply { waited_ms } => println!(
                "FETCH idx={} target={} outcome=no_reply issued_unix_ms={} waited_ms={:.3}",
                row.idx,
                fmt_eb(&row.target),
                row.issued_unix_ms,
                waited_ms
            ),
        }
        self.rows.push(row);
    }

    fn handle_event(&mut self, event: InitiatorEvent) {
        match event {
            InitiatorEvent::PeerInitialized(pid, (version, _)) => {
                let leios = version >= LEIOS_MIN_VERSION;
                println!("PEER pid={pid} version={version} leios={leios}");
                if !leios {
                    println!("FATAL peer negotiated a pre-Leios version {version}");
                    self.stopped = true;
                    return;
                }
                self.peer = Some(pid);
                self.issue_next();
            }
            InitiatorEvent::EbNotification(_, leiosnotify::Notification::BlockOffer(eb, size)) => {
                println!("OFFER eb={} size={}", fmt_eb(&eb), size);
                if let Mode::Discover { count } = self.args.mode
                    && self.discovered.len() < count
                {
                    self.discovered.push(eb);
                    if self.discovered.len() == count {
                        self.stopped = true;
                    }
                }
            }
            InitiatorEvent::EbFetched(_, eb, leiosfetch::Response::Block(body)) => {
                let bytes = body.raw_bytes().to_vec();
                self.on_body(eb, &bytes);
            }
            InitiatorEvent::EbFetched(_, eb, leiosfetch::Response::BlockTxs { txs }) => {
                println!("UNEXPECTED_TXS eb={} count={}", fmt_eb(&eb), txs.len());
            }
            other => tracing::debug!(?other, "unhandled event"),
        }
    }

    async fn run(&mut self) -> i32 {
        let peer: PeerId = self
            .args
            .relay
            .parse()
            .expect("relay should be host:port");
        self.network.execute(InitiatorCommand::IncludePeer(peer));

        let mut tick = tokio::time::interval(Duration::from_millis(self.args.tick_ms));
        let connect_deadline = Instant::now() + Duration::from_millis(CONNECT_TIMEOUT_MS);
        let run_deadline = Instant::now() + Duration::from_secs(self.args.run_secs);

        while !self.stopped {
            // The deadline that applies right now: the request in flight has
            // its own, and before the handshake the whole run has one.
            let deadline = match &self.inflight {
                Some((_, started, _, _)) => {
                    *started + Duration::from_millis(self.args.timeout_ms)
                }
                None if self.peer.is_none() => connect_deadline,
                None => run_deadline,
            };

            select! {
                _ = tick.tick() => {
                    self.network.execute(InitiatorCommand::Housekeeping);
                }
                evt = self.network.poll_next() => {
                    if let Some(evt) = evt {
                        self.handle_event(evt);
                    }
                }
                _ = tokio::time::sleep_until(deadline.into()) => {
                    if self.peer.is_none() {
                        println!("FATAL no peer was initialized within {CONNECT_TIMEOUT_MS} ms");
                        return 2;
                    }
                    if self.inflight.is_some() {
                        self.on_deadline();
                    } else {
                        println!("RUN_DEADLINE reached after {} s", self.args.run_secs);
                        self.stopped = true;
                    }
                }
            }
        }

        self.report()
    }

    fn report(&self) -> i32 {
        match &self.args.mode {
            Mode::Discover { count } => {
                for eb in &self.discovered {
                    if let Point::Specific(slot, hash) = eb {
                        println!("TARGET {} {}", slot, hex::encode(hash));
                    }
                }
                println!(
                    "SUMMARY mode=discover asked={} offered={}",
                    count,
                    self.discovered.len()
                );
                if self.discovered.len() < *count { 1 } else { 0 }
            }
            Mode::Replay { targets } => {
                let mut served = Vec::new();
                let mut wrong = 0usize;
                let mut no_reply = 0usize;
                for row in &self.rows {
                    match &row.outcome {
                        Outcome::Served { elapsed_ms, .. } => served.push(*elapsed_ms),
                        Outcome::WrongBlock { .. } => wrong += 1,
                        Outcome::NoReply { .. } => no_reply += 1,
                    }
                }
                let not_issued = targets.len() - self.rows.len();

                println!(
                    "SUMMARY label={} tick_ms={} relay={} targets={} served={} wrong_block={} no_reply={} not_issued={}",
                    self.args.label,
                    self.args.tick_ms,
                    self.args.relay,
                    targets.len(),
                    served.len(),
                    wrong,
                    no_reply,
                    not_issued
                );

                if served.is_empty() {
                    println!("STATS n=0 no request was served, so there is no timing to report");
                    return 1;
                }

                // The first request in a chain is issued at whatever phase of
                // the client's tick it happens to land on, and every later one
                // is issued the instant its predecessor's reply arrives. Those
                // are different experiments, so they are reported apart.
                let mut sorted = served.clone();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let tail: Vec<f64> = served.iter().skip(1).copied().collect();
                let mut tail_sorted = tail.clone();
                tail_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

                println!(
                    "STATS n={} median_ms={:.3} p95_ms={:.3} min_ms={:.3} max_ms={:.3} total_ms={:.3}",
                    sorted.len(),
                    median(&sorted),
                    p95(&sorted),
                    sorted[0],
                    sorted[sorted.len() - 1],
                    served.iter().sum::<f64>()
                );
                if !tail_sorted.is_empty() {
                    println!(
                        "STATS_AFTER_FIRST n={} median_ms={:.3} p95_ms={:.3}",
                        tail_sorted.len(),
                        median(&tail_sorted),
                        p95(&tail_sorted)
                    );
                }

                if wrong + no_reply + not_issued > 0 { 1 } else { 0 }
            }
        }
    }
}

fn median(sorted: &[f64]) -> f64 {
    let n = sorted.len();
    if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    }
}

fn p95(sorted: &[f64]) -> f64 {
    let n = sorted.len();
    let idx = ((0.95 * n as f64).ceil() as usize).clamp(1, n) - 1;
    sorted[idx]
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after the epoch")
        .as_millis()
}

fn fmt_eb(eb: &Point) -> String {
    match eb {
        Point::Origin => "origin".to_string(),
        Point::Specific(slot, hash) => format!("{slot}@{}", hex::encode(hash)),
    }
}

/// Counts the transactions in an endorser block body, which is a
/// `{ tx_hash => size }` CBOR map, so its entry count is the transaction count.
fn eb_tx_count(body: &[u8]) -> usize {
    let mut d = Decoder::new(body);
    match d.map() {
        Ok(Some(n)) => n as usize,
        Ok(None) => {
            let mut n = 0;
            while !matches!(d.datatype(), Ok(Type::Break)) {
                if d.skip().is_err() || d.skip().is_err() {
                    break;
                }
                n += 1;
            }
            n
        }
        Err(_) => 0,
    }
}

fn read_targets(path: &str) -> Vec<Point> {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("could not read targets from {path}: {e}"));

    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|line| {
            let mut parts = line.split_whitespace();
            let slot: u64 = parts
                .next()
                .unwrap_or_else(|| panic!("target line has no slot: {line}"))
                .parse()
                .unwrap_or_else(|e| panic!("target line has a bad slot: {line}: {e}"));
            let hash = hex::decode(
                parts
                    .next()
                    .unwrap_or_else(|| panic!("target line has no hash: {line}")),
            )
            .unwrap_or_else(|e| panic!("target line has a bad hash: {line}: {e}"));
            Point::Specific(slot, hash)
        })
        .collect()
}

fn parse_args() -> Args {
    let mut relay = DEFAULT_RELAY.to_string();
    let mut magic = DEFAULT_MAGIC;
    let mut tick_ms = DEFAULT_TICK_MS;
    let mut timeout_ms = DEFAULT_TIMEOUT_MS;
    let mut run_secs = DEFAULT_RUN_SECS;
    let mut label = "unlabelled".to_string();
    let mut dump = None;
    let mut mode = "replay".to_string();
    let mut count = 12usize;
    let mut targets_path = "targets.txt".to_string();

    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < argv.len() {
        let take = |i: usize| -> String {
            argv.get(i + 1)
                .unwrap_or_else(|| panic!("{} needs a value", argv[i]))
                .clone()
        };
        match argv[i].as_str() {
            "--relay" => relay = take(i),
            "--magic" => magic = take(i).parse().expect("magic should be a number"),
            "--tick-ms" => tick_ms = take(i).parse().expect("tick-ms should be a number"),
            "--timeout-ms" => timeout_ms = take(i).parse().expect("timeout-ms should be a number"),
            "--run-secs" => run_secs = take(i).parse().expect("run-secs should be a number"),
            "--label" => label = take(i),
            "--dump" => dump = Some(take(i)),
            "--mode" => mode = take(i),
            "--count" => count = take(i).parse().expect("count should be a number"),
            "--targets" => targets_path = take(i),
            other => panic!("unknown argument {other}"),
        }
        i += 2;
    }

    let mode = match mode.as_str() {
        "discover" => Mode::Discover { count },
        "replay" => Mode::Replay {
            targets: read_targets(&targets_path),
        },
        other => panic!("unknown mode {other}, expected discover or replay"),
    };

    Args {
        relay,
        magic,
        tick_ms,
        timeout_ms,
        run_secs,
        label,
        dump,
        mode,
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let args = parse_args();
    if let Some(dir) = &args.dump {
        std::fs::create_dir_all(dir).unwrap_or_else(|e| panic!("could not create {dir}: {e}"));
    }
    println!(
        "START label={} relay={} magic={} tick_ms={} timeout_ms={} unix_ms={}",
        args.label,
        args.relay,
        args.magic,
        args.tick_ms,
        args.timeout_ms,
        unix_ms()
    );

    let mut run = Run::new(args);
    let code = run.run().await;
    println!("EXIT code={code} unix_ms={}", unix_ms());
    std::process::exit(code);
}
