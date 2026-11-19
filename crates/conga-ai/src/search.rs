//! `SearchPlayer`: PIMC (flat determinized rollouts) with shared
//! determinization (D33). Per iteration:
//!
//! 1. Sample a world consistent with the observation and the belief
//!    tracker's `known_held` constraints.
//! 2. For every legal action, apply it to a clone of that world and roll
//!    out to hand terminal with `GreedyPlayer` driving both seats.
//! 3. Score the terminal from the acting seat's view: `-points[own]`
//!    (lower residual + any closer bonus = higher value).
//!
//! After `samples` iterations the argmax of per-action totals wins; ties
//! break by legal-list order for determinism.
//!
//! `Phase::Showdown` falls back to greedy because sample_state only
//! covers Draw / Discard (D34); the fast-path also short-circuits when
//! the engine offers only one legal action.

use conga_core::{Action, ActionList, Event, Observation, Phase, RandomChance, RuleSet, Seat};
use conga_rules::{StandardConga, State};
use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};

use crate::belief::BeliefTracker;
use crate::greedy::GreedyPlayer;
use crate::player::Player;

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
        }
    }

    pub fn samples(&self) -> u32 {
        self.samples
    }

    pub fn rollouts(&self) -> u64 {
        self.rollouts
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

    fn rollout(
        &self,
        mut state: State,
        own_seat: Seat,
        chance: &mut RandomChance<ChaCha8Rng>,
    ) -> f64 {
        let mut greedy = GreedyPlayer;
        let mut legal = ActionList::new();
        let mut events = Vec::new();
        loop {
            if let Some(result) = self.rules.hand_result(&state) {
                return -(result.points[own_seat.index() as usize] as f64);
            }
            legal.clear();
            self.rules.legal_actions(&state, &mut legal);
            if legal.is_empty() {
                return 0.0;
            }
            let obs = self.rules.observe(&state, state.current_seat);
            let action = greedy.act(&obs, &legal);
            events.clear();
            self.rules.apply(&mut state, action, chance, &mut events);
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
    use conga_core::RandomChance;
    use rand_chacha::ChaCha8Rng;
    use rand_core::SeedableRng;

    fn chance(seed: u64) -> RandomChance<ChaCha8Rng> {
        RandomChance::new(ChaCha8Rng::seed_from_u64(seed))
    }

    #[test]
    fn act_returns_a_legal_action_on_first_turn() {
        let rules = StandardConga::default();
        let mut c = chance(1);
        let state = rules.new_hand(&[0, 0], Seat::new(0), &mut c);
        // Starter = Seat(1), discard phase.
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
        // Force a singleton legal set to exercise the fast path.
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

        // Hand-built showdown observation: closer's residual (seat 1
        // closed, seat 0 is the non-closer being asked to act). Seat 0
        // holds a 5S, 6S, 7S run that would be laid down; greedy picks
        // LayMeld first.
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
}
