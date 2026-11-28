//! `SearchPlayer`: PIMC (flat determinized rollouts) with shared
//! determinization (D33). Per iteration:
//!
//! 1. Sample a world consistent with the observation and the belief
//!    tracker's `known_held` constraints.
//! 2. For every legal action, apply it to a clone of that world and roll
//!    out with [`RolloutPolicy`] driving both seats.
//! 3. Score the leaf in *match-equity units* (D55):
//!    - On a true terminal: `committed_table().lookup(own_total, opp_total)`
//!      where totals are the running match scores plus the hand's `points`
//!      (2-player). For 3-4 players we fall back to the legacy
//!      `-points[own] + opp_weight * sum(opp)` formula because the committed
//!      equity table is 2-player only.
//!    - At depth `leaf_depth` with [`LeafEvaluator::ValueModel`] active and
//!      the state not in `Phase::Showdown`: call `encode(leaf_obs, belief)`
//!      and the model's `forward` as the leaf value. Showdown entry during
//!      a rollout continues with the existing rollout policy to a true
//!      terminal (the network's training distribution doesn't cover it;
//!      see D54).
//!
//! After `samples` iterations the argmax of per-action totals wins; ties
//! break by legal-list order for determinism.
//!
//! `Phase::Showdown` at the *root* falls back to greedy because
//! `sample_state` only covers Draw / Discard (D34); the fast-path also
//! short-circuits when the engine offers only one legal action.

use std::sync::Arc;

use conga_core::{
    Action, ActionList, Event, HandResult, HandTerminal, Observation, Phase, RandomChance, RuleSet,
    Seat,
};
use conga_rules::{StandardConga, State};
use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};

use crate::belief::BeliefTracker;
use crate::encoding;
use crate::equity::committed_table;
use crate::greedy::GreedyPlayer;
use crate::player::Player;
use crate::value_model::ValueModel;

/// Which policy drives both seats inside a rollout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RolloutPolicy {
    /// Deterministic greedy (D27). The default.
    Greedy,
    /// Uniform random over legal actions. Adds variance so action-value
    /// averages can actually separate even when greedy would make the
    /// same choice.
    Random,
}

/// What produces the value at a rollout leaf. `Greedy` plays the rollout
/// all the way to terminal (matching the m4 scaffolding behaviour, now
/// converted to equity units). `ValueModel` plays `leaf_depth` plies with
/// the rollout policy and then calls the network on the leaf observation.
#[derive(Debug, Clone, Default)]
pub enum LeafEvaluator {
    /// Legacy: roll out to terminal, equity-convert on 2P or fall back to
    /// the raw-points formula on 3-4P.
    #[default]
    Greedy,
    /// Call `ValueModel::forward` on the encoded leaf observation at depth
    /// `leaf_depth`.
    ValueModel(Arc<ValueModel>),
}

/// Default number of post-root plies to play before the value model
/// scores the leaf (D55).
pub const DEFAULT_LEAF_DEPTH: u32 = 3;

#[derive(Debug, Clone)]
pub struct SearchPlayer {
    rules: StandardConga,
    belief: Option<BeliefTracker>,
    /// Events received before the first `act` call. Replayed into the
    /// belief tracker on its lazy construction so the tracker sees every
    /// event in order regardless of when we figured out our own seat.
    pending_events: Vec<Event>,
    rng: ChaCha8Rng,
    samples: u32,
    /// Total rollouts executed so far. Useful for logging rollouts/sec.
    rollouts: u64,
    /// Breakdown of rollout terminal types. Diagnostic: large stalemate
    /// counts flatten the value signal across actions, see D33 notes.
    stalemate_rollouts: u64,
    own_close_rollouts: u64,
    opp_close_rollouts: u64,
    /// Calls to `ValueModel::forward` from within a rollout. Zero when
    /// the leaf evaluator is `Greedy`. Used by slice 6's cost metric.
    neural_evals: u64,
    rollout_policy: RolloutPolicy,
    /// Weight applied to the sum of opponents' terminal points when
    /// falling back to the 3-4P raw-points scoring path. `value = -points[own] +
    /// opp_weight * sum(points[opp])`. Default 0.0. No-op in 2-player play
    /// because the equity path replaces this (D55).
    opp_weight: f64,
    leaf_evaluator: LeafEvaluator,
    leaf_depth: u32,
}

