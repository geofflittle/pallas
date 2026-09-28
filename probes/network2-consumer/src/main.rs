use std::collections::HashMap;
use std::task::{Context, Poll};

use futures::StreamExt;
use pallas_network2::behavior::{
    AnyMessage, InitiatorBehavior, InitiatorCommand, InitiatorEvent, InitiatorState, PeerVisitor,
    PromotionTag,
};
use pallas_network2::behavior::responder::{ResponderBehavior, ResponderCommand};
use pallas_network2::protocol::{
    MAINNET_MAGIC, Point, blockfetch as bf, chainsync as cs, handshake, keepalive, leiosfetch as lf,
    leiosnotify as ln, txsubmission as tx,
};
use pallas_network2::{
    Behavior, BehaviorOutput, InterfaceCommand, InterfaceEvent, OutboundQueue, PeerId,
};

fn pid(n: u16) -> PeerId {
    PeerId {
        host: "10.0.0.1".into(),
        port: n,
    }
}

fn drain(b: &mut InitiatorBehavior) -> Vec<BehaviorOutput<InitiatorBehavior>> {
    let waker = futures::task::noop_waker();
    let mut cx = Context::from_waker(&waker);
    let mut out = Vec::new();
    while let Poll::Ready(Some(o)) = b.poll_next_unpin(&mut cx) {
        out.push(o);
    }
    out
}

fn sends(outs: &[BehaviorOutput<InitiatorBehavior>], p: &PeerId) -> Vec<AnyMessage> {
    outs.iter()
        .filter_map(|o| match o {
            BehaviorOutput::InterfaceCommand(InterfaceCommand::Send(q, m)) if q == p => {
                Some(m.clone())
            }
            _ => None,
        })
        .collect()
}

fn version_data() -> handshake::n2n::VersionData {
    handshake::n2n::VersionData::new(MAINNET_MAGIC, false, Some(1), Some(false))
}

fn init_peer(b: &mut InitiatorBehavior, p: &PeerId) {
    b.execute(InitiatorCommand::IncludePeer(p.clone()));
    b.execute(InitiatorCommand::Housekeeping);
    drain(b);
    b.handle_io(InterfaceEvent::Connected(p.clone()));
    let propose = sends(&drain(b), p);
    for m in propose {
        b.handle_io(InterfaceEvent::Sent(p.clone(), m));
    }
    let accept = AnyMessage::Handshake(handshake::Message::Accept(13, version_data()));
    b.handle_io(InterfaceEvent::Recv(p.clone(), vec![accept]));
    drain(b);
    assert!(b.peers[p].is_initialized(), "peer is initialized");
}

fn banned(b: &mut InitiatorBehavior, p: &PeerId) -> bool {
    b.execute(InitiatorCommand::Housekeeping);
    drain(b);
    b.peers[p].promotion() == PromotionTag::Banned
}

fn arm_normal() {
    let mut b = InitiatorBehavior::default();
    let p = pid(1);
    init_peer(&mut b, &p);

    b.execute(InitiatorCommand::RequestBlocks((Point::Origin, Point::Origin)));
    b.execute(InitiatorCommand::Housekeeping);
    let outs = drain(&mut b);
    for m in sends(&outs, &p) {
        if matches!(m, AnyMessage::BlockFetch(bf::Message::RequestRange(_))) {
            b.handle_io(InterfaceEvent::Sent(p.clone(), m));
        }
    }
    let replies = vec![
        AnyMessage::BlockFetch(bf::Message::StartBatch),
        AnyMessage::BlockFetch(bf::Message::Block(vec![1])),
        AnyMessage::BlockFetch(bf::Message::BatchDone),
    ];
    b.handle_io(InterfaceEvent::Recv(p.clone(), replies));
    let events = drain(&mut b)
        .into_iter()
        .filter(|o| {
            matches!(
                o,
                BehaviorOutput::ExternalEvent(InitiatorEvent::BlockBodyReceived(..))
            )
        })
        .count();
    println!("A normal: body_events={events} banned={}", banned(&mut b, &p));
}

fn arm_helper() {
    let mut b = InitiatorBehavior::default();
    let p = pid(2);
    init_peer(&mut b, &p);

    let InitiatorBehavior {
        blockfetch,
        outbound,
        ..
    } = &mut b;
    blockfetch.request_block_batch(&p, (Point::Origin, Point::Origin), outbound);
    for m in sends(&drain(&mut b), &p) {
        b.handle_io(InterfaceEvent::Sent(p.clone(), m));
    }
    let start = AnyMessage::BlockFetch(bf::Message::StartBatch);
    b.handle_io(InterfaceEvent::Recv(p.clone(), vec![start]));
    drain(&mut b);
    println!("B helper: banned={}", banned(&mut b, &p));
}

