//! `GameLog` and the `replay` helper.
//!
//! A hand is fully determined by the rule set id, the starting scores, the
//! dealer, the ordered shuffle results and the ordered actions. `replay`
//! turns those pieces back into the sequence of states the live run passed
//! through.

use crate::action::Action;
use crate::card::Card;
use crate::chance::ReplayChance;
use crate::event::Event;
use crate::ruleset::{RuleSet, RuleSetId};
use crate::seat::Seat;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameLog {
    pub ruleset_id: RuleSetId,
    pub seats: u8,
    pub dealer: Seat,
    pub starting_scores: Vec<i32>,
    pub actions: Vec<Action>,
    pub shuffles: Vec<Vec<Card>>,
}

/// Replay a logged hand. Returns the sequence of states and the events each
/// action emitted during the rerun.
///
/// `states[0]` is the state produced by `new_hand`; `states[i+1]` is the
/// state after applying `log.actions[i]`.
pub struct ReplayedHand<R: RuleSet> {
    pub states: Vec<R::State>,
    pub events_per_action: Vec<Vec<Event>>,
}

pub fn replay<R: RuleSet>(rules: &R, log: &GameLog) -> ReplayedHand<R> {
    let mut chance = ReplayChance::new(log.shuffles.clone());
    let mut state = rules.new_hand(&log.starting_scores, log.dealer, &mut chance);
    let mut states = Vec::with_capacity(log.actions.len() + 1);
    states.push(state.clone());
    let mut events_per_action = Vec::with_capacity(log.actions.len());
    for action in &log.actions {
        let mut events = Vec::new();
        rules.apply(&mut state, action.clone(), &mut chance, &mut events);
        states.push(state.clone());
        events_per_action.push(events);
    }
    ReplayedHand {
        states,
        events_per_action,
    }
}
