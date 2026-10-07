// Review probes for d436ced2, included at the end of initiator/mod.rs tests.
// Variant b: the other peer's handshake outputs count toward recovery.
// Arm neutral: compiles on a7b6e5e7 and on d436ced2.

fn p819_ranges_to(outputs: &[BehaviorOutput<InitiatorBehavior>], pid: &PeerId) -> Vec<(Point, Point)> {
    outputs
        .sends()
        .filter(|&(p, _)| p == pid)
        .filter_map(|(_, m)| match m {
            AnyMessage::BlockFetch(bf::Message::RequestRange(r)) => Some(r.clone()),
            _ => None,
        })
        .collect()
}

fn p819_pos_disconnect(outputs: &[BehaviorOutput<InitiatorBehavior>], pid: &PeerId) -> Option<usize> {
    outputs.iter().position(|o| {
        matches!(o, BehaviorOutput::InterfaceCommand(crate::InterfaceCommand::Disconnect(p)) if p == pid)
    })
}

fn p819_pos_range(outputs: &[BehaviorOutput<InitiatorBehavior>], pid: &PeerId) -> Option<usize> {
    outputs.iter().position(|o| {
        matches!(o, BehaviorOutput::InterfaceCommand(crate::InterfaceCommand::Send(p, m)) if p == pid && is_any_range(m))
    })
}

fn p819_send_range(behavior: &mut InitiatorBehavior, pid: &PeerId, r: &(Point, Point)) {
    behavior.handle_io(InterfaceEvent::Sent(
        pid.clone(),
        AnyMessage::BlockFetch(bf::Message::RequestRange(r.clone())),
    ));
}

/// Tail shared by the ban probes. Models the interface: a Send pushed while
/// no Disconnect is ahead of it gets its Sent. A Send pushed after a
/// Disconnect for the same peer loses the writer race (tokio Mutex is FIFO,
/// the writer is shut down first) and gets no Sent.
fn p819_tail(
    tag: &str,
    behavior: &mut InitiatorBehavior,
    a: &PeerId,
    sent_early: &[(Point, Point)],
    want: &(Point, Point),
) {
    for r in sent_early {
        p819_send_range(behavior, a, r);
    }

    behavior.execute(InitiatorCommand::Housekeeping);
    let hk = drain_outputs(behavior);
    let disc = p819_pos_disconnect(&hk, a);
    let rr = p819_pos_range(&hk, a);
    if let (Some(d), Some(r)) = (disc, rr)
        && r < d
    {
        let sent = p819_ranges_to(&hk, a);
        for x in &sent {
            p819_send_range(behavior, a, x);
        }
    }
    behavior.handle_io(InterfaceEvent::Disconnected(a.clone()));
    drain_outputs(behavior);

    let b = PeerId::test(99);
    include_and_connect(behavior, &b);
    let mut outs = Vec::new();
    outs.extend(complete_handshake(behavior, &b));
    behavior.execute(InitiatorCommand::Housekeeping);
    outs.extend(drain_outputs(behavior));
    // handshake outputs were dropped above on the parent arm, so count on a
    // fresh housekeeping too and union both
    let hs_b = p819_ranges_to(&outs, &b);
    let recovered = hs_b.iter().filter(|x| *x == want).count();
    println!(
        "PROBE819 {tag} early_sends={} hk_disconnect={:?} hk_range={:?} recovered_on_other_peer={}",
        sent_early.len(),
        disc,
        rr,
        recovered
    );
}

#[tokio::test]
async fn probe819_ban_command_then_request_blocks() {
    tokio::time::pause();
    let mut behavior = InitiatorBehavior::default();
    let a = PeerId::test(81);
    include_and_connect(&mut behavior, &a);
    let _ = complete_handshake(&mut behavior, &a);
    drain_outputs(&mut behavior);

    behavior.execute(InitiatorCommand::BanPeer(a.clone()));
    drain_outputs(&mut behavior);
    behavior.execute(InitiatorCommand::RequestBlocks(range(1)));
    let out = drain_outputs(&mut behavior);
    let early = p819_ranges_to(&out, &a);
    println!(
        "PROBE819 ban_cmd promotion={:?} sends_to_banned_at_request={}",
        behavior.peers[&a].promotion,
        early.len()
    );
    p819_tail("ban_cmd", &mut behavior, &a, &early, &range(1));
}

