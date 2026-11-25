//! Property checks every rule set must pass before anything else uses it.
//!
//! The checks are:
//!
//! 1. All cards of the deck are accounted for after every action.
//! 2. No observation contains a card its seat should not see.
//! 3. A hand that is still running always has a legal action.
//! 4. A logged game replays to identical states.
//! 5. Every sampled state reproduces the observation it was sampled from.
//!
//! Checks 1 to 4 arrive in slice 5 alongside the first `RuleSet`
//! implementation (`StandardConga`). Check 5 lives in
//! [`run_sample_state_checks`] and arrives with milestone 4.

use rand_core::Rng;

use crate::card::Card;
use crate::chance::Chance;
use crate::phase::Phase;
use crate::ruleset::{ActionList, RuleSet};
use crate::seat::Seat;

/// Play `games_per_seed` random games with each seed in `seeds`, running
/// property checks 1 to 4 after every action.
///
/// This function is defined now so the trait, the invariants list and the
/// `#[cfg(test)]` wiring stay in one module. The body lands in slice 5 when
/// the first `RuleSet` implementation makes it exercisable.
pub fn run_random_games<R: RuleSet>(_rules: &R, _seeds: &[u64], _games_per_seed: u32) {
    unimplemented!("conformance tests activate in slice 5 with StandardConga")
}

/// Check 5. For each of `games` hands, play a random game and, at every
/// Draw/Discard turn, draw `samples_per_turn` sampled states from the
/// current seat's observation and assert that each sample round-trips
/// through `observe` back to the original observation.
///
/// The caller supplies a single `rng` that drives both shuffles (via an
/// internal `Chance`) and the uniform-action policy; results are
/// deterministic given its seed and the rule set.
pub fn run_sample_state_checks<R: RuleSet>(
    rules: &R,
    rng: &mut dyn Rng,
    games: u32,
    samples_per_turn: u32,
) {
    let seats_count = *rules.seats().start();
    assert!(
        seats_count >= 2,
        "sample-state checks need at least 2 seats, rule set reports {seats_count}"
    );

    let mut chance = RngChance { rng };
    let mut legal: ActionList = Vec::new();

    for _ in 0..games {
        let scores = vec![0i32; seats_count as usize];
        let dealer = Seat::new(0);
        let mut state = rules.new_hand(&scores, dealer, &mut chance);

        loop {
            if rules.hand_result(&state).is_some() {
                break;
            }

            legal.clear();
            rules.legal_actions(&state, &mut legal);
            assert!(!legal.is_empty(), "running hand with no legal action");

            let obs = rules.observe(&state, obs_current_seat(rules, &state));
            if matches!(obs.phase, Phase::Draw | Phase::Discard) {
                for _ in 0..samples_per_turn {
                    let sample = rules.sample_state(&obs, &mut *chance.rng);
                    let round_trip = rules.observe(&sample, obs.own_seat);
                    assert_eq!(
                        round_trip, obs,
                        "sampled state does not reproduce its observation"
                    );
                }
            }

            let idx = (chance.rng.next_u32() as usize) % legal.len();
            let action = legal[idx].clone();
            let mut events = Vec::new();
            rules.apply(&mut state, action, &mut chance, &mut events);
        }
    }
}

/// Grab the current seat out of an observation taken from an arbitrary
/// vantage point; keeps `run_sample_state_checks` free of rule-specific
/// state accessors.
fn obs_current_seat<R: RuleSet>(rules: &R, state: &R::State) -> Seat {
    rules.observe(state, Seat::new(0)).current_seat
}

/// Minimal `Chance` adapter that shuffles via a borrowed `Rng` without
/// recording the shuffle history. Replay fidelity is not needed for the
/// property checks.
struct RngChance<'a> {
    rng: &'a mut dyn Rng,
}

impl Chance for RngChance<'_> {
    fn shuffle(&mut self, deck: &mut Vec<Card>) {
        for i in (1..deck.len()).rev() {
            let j = (self.rng.next_u32() as usize) % (i + 1);
            deck.swap(i, j);
        }
    }
}
