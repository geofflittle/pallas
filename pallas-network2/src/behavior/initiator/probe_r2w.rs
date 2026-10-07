// Round 2 consumer reaction probe, included after probe_r2v.rs. Compiles on a7b6e5e7 and 2da54519.
// Peer a holds range 1 (Sent) with range 3 queued. a's batch ends, then the consumer issues
// DemotePeer or BanPeer for a (a reaction to the delivered block), then a Housekeeping runs.
// Prints whether range 3 reaches a second peer after a is disconnected.

fn r2w_run(ban: bool) -> String {
    let mut behavior = InitiatorBehavior::default();
    let (a, b) = (PeerId::test(77), PeerId::test(78));
    include_and_connect(&mut behavior, &a);
    let _ = complete_handshake(&mut behavior, &a).into_outs();
    drain_outputs(&mut behavior);
    behavior.execute(InitiatorCommand::RequestBlocks(range(1)));
    behavior.execute(InitiatorCommand::RequestBlocks(range(3)));
    behavior.execute(InitiatorCommand::Housekeeping);
    let first = r2v_ranges(&drain_outputs(&mut behavior), &a);
    assert_eq!(first.len(), 1, "setup: one range to a");
    behavior.handle_io(InterfaceEvent::Sent(a.clone(), first[0].clone()));
    let batch = vec![
        AnyMessage::BlockFetch(bf::Message::StartBatch),
        AnyMessage::BlockFetch(bf::Message::Block(vec![0xBE; 8])),
        AnyMessage::BlockFetch(bf::Message::BatchDone),
    ];
    behavior.handle_io(InterfaceEvent::Recv(a.clone(), batch));
    let o = drain_outputs(&mut behavior);
    let mut written = r2v_write_before_disconnect(&mut behavior, &o, &a);
    if ban {
        behavior.execute(InitiatorCommand::BanPeer(a.clone()));
    } else {
        behavior.execute(InitiatorCommand::DemotePeer(a.clone()));
    }
    drain_outputs(&mut behavior);
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
    let recovered = r2v_ranges(&outs, &b).len();
    format!(
        "cmd={} promotion={promo:?} range3_written_to_a={written} disconnect={disc} ranges_to_other_peer={recovered}",
        if ban { "BanPeer" } else { "DemotePeer" }
    )
}

#[tokio::test]
async fn probe_r2_batch_done_then_consumer_demotes() {
    tokio::time::pause();
    println!("PROBE_R2W {}", r2w_run(false));
    println!("PROBE_R2W {}", r2w_run(true));
}