#[tokio::test]
async fn probe819_violation_then_batch_done() {
    tokio::time::pause();
    let mut behavior = InitiatorBehavior::default();
    let a = PeerId::test(82);
    include_and_connect(&mut behavior, &a);
    let _ = complete_handshake(&mut behavior, &a);
    drain_outputs(&mut behavior);

    behavior.execute(InitiatorCommand::RequestBlocks(range(1)));
    behavior.execute(InitiatorCommand::RequestBlocks(range(3)));
    behavior.execute(InitiatorCommand::Housekeeping);
    let first = p819_ranges_to(&drain_outputs(&mut behavior), &a);
    assert_eq!(first, vec![range(1)], "first range out");
    p819_send_range(&mut behavior, &a, &range(1));

    let bad = AnyMessage::KeepAlive(keepalive::Message::ResponseKeepAlive(7));
    behavior.handle_io(InterfaceEvent::Recv(a.clone(), vec![bad]));
    drain_outputs(&mut behavior);
    let promo = behavior.peers[&a].promotion;
    let viol = behavior.peers[&a].violation;

    let batch = vec![
        AnyMessage::BlockFetch(bf::Message::StartBatch),
        AnyMessage::BlockFetch(bf::Message::Block(vec![0xBE; 8])),
        AnyMessage::BlockFetch(bf::Message::BatchDone),
    ];
    behavior.handle_io(InterfaceEvent::Recv(a.clone(), batch));
    let out = drain_outputs(&mut behavior);
    let early = p819_ranges_to(&out, &a);
    println!(
        "PROBE819 violation violation={viol} promotion={promo:?} sends_to_banned_at_batch_done={}",
        early.len()
    );
    p819_tail("violation", &mut behavior, &a, &early, &range(3));
}

#[tokio::test]
async fn probe819_demote_then_batch_done() {
    tokio::time::pause();
    let mut behavior = InitiatorBehavior::default();
    let a = PeerId::test(83);
    include_and_connect(&mut behavior, &a);
    let _ = complete_handshake(&mut behavior, &a);
    drain_outputs(&mut behavior);

    behavior.execute(InitiatorCommand::RequestBlocks(range(1)));
    behavior.execute(InitiatorCommand::RequestBlocks(range(3)));
    behavior.execute(InitiatorCommand::Housekeeping);
    let first = p819_ranges_to(&drain_outputs(&mut behavior), &a);
    assert_eq!(first, vec![range(1)], "first range out");
    p819_send_range(&mut behavior, &a, &range(1));

    behavior.execute(InitiatorCommand::DemotePeer(a.clone()));
    drain_outputs(&mut behavior);

    let batch = vec![
        AnyMessage::BlockFetch(bf::Message::StartBatch),
        AnyMessage::BlockFetch(bf::Message::Block(vec![0xBE; 8])),
        AnyMessage::BlockFetch(bf::Message::BatchDone),
    ];
    behavior.handle_io(InterfaceEvent::Recv(a.clone(), batch));
    let out = drain_outputs(&mut behavior);
    let early = p819_ranges_to(&out, &a);
    println!(
        "PROBE819 demote promotion={:?} sends_to_demoted_at_batch_done={}",
        behavior.peers[&a].promotion,
        early.len()
    );
    p819_tail("demote", &mut behavior, &a, &early, &range(3));
}

