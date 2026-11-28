//! `CleanSplitCloseWrapper` — the first candidate tactic (D47).
//!
//! When the wrapped `inner` player decides to `Close(_)`, the wrapper
//! enumerates every legal `Close(_)` option, draws K belief samples
//! *once* (shared across all options so variance cancels in the
//! relative comparison) and picks the close-card whose post-close
//! equity is highest. Ties are resolved by legal-action order.
//!
//! Scope: the wrapper never declines to close. If the inner says
//! "close", the wrapper closes — it just might pick a different card.
//! That isolates the clean-split signal (D45) in the simplest
//! wrapper possible; any gain over the unchanged baseline is
//! attributable to the picked close-card, not to a decision about
//! whether to close at all.

use conga_core::{Action, Card, Event, MeldParams, Observation, Seat, best_meld_split};
use conga_rules::StandardConga;
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;

use crate::belief::BeliefTracker;
use crate::close_helpers::own_close_delta;
use crate::equity::{EquityTable, committed_table};
use crate::player::Player;

/// Belief samples drawn per Close decision. Matches
/// `DEFAULT_SCORE_AWARE_SAMPLES` so paired comparisons are on the
/// same sampling budget.
pub const DEFAULT_CLEAN_SPLIT_SAMPLES: u32 = 20;

pub struct CleanSplitCloseWrapper<P: Player> {
    inner: P,
    belief: Option<BeliefTracker>,
    pending_events: Vec<Event>,
    rng: ChaCha8Rng,
    samples: u32,
    rules: StandardConga,
    table: &'static EquityTable,
}

impl<P: Player> CleanSplitCloseWrapper<P> {
    pub fn new(inner: P, samples: u32, seed: u64) -> Self {
        Self {
            inner,
            belief: None,
            pending_events: Vec::new(),
            rng: ChaCha8Rng::seed_from_u64(seed),
            samples,
            rules: StandardConga::default(),
            table: committed_table(),
        }
    }

    fn ensure_belief(&mut self, obs: &Observation) {
        if self.belief.is_none() {
            let mut b = BeliefTracker::new(obs.own_seat, obs.hand_sizes.len() as u8);
            for e in self.pending_events.drain(..) {
                b.notify(&e);
            }
            self.belief = Some(b);
        }
    }

    fn opp_seat(obs: &Observation) -> Seat {
        let seats = obs.hand_sizes.len() as u8;
        let own = obs.own_seat.index();
        Seat::new((own + 1) % seats)
    }
}

impl<P: Player> Player for CleanSplitCloseWrapper<P> {
    fn act(&mut self, obs: &Observation, legal: &[Action]) -> Action {
        let base_action = self.inner.act(obs, legal);
        let Action::Close(_) = base_action else {
            return base_action;
        };

        let legal_closes: Vec<Card> = legal
            .iter()
            .filter_map(|a| {
                if let Action::Close(c) = a {
                    Some(*c)
                } else {
                    None
                }
            })
            .collect();
        if legal_closes.len() <= 1 {
            // No alternative to compare against; inner's choice is the
            // only legal close.
            return base_action;
        }

        self.ensure_belief(obs);

        let opp = Self::opp_seat(obs);
        let opp_idx = opp.index() as usize;
        let own_idx = obs.own_seat.index() as usize;
        let own_score = obs.scores[own_idx];
        let opp_score = obs.scores[opp_idx];

        let belief = self.belief.as_ref().expect("ensure_belief populated");
        let sample_count = self.samples.max(1) as usize;
        let meld_params = MeldParams::standard();
        let mut opp_residuals: Vec<i32> = Vec::with_capacity(sample_count);
        for _ in 0..sample_count {
            let sampled = belief.sample_state(&mut self.rng, obs, &self.rules);
            let opp_hand = sampled.hands[opp_idx];
            opp_residuals.push(best_meld_split(opp_hand, &meld_params).unmatched_total as i32);
        }

        let mut best_idx = 0usize;
        let mut best_eq = f32::MIN;
        for (i, &c) in legal_closes.iter().enumerate() {
            let mut hand_after = obs.own_hand;
            hand_after.remove(c);
            let own_delta = own_close_delta(hand_after);
            let new_own = own_score + own_delta;
            let mut sum = 0.0f64;
            for &r in &opp_residuals {
                sum += self.table.lookup(new_own, opp_score + r) as f64;
            }
            let eq = (sum / sample_count as f64) as f32;
            if eq > best_eq {
                best_eq = eq;
                best_idx = i;
            }
        }
        Action::Close(legal_closes[best_idx])
    }

