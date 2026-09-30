use pallas_network2::behavior::InitiatorEvent;

pub fn name(event: &InitiatorEvent) -> &'static str {
    match event {
        InitiatorEvent::PeerInitialized(..) => "initialized",
        InitiatorEvent::IntersectionFound(..) => "intersection",
        InitiatorEvent::BlockHeaderReceived(..) => "header",
        InitiatorEvent::RollbackReceived(..) => "rollback",
        InitiatorEvent::BlockBodyReceived(..) => "body",
        InitiatorEvent::TxRequested(..) => "tx",
        InitiatorEvent::EbNotification(..) => "notification",
        InitiatorEvent::EbFetched(..) => "fetched",
    }
}
