//! Own requests pushed to a peer whose Sent the protocol state has not
//! accepted, at most one per channel.

use std::collections::HashMap;

use crate::{Channel, InterfaceCommand, Message as _, OutboundQueue, PeerId, protocol as proto};

use super::{
    AnyMessage, BlockRange, InitiatorBehavior, InitiatorState,
    responder::{ResponderBehavior, ResponderState},
};

/// One peer's own requests whose Sent the protocol state has not accepted, by channel.
#[derive(Debug, Default)]
pub(crate) struct OwnSends {
    held: HashMap<Channel, AnyMessage>,
    next_owed: bool,
}

impl OwnSends {
    fn holds(&self, msg: &AnyMessage) -> bool {
        self.held
            .get(&msg.channel())
            .is_some_and(|held| held.payload() == msg.payload())
    }

    fn hold(&mut self, msg: &AnyMessage, accepted: bool) -> bool {
        if !is_request(msg) || self.held.contains_key(&msg.channel()) || !accepted {
            return false;
        }

        self.held.insert(msg.channel(), msg.clone());
        true
    }
}

fn is_request(msg: &AnyMessage) -> bool {
    use proto::{
        blockfetch, chainsync, keepalive, leiosfetch, leiosnotify, peersharing, txsubmission,
    };

    matches!(
        msg,
        AnyMessage::KeepAlive(keepalive::Message::KeepAlive(_))
            | AnyMessage::PeerSharing(peersharing::Message::ShareRequest(_))
            | AnyMessage::BlockFetch(blockfetch::Message::RequestRange(_))
            | AnyMessage::ChainSync(
                chainsync::Message::RequestNext | chainsync::Message::FindIntersect(_)
            )
            | AnyMessage::LeiosNotify(leiosnotify::Message::RequestNext)
            | AnyMessage::LeiosFetch(
                leiosfetch::Message::BlockRequest(_) | leiosfetch::Message::BlockTxsRequest(..)
            )
            | AnyMessage::TxSubmission(txsubmission::Message::RequestTxIds(..))
    )
}

impl InitiatorState {
    fn accepts(&self, msg: &AnyMessage) -> bool {
        match msg {
            AnyMessage::KeepAlive(m) => self.keepalive.apply(m).is_ok(),
            AnyMessage::PeerSharing(m) => self.peersharing.apply(m).is_ok(),
            AnyMessage::BlockFetch(m) => self.blockfetch.apply(m).is_ok(),
            AnyMessage::ChainSync(m) => self.chainsync.apply(m).is_ok(),
            AnyMessage::LeiosNotify(m) => self.leios_notify.apply(m).is_ok(),
            AnyMessage::LeiosFetch(m) => self.leios_fetch.apply(m).is_ok(),
            AnyMessage::Handshake(_) | AnyMessage::TxSubmission(_) => false,
        }
    }

    /// Pushes `msg` as this peer's own request when its channel holds none and
    /// the protocol accepts it.
    pub(crate) fn send_own(
        &mut self,
        pid: &PeerId,
        msg: AnyMessage,
        outbound: &mut OutboundQueue<InitiatorBehavior>,
    ) -> bool {
        let accepted = self.accepts(&msg);

        if !self.own.hold(&msg, accepted) {
            return false;
        }

        outbound.push_ready(InterfaceCommand::Send(pid.clone(), msg));
        true
    }

    /// Marks a `RequestNext` as owed.
    pub(crate) fn owe_next(&mut self) {
        self.own.next_owed = true;
    }

    /// Pushes the owed `RequestNext` when one is owed and [`Self::send_own`] pushes it.
    pub(crate) fn send_owed_next(
        &mut self,
        pid: &PeerId,
        outbound: &mut OutboundQueue<InitiatorBehavior>,
    ) -> bool {
        let next = AnyMessage::ChainSync(proto::chainsync::Message::RequestNext);

        if !self.own.next_owed || !self.send_own(pid, next, outbound) {
            return false;
        }

        self.own.next_owed = false;
        true
    }

    /// Removes and returns the block range this peer holds for block fetch.
    pub(crate) fn take_own_range(&mut self) -> Option<BlockRange> {
        match self.own.held.remove(&proto::blockfetch::CHANNEL_ID) {
            Some(AnyMessage::BlockFetch(proto::blockfetch::Message::RequestRange(range))) => {
                Some(range)
            }
            _ => None,
        }
    }

    /// Drops the held copy of `msg` when the protocol state before `msg`
    /// accepts it.
    pub(crate) fn clear_own_sent(&mut self, msg: &AnyMessage) {
        if self.own.holds(msg) && self.accepts(msg) {
            self.own.held.remove(&msg.channel());
        }
    }
}

impl ResponderState {
    fn accepts(&self, msg: &AnyMessage) -> bool {
        match msg {
            AnyMessage::TxSubmission(m) => self.tx_submission.apply(m).is_ok(),
            _ => false,
        }
    }

    /// Pushes `msg` as this peer's own request when its channel holds none and
    /// the protocol accepts it.
    pub(crate) fn send_own(
        &mut self,
        pid: &PeerId,
        msg: AnyMessage,
        outbound: &mut OutboundQueue<ResponderBehavior>,
    ) -> bool {
        let accepted = self.accepts(&msg);

        if !self.own.hold(&msg, accepted) {
            return false;
        }

        outbound.push_ready(InterfaceCommand::Send(pid.clone(), msg));
        true
    }

    /// Drops the held copy of `msg` when the protocol state before `msg`
    /// accepts it.
    pub(crate) fn clear_own_sent(&mut self, msg: &AnyMessage) {
        if self.own.holds(msg) && self.accepts(msg) {
            self.own.held.remove(&msg.channel());
        }
    }
}