impl SearchPlayer {
    pub fn new(samples: u32, seed: u64) -> Self {
        SearchPlayer {
            rules: StandardConga::default(),
            belief: None,
            pending_events: Vec::new(),
            rng: ChaCha8Rng::seed_from_u64(seed),
            samples,
            rollouts: 0,
            stalemate_rollouts: 0,
            own_close_rollouts: 0,
            opp_close_rollouts: 0,
            neural_evals: 0,
            rollout_policy: RolloutPolicy::Greedy,
            opp_weight: 0.0,
            leaf_evaluator: LeafEvaluator::Greedy,
            leaf_depth: DEFAULT_LEAF_DEPTH,
        }
    }

    pub fn with_rollout_policy(mut self, policy: RolloutPolicy) -> Self {
        self.rollout_policy = policy;
        self
    }

    pub fn with_opponent_weight(mut self, w: f64) -> Self {
        self.opp_weight = w;
        self
    }

    pub fn with_leaf_evaluator(mut self, evaluator: LeafEvaluator) -> Self {
        self.leaf_evaluator = evaluator;
        self
    }

    pub fn with_leaf_depth(mut self, depth: u32) -> Self {
        self.leaf_depth = depth;
        self
    }

    pub fn samples(&self) -> u32 {
        self.samples
    }

    pub fn rollouts(&self) -> u64 {
        self.rollouts
    }

    pub fn stalemate_rollouts(&self) -> u64 {
        self.stalemate_rollouts
    }

    pub fn own_close_rollouts(&self) -> u64 {
        self.own_close_rollouts
    }

    pub fn opp_close_rollouts(&self) -> u64 {
        self.opp_close_rollouts
    }

    pub fn neural_evals(&self) -> u64 {
        self.neural_evals
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

    /// Match-equity for a hand terminal, in `[0, 1]` for 2-player
    /// play. For 3-4 players the equity table is absent so we return the
    /// legacy `-own_points + opp_weight * sum(opp_points)` scalar; the
    /// m7 exit test is 2P only, so this path is only for future seat
    /// counts.
    fn score_terminal(&self, state: &State, result: &HandResult, own_seat: Seat) -> f64 {
        let own_idx = own_seat.index() as usize;
        if state.seats_count == 2 {
            let opp_idx = 1 - own_idx;
            let own_total = state.scores[own_idx] + result.points[own_idx];
            let opp_total = state.scores[opp_idx] + result.points[opp_idx];
            committed_table().lookup(own_total, opp_total) as f64
        } else {
            let own_points = result.points[own_idx] as f64;
            let opp_sum: f64 = result
                .points
                .iter()
                .enumerate()
                .filter_map(|(i, &p)| if i == own_idx { None } else { Some(p as f64) })
                .sum();
            -own_points + self.opp_weight * opp_sum
        }
    }

    fn rollout(
        &mut self,
        mut state: State,
        own_seat: Seat,
        chance: &mut RandomChance<ChaCha8Rng>,
    ) -> f64 {
        let mut greedy = GreedyPlayer;
        let mut legal = ActionList::new();
        let mut events = Vec::new();
        let mut ply: u32 = 0;
        loop {
            if let Some(result) = self.rules.hand_result(&state) {
                match result.terminal {
                    HandTerminal::Stalemate => self.stalemate_rollouts += 1,
                    HandTerminal::Closed => match result.closer {
                        Some(s) if s == own_seat => self.own_close_rollouts += 1,
                        Some(_) => self.opp_close_rollouts += 1,
                        None => {}
                    },
                }
                return self.score_terminal(&state, &result, own_seat);
            }

            // Depth-D leaf check: only engages for `LeafEvaluator::ValueModel`
            // and only in Draw / Discard phases. Showdown-mid-rollout
            // continues with the rollout policy to a true terminal because
            // the value model's training distribution excludes Showdown (D54).
            if ply >= self.leaf_depth
                && !matches!(state.phase, Phase::Showdown)
                && let LeafEvaluator::ValueModel(model) = &self.leaf_evaluator
            {
                let leaf_obs = self.rules.observe(&state, own_seat);
                let belief = self.belief.as_ref().expect("belief initialised");
                let x = encoding::encode(&leaf_obs, belief);
                let v = model.forward(&x) as f64;
                self.neural_evals += 1;
                return v;
            }

            legal.clear();
            self.rules.legal_actions(&state, &mut legal);
            if legal.is_empty() {
                // Defensive: no legal actions and no terminal. Return
                // neutral equity (0.5) for 2P and 0 otherwise.
                return if state.seats_count == 2 { 0.5 } else { 0.0 };
            }
            let obs = self.rules.observe(&state, state.current_seat);
            let action = match self.rollout_policy {
                RolloutPolicy::Greedy => greedy.act(&obs, &legal),
                RolloutPolicy::Random => {
                    let idx = (chance.rng.next_u32() as usize) % legal.len();
                    legal[idx].clone()
                }
            };
            events.clear();
            self.rules.apply(&mut state, action, chance, &mut events);
            ply += 1;
        }
    }
}

impl Player for SearchPlayer {
    fn act(&mut self, obs: &Observation, legal: &[Action]) -> Action {
        self.ensure_belief(obs);

        if legal.len() == 1 {
            return legal[0].clone();
        }
        if obs.phase == Phase::Showdown {
            return GreedyPlayer.act(obs, legal);
        }

        let mut values = vec![0f64; legal.len()];

        for _ in 0..self.samples {
            let sample = self
                .belief
                .as_ref()
                .expect("belief initialised by ensure_belief")
                .sample_state(&mut self.rng, obs, &self.rules);
            for (i, action) in legal.iter().enumerate() {
                let mut sim = sample.clone();
                let sub_seed = self.rng.next_u64();
                let mut sim_chance = RandomChance::new(ChaCha8Rng::seed_from_u64(sub_seed));
                let mut events = Vec::new();
                self.rules
                    .apply(&mut sim, action.clone(), &mut sim_chance, &mut events);
                let v = self.rollout(sim, obs.own_seat, &mut sim_chance);
                values[i] += v;
                self.rollouts += 1;
            }
        }

        let mut best = 0;
        for i in 1..legal.len() {
            if values[i] > values[best] {
                best = i;
            }
        }
        legal[best].clone()
    }