fn arm_on_outbound() {
    let mut b = InitiatorBehavior::default();
    let p = pid(3);
    init_peer(&mut b, &p);

    let ping = AnyMessage::KeepAlive(keepalive::Message::KeepAlive(7));
    b.on_outbound_msg(&p, &ping);
    let pong = AnyMessage::KeepAlive(keepalive::Message::ResponseKeepAlive(7));
    b.handle_io(InterfaceEvent::Recv(p.clone(), vec![pong]));
    drain(&mut b);
    println!("C on_outbound_msg: banned={}", banned(&mut b, &p));
}

fn drain_responder(b: &mut ResponderBehavior) -> Vec<BehaviorOutput<ResponderBehavior>> {
    let waker = futures::task::noop_waker();
    let mut cx = Context::from_waker(&waker);
    let mut out = Vec::new();
    while let Poll::Ready(Some(o)) = b.poll_next_unpin(&mut cx) {
        out.push(o);
    }
    out
}

fn init_responder_peer(b: &mut ResponderBehavior, p: &PeerId) {
    b.handle_io(InterfaceEvent::Connected(p.clone()));
    drain_responder(b);
    let mut values = HashMap::new();
    values.insert(13u64, version_data());
    let propose = AnyMessage::Handshake(handshake::Message::Propose(handshake::VersionTable {
        values,
    }));
    b.handle_io(InterfaceEvent::Recv(p.clone(), vec![propose]));
    for m in sends_responder(&drain_responder(b), p) {
        b.handle_io(InterfaceEvent::Sent(p.clone(), m));
    }
    drain_responder(b);
}

fn sends_responder(outs: &[BehaviorOutput<ResponderBehavior>], p: &PeerId) -> Vec<AnyMessage> {
    outs.iter()
        .filter_map(|o| match o {
            BehaviorOutput::InterfaceCommand(InterfaceCommand::Send(q, m)) if q == p => {
                Some(m.clone())
            }
            _ => None,
        })
        .collect()
}

fn responder_banned(b: &mut ResponderBehavior, p: &PeerId) -> bool {
    b.execute(ResponderCommand::Housekeeping);
    drain_responder(b).iter().any(|o| {
        matches!(o, BehaviorOutput::InterfaceCommand(InterfaceCommand::Disconnect(q)) if q == p)
    })
}

fn txsubmission_requests() -> [AnyMessage; 2] {
    [
        AnyMessage::TxSubmission(tx::Message::Init),
        AnyMessage::TxSubmission(tx::Message::RequestTxIds(true, 0, 3)),
    ]
}

fn arm_responder_on_outbound() {
    let mut b = ResponderBehavior::default();
    let p = pid(4);
    init_responder_peer(&mut b, &p);

    for m in txsubmission_requests() {
        b.outbound
            .push_ready(InterfaceCommand::Send(p.clone(), m.clone()));
        b.on_outbound_msg(&p, &m);
    }
    drain_responder(&mut b);
    let reply = AnyMessage::TxSubmission(tx::Message::ReplyTxIds(vec![]));
    b.handle_io(InterfaceEvent::Recv(p.clone(), vec![reply]));
    drain_responder(&mut b);
    println!(
        "D responder on_outbound_msg: banned={}",
        responder_banned(&mut b, &p)
    );
}

fn arm_responder_apply_msg() {
    let mut b = ResponderBehavior::default();
    let p = pid(5);
    init_responder_peer(&mut b, &p);

    for m in txsubmission_requests() {
        b.peers.get_mut(&p).unwrap().apply_msg(&m);
        b.outbound
            .push_ready(InterfaceCommand::Send(p.clone(), m.clone()));
        b.handle_io(InterfaceEvent::Sent(p.clone(), m));
    }
    drain_responder(&mut b);
    let reply = AnyMessage::TxSubmission(tx::Message::ReplyTxIds(vec![]));
    b.handle_io(InterfaceEvent::Recv(p.clone(), vec![reply]));
    drain_responder(&mut b);
    println!(
        "E responder apply_msg: banned={}",
        responder_banned(&mut b, &p)
    );
}

