//! Authoritative game server.
//!
//! m2 introduces the server in four slices: the `Transport` primitive
//! (slice 3), `MatchState` + hand loop + log writer (this slice, 4), the
//! random AI player (slice 5, in `conga-ai`), the TUI (slice 6) and the
//! `conga-cli play` / `replay` subcommands that bring everything together
//! (slice 7).

pub mod log;
pub mod match_state;
pub mod server;
pub mod transport;

pub use log::{JsonLinesWriter, MatchLogRecord, MatchLogWriter, NullWriter};
pub use match_state::MatchState;
pub use server::{GameServer, ServerError};
pub use transport::{
    ClientEnd, InMemoryTransport, ServerEnd, Transport, TransportError, in_memory_pair,
};
