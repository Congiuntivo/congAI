//! Mutable state of a single hand under `StandardConga`.
//!
//! Only `StandardConga::apply` and `::new_hand` write to this struct. Agents
//! and the server read it through observations, never directly.

use conga_core::{Card, Hand, Meld, Phase, Seat};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    /// Number of seats in this hand (2..=4).
    pub seats_count: u8,
    /// One hand per seat, in seat order.
    pub hands: Vec<Hand>,
    /// Stock, top of the stock is `stock.last()`.
    pub stock: Vec<Card>,
    /// Face-up discard pile. Top of the pile is `discard_pile.last()`.
    pub discard_pile: Vec<Card>,
    /// Match scores coming into this hand, in seat order.
    pub scores: Vec<i32>,
    pub dealer: Seat,
    pub current_seat: Seat,
    pub phase: Phase,
    /// Melds laid on the table during the showdown. Empty during play.
    pub melds_on_table: Vec<Meld>,
    /// `first_turn_done[s]` becomes true after seat `s` has completed its
    /// first turn (first discard or close). Used to enforce the first-turn
    /// close ban.
    pub first_turn_done: Vec<bool>,
    /// Card the current seat just took from the top of the discard pile and
    /// may not re-discard this turn (rule 6). Cleared on discard.
    pub must_not_discard: Option<Card>,
    /// Seat that closed this hand, if any.
    pub closer: Option<Seat>,
    /// The closer's hand at the moment of close, before any showdown action.
    /// Used in slice 7 to detect the -10 and -50 bonuses.
    pub closer_hand_at_close: Option<Hand>,
    /// Flips to true when the closer passes for the first time in the
    /// showdown. Until then, the closer must lay down a split that reduces
    /// their hand to a closing residual (detail c).
    pub closer_has_opened: bool,
    /// Number of consecutive `Pass` actions since the last placement. When
    /// this reaches `seats_count`, the showdown ends.
    pub showdown_consecutive_passes: u8,
    /// The face-down card placed by `Close(c)`. Kept so every card remains
    /// accounted for in the state; not exposed in observations.
    pub face_down_close: Option<Card>,
}
