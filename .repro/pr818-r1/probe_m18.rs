
    #[tokio::test]
    async fn probe_m18_steps() {
        tokio::time::pause();
        let mut b = InitiatorBehavior::default();
        let a = PeerId::test(65);
        intersected(&mut b, &a);
        let held = continue_syncs(&mut b, &a, 2);
        let owed1 = b.peers.get(&a).unwrap().next_owed;
        b.handle_io(InterfaceEvent::Disconnected(a.clone()));
        let d = drain_outputs(&mut b);
        let owed2 = b.peers.get(&a).unwrap().next_owed;
        let conn = format!("{:?}", b.peers.get(&a).unwrap().connection);
        b.execute(InitiatorCommand::StartSync(vec![Point::Origin]));
        include_and_connect(&mut b, &a);
        println!("PROBE0 after connect owed={}", b.peers.get(&a).unwrap().next_owed);
        complete_handshake(&mut b, &a);
        println!("PROBE0 after handshake owed={}", b.peers.get(&a).unwrap().next_owed);
        b.execute(InitiatorCommand::Housekeeping);
        let hk = drain_outputs(&mut b);
        for (_, m) in hk.sends() {
            println!("PROBE0 hk send {m:?}");
        }
        println!("PROBE0 after hk owed={}", b.peers.get(&a).unwrap().next_owed);
        let fi = hk.sends().filter(|&(_, m)| matches!(m, AnyMessage::ChainSync(_))).count();
        let cs_state = format!("{:?}", b.peers.get(&a).unwrap().chainsync);
        println!("PROBE held={held} owed1={owed1} disc_outs={} owed2={owed2} conn={conn} fi={fi} cs={cs_state}", d.len());
        b.handle_io(InterfaceEvent::Sent(
            a.clone(),
            AnyMessage::ChainSync(cs::Message::FindIntersect(vec![Point::Origin])),
        ));
        let s = drain_outputs(&mut b);
        let tip = cs::Tip(Point::new(9, vec![9; 32]), 9);
        b.handle_io(InterfaceEvent::Recv(
            a.clone(),
            vec![AnyMessage::ChainSync(cs::Message::IntersectFound(Point::Origin, tip))],
        ));
        let r = drain_outputs(&mut b);
        for o in &r {
            if let BehaviorOutput::InterfaceCommand(crate::InterfaceCommand::Send(_, m)) = o {
                println!("PROBE3 send {m:?}");
            } else {
                println!("PROBE3 other");
            }
        }
        let st = b.peers.get(&a).unwrap();
        println!(
            "PROBE2 sent_outs={} recv_next={} recv_outs={} owed3={} cs={:?} violation={}",
            s.len(),
            r.sends().filter(|&(_, m)| is_next(m)).count(),
            r.len(),
            st.next_owed,
            st.chainsync,
            st.violation
        );
    }
