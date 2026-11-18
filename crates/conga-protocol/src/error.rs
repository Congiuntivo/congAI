//! Protocol-layer errors and the error codes carried inside
//! `ServerToClient::Error`.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Error codes the server can send to a client inside a wire message.
/// Separate from [`ProtocolError`] because this one travels over the wire
/// and is part of the versioned schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ErrorCode {
    /// The action the client sent was not in `legal_actions`.
    IllegalAction,
    /// The client sent an action but it is not their turn.
    WrongTurn,
    /// The client's protocol version does not match the server's.
    ProtocolMismatch,
    /// The server could not parse the client's message.
    MalformedMessage,
    /// A server-side problem the client cannot resolve by retrying.
    InternalError,
}

/// Errors returned by the protocol's IO helpers. Not sent over the wire;
/// the wire-level counterpart is `ServerToClient::Error { code, message }`.
#[derive(Debug)]
pub enum ProtocolError {
    Io(std::io::Error),
    Parse(serde_json::Error),
    ProtocolMismatch {
        got: u32,
        want: u32,
    },
    /// Clean EOF before any message was read. Distinguished from `Io` so a
    /// server loop can treat it as a normal disconnect.
    UnexpectedEof,
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProtocolError::Io(e) => write!(f, "io error: {e}"),
            ProtocolError::Parse(e) => write!(f, "malformed JSON: {e}"),
            ProtocolError::ProtocolMismatch { got, want } => {
                write!(f, "protocol version {got} does not match {want}")
            }
            ProtocolError::UnexpectedEof => write!(f, "unexpected end of stream"),
        }
    }
}

impl std::error::Error for ProtocolError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ProtocolError::Io(e) => Some(e),
            ProtocolError::Parse(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ProtocolError {
    fn from(value: std::io::Error) -> Self {
        ProtocolError::Io(value)
    }
}

impl From<serde_json::Error> for ProtocolError {
    fn from(value: serde_json::Error) -> Self {
        ProtocolError::Parse(value)
    }
}
