//! Own requests pushed to a peer whose Sent the protocol state has not
//! accepted, at most one per channel.

use crate::{InterfaceCommand, OutboundQueue, PeerId, protocol as proto};

use super::{
    AnyMessage, BlockRange, FetchRequest, InitiatorBehavior, InitiatorState,
    initiator::{fetch_request, intersect_request, keepalive_request, next_request, range_request},
    responder::{ResponderBehavior, ResponderState},
};

type TxIdsRequest = (
    proto::txsubmission::Blocking,
    proto::txsubmission::TxCount,
    proto::txsubmission::TxCount,
);

/// A chain-sync request held by the initiator.
#[derive(Debug)]
pub(crate) enum ChainSyncRequest {
    Intersect(Vec<proto::Point>),
    Next,
}

/// The initiator's own requests for one peer whose Sent the protocol state has not accepted.
#[derive(Debug, Default)]
pub(crate) struct InitiatorOwnSends {
    keepalive: Option<proto::keepalive::Cookie>,
    peersharing: Option<proto::peersharing::Amount>,
    blockfetch: Option<BlockRange>,
    chainsync: Option<ChainSyncRequest>,
    leiosnotify: bool,
    leiosfetch: Option<FetchRequest>,
    next_owed: bool,
}

/// The responder's own requests for one peer whose Sent the protocol state has not accepted.
#[derive(Debug, Default)]
pub(crate) struct ResponderOwnSends {
    tx_ids: Option<TxIdsRequest>,
}

fn push(pid: &PeerId, msg: AnyMessage, outbound: &mut OutboundQueue<InitiatorBehavior>) {
    outbound.push_ready(InterfaceCommand::Send(pid.clone(), msg));
}

impl InitiatorState {
    /// Pushes this peer's keepalive request for `cookie` when that slot is
    /// empty and the protocol accepts it.
    pub(crate) fn send_own_keepalive(
        &mut self,
        pid: &PeerId,
        cookie: proto::keepalive::Cookie,
        outbound: &mut OutboundQueue<InitiatorBehavior>,
    ) -> bool {
        let msg = keepalive_request(cookie);

        if self.own.keepalive.is_some() || self.keepalive.apply(&msg).is_err() {
            return false;
        }

        self.own.keepalive = Some(cookie);
        push(pid, AnyMessage::KeepAlive(msg), outbound);
        true
    }

    /// Pushes this peer's peer-sharing request for `amount` when that slot is
    /// empty and the protocol accepts it.
    pub(crate) fn send_own_share(
        &mut self,
        pid: &PeerId,
        amount: proto::peersharing::Amount,
        outbound: &mut OutboundQueue<InitiatorBehavior>,
    ) -> bool {
        let msg = proto::peersharing::Message::ShareRequest(amount);

        if self.own.peersharing.is_some() || self.peersharing.apply(&msg).is_err() {
            return false;
        }

        self.own.peersharing = Some(amount);
        push(pid, AnyMessage::PeerSharing(msg), outbound);
        true
    }

    /// Pushes this peer's block-fetch request for `range` when that slot is
    /// empty and the protocol accepts it.
    pub(crate) fn send_own_range(
        &mut self,
        pid: &PeerId,
        range: BlockRange,
        outbound: &mut OutboundQueue<InitiatorBehavior>,
    ) -> bool {
        let msg = range_request(range.clone());

        if self.own.blockfetch.is_some() || self.blockfetch.apply(&msg).is_err() {
            return false;
        }

        self.own.blockfetch = Some(range);
        push(pid, AnyMessage::BlockFetch(msg), outbound);
        true
    }

    /// Pushes this peer's chain-sync request for `request` when that slot is
    /// empty and the protocol accepts it.
    pub(crate) fn send_own_chainsync(
        &mut self,
        pid: &PeerId,
        request: ChainSyncRequest,
        outbound: &mut OutboundQueue<InitiatorBehavior>,
    ) -> bool {
        let msg = match &request {
            ChainSyncRequest::Intersect(points) => intersect_request(points),
            ChainSyncRequest::Next => next_request(),
        };

        if self.own.chainsync.is_some() || self.chainsync.apply(&msg).is_err() {
            return false;
        }

        self.own.chainsync = Some(request);
        push(pid, AnyMessage::ChainSync(msg), outbound);
        true
    }

    /// Pushes this peer's leios-notify `RequestNext` when that slot is empty
    /// and the protocol accepts it.
    pub(crate) fn send_own_notify(
        &mut self,
        pid: &PeerId,
        outbound: &mut OutboundQueue<InitiatorBehavior>,
    ) -> bool {
        let msg = proto::leiosnotify::Message::RequestNext;

        if self.own.leiosnotify || self.leios_notify.apply(&msg).is_err() {
            return false;
        }

        self.own.leiosnotify = true;
        push(pid, AnyMessage::LeiosNotify(msg), outbound);
        true
    }

    /// Pushes this peer's leios-fetch request for `request` when that slot is
    /// empty and the protocol accepts it.
    pub(crate) fn send_own_fetch(
        &mut self,
        pid: &PeerId,
        request: &FetchRequest,
        outbound: &mut OutboundQueue<InitiatorBehavior>,
    ) -> bool {
        let msg = fetch_request(request);

        if self.own.leiosfetch.is_some() || self.leios_fetch.apply(&msg).is_err() {
            return false;
        }

        self.own.leiosfetch = Some(request.clone());
        push(pid, AnyMessage::LeiosFetch(msg), outbound);
        true
    }

