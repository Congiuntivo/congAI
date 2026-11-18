//! The `ClientToServer` and `ServerToClient` message enums.
//!
//! Both use `#[serde(tag = "type")]` so a wire line reads cleanly as
//! `{"type":"Act","turn":5,"action":{...}}` rather than nesting under the
//! variant name.

use conga_core::{Action, Event, HandResult, MatchStatus, Observation};
use serde::{Deserialize, Serialize};

use crate::error::ErrorCode;

/// Policy the server applies when a client's turn timer expires. Reserved
/// in m2: the server does not run timers yet. Carried on `AwaitAction` so
/// a future server can switch policy per prompt (protocol detail 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TimeoutPolicy {
    /// Disconnect the seat; the match can continue with a stub Player.
    DisconnectOnTimeout,
    /// Take over the seat with a default AI player.
    TakeOverByAi,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientToServer {
    /// Answer the server's most recent `AwaitAction` on turn `turn`. The
    /// turn number lets the server reject a duplicate or stale message
    /// (protocol detail 2).
    Act { turn: u32, action: Action },
    /// Request the server replay events since `since_event_seq`. Reserved
    /// for the m9 reconnect flow; ignored in m2 (protocol detail 1).
    Resume { since_event_seq: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerToClient {
    /// A seat-relative view of the current state. Sent when the state
    /// visible to this seat changes.
    Observe { turn: u32, observation: Observation },
    /// The server is waiting for an action from this seat. `legal` is the
    /// list of actions the server will accept; sending any other action
    /// draws an `Error { code: IllegalAction, .. }` reply.
    ///
    /// `turn_timer_ms` and `timeout_policy` are reserved and always
    /// `None` in m2 (protocol detail 3).
    AwaitAction {
        legal: Vec<Action>,
        turn_timer_ms: Option<u32>,
        timeout_policy: Option<TimeoutPolicy>,
    },
    /// A per-seat monotonically-increasing event. The server stamps `seq`
    /// per recipient seat (protocol detail 1), so each seat sees its own
    /// consecutive `0, 1, 2, ...` sequence.
    Event { seq: u32, event: Event },
    /// The hand has ended; `result` holds the per-seat points for this
    /// hand. The server will either start a new hand or send
    /// `MatchEnded` immediately after.
    HandEnded { result: HandResult },
    /// The match has ended; `status` names the winner and `final_scores`
    /// are the cumulative points per seat.
    MatchEnded {
        status: MatchStatus,
        final_scores: Vec<i32>,
    },
    /// The server rejected something the client did. `message` is a short
    /// human-readable string; machine-readable fault selection uses
    /// `code`.
    Error { code: ErrorCode, message: String },
}
