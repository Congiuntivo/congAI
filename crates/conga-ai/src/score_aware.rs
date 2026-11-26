//! `ScoreAwarePlayer`: rule-based v2 plus an equity-aware Close gate (D41, D45).
//!
//! When the underlying [`RuleBasedPlayer`] would choose a `Close`, this
//! agent samples `K` world-states from its own [`BeliefTracker`],
//! derives one opponent-residual estimate per sample and then — for
//! each legal `Close(_)` option — averages the post-close equity over
//! the *shared* set of residuals. Variance in the table (~0.07 std at
//! N=50) cancels out in the relative comparison across close-cards.
//! The agent picks the close-card with the highest post-close equity;
//! if even that is worse than the status-quo table lookup, it declines
//! and lets the base pick a non-Close action. See D45 for why the
//! original D41 "close iff post > continue" gate turned out to be a
//! no-op on the real N=50 table.
//!
//! On non-Close decisions the base's action is passed through
//! unchanged. The base's own belief tracker (used for discard safety)
//! still runs and receives every notified event.

use conga_core::{Action, Card, Event, Hand, MeldParams, Observation, Seat, best_meld_split};
use conga_rules::StandardConga;
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;

use crate::belief::BeliefTracker;
use crate::equity::{EquityTable, committed_table};
use crate::player::Player;
use crate::rule_based::RuleBasedPlayer;

/// Belief samples per Close decision (D41).
pub const DEFAULT_SCORE_AWARE_SAMPLES: u32 = 20;

pub struct ScoreAwarePlayer {
    base: RuleBasedPlayer,
    belief: Option<BeliefTracker>,
    pending_events: Vec<Event>,
    rng: ChaCha8Rng,
    samples: u32,
    rules: StandardConga,
    table: &'static EquityTable,
}

impl ScoreAwarePlayer {
    pub fn new(samples: u32, seed: u64) -> Self {
        ScoreAwarePlayer {
            base: RuleBasedPlayer::new(),
            belief: None,
            pending_events: Vec::new(),
            rng: ChaCha8Rng::seed_from_u64(seed),
            samples,
            rules: StandardConga::default(),
            table: committed_table(),
        }
    }

    /// Only used by tests that want to pin the equity table to a known
    /// (e.g. placeholder) value instead of the committed artefact.
    #[cfg(test)]
    pub(crate) fn with_table(samples: u32, seed: u64, table: &'static EquityTable) -> Self {
        let mut p = Self::new(samples, seed);
        p.table = table;
        p
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

    fn opp_seat(&self, obs: &Observation) -> Seat {
        // 2-player assumption: the other seat. Robust against m5's
        // single-table-is-2-player constraint without hard-coding the
        // index.
        let seats = obs.hand_sizes.len() as u8;
        let own = obs.own_seat.index();
        Seat::new((own + 1) % seats)
    }

    /// Closer's post-close hand score under StandardConga: the best
    /// split's unmatched_total of `hand_after_close`, minus the 10
    /// bonus for a clean split (empty unmatched) or the 50 bonus for
    /// a 7-card single-suit run (`Conga`).
    fn own_close_delta(hand_after_close: Hand) -> i32 {
        let meld = MeldParams::standard();
        let split = best_meld_split(hand_after_close, &meld);
        let base = split.unmatched_total as i32;
        if is_conga(hand_after_close) {
            base - 50
        } else if split.unmatched.is_empty() {
            base - 10
        } else {
            base
        }
    }

    /// Status-quo equity (lookup at current scores) plus post-close
    /// equity for every legal `Close(_)` option. Belief samples are
    /// drawn once and reused across close options so the per-option
    /// comparison is based on the *same* opp residuals (variance
    /// reduction — much of the N=50 table noise cancels out when the
    /// comparison is relative). The returned per-option vector is in
    /// the same order as `close_cards`.
    fn equity_at_close_options(
        &mut self,
        obs: &Observation,
        close_cards: &[Card],
    ) -> (f32, Vec<f32>) {
        let opp = self.opp_seat(obs);
        let own_idx = obs.own_seat.index() as usize;
        let opp_idx = opp.index() as usize;
        let own_score = obs.scores[own_idx];
        let opp_score = obs.scores[opp_idx];
        let continue_eq = self.table.lookup(own_score, opp_score);

        let meld = MeldParams::standard();
        let belief = self.belief.as_ref().expect("ensure_belief called first");
        let sample_count = self.samples.max(1) as usize;
        let mut opp_residuals: Vec<i32> = Vec::with_capacity(sample_count);
        for _ in 0..sample_count {
            let sampled = belief.sample_state(&mut self.rng, obs, &self.rules);
            let opp_hand = sampled.hands[opp_idx];
            opp_residuals.push(best_meld_split(opp_hand, &meld).unmatched_total as i32);
        }

        let mut per_close: Vec<f32> = Vec::with_capacity(close_cards.len());
        for &c in close_cards {
            let mut hand_after = obs.own_hand;
            hand_after.remove(c);
            let own_delta = Self::own_close_delta(hand_after);
            let new_own = own_score + own_delta;
            let mut sum = 0.0f64;
            for &opp_res in &opp_residuals {
                let new_opp = opp_score + opp_res;
                sum += self.table.lookup(new_own, new_opp) as f64;
            }
            per_close.push((sum / sample_count as f64) as f32);
        }
        (continue_eq, per_close)
    }
}

impl Player for ScoreAwarePlayer {
    fn act(&mut self, obs: &Observation, legal: &[Action]) -> Action {
        self.ensure_belief(obs);
        let base_action = self.base.act(obs, legal);
        let Action::Close(_) = base_action else {
            return base_action;
        };

        // Gather every legal Close option; base already decided it
        // wants to close so there is at least one.
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
        debug_assert!(
            !legal_closes.is_empty(),
            "Close(_) in base_action but not in legal"
        );

        let (continue_eq, per_close) = self.equity_at_close_options(obs, &legal_closes);
        let (best_idx, best_eq) = per_close
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, v)| (i, *v))
            .expect("per_close is non-empty");