    fn notify(&mut self, event: &Event) {
        self.inner.notify(event);
        match &mut self.belief {
            Some(b) => b.notify(event),
            None => self.pending_events.push(event.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::Player;
    use conga_core::{Hand, Phase, Rank, Suit};

    fn c(suit: Suit, rank: u8) -> Card {
        Card::new(suit, Rank::new(rank).unwrap())
    }

    fn seat(i: u8) -> Seat {
        Seat::new(i)
    }

    /// Dummy inner that always returns a fixed action from the legal
    /// set, used to pin inner's choice so wrapper's override is
    /// observable.
    struct FixedChoicePlayer {
        fixed: Action,
    }

    impl Player for FixedChoicePlayer {
        fn act(&mut self, _obs: &Observation, _legal: &[Action]) -> Action {
            self.fixed.clone()
        }
        fn notify(&mut self, _event: &Event) {}
    }

    /// Hand where Close(1C) leaves a *clean* split (2-3-4-5C run +
    /// 1-2-3H run = 7 cards, 0 unmatched) while Close(4C) leaves a
    /// 1-card residual (1-2-3C + 1-2-3H + 5C unmatched = 1 card,
    /// rank 5). The wrapper should prefer Close(1C) for its -10
    /// clean-split bonus regardless of opp residual.
    fn clean_split_vs_residual_observation() -> (Observation, Vec<Action>) {
        let own: Hand = [
            c(Suit::Clubs, 1),
            c(Suit::Clubs, 2),
            c(Suit::Clubs, 3),
            c(Suit::Clubs, 4),
            c(Suit::Clubs, 5),
            c(Suit::Hearts, 1),
            c(Suit::Hearts, 2),
            c(Suit::Hearts, 3),
        ]
        .as_slice()
        .into();
        // Legal-action order intentionally puts Close(4C) first so a
        // "pick first legal close" dummy picks the sub-optimal option.
        let legal = vec![
            Action::Close(c(Suit::Clubs, 4)),
            Action::Close(c(Suit::Clubs, 1)),
            Action::Discard(c(Suit::Clubs, 4)),
            Action::Discard(c(Suit::Clubs, 1)),
        ];
        let obs = Observation {
            own_seat: seat(0),
            current_seat: seat(0),
            phase: Phase::Discard,
            own_hand: own,
            top_discard: None,
            discard_history: vec![c(Suit::Spades, 7), c(Suit::Diamonds, 8)],
            stock_size: 20,
            melds_on_table: Vec::new(),
            scores: vec![30, 40],
            hand_sizes: vec![8, 7],
        };
        (obs, legal)
    }

    #[test]
    fn wrapper_overrides_inner_toward_clean_split() {
        let (obs, legal) = clean_split_vs_residual_observation();
        let inner = FixedChoicePlayer {
            fixed: Action::Close(c(Suit::Clubs, 4)),
        };
        let mut wrap = CleanSplitCloseWrapper::new(inner, 8, 42);
        let action = wrap.act(&obs, &legal);
        assert_eq!(
            action,
            Action::Close(c(Suit::Clubs, 1)),
            "wrapper should pick the clean-split close over the residual close"
        );
    }

    #[test]
    fn wrapper_passes_through_non_close_actions() {
        let (obs, legal) = clean_split_vs_residual_observation();
        // Inner returns a Discard; wrapper must not touch it.
        let inner = FixedChoicePlayer {
            fixed: Action::Discard(c(Suit::Clubs, 4)),
        };
        let mut wrap = CleanSplitCloseWrapper::new(inner, 8, 42);
        let action = wrap.act(&obs, &legal);
        assert_eq!(action, Action::Discard(c(Suit::Clubs, 4)));
    }

    #[test]
    fn wrapper_short_circuits_when_only_one_legal_close() {
        let (obs, mut legal) = clean_split_vs_residual_observation();
        // Strip all but one Close option.
        legal.retain(|a| !matches!(a, Action::Close(c_) if *c_ == c(Suit::Clubs, 1)));
        assert_eq!(
            legal
                .iter()
                .filter(|a| matches!(a, Action::Close(_)))
                .count(),
            1
        );
        let inner = FixedChoicePlayer {
            fixed: Action::Close(c(Suit::Clubs, 4)),
        };
        let mut wrap = CleanSplitCloseWrapper::new(inner, 8, 42);
        let action = wrap.act(&obs, &legal);
        // Only one close to pick; inner's choice stays.
        assert_eq!(action, Action::Close(c(Suit::Clubs, 4)));
    }

    #[test]
    fn notify_forwards_to_inner_and_belief_once_initialised() {
        let (obs, legal) = clean_split_vs_residual_observation();
        let inner = FixedChoicePlayer {
            fixed: Action::Close(c(Suit::Clubs, 4)),
        };
        let mut wrap = CleanSplitCloseWrapper::new(inner, 8, 42);
        // Pre-act notify queues an event into pending_events.
        wrap.notify(&Event::Discarded {
            seat: seat(0),
            card: c(Suit::Spades, 7),
        });
        // Trigger belief init via act.
        let _ = wrap.act(&obs, &legal);
        // Post-init notify now goes straight to the belief.
        wrap.notify(&Event::Drew {
            seat: seat(1),
            source: conga_core::DrawSource::Stock,
        });
        // No panic; belief is Some now.
        assert!(wrap.belief.is_some());
    }
}
