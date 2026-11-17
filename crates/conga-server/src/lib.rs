//! Authoritative game server.
//!
//! This slice (m2 slice 3) only establishes the transport primitive: a
//! generic `Transport` trait with an in-memory `tokio::sync::mpsc`-backed
//! implementation. The match loop, event sequence numbers, log writer and
//! the rest of the server land in later slices.

pub mod transport;

pub use transport::{
    ClientEnd, InMemoryTransport, ServerEnd, Transport, TransportError, in_memory_pair,
};