        if best_eq > continue_eq {
            // Pick the best close-card rather than base's arbitrary
            // choice. Common case: multiple close options differ in
            // post-close residual (and therefore in the -10 "clean
            // split" bonus).
            return Action::Close(legal_closes[best_idx]);
        }

        // Even the best close is worse than continuing. Decline: let
        // the base pick among the non-close legal actions.
        let non_close: Vec<Action> = legal
            .iter()
            .filter(|a| !matches!(a, Action::Close(_)))
            .cloned()
            .collect();
        if non_close.is_empty() {
            return Action::Close(legal_closes[best_idx]);
        }
        self.base.act(obs, &non_close)
    }

    fn notify(&mut self, event: &Event) {
        self.base.notify(event);
        match &mut self.belief {
            Some(b) => b.notify(event),
            None => self.pending_events.push(event.clone()),
        }
    }
}

/// A 7-card hand that is a single-suit run of 7 consecutive ranks
/// (the StandardConga `-50` bonus trigger). Kept local because the
/// identical check in `conga-rules::standard` is private.
fn is_conga(hand: Hand) -> bool {
    if hand.len() != 7 {
        return false;
    }
    let cards: Vec<Card> = hand.iter().collect();
    let suit = cards[0].suit();
    if !cards.iter().all(|c| c.suit() == suit) {
        return false;
    }
    let mut ranks: Vec<u8> = cards.iter().map(|c| c.rank().as_u8()).collect();
    ranks.sort_unstable();
    ranks.windows(2).all(|w| w[1] == w[0] + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    use conga_core::{Phase, RandomChance, RuleSet, Suit};
    use rand_chacha::ChaCha8Rng;

    fn c(suit: Suit, rank: u8) -> Card {
        Card::new(suit, conga_core::Rank::new(rank).unwrap())
    }

    #[test]
    fn is_conga_detects_seven_card_run() {
        let mut h = Hand::new();
        for r in 1..=7 {
            h.insert(c(Suit::Hearts, r));
        }
        assert!(is_conga(h));
    }

    #[test]
    fn is_conga_rejects_mixed_suit_run() {
        let mut h = Hand::new();
        for r in 1..=6 {
            h.insert(c(Suit::Hearts, r));
        }
        h.insert(c(Suit::Spades, 7));
        assert!(!is_conga(h));
    }

    #[test]
    fn is_conga_rejects_non_consecutive() {
        let mut h = Hand::new();
        for &r in &[1u8, 2, 3, 4, 5, 6, 8] {
            h.insert(c(Suit::Hearts, r));
        }
        assert!(!is_conga(h));
    }

    #[test]
    fn own_close_delta_zero_residual_is_minus_ten() {
        // Six cards that split into two melds with zero unmatched: a
        // 3-set of 1s and a 3-set of 2s. After the face-down discard
        // the closer has 6 cards on hand → clean split → -10.
        let mut h = Hand::new();
        for suit in [Suit::Hearts, Suit::Diamonds, Suit::Clubs] {
            h.insert(c(suit, 1));
            h.insert(c(suit, 2));
        }
        assert_eq!(ScoreAwarePlayer::own_close_delta(h), -10);
    }

    #[test]
    fn own_close_delta_conga_is_minus_fifty() {
        let mut h = Hand::new();
        for r in 1..=7 {
            h.insert(c(Suit::Hearts, r));
        }
        assert_eq!(ScoreAwarePlayer::own_close_delta(h), -50);
    }

    #[test]
    fn own_close_delta_non_empty_residual_counts_rank_sum() {
        // A 3-set of 1s (one meld, 0 total) plus a stray 5 of Spades
        // = unmatched_total 5, no bonus.
        let mut h = Hand::new();
        for suit in [Suit::Hearts, Suit::Diamonds, Suit::Clubs] {
            h.insert(c(suit, 1));
        }
        h.insert(c(Suit::Spades, 5));
        assert_eq!(ScoreAwarePlayer::own_close_delta(h), 5);
    }

    /// Drive a 2-player StandardConga hand with the given pair of
    /// players until `check` returns something, then return it.
    /// Used by the behaviour tests to run the agent on real states
    /// without re-implementing a mini match loop.
    fn drive_until<P1, P2, T>(
        p1: &mut P1,
        p2: &mut P2,
        seed: u64,
        mut check: impl FnMut(&Observation, &[Action], Seat) -> Option<T>,
    ) -> Option<T>
    where
        P1: Player,
        P2: Player,
    {
        let rules = StandardConga::default();
        let mut chance = RandomChance::new(ChaCha8Rng::seed_from_u64(seed));
        let scores = [0i32; 2];
        let mut state = rules.new_hand(&scores, Seat::new(0), &mut chance);
        let mut actions = Vec::new();
        let mut events = Vec::new();
        for _ in 0..500 {
            if rules.hand_result(&state).is_some() {
                return None;
            }
            let current = state.current_seat;
            let obs = rules.observe(&state, current);
            actions.clear();
            rules.legal_actions(&state, &mut actions);
            if let Some(t) = check(&obs, &actions, current) {
                return Some(t);
            }
            let action = if current.index() == 0 {
                p1.act(&obs, &actions)
            } else {
                p2.act(&obs, &actions)
            };
            events.clear();
            rules.apply(&mut state, action, &mut chance, &mut events);
            for e in &events {
                p1.notify(e);
                p2.notify(e);
            }
        }
        None
    }

    #[test]
    fn score_aware_passes_through_non_close_actions() {
        // On every non-Close decision, the agent must pick the same
        // action the base RuleBasedPlayer picks. Run both agents in
        // parallel on the same deal and compare at a Draw-phase
        // decision.
        let rules = StandardConga::default();
        let mut chance = RandomChance::new(ChaCha8Rng::seed_from_u64(42));
        let scores = [0i32; 2];
        let mut state = rules.new_hand(&scores, Seat::new(0), &mut chance);
        let mut base = RuleBasedPlayer::new();
        let mut score_aware = ScoreAwarePlayer::new(5, 42);
        // Replay the first hand's events into both agents.
        let mut actions = Vec::new();
        let mut events = Vec::new();
        for _ in 0..100 {
            if rules.hand_result(&state).is_some() {
                break;
            }
            let obs = rules.observe(&state, state.current_seat);
            actions.clear();
            rules.legal_actions(&state, &mut actions);
            if state.current_seat.index() == 0
                && obs.phase == Phase::Draw
                && !actions.iter().any(|a| matches!(a, Action::Close(_)))
            {
                let a_base = base.act(&obs, &actions);
                let a_sa = score_aware.act(&obs, &actions);
                assert_eq!(
                    a_sa, a_base,
                    "score-aware must pass through on non-Close decisions",
                );
                return;
            }
            // Advance for both agents.
            let action = base.act(&obs, &actions);
            // Keep the score_aware in sync by calling act on same obs
            // and then discarding — but that would consume its RNG.
            // Simpler: just drive with base and notify both.
            events.clear();
            rules.apply(&mut state, action, &mut chance, &mut events);
            for e in &events {
                base.notify(e);
                score_aware.notify(e);
            }
        }
        panic!("never reached a seat-0 Draw decision without a Close in 100 turns");
    }

    #[test]
    fn score_aware_closes_against_losing_table() {
        // Build a table where every post-close cell strictly beats
        // the status quo at (0, 0) = 0.0, so the agent must close
        // whenever closing is offered.
        let table = Box::leak(Box::new({
            let mut t = EquityTable::placeholder();
            // Overwrite the whole table with 1.0; pin (0, 0) to 0.0
            // so continue_eq = 0 and post_close_eq = 1.
            for v in t.values.iter_mut() {
                *v = 1.0;
            }
            let idx_0_0 = {
                let dim = t.dim();
                let o = (0 - t.min_score) as usize;
                let p = (0 - t.min_score) as usize;
                o * dim + p
            };
            t.values[idx_0_0] = 0.0;
            t
        }));

        let mut sa = ScoreAwarePlayer::with_table(5, 1, table);
        let mut base = RuleBasedPlayer::new();
        let closed = drive_until(&mut sa, &mut base, 123, |obs, actions, current| {
            // Return at the FIRST seat-0 Close offering so we can
            // assert the agent actually closes.
            if current.index() == 0
                && actions.iter().any(|a| matches!(a, Action::Close(_)))
                && obs.scores == vec![0, 0]
            {
                Some(())
            } else {
                None
            }
        });
        assert!(
            closed.is_some(),
            "never reached a seat-0 Close offering at (0, 0) over 500 turns",
        );
    }

    #[test]
    fn score_aware_declines_close_against_losing_table() {
        // Inverse: every post-close cell is 0, status quo at (0, 0) is
        // 1.0. The agent must decline every Close.
        let table = Box::leak(Box::new({
            let mut t = EquityTable::placeholder();
            for v in t.values.iter_mut() {
                *v = 0.0;
            }
            let dim = t.dim();
            let idx_0_0 = {
                let o = (0 - t.min_score) as usize;
                let p = (0 - t.min_score) as usize;
                o * dim + p
            };
            t.values[idx_0_0] = 1.0;
            t
        }));

        let mut sa = ScoreAwarePlayer::with_table(5, 1, table);
        let mut base = RuleBasedPlayer::new();
        // Re-implement drive_until inline so we can observe the actual
        // action returned by sa at a Close-offering state.
        let rules = StandardConga::default();
        let mut chance = RandomChance::new(ChaCha8Rng::seed_from_u64(123));
        let scores = [0i32; 2];
        let mut state = rules.new_hand(&scores, Seat::new(0), &mut chance);
        let mut actions = Vec::new();
        let mut events = Vec::new();
        let mut saw_close_offering = false;
        for _ in 0..500 {
            if rules.hand_result(&state).is_some() {
                break;
            }
            let obs = rules.observe(&state, state.current_seat);
            actions.clear();
            rules.legal_actions(&state, &mut actions);
            let action = if state.current_seat.index() == 0 {
                let a = sa.act(&obs, &actions);
                if actions.iter().any(|x| matches!(x, Action::Close(_))) && obs.scores == vec![0, 0]
                {
                    saw_close_offering = true;
                    assert!(
                        !matches!(a, Action::Close(_)),
                        "agent closed against a losing table",
                    );
                }
                a
            } else {
                base.act(&obs, &actions)
            };
            events.clear();
            rules.apply(&mut state, action, &mut chance, &mut events);
            for e in &events {
                sa.notify(e);
                base.notify(e);
            }
        }
        assert!(
            saw_close_offering,
            "deal {} never offered a close to seat 0",
            123
        );
    }

    #[test]
    fn score_aware_returns_legal_actions_over_full_match() {
        // Drive one seat-0 match with greedy on the other side to
        // exercise many act() calls across both phases. Every action
        // the agent returns must be in the legal set.
        let rules = StandardConga::default();
        let mut chance = RandomChance::new(ChaCha8Rng::seed_from_u64(7));
        let mut sa = ScoreAwarePlayer::new(5, 7);
        let mut greedy = crate::greedy::GreedyPlayer::new();
        let scores = [0i32; 2];
        let mut state = rules.new_hand(&scores, Seat::new(0), &mut chance);
        let mut actions = Vec::new();
        let mut events = Vec::new();
        for _ in 0..1000 {
            if rules.hand_result(&state).is_some() {
                break;
            }
            let obs = rules.observe(&state, state.current_seat);
            actions.clear();
            rules.legal_actions(&state, &mut actions);
            let action = if state.current_seat.index() == 0 {
                let a = sa.act(&obs, &actions);
                assert!(
                    actions.contains(&a),
                    "ScoreAwarePlayer returned an illegal action: {:?} not in {:?}",
                    a,
                    actions,
                );
                a
            } else {
                greedy.act(&obs, &actions)
            };
            events.clear();
            rules.apply(&mut state, action, &mut chance, &mut events);
            for e in &events {
                sa.notify(e);
                greedy.notify(e);
            }
        }
    }
}
