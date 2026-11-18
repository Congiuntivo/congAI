//! Phases a hand passes through.
//!
//! Every hand goes Draw -> Discard -> (optionally Showdown) -> Ended. The
//! Showdown phase is entered when a player closes; otherwise the hand ends
//! after the empty stock is exhausted and the final discard is made (handled
//! in the rule set).

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Phase {
    Draw,
    Discard,
    Showdown,
    Ended,
}
