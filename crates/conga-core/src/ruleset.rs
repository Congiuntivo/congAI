//! The `RuleSet` trait and the identifiers / status types every rule set
//! shares.
//!
//! Hot code is generic over `RuleSet` so each rule set compiles to its own
//! fast path; the trait is deliberately not object-safe (associated `State`
//! type plus `Clone` supertrait).

use std::ops::RangeInclusive;

use rand_core::Rng;

use crate::action::Action;
use crate::chance::Chance;
use crate::deck::DeckSpec;
use crate::event::{Event, HandResult};
use crate::hand::Hand;
use crate::meld::HandEval;
use crate::observation::Observation;
use crate::seat::Seat;

/// Scratch buffer for `legal_actions`. The caller clears and the rule set
/// fills, so the allocation can be reused across turns.
pub type ActionList = Vec<Action>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RuleSetId {
    pub name: &'static str,
    pub version: u32,
    pub param_hash: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MatchStatus {
    /// The match is not over.
    InProgress,
    /// A seat has won the match. Ties among lowest-score simultaneous
    /// eliminations are broken by seat order as per rule 16.
    Winner(Seat),
}

/// Every rule set implements this trait. See the design doc for the semantics
/// of each method.
///
/// The trait is not object-safe. Code that wants dynamic dispatch uses an
/// enum of registered rule sets.
pub trait RuleSet: Clone + Send + Sync + 'static {
    /// The full hidden state. Only the rule set reads or changes it.
    type State: Clone + Send;

    fn id(&self) -> RuleSetId;
    fn deck(&self) -> DeckSpec;
    fn seats(&self) -> RangeInclusive<u8>;

    fn new_hand(&self, scores: &[i32], dealer: Seat, chance: &mut dyn Chance) -> Self::State;

    fn legal_actions(&self, state: &Self::State, out: &mut ActionList);

    fn apply(
        &self,
        state: &mut Self::State,
        action: Action,
        chance: &mut dyn Chance,
        events: &mut Vec<Event>,
    );

    fn observe(&self, state: &Self::State, seat: Seat) -> Observation;

    /// `None` while the hand is still running.
    fn hand_result(&self, state: &Self::State) -> Option<HandResult>;

    fn match_status(&self, scores: &[i32]) -> MatchStatus;

    /// Best split, unmatched total and whether the hand can close.
    fn evaluate_hand(&self, hand: Hand) -> HandEval;

    /// A full state consistent with what one seat knows. Used by the search.
    /// Milestone-4 concern; implementations may `unimplemented!()` for
    /// milestone 1.
    fn sample_state(&self, obs: &Observation, rng: &mut dyn Rng) -> Self::State;
}