    fn notify(&mut self, event: &Event) {
        match &mut self.belief {
            Some(b) => b.notify(event),
            None => self.pending_events.push(event.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_model::toy_proxy;
    use conga_core::RandomChance;
    use rand_chacha::ChaCha8Rng;
    use rand_core::SeedableRng;

    fn chance(seed: u64) -> RandomChance<ChaCha8Rng> {
        RandomChance::new(ChaCha8Rng::seed_from_u64(seed))
    }

    #[test]
    fn leaf_evaluator_default_is_greedy() {
        let p = SearchPlayer::new(1, 0);
        assert!(matches!(p.leaf_evaluator, LeafEvaluator::Greedy));
        assert_eq!(p.leaf_depth, DEFAULT_LEAF_DEPTH);
    }

    #[test]
    fn act_returns_a_legal_action_on_first_turn() {
        let rules = StandardConga::default();
        let mut c = chance(1);
        let state = rules.new_hand(&[0, 0], Seat::new(0), &mut c);
        let obs = rules.observe(&state, state.current_seat);
        let mut legal = ActionList::new();
        rules.legal_actions(&state, &mut legal);

        let mut player = SearchPlayer::new(2, 1234);
        let action = player.act(&obs, &legal);
        assert!(
            legal.iter().any(|a| a == &action),
            "search returned non-legal action {action:?}"
        );
    }

    #[test]
    fn act_is_reproducible_from_the_seed() {
        let rules = StandardConga::default();
        let mut c = chance(7);
        let state = rules.new_hand(&[0, 0], Seat::new(0), &mut c);
        let obs = rules.observe(&state, state.current_seat);
        let mut legal = ActionList::new();
        rules.legal_actions(&state, &mut legal);

        let mut p1 = SearchPlayer::new(3, 42);
        let mut p2 = SearchPlayer::new(3, 42);
        let a = p1.act(&obs, &legal);
        let b = p2.act(&obs, &legal);
        assert_eq!(a, b);
    }

    #[test]
    fn single_legal_action_fast_path() {
        let rules = StandardConga::default();
        let mut c = chance(2);
        let state = rules.new_hand(&[0, 0], Seat::new(0), &mut c);
        let obs = rules.observe(&state, state.current_seat);
        let only = Action::DrawStock;
        let mut player = SearchPlayer::new(1000, 0);
        let action = player.act(&obs, std::slice::from_ref(&only));
        assert_eq!(action, only);
    }

    #[test]
    fn showdown_phase_falls_back_to_greedy() {
        use conga_core::{Card, Hand, Meld, MeldKind, Observation, Phase, Rank, Suit};

        fn cd(suit: Suit, rank: u8) -> Card {
            Card::new(suit, Rank::new(rank).unwrap())
        }

        let own: Hand = [
            cd(Suit::Spades, 5),
            cd(Suit::Spades, 6),
            cd(Suit::Spades, 7),
            cd(Suit::Clubs, 2),
            cd(Suit::Hearts, 9),
        ]
        .as_slice()
        .into();
        let obs = Observation {
            own_seat: Seat::new(0),
            current_seat: Seat::new(0),
            phase: Phase::Showdown,
            own_hand: own,
            top_discard: Some(cd(Suit::Diamonds, 10)),
            discard_history: vec![cd(Suit::Diamonds, 10)],
            stock_size: 0,
            melds_on_table: vec![],
            scores: vec![0, 0],
            hand_sizes: vec![5, 0],
        };

        let lay = Action::LayMeld(Meld {
            kind: MeldKind::Run,
            cards: [
                cd(Suit::Spades, 5),
                cd(Suit::Spades, 6),
                cd(Suit::Spades, 7),
            ]
            .as_slice()
            .into(),
        });
        let legal = vec![lay.clone(), Action::Pass];

        let mut search = SearchPlayer::new(10, 0);
        let got = search.act(&obs, &legal);
        let mut greedy = GreedyPlayer;
        let expected = greedy.act(&obs, &legal);
        assert_eq!(got, expected);
        assert_eq!(got, lay);
    }

    #[test]
    fn value_leaf_depth_zero_evaluates_root_action() {
        // With leaf_depth=0 and a value model, the rollout returns the
        // model's forward output on the post-action state without playing
        // any further plies. We build a tiny value model whose only
        // distinguishing signal is the current-seat one-hot block at
        // OFF_CURRENT_SEAT: it rewards a leaf where it's *our* turn and
        // penalises a leaf where it's the *opponent's* turn. In 2-player
        // play, applying our action flips the current seat, so the root
        // action keeps the model's output low. The point is simply that
        // the model is *called* at depth 0 and its output drives the
        // argmax; we assert the counter increments and the action is
        // legal.
        let rules = StandardConga::default();
        let mut c = chance(3);
        let state = rules.new_hand(&[0, 0], Seat::new(0), &mut c);
        let obs = rules.observe(&state, state.current_seat);
        let mut legal = ActionList::new();
        rules.legal_actions(&state, &mut legal);

        let model = Arc::new(toy_proxy());
        let mut player = SearchPlayer::new(2, 5)
            .with_leaf_evaluator(LeafEvaluator::ValueModel(model))
            .with_leaf_depth(0);
        let action = player.act(&obs, &legal);
        assert!(legal.iter().any(|a| a == &action));
        assert_eq!(
            player.neural_evals(),
            2 * legal.len() as u64,
            "forward must run once per (sample, legal action)"
        );
    }

    #[test]
    fn value_model_gets_called_during_rollout() {
        // A fast non-ignored smoke: one `act` call with a value model at
        // `leaf_depth = 2` must trigger at least one `forward` call.
        // Confirms the leaf path is wired without running a full match.
        let rules = StandardConga::default();
        let mut c = chance(0);
        let state = rules.new_hand(&[0, 0], Seat::new(0), &mut c);
        let obs = rules.observe(&state, state.current_seat);
        let mut legal = ActionList::new();
        rules.legal_actions(&state, &mut legal);

        let model = Arc::new(toy_proxy());
        let mut player = SearchPlayer::new(2, 7)
            .with_leaf_evaluator(LeafEvaluator::ValueModel(model))
            .with_leaf_depth(2);
        let _ = player.act(&obs, &legal);
        assert!(
            player.neural_evals() > 0,
            "forward must run at least once per act with ValueModel leaf"
        );
    }

    #[test]
    #[ignore = "slow (full matches, ~minutes debug / ~tens of seconds release); run with --release --ignored"]
    fn value_search_toy_beats_random() {
        use crate::eval::run_paired;
        use crate::random::RandomPlayer;

        let rules = StandardConga::default();
        let model = Arc::new(toy_proxy());
        let games = 10u32;
        let mut wins = 0u32;
        for seed in 0..games {
            let model = model.clone();
            let builder_value = move || {
                SearchPlayer::new(6, 42)
                    .with_leaf_evaluator(LeafEvaluator::ValueModel(model.clone()))
                    .with_leaf_depth(2)
            };
            let builder_random = || RandomPlayer::<ChaCha8Rng>::from_seed(0);
            let pair = run_paired(
                &rules,
                builder_value,
                builder_random,
                Seat::new(0),
                seed as u64,
            );
            wins += pair.a_wins();
        }
        let total = games * 2;
        let rate = wins as f64 / total as f64;
        assert!(
            rate > 0.7,
            "value-search (toy) vs random at {total} games: {wins} wins = {rate:.2}, expected > 0.70"
        );
    }
}
