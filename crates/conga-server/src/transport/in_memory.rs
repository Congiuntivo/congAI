//! In-memory transport, backed by `tokio::sync::mpsc` unbounded channels.
//!
//! One `in_memory_pair()` call returns a `(ServerEnd, ClientEnd)` pair;
//! messages sent on one end arrive on the other in order. Unbounded so a
//! slow consumer does not deadlock the server loop; backpressure is not a
//! concern inside a single process.

use conga_protocol::{ClientToServer, ServerToClient};
use tokio::sync::mpsc;

use super::{Transport, TransportError};

/// Marker type for the pair of ends produced by [`in_memory_pair`]. Kept
/// as an empty struct for API discoverability; consumers interact only
/// with [`ServerEnd`] and [`ClientEnd`].
pub struct InMemoryTransport;

impl InMemoryTransport {
    /// Convenience alias for [`in_memory_pair`].
    pub fn pair() -> (ServerEnd, ClientEnd) {
        in_memory_pair()
    }
}

/// The server's side of an in-memory connection for one seat.
pub struct ServerEnd {
    outgoing: mpsc::UnboundedSender<ServerToClient>,
    incoming: mpsc::UnboundedReceiver<ClientToServer>,
}

/// The client's side of an in-memory connection for one seat.
pub struct ClientEnd {
    outgoing: mpsc::UnboundedSender<ClientToServer>,
    incoming: mpsc::UnboundedReceiver<ServerToClient>,
}

/// Build a fresh in-memory `(ServerEnd, ClientEnd)` pair.
pub fn in_memory_pair() -> (ServerEnd, ClientEnd) {
    let (s2c_tx, s2c_rx) = mpsc::unbounded_channel::<ServerToClient>();
    let (c2s_tx, c2s_rx) = mpsc::unbounded_channel::<ClientToServer>();
    (
        ServerEnd {
            outgoing: s2c_tx,
            incoming: c2s_rx,
        },
        ClientEnd {
            outgoing: c2s_tx,
            incoming: s2c_rx,
        },
    )
}

impl Transport for ServerEnd {
    type Outgoing = ServerToClient;
    type Incoming = ClientToServer;

    async fn send(&mut self, msg: ServerToClient) -> Result<(), TransportError> {
        self.outgoing
            .send(msg)
            .map_err(|_| TransportError::ChannelClosed)
    }

    async fn recv(&mut self) -> Result<ClientToServer, TransportError> {
        self.incoming
            .recv()
            .await
            .ok_or(TransportError::ChannelClosed)
    }
}

impl Transport for ClientEnd {
    type Outgoing = ClientToServer;
    type Incoming = ServerToClient;

    async fn send(&mut self, msg: ClientToServer) -> Result<(), TransportError> {
        self.outgoing
            .send(msg)
            .map_err(|_| TransportError::ChannelClosed)
    }

    async fn recv(&mut self) -> Result<ServerToClient, TransportError> {
        self.incoming
            .recv()
            .await
            .ok_or(TransportError::ChannelClosed)
    }
}