#[tokio::test]
async fn probe819_fifo_two_peers_three_ranges() {
    tokio::time::pause();
    let mut behavior = InitiatorBehavior::default();
    let (a, b) = (PeerId::test(84), PeerId::test(85));
    include_and_connect(&mut behavior, &a);
    include_and_connect(&mut behavior, &b);
    let _ = complete_handshake(&mut behavior, &a);
    let _ = complete_handshake(&mut behavior, &b);
    drain_outputs(&mut behavior);

    for n in [1, 3, 5] {
        behavior.execute(InitiatorCommand::RequestBlocks(range(n)));
    }
    behavior.execute(InitiatorCommand::Housekeeping);
    let out = drain_outputs(&mut behavior);
    let order: Vec<_> = out
        .sends()
        .filter_map(|(p, m)| match m {
            AnyMessage::BlockFetch(bf::Message::RequestRange(r)) => Some((p.clone(), r.0.slot_or_default())),
            _ => None,
        })
        .collect();
    // the peer that got range 1 finishes, the third range must go to it
    let (p1, _) = order[0].clone();
    p819_send_range(&mut behavior, &p1, &range(1));
    let other = if p1 == a { b.clone() } else { a.clone() };
    p819_send_range(&mut behavior, &other, &range(3));
    behavior.handle_io(InterfaceEvent::Recv(
        p1.clone(),
        vec![AnyMessage::BlockFetch(bf::Message::NoBlocks)],
    ));
    behavior.execute(InitiatorCommand::Housekeeping);
    let next = drain_outputs(&mut behavior);
    let to_p1 = p819_ranges_to(&next, &p1);
    let to_other = p819_ranges_to(&next, &other);
    println!(
        "PROBE819 fifo first_pass_slots={:?} next_to_finisher={:?} next_to_busy={} violations={},{}",
        order.iter().map(|(_, s)| *s).collect::<Vec<_>>(),
        to_p1.iter().map(|r| r.0.slot_or_default()).collect::<Vec<_>>(),
        to_other.len(),
        behavior.peers[&a].violation,
        behavior.peers[&b].violation
    );
}

#[tokio::test]
async fn probe819_inbound_in_unsent_window_sends_nothing() {
    tokio::time::pause();
    let mut behavior = InitiatorBehavior::default();
    let a = PeerId::test(86);
    include_and_connect(&mut behavior, &a);
    let _ = complete_handshake(&mut behavior, &a);
    drain_outputs(&mut behavior);

    behavior.execute(InitiatorCommand::RequestBlocks(range(1)));
    behavior.execute(InitiatorCommand::RequestBlocks(range(3)));
    behavior.execute(InitiatorCommand::Housekeeping);
    let out = drain_outputs(&mut behavior);
    let ka: Vec<_> = out
        .sends()
        .filter(|&(p, m)| p == &a && matches!(m, AnyMessage::KeepAlive(_)))
        .map(|(_, m)| m.clone())
        .collect();
    let first = p819_ranges_to(&out, &a).len();
    for m in &ka {
        behavior.handle_io(InterfaceEvent::Sent(a.clone(), m.clone()));
    }
    let mut during = 0;
    if let Some(AnyMessage::KeepAlive(keepalive::Message::KeepAlive(c))) = ka.first() {
        behavior.handle_io(InterfaceEvent::Recv(
            a.clone(),
            vec![AnyMessage::KeepAlive(keepalive::Message::ResponseKeepAlive(*c))],
        ));
        during = p819_ranges_to(&drain_outputs(&mut behavior), &a).len();
    }
    println!(
        "PROBE819 unsent_window first={first} keepalives={} ranges_on_inbound_before_sent={during} violation={}",
        ka.len(),
        behavior.peers[&a].violation
    );
}

#[tokio::test]
async fn probe819_requeued_range_waits_for_idle_peer() {
    tokio::time::pause();
    let mut behavior = InitiatorBehavior::default();
    let (a, b) = (PeerId::test(87), PeerId::test(88));
    include_and_connect(&mut behavior, &a);
    include_and_connect(&mut behavior, &b);
    let _ = complete_handshake(&mut behavior, &a);
    let _ = complete_handshake(&mut behavior, &b);
    drain_outputs(&mut behavior);

    behavior.execute(InitiatorCommand::RequestBlocks(range(1)));
    behavior.execute(InitiatorCommand::Housekeeping);
    let out = drain_outputs(&mut behavior);
    let x = if !p819_ranges_to(&out, &a).is_empty() { a.clone() } else { b.clone() };
    let y = if x == a { b.clone() } else { a.clone() };
    behavior.handle_io(InterfaceEvent::Disconnected(x.clone()));
    let at_disconnect = p819_ranges_to(&drain_outputs(&mut behavior), &y).len();
    behavior.execute(InitiatorCommand::Housekeeping);
    let at_housekeeping = p819_ranges_to(&drain_outputs(&mut behavior), &y).len();
    println!(
        "PROBE819 requeue y_idle={:?} to_y_at_disconnect={at_disconnect} to_y_at_next_housekeeping={at_housekeeping}",
        behavior.peers[&y].blockfetch
    );
}
