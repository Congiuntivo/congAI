//! Envelope that wraps every wire message.
//!
//! Every message carries the protocol version and the ruleset hash so the
//! receiver can refuse a mismatched peer at the first line, without parsing
//! anything that depends on a schema that may have changed between versions.

use serde::{Deserialize, Serialize};

/// Compiled-in protocol version. Bumped whenever a schema change breaks
/// compatibility; version 1 is the initial milestone-2 shape.
pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Envelope<B> {
    pub protocol_version: u32,
    /// `RuleSetId::param_hash` of the server's rule set. A client that was
    /// trained for a different variant refuses the session at the first
    /// message.
    pub ruleset_hash: u64,
    pub body: B,
}

impl<B> Envelope<B> {
    /// Build an envelope stamped with the compiled-in `PROTOCOL_VERSION`.
    pub fn new(body: B, ruleset_hash: u64) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            ruleset_hash,
            body,
        }
    }
}
