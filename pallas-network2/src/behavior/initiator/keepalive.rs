use crate::{InterfaceCommand, OutboundQueue, PeerId, behavior::AnyMessage};

use super::{InitiatorBehavior, InitiatorState, PeerVisitor};

/// Sub-behavior that sends periodic keepalive messages to maintain connections.
pub struct KeepaliveBehavior {
    token: u16,
}

impl Default for KeepaliveBehavior {
    fn default() -> Self {
        Self { token: u16::MAX }
    }
}

impl KeepaliveBehavior {
    /// Sends a keepalive message to the peer if the protocol state allows it.
    pub fn send_keepalive(
        &mut self,
        pid: &PeerId,
        peer: &InitiatorState,
        outbound: &mut OutboundQueue<super::InitiatorBehavior>,
    ) {
        if peer_is_due(peer) {
            let out = InterfaceCommand::Send(pid.clone(), AnyMessage::KeepAlive(self.request()));

            outbound.push_ready(out);
        }
    }

    fn request(&self) -> crate::protocol::keepalive::Message {
        crate::protocol::keepalive::Message::KeepAlive(self.token)
    }
}

fn peer_is_due(peer: &InitiatorState) -> bool {
    peer.is_initialized() && matches!(peer.keepalive, crate::protocol::keepalive::State::Client(_))
}

impl PeerVisitor for KeepaliveBehavior {
    fn visit_housekeeping(
        &mut self,
        pid: &PeerId,
        state: &mut InitiatorState,
        outbound: &mut OutboundQueue<InitiatorBehavior>,
    ) {
        if peer_is_due(state) {
            state.send_own_keepalive(pid, self.request(), outbound);
        }
    }
}
