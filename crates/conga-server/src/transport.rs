//! `Transport` trait and its errors.
//!
//! The trait is intentionally symmetric: each side has an `Outgoing` and an
//! `Incoming` type. `ServerEnd` sends `ServerToClient` and receives
//! `ClientToServer`; `ClientEnd` is the mirror. A future WebSocket
//! implementation lives behind the same trait and the server loop stays
//! unchanged.
//!
//! m2 adds the in-memory implementation only. The WS implementation waits
//! for milestone 9.

pub mod in_memory;

use std::fmt;

use conga_protocol::ProtocolError;

pub use in_memory::{ClientEnd, InMemoryTransport, ServerEnd, in_memory_pair};

/// Reasons a transport operation can fail.
#[derive(Debug)]
pub enum TransportError {
    /// The peer dropped its end of the channel. For an in-memory transport
    /// this is the normal clean-disconnect signal; for a WS transport it
    /// is the close frame or a TCP reset.
    ChannelClosed,
    /// A wire-level problem surfaced by the transport (e.g. malformed JSON
    /// on a real WS link). The in-memory transport never produces this
    /// variant.
    Protocol(ProtocolError),
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransportError::ChannelClosed => f.write_str("transport channel closed"),
            TransportError::Protocol(e) => write!(f, "protocol: {e}"),
        }
    }
}

impl std::error::Error for TransportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TransportError::ChannelClosed => None,
            TransportError::Protocol(e) => Some(e),
        }
    }
}

impl From<ProtocolError> for TransportError {
    fn from(value: ProtocolError) -> Self {
        TransportError::Protocol(value)
    }
}

/// Message-passing primitive that sits between the server and one seat's
/// client. Both sides of a connection implement this trait, swapping
/// `Outgoing` and `Incoming`.
///
/// The trait is not object-safe (associated types + async fn); the server
/// loop is generic over `T: Transport` and compiles to a fast per-impl
/// path. Dynamic dispatch, if ever needed, uses `Box<dyn Transport<...>>`
/// through a wrapper, but m2 has no need for it.
pub trait Transport: Send {
    type Outgoing: Send + 'static;
    type Incoming: Send + 'static;

    /// Deliver one message to the peer. Returns `ChannelClosed` once the
    /// peer has gone away. Order is preserved: later sends arrive later.
    fn send(
        &mut self,
        msg: Self::Outgoing,
    ) -> impl core::future::Future<Output = Result<(), TransportError>> + Send;

    /// Wait for the next message from the peer. Returns `ChannelClosed`
    /// on a clean peer disconnect so a server loop can treat it as a
    /// normal exit.
    fn recv(
        &mut self,
    ) -> impl core::future::Future<Output = Result<Self::Incoming, TransportError>> + Send;
}