    /// Marks a `RequestNext` as owed when the chain-sync slot is full.
    pub(crate) fn owe_next(&mut self) {
        if self.own.chainsync.is_some() {
            self.own.next_owed = true;
        }
    }

    /// Pushes the owed `RequestNext` when one is owed, the chain-sync slot is
    /// empty and the protocol accepts it.
    pub(crate) fn send_owed_next(
        &mut self,
        pid: &PeerId,
        outbound: &mut OutboundQueue<InitiatorBehavior>,
    ) -> bool {
        if !self.own.next_owed || !self.send_own_chainsync(pid, ChainSyncRequest::Next, outbound) {
            return false;
        }

        self.own.next_owed = false;
        true
    }

    /// Takes the block range held in this peer's block-fetch slot, emptying it.
    pub(crate) fn take_own_range(&mut self) -> Option<BlockRange> {
        self.own.blockfetch.take()
    }

    /// Empties the slot holding `msg` when the protocol state before `msg`
    /// accepts it.
    pub(crate) fn clear_own_sent(&mut self, msg: &AnyMessage) {
        match msg {
            AnyMessage::KeepAlive(sent) => {
                let held = matches!(
                    (self.own.keepalive, sent),
                    (Some(cookie), proto::keepalive::Message::KeepAlive(c)) if cookie == *c
                );

                if held && self.keepalive.apply(sent).is_ok() {
                    self.own.keepalive = None;
                }
            }
            AnyMessage::PeerSharing(sent) => {
                let held = matches!(
                    (self.own.peersharing, sent),
                    (Some(amount), proto::peersharing::Message::ShareRequest(a)) if amount == *a
                );

                if held && self.peersharing.apply(sent).is_ok() {
                    self.own.peersharing = None;
                }
            }
            AnyMessage::BlockFetch(sent) => {
                let held = matches!(
                    (&self.own.blockfetch, sent),
                    (Some(range), proto::blockfetch::Message::RequestRange(r)) if range == r
                );

                if held && self.blockfetch.apply(sent).is_ok() {
                    self.own.blockfetch = None;
                }
            }
            AnyMessage::ChainSync(sent) => {
                let held = match (&self.own.chainsync, sent) {
                    (
                        Some(ChainSyncRequest::Intersect(points)),
                        proto::chainsync::Message::FindIntersect(p),
                    ) => points == p,
                    (Some(ChainSyncRequest::Next), proto::chainsync::Message::RequestNext) => true,
                    _ => false,
                };

                if held && self.chainsync.apply(sent).is_ok() {
                    self.own.chainsync = None;
                }
            }
            AnyMessage::LeiosNotify(sent) => {
                let held = self.own.leiosnotify
                    && matches!(sent, proto::leiosnotify::Message::RequestNext);

                if held && self.leios_notify.apply(sent).is_ok() {
                    self.own.leiosnotify = false;
                }
            }
            AnyMessage::LeiosFetch(sent) => {
                let held = match (&self.own.leiosfetch, sent) {
                    (
                        Some(FetchRequest::Block(eb)),
                        proto::leiosfetch::Message::BlockRequest(e),
                    ) => eb == e,
                    (
                        Some(FetchRequest::BlockTxs(eb, bitmaps)),
                        proto::leiosfetch::Message::BlockTxsRequest(e, b),
                    ) => eb == e && bitmaps == b,
                    _ => false,
                };

                if held && self.leios_fetch.apply(sent).is_ok() {
                    self.own.leiosfetch = None;
                }
            }
            AnyMessage::Handshake(_) | AnyMessage::TxSubmission(_) => {}
        }
    }
}

impl ResponderState {
    /// Pushes this peer's tx-submission `RequestTxIds` for `ids` when that slot
    /// is empty and the protocol accepts it.
    pub(crate) fn send_own_tx_ids(
        &mut self,
        pid: &PeerId,
        ids: TxIdsRequest,
        outbound: &mut OutboundQueue<ResponderBehavior>,
    ) -> bool {
        let (blocking, ack, req) = ids;
        let msg = proto::txsubmission::Message::RequestTxIds(blocking, ack, req);

        if self.own.tx_ids.is_some() || self.tx_submission.apply(&msg).is_err() {
            return false;
        }

        self.own.tx_ids = Some(ids);
        outbound.push_ready(InterfaceCommand::Send(
            pid.clone(),
            AnyMessage::TxSubmission(msg),
        ));
        true
    }

    /// Empties the `RequestTxIds` slot when `msg` is that request and the
    /// protocol state before `msg` accepts it.
    pub(crate) fn clear_own_sent(&mut self, msg: &AnyMessage) {
        let AnyMessage::TxSubmission(sent) = msg else {
            return;
        };

        let held = matches!(
            (self.own.tx_ids, sent),
            (Some(ids), proto::txsubmission::Message::RequestTxIds(b, a, r)) if ids == (*b, *a, *r)
        );

        if held && self.tx_submission.apply(sent).is_ok() {
            self.own.tx_ids = None;
        }
    }
}
