// Round 2 in flight probe, included at the end of initiator/mod.rs tests. Compiles on a7b6e5e7 and 2da54519.
// Peer a holds range 1 (Sent) with range 3 queued. V1: a's reply ends and a protocol violation follows
// in the same Recv. V2: a's reply ends, a Housekeeping runs, then the violation arrives.
// Model of the interface: a Send pushed with no Disconnect ahead of it gets its Sent.
// Prints whether range 3 reaches a second peer after a is disconnected.

trait R2Outs {
    fn into_outs(self) -> Vec<BehaviorOutput<InitiatorBehavior>>;
}

impl R2Outs for () {
    fn into_outs(self) -> Vec<BehaviorOutput<InitiatorBehavior>> {
        Vec::new()
    }
}

impl R2Outs for Vec<BehaviorOutput<InitiatorBehavior>> {
    fn into_outs(self) -> Vec<BehaviorOutput<InitiatorBehavior>> {
        self
    }
}

fn r2v_ranges(outs: &[BehaviorOutput<InitiatorBehavior>], pid: &PeerId) -> Vec<AnyMessage> {
    outs.sends()
        .filter(|&(q, m)| q == pid && is_any_range(m))
        .map(|(_, m)| m.clone())
        .collect()
}

fn r2v_write_before_disconnect(behavior: &mut InitiatorBehavior, outs: &[BehaviorOutput<InitiatorBehavior>], a: &PeerId) -> usize {
    let mut n = 0;
    for o in outs {
        match o {
            BehaviorOutput::InterfaceCommand(crate::InterfaceCommand::Disconnect(p)) if p == a => break,
            BehaviorOutput::InterfaceCommand(crate::InterfaceCommand::Send(p, m)) if p == a && is_any_range(m) => {
                behavior.handle_io(InterfaceEvent::Sent(a.clone(), m.clone()));
                n += 1;
            }
            _ => {}
        }
    }
    n
}

fn r2v_run(same_recv: bool) -> String {
    let mut behavior = InitiatorBehavior::default();
    let (a, b) = (PeerId::test(75), PeerId::test(76));
    include_and_connect(&mut behavior, &a);
    let _ = complete_handshake(&mut behavior, &a).into_outs();
    drain_outputs(&mut behavior);
    behavior.execute(InitiatorCommand::RequestBlocks(range(1)));
    behavior.execute(InitiatorCommand::RequestBlocks(range(3)));
    behavior.execute(InitiatorCommand::Housekeeping);
    let first = r2v_ranges(&drain_outputs(&mut behavior), &a);
    assert_eq!(first.len(), 1, "setup: one range to a");
    behavior.handle_io(InterfaceEvent::Sent(a.clone(), first[0].clone()));

    let bad = AnyMessage::KeepAlive(keepalive::Message::ResponseKeepAlive(7));
    let mut batch = vec![
        AnyMessage::BlockFetch(bf::Message::StartBatch),
        AnyMessage::BlockFetch(bf::Message::Block(vec![0xBE; 8])),
        AnyMessage::BlockFetch(bf::Message::BatchDone),
    ];
    let mut written = 0;
    if same_recv {
        batch.push(bad.clone());
        behavior.handle_io(InterfaceEvent::Recv(a.clone(), batch));
        let o = drain_outputs(&mut behavior);
        written += r2v_write_before_disconnect(&mut behavior, &o, &a);
    } else {
        behavior.handle_io(InterfaceEvent::Recv(a.clone(), batch));
        let o = drain_outputs(&mut behavior);
        written += r2v_write_before_disconnect(&mut behavior, &o, &a);
        behavior.execute(InitiatorCommand::Housekeeping);
        let o = drain_outputs(&mut behavior);
        written += r2v_write_before_disconnect(&mut behavior, &o, &a);
        behavior.handle_io(InterfaceEvent::Recv(a.clone(), vec![bad.clone()]));
        let o = drain_outputs(&mut behavior);
        written += r2v_write_before_disconnect(&mut behavior, &o, &a);
    }
    let promo = behavior.peers[&a].promotion;
    behavior.execute(InitiatorCommand::Housekeeping);
    let hk = drain_outputs(&mut behavior);
    let disc = hk.has_disconnect_for(&a);
    written += r2v_write_before_disconnect(&mut behavior, &hk, &a);
    behavior.handle_io(InterfaceEvent::Disconnected(a.clone()));
    drain_outputs(&mut behavior);

    include_and_connect(&mut behavior, &b);
    let mut outs = complete_handshake(&mut behavior, &b).into_outs();
    behavior.execute(InitiatorCommand::Housekeeping);
    outs.extend(drain_outputs(&mut behavior));
    let recovered = r2v_ranges(&outs, &b).iter().filter(|m| is_any_range(m)).count();
    format!(
        "same_recv={same_recv} promotion={promo:?} range3_written_to_a={written} disconnect={disc} ranges_to_other_peer={recovered}"
    )
}

#[tokio::test]
async fn probe_r2_in_flight_then_violation() {
    tokio::time::pause();
    println!("PROBE_R2V {}", r2v_run(true));
    println!("PROBE_R2V {}", r2v_run(false));
}
