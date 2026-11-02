#![cfg(feature = "unstable")]

use std::time::Duration;

use tokio::net::TcpListener;

use pallas_network2::behavior::responder::{ResponderBehavior, ResponderCommand, ResponderEvent};
use pallas_network2::behavior::{
    AnyMessage, DisconnectReason, InitiatorBehavior, InitiatorCommand, InitiatorEvent,
};
use pallas_network2::interface::{TcpInterface, TcpListenerInterface};
use pallas_network2::{Manager, PeerId};

type Initiator = Manager<TcpInterface<AnyMessage>, InitiatorBehavior, AnyMessage>;

const WINDOW: Duration = Duration::from_millis(300);

async fn disconnects_within(manager: &mut Initiator, window: Duration) -> Vec<DisconnectReason> {
    let mut reasons = Vec::new();

    let _ = tokio::time::timeout(window, async {
        loop {
            if let Some(InitiatorEvent::PeerDisconnected(_, reason)) = manager.poll_next().await {
                reasons.push(reason);
            }
        }
    })
    .await;

    reasons
}

async fn bound() -> (TcpListener, PeerId) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let pid = PeerId {
        host: addr.ip().to_string(),
        port: addr.port(),
    };

    (listener, pid)
}

#[tokio::test]
async fn closing_the_previous_connection_after_a_reconnect_reports_nothing() {
    let (listener, pid) = bound().await;
    let mut initiator = Initiator::new(TcpInterface::new(), InitiatorBehavior::default());

    initiator.execute(InitiatorCommand::IncludePeer(pid.clone()));
    initiator.execute(InitiatorCommand::Housekeeping);
    let (first, connected) = tokio::join!(
        listener.accept(),
        disconnects_within(&mut initiator, WINDOW)
    );
    let (first, _) = first.unwrap();

    initiator.execute(InitiatorCommand::DemotePeer(pid.clone()));
    initiator.execute(InitiatorCommand::Housekeeping);
    let demoted = disconnects_within(&mut initiator, WINDOW).await;

    initiator.execute(InitiatorCommand::IncludePeer(pid.clone()));
    initiator.execute(InitiatorCommand::Housekeeping);
    let (second, reconnected) = tokio::join!(
        listener.accept(),
        disconnects_within(&mut initiator, WINDOW)
    );
    let (second, _) = second.unwrap();

    drop(first);
    let first_closed = disconnects_within(&mut initiator, WINDOW).await;

    drop(second);
    let second_closed = disconnects_within(&mut initiator, WINDOW).await;

    assert_eq!(
        (connected, demoted, reconnected, first_closed, second_closed),
        (
            vec![],
            vec![DisconnectReason::Closed],
            vec![],
            vec![],
            vec![DisconnectReason::Errored]
        ),
        "session ends reported on connect, demote, reconnect, first close, second close"
    );
}

async fn reconnect_until_initialized(
    manager: &mut Initiator,
    pid: &PeerId,
    window: Duration,
) -> (Vec<DisconnectReason>, bool) {
    let mut reasons = Vec::new();

    let initialized = tokio::time::timeout(window, async {
        loop {
            match manager.poll_next().await {
                Some(InitiatorEvent::PeerInitialized(..)) => break,
                Some(InitiatorEvent::PeerDisconnected(_, reason)) => {
                    reasons.push(reason);
                    manager.execute(InitiatorCommand::IncludePeer(pid.clone()));
                    manager.execute(InitiatorCommand::Housekeeping);
                }
                _ => {}
            }
        }
    })
    .await
    .is_ok();

    (reasons, initialized)
}

const SESSIONS: usize = 20;

const SETTLE: Duration = Duration::from_millis(50);

#[tokio::test]
async fn demoting_an_initialized_peer_reports_one_close_and_reconnects() {
    let (listener, pid) = bound().await;
    let responder = tokio::spawn(async move {
        let mut responder = Manager::new(
            TcpListenerInterface::<AnyMessage>::new(listener),
            ResponderBehavior::default(),
        );
        loop {
            if let Some(ResponderEvent::PeerInitialized(..)) = responder.poll_next().await {
                responder.execute(ResponderCommand::Housekeeping);
            }
        }
    });
    let mut initiator = Initiator::new(TcpInterface::new(), InitiatorBehavior::default());
    let mut sessions = Vec::new();

    initiator.execute(InitiatorCommand::IncludePeer(pid.clone()));
    initiator.execute(InitiatorCommand::Housekeeping);
    let (mut reasons, mut initialized) =
        reconnect_until_initialized(&mut initiator, &pid, WINDOW).await;

    for _ in 0..SESSIONS {
        // Promotes the peer to hot, which the demote needs to take effect, and lets
        // one keepalive finish, so the demote housekeeping sends another.
        initiator.execute(InitiatorCommand::Housekeeping);
        reasons.extend(disconnects_within(&mut initiator, SETTLE).await);

        initiator.execute(InitiatorCommand::DemotePeer(pid.clone()));
        initiator.execute(InitiatorCommand::Housekeeping);
        let (closes, next) = reconnect_until_initialized(&mut initiator, &pid, WINDOW).await;
        reasons.extend(closes);

        sessions.push((initialized, std::mem::take(&mut reasons)));
        initialized = next;
    }
    responder.abort();

    assert_eq!(
        sessions,
        vec![(true, vec![DisconnectReason::Closed]); SESSIONS],
        "initialized and session ends reported per session"
    );
}