/// Names every public item of the initiator surface as it stands on 0ea294c1.
#[allow(dead_code, unreachable_code, unused_variables, unused_must_use)]
fn surface(b: &mut InitiatorBehavior, e: InitiatorEvent, c: InitiatorCommand) {
    let p = pid(9);
    let msg = AnyMessage::KeepAlive(keepalive::Message::KeepAlive(1));

    let mut st = InitiatorState::new();
    let _: InitiatorState = Default::default();
    let _: bool = st.is_initialized();
    let _: Option<handshake::n2n::VersionData> = st.version();
    let _: PromotionTag = st.promotion();
    let _: bool = st.supports_peer_sharing();
    let _: Option<u64> = st.accepted_version();
    let _: bool = st.supports_leios();
    st.apply_msg(&msg);
    st.reset();

    b.on_inbound_msg(&p, &msg);
    b.on_outbound_msg(&p, &msg);
    let _: &mut HashMap<PeerId, InitiatorState> = &mut b.peers;

    let mut q: OutboundQueue<InitiatorBehavior> = OutboundQueue::new();
    b.blockfetch
        .request_block_batch(&p, (Point::Origin, Point::Origin), &mut q);
    b.chainsync.request_intersection(&p, &vec![Point::Origin], &mut q);
    b.chainsync.request_next(&p, &mut st, &mut q);
    b.keepalive.send_keepalive(&p, &st, &mut q);
    b.keepalive.visit_housekeeping(&p, &mut st, &mut q);
    b.leiosfetch.visit_housekeeping(&p, &mut st, &mut q);
    b.leiosnotify.visit_housekeeping(&p, &mut st, &mut q);
    b.discovery.visit_housekeeping(&p, &mut st, &mut q);
    b.blockfetch.visit_housekeeping(&p, &mut st, &mut q);
    b.chainsync.visit_housekeeping(&p, &mut st, &mut q);
    b.chainsync.visit_tagged(&p, &mut st, &mut q);
    b.handshake.visit_connected(&p, &mut st, &mut q);
    b.keepalive.visit_outbound_msg(&p, &mut st, &mut q);

    let point = Point::Origin;
    let tip = cs::Tip(Point::Origin, 0);
    let commands = vec![
        InitiatorCommand::IncludePeer(p.clone()),
        InitiatorCommand::Housekeeping,
        InitiatorCommand::StartSync(vec![point.clone()]),
        InitiatorCommand::ContinueSync(p.clone()),
        InitiatorCommand::RequestBlocks((point.clone(), point.clone())),
        InitiatorCommand::SendTx(p.clone(), tx::EraTxId(0, vec![]), tx::EraTxBody(0, vec![])),
        InitiatorCommand::FetchEb(p.clone(), point.clone()),
        InitiatorCommand::FetchEbTxs(p.clone(), point.clone(), lf::Bitmaps::default()),
        InitiatorCommand::BanPeer(p.clone()),
        InitiatorCommand::DemotePeer(p.clone()),
    ];
    for cmd in commands {
        b.execute(cmd);
    }

    match c {
        InitiatorCommand::IncludePeer(_) => {}
        InitiatorCommand::Housekeeping => {}
        InitiatorCommand::StartSync(_) => {}
        InitiatorCommand::ContinueSync(_) => {}
        InitiatorCommand::RequestBlocks(_) => {}
        InitiatorCommand::SendTx(_, _, _) => {}
        InitiatorCommand::FetchEb(_, _) => {}
        InitiatorCommand::FetchEbTxs(_, _, _) => {}
        InitiatorCommand::BanPeer(_) => {}
        InitiatorCommand::DemotePeer(_) => {}
    }

    match e {
        InitiatorEvent::PeerInitialized(_, (_, _)) => {}
        InitiatorEvent::IntersectionFound(_, _, cs::Tip(_, _)) => {}
        InitiatorEvent::BlockHeaderReceived(_, _, _) => {}
        InitiatorEvent::RollbackReceived(_, _, _) => {}
        InitiatorEvent::BlockBodyReceived(_, _) => {}
        InitiatorEvent::TxRequested(_, _) => {}
        InitiatorEvent::EbNotification(_, ln::Notification::BlockOffer(_, _)) => {}
        InitiatorEvent::EbNotification(_, _) => {}
        InitiatorEvent::EbFetched(_, _, lf::Response::Block(_)) => {}
        InitiatorEvent::EbFetched(_, _, _) => {}
    }
    let _ = tip;
}

fn main() {
    let _ = surface as fn(&mut InitiatorBehavior, InitiatorEvent, InitiatorCommand);
    arm_normal();
    arm_helper();
    arm_on_outbound();
    arm_responder_on_outbound();
    arm_responder_apply_msg();
}
