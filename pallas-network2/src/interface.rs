use std::{
    collections::HashMap,
    convert::Infallible,
    future::ready,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use futures::{
    FutureExt, Stream, StreamExt,
    channel::oneshot::{self, Canceled},
    future::{Either, Shared, select},
    stream::{FusedStream, FuturesUnordered},
};

use tokio::{sync::Mutex, time::Instant};

use crate::{
    Channel, Interface, InterfaceCommand, InterfaceError, InterfaceEvent, Message, Payload, PeerId,
    bearer::{Bearer, BearerReadHalf, BearerWriteHalf, Timestamp},
};

enum InternalEvent<M: Message> {
    Connected(PeerId, Bearer),
    Disconnected(PeerId),
    Sent(PeerId, M),
    Recv(PeerId, Vec<M>, BearerReadHalf, ChunkBuffer, Closed),
    Error(PeerId, tokio::io::Error),
}

type InterfaceFuture<M> = Pin<Box<dyn Future<Output = InternalEvent<M>> + Send>>;

type PoolFuture<M> = Pin<Box<dyn Future<Output = Result<InternalEvent<M>, Canceled>> + Send>>;

async fn connect<M: Message>(pid: PeerId) -> InternalEvent<M> {
    let pid = pid.clone();

    tracing::debug!(%pid, "connecting bearer");
    let bearer = Bearer::connect_tcp((pid.host.clone(), pid.port)).await;

    match bearer {
        Ok(bearer) => InternalEvent::Connected(pid.clone(), bearer),
        Err(e) => InternalEvent::Error(pid.clone(), e),
    }
}

async fn send<M: Message>(
    pid: PeerId,
    writer: SharedWriter,
    msg: M,
    ts: Timestamp,
    mode: u16,
) -> InternalEvent<M> {
    let pid = pid.clone();
    let copy = msg.clone();

    let mut writer = writer.lock().await;

    let result = writer.write_message(msg, ts, mode).await;

    match result {
        Ok(_) => InternalEvent::Sent(pid.clone(), copy),
        Err(e) => InternalEvent::Error(pid.clone(), e),
    }
}

/// Buffer of partial payload chunks keyed by channel, used to reassemble
/// multi-segment messages.
pub type ChunkBuffer = HashMap<Channel, Payload>;

async fn recv<M: Message>(
    pid: PeerId,
    mut reader: BearerReadHalf,
    mut partial_chunks: ChunkBuffer,
    closed: Closed,
) -> InternalEvent<M> {
    let pid = pid.clone();

    let result = reader.read_full_msgs(&mut partial_chunks).await;

    match result {
        Ok(msgs) => InternalEvent::Recv(pid.clone(), msgs, reader, partial_chunks, closed),
        Err(e) => InternalEvent::Error(pid.clone(), e),
    }
}

async fn disconnect<M: Message>(pid: PeerId, writer: SharedWriter) -> InternalEvent<M> {
    let pid = pid.clone();

    let mut writer = writer.lock().await;

    writer.shutdown().await.unwrap();

    InternalEvent::Disconnected(pid.clone())
}

/// A thread-safe handle to a shared [`BearerWriteHalf`].
pub type SharedWriter = Arc<Mutex<BearerWriteHalf>>;

type Closed = Shared<oneshot::Receiver<Infallible>>;

struct Connection {
    writer: SharedWriter,
    closed: Closed,
    /// Dropping it resolves `closed`, which ends the connection's pending reads and sends.
    _close: oneshot::Sender<Infallible>,
}

// ---------------------------------------------------------------------------
// TcpConnectionPool — shared connection-management logic
// ---------------------------------------------------------------------------

struct TcpConnectionPool<M: Message> {
    futures: FuturesUnordered<PoolFuture<M>>,
    connections: HashMap<PeerId, Connection>,
    clock: Instant,
    /// The mode bit to set on outgoing segments (0 for initiator, PROTOCOL_SERVER for responder).
    mode: u16,
}

impl<M: Message> TcpConnectionPool<M> {
    fn new(mode: u16) -> Self {
        Self {
            futures: FuturesUnordered::new(),
            connections: HashMap::new(),
            clock: Instant::now(),
            mode,
        }
    }

    fn push_future(&mut self, f: InterfaceFuture<M>) {
        self.futures.push(Box::pin(f.map(Ok)));
    }

    fn push_connection_future(&mut self, closed: Closed, f: InterfaceFuture<M>) {
        let future = select(closed, f).map(|either| match either {
            Either::Left((Err(canceled), _)) => Err(canceled),
            Either::Right((event, _)) => Ok(event),
        });

        self.futures.push(Box::pin(future));
    }

    fn push_reader(
        &mut self,
        pid: PeerId,
        reader: BearerReadHalf,
        partial_chunks: ChunkBuffer,
        closed: Closed,
    ) {
        let future = recv(pid, reader, partial_chunks, closed.clone());
        self.push_connection_future(closed, Box::pin(future));
    }

    fn on_connected(&mut self, pid: PeerId, bearer: Bearer) -> InterfaceEvent<M> {
        let (read, write) = bearer.into_split();
        let (close, closed) = oneshot::channel();
        let closed = closed.shared();

        let connection = Connection {
            writer: Arc::new(Mutex::new(write)),
            closed: closed.clone(),
            _close: close,
        };
        self.connections.insert(pid.clone(), connection);

        self.push_reader(pid.clone(), read, HashMap::new(), closed);

        InterfaceEvent::Connected(pid)
    }

    fn on_disconnected(&mut self, pid: PeerId) -> InterfaceEvent<M> {
        InterfaceEvent::Disconnected(pid)
    }

    fn on_sent(&mut self, pid: PeerId, msg: M) -> InterfaceEvent<M> {
        InterfaceEvent::Sent(pid, msg)
    }

    fn on_recv(
        &mut self,
        pid: PeerId,
        msgs: Vec<M>,
        reader: BearerReadHalf,
        partial_chunks: ChunkBuffer,
        closed: Closed,
    ) -> InterfaceEvent<M> {
        self.push_reader(pid.clone(), reader, partial_chunks, closed);

        InterfaceEvent::Recv(pid, msgs)
    }

    fn on_error(&mut self, pid: PeerId, error: tokio::io::Error) -> InterfaceEvent<M> {
        tracing::error!("error: {:?}", error);
        InterfaceEvent::Error(pid, InterfaceError::Other(error.to_string()))
    }

    fn handle_internal_event(&mut self, event: InternalEvent<M>) -> InterfaceEvent<M> {
        match event {
            InternalEvent::Connected(pid, stream) => self.on_connected(pid, stream),
            InternalEvent::Sent(pid, msg) => self.on_sent(pid, msg),
            InternalEvent::Recv(pid, msgs, stream, buf, closed) => {
                self.on_recv(pid, msgs, stream, buf, closed)
            }
            InternalEvent::Disconnected(pid) => self.on_disconnected(pid),
            InternalEvent::Error(pid, error) => self.on_error(pid, error),
        }
    }

    fn dispatch_send(&mut self, pid: PeerId, msg: M) {
        let ts = self.clock.elapsed().as_micros() as u32;

        let Some(connection) = self.connections.get(&pid) else {
            tracing::error!(%pid, "trying to send to a peer not connected");
            return;
        };
        let closed = connection.closed.clone();

        let future = send(pid, connection.writer.clone(), msg, ts, self.mode);
        self.push_connection_future(closed, Box::pin(future));
    }

    fn dispatch_disconnect(&mut self, pid: PeerId) {
        let Some(connection) = self.connections.remove(&pid) else {
            tracing::warn!(%pid, "trying to disconnect a peer not connected");
            self.push_future(Box::pin(ready(InternalEvent::Disconnected(pid.clone()))));
            return;
        };

        let future = disconnect(pid, connection.writer);
        self.push_future(Box::pin(future));
    }

    fn poll_next_event(&mut self, cx: &mut Context<'_>) -> Poll<Option<InterfaceEvent<M>>> {
        loop {
            let event = self.futures.poll_next_unpin(cx);

            match event {
                Poll::Ready(Some(Ok(event))) => {
                    let event = self.handle_internal_event(event);
                    return Poll::Ready(Some(event));
                }
                Poll::Ready(Some(Err(Canceled))) => continue,
                Poll::Ready(None) => return Poll::Pending,
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// TcpInterface — outbound connections
// ---------------------------------------------------------------------------

/// A network interface that initiates outbound TCP connections to peers.
///
/// Implements [`Interface`] by managing a pool of TCP connections and
/// dispatching connect/send/disconnect commands.
pub struct TcpInterface<M: Message> {
    pool: TcpConnectionPool<M>,
}

impl<M: Message> Default for TcpInterface<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: Message> TcpInterface<M> {
    /// Creates a new TCP interface for initiating outbound connections.
    pub fn new() -> Self {
        Self {
            pool: TcpConnectionPool::new(crate::protocol::PROTOCOL_CLIENT),
        }
    }
}

impl<M: Message> Interface<M> for TcpInterface<M> {
    fn dispatch(&mut self, cmd: InterfaceCommand<M>) {
        match cmd {
            InterfaceCommand::Connect(pid) => {
                let future = connect(pid.clone());
                self.pool.push_future(Box::pin(future));
            }
            InterfaceCommand::Send(pid, msg) => {
                self.pool.dispatch_send(pid, msg);
            }
            InterfaceCommand::Disconnect(pid) => {
                self.pool.dispatch_disconnect(pid);
            }
        }
    }
}

impl<M: Message> FusedStream for TcpInterface<M> {
    fn is_terminated(&self) -> bool {
        false
    }
}

impl<M: Message> Stream for TcpInterface<M> {
    type Item = InterfaceEvent<M>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.pool.poll_next_event(cx)
    }
}

// ---------------------------------------------------------------------------
// TcpListenerInterface — inbound connections via a bound TCP listener
// ---------------------------------------------------------------------------

async fn accept_tcp<M: Message>(listener: Arc<tokio::net::TcpListener>) -> InternalEvent<M> {
    match Bearer::accept_tcp(&listener).await {
        Ok((bearer, addr)) => {
            let pid = PeerId {
                host: addr.ip().to_string(),
                port: addr.port(),
            };
            tracing::info!(%pid, "accepted inbound connection");
            InternalEvent::Connected(pid, bearer)
        }
        Err(e) => {
            tracing::error!("accept error: {:?}", e);
            // Use a sentinel peer id for accept errors
            let pid = PeerId {
                host: "accept-error".to_string(),
                port: 0,
            };
            InternalEvent::Error(pid, e)
        }
    }
}

/// A network interface that accepts inbound TCP connections from a bound
/// listener.
///
/// Implements [`Interface`] by continuously accepting new connections and
/// managing the resulting peer sessions. Outbound `Connect` commands are
/// ignored since connections are initiated by remote peers.
pub struct TcpListenerInterface<M: Message> {
    pool: TcpConnectionPool<M>,
    listener: Arc<tokio::net::TcpListener>,
    accept_fut: InterfaceFuture<M>,
}

impl<M: Message> TcpListenerInterface<M> {
    /// Creates a new listener interface that will accept connections on the
    /// given [`TcpListener`](tokio::net::TcpListener).
    pub fn new(listener: tokio::net::TcpListener) -> Self {
        let listener = Arc::new(listener);
        let accept_fut = Box::pin(accept_tcp(Arc::clone(&listener)));

        Self {
            pool: TcpConnectionPool::new(crate::protocol::PROTOCOL_SERVER),
            listener,
            accept_fut,
        }
    }
}

impl<M: Message> Interface<M> for TcpListenerInterface<M> {
    fn dispatch(&mut self, cmd: InterfaceCommand<M>) {
        match cmd {
            InterfaceCommand::Connect(pid) => {
                tracing::warn!(%pid, "TcpListenerInterface does not support outbound Connect, ignoring");
            }
            InterfaceCommand::Send(pid, msg) => {
                self.pool.dispatch_send(pid, msg);
            }
            InterfaceCommand::Disconnect(pid) => {
                self.pool.dispatch_disconnect(pid);
            }
        }
    }
}

impl<M: Message> FusedStream for TcpListenerInterface<M> {
    fn is_terminated(&self) -> bool {
        false
    }
}

impl<M: Message> Stream for TcpListenerInterface<M> {
    type Item = InterfaceEvent<M>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // First, poll the accept future for new inbound connections
        if let Poll::Ready(event) = self.accept_fut.as_mut().poll(cx) {
            let ie = self.pool.handle_internal_event(event);

            // Re-arm the accept future for the next connection
            self.accept_fut = Box::pin(accept_tcp(Arc::clone(&self.listener)));

            return Poll::Ready(Some(ie));
        }

        // Then poll existing connections
        self.pool.poll_next_event(cx)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::net::{TcpListener, TcpStream};

    use super::*;
    use crate::behavior::AnyMessage;

    const WINDOW: Duration = Duration::from_millis(300);

    async fn kinds_within(interface: &mut TcpInterface<AnyMessage>) -> Vec<&'static str> {
        let mut kinds = Vec::new();

        let _ = tokio::time::timeout(WINDOW, async {
            loop {
                let kind = match interface.next().await {
                    Some(InterfaceEvent::Connected(_)) => "connected",
                    Some(InterfaceEvent::Disconnected(_)) => "disconnected",
                    Some(InterfaceEvent::Sent(..)) => "sent",
                    Some(InterfaceEvent::Error(..)) => "error",
                    Some(_) => "other",
                    None => continue,
                };
                kinds.push(kind);
            }
        })
        .await;

        kinds
    }

    async fn connect_once(
        interface: &mut TcpInterface<AnyMessage>,
        listener: &TcpListener,
        pid: &PeerId,
    ) -> (TcpStream, Vec<&'static str>) {
        interface.dispatch(InterfaceCommand::Connect(pid.clone()));
        let (accepted, kinds) = tokio::join!(listener.accept(), kinds_within(interface));

        (accepted.unwrap().0, kinds)
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
    async fn a_replaced_connection_closing_reports_nothing() {
        let (listener, pid) = bound().await;
        let mut interface = TcpInterface::<AnyMessage>::new();

        let (first, first_connected) = connect_once(&mut interface, &listener, &pid).await;
        let (second, second_connected) = connect_once(&mut interface, &listener, &pid).await;

        drop(first);
        let first_closed = kinds_within(&mut interface).await;

        drop(second);
        let second_closed = kinds_within(&mut interface).await;

        assert_eq!(
            (
                first_connected,
                second_connected,
                first_closed,
                second_closed
            ),
            (vec!["connected"], vec!["connected"], vec![], vec!["error"]),
            "events on each connect, then on each server side close"
        );
    }

    fn keepalive() -> AnyMessage {
        AnyMessage::KeepAlive(crate::protocol::keepalive::Message::KeepAlive(7))
    }

    #[tokio::test]
    async fn a_send_unwritten_at_a_disconnect_reports_only_the_disconnect() {
        let mut kinds = Vec::new();

        for send_first in [true, false] {
            let (listener, pid) = bound().await;
            let mut interface = TcpInterface::<AnyMessage>::new();
            let (_server, _) = connect_once(&mut interface, &listener, &pid).await;
            let send = InterfaceCommand::Send(pid.clone(), keepalive());
            let disconnect = InterfaceCommand::Disconnect(pid.clone());

            if send_first {
                interface.dispatch(send);
                interface.dispatch(disconnect);
            } else {
                interface.dispatch(disconnect);
                interface.dispatch(send);
            }
            kinds.push(kinds_within(&mut interface).await);
        }

        assert_eq!(
            kinds,
            vec![vec!["disconnected"], vec!["disconnected"]],
            "events with the send dispatched before, then after, the disconnect"
        );
    }

    const BLOCKS: usize = 400;

    #[tokio::test]
    async fn a_disconnect_behind_a_blocked_send_reports_only_the_disconnect() {
        let (listener, pid) = bound().await;
        let mut interface = TcpInterface::<AnyMessage>::new();
        let (server, _) = connect_once(&mut interface, &listener, &pid).await;

        for _ in 0..BLOCKS {
            let block = crate::protocol::blockfetch::Message::Block(vec![0; 65536]);
            interface.dispatch(InterfaceCommand::Send(
                pid.clone(),
                AnyMessage::BlockFetch(block),
            ));
        }
        let filling = kinds_within(&mut interface).await;

        interface.dispatch(InterfaceCommand::Disconnect(pid.clone()));
        let disconnecting = kinds_within(&mut interface).await;

        drop(server);
        let closed = kinds_within(&mut interface).await;

        assert_eq!(
            (
                filling.iter().all(|kind| *kind == "sent"),
                filling.len() < BLOCKS,
                disconnecting,
                closed
            ),
            (true, true, vec!["disconnected"], vec![]),
            "only sends while filling, a send left blocked, then events on disconnect and on close"
        );
    }
}
