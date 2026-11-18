//! The `Player` trait: one async decision per turn, optional notify on
//! every event.
//!
//! Every Player implementation must:
//! - Return an `Action` that is in `legal` (the server rejects anything
//!   else with `IllegalAction` and re-prompts).
//! - Make no assumption about the global state; only the `Observation`
//!   is available. This is enforced by the type: the server never hands
//!   out its `State`.

use core::future::Future;

use conga_core::{Action, Event, Observation};

pub trait Player: Send {
    /// Decide this turn's action. Must return a value from `legal`.
    fn act(&mut self, obs: &Observation, legal: &[Action]) -> impl Future<Output = Action> + Send;

    /// Invoked for every `Event` the server broadcasts to this seat.
    /// The default is a no-op; agents with a belief tracker override
    /// this to update it. Not async: the driver may deliver many events
    /// between two `AwaitAction` messages and must not yield for each.
    fn notify(&mut self, _event: &Event) {}
}
