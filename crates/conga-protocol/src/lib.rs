//! Wire protocol: versioned envelope, message enums and JSON-lines IO.
//!
//! The server and every client exchange `Envelope<ClientToServer>` and
//! `Envelope<ServerToClient>` values. Each envelope carries a protocol
//! version and a ruleset hash so a mismatch is caught at the first message.
//! The wire format is JSON lines (one object per line, `\n`-terminated).
//! This crate does no IO of its own beyond the small `read_message` and
//! `write_message` helpers, so it is usable from blocking and async callers.

pub mod envelope;
pub mod error;
pub mod io;
pub mod message;

pub use envelope::{Envelope, PROTOCOL_VERSION};
pub use error::{ErrorCode, ProtocolError};
pub use io::{read_message, write_message};
pub use message::{ClientToServer, ServerToClient, TimeoutPolicy};
