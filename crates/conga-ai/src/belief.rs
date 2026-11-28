//! Opponent-hand belief tracker (D32).
//!
//! Watches public events and maintains, per opponent, the set of cards it
//! has inferred they currently hold. Cards leave `known_held` when the
//! opponent places them (`Discarded`, `LaidMeld`, `AddedToMeld`, `Closed`)
//! and arrive when the opponent takes the previous top discard.
//!
//! Unconstrained cards stay uniform. `sample_state` hands each opponent
//! their known cards first and fills the remainder (and the stock) with
//! a Fisher-Yates shuffle of the uncertain pool.

use std::collections::{HashMap, HashSet};

use conga_core::{Card, DrawSource, Event, Hand, Observation, Phase, RuleSet, Seat};
use conga_rules::{StandardConga, State};
use rand_core::Rng;

#[derive(Debug, Clone)]
pub struct BeliefTracker {
    own_seat: Seat,
    seats_count: u8,
    /// Cards each opponent definitely holds right now. Keys are every seat
    /// except `own_seat`.
    known_held: HashMap<Seat, HashSet<Card>>,
    /// The card most recently placed on the top of the discard pile. The
    /// next `Drew { source: TopDiscard }` consumes this to place that card
    /// into that seat's `known_held`.
    last_discarded: Option<Card>,
}

impl BeliefTracker {
    pub fn new(own_seat: Seat, seats_count: u8) -> Self {
        let mut known_held = HashMap::new();
        for i in 0..seats_count {
            let seat = Seat::new(i);
            if seat != own_seat {
                known_held.insert(seat, HashSet::new());
            }
        }
        BeliefTracker {
            own_seat,
            seats_count,
            known_held,
            last_discarded: None,
        }
    }

    pub fn own_seat(&self) -> Seat {
        self.own_seat
    }

    pub fn seats_count(&self) -> u8 {
        self.seats_count
    }

    /// Cards we infer `seat` currently holds. Returns `None` for the
    /// tracker's own seat (we don't track ourselves; use the observation's
    /// own_hand).
    pub fn known_held(&self, seat: Seat) -> Option<&HashSet<Card>> {
        self.known_held.get(&seat)
    }

    /// Drop a specific inferred card from `seat`'s `known_held` without
    /// producing an event. Used by counterfactual probes to answer
    /// "what would the agent do if it did not know `seat` was holding
    /// this card?" without rewriting the event history. Returns true
    /// if the card was removed.
    pub fn remove_known(&mut self, seat: Seat, card: Card) -> bool {
        match self.known_held.get_mut(&seat) {
            Some(set) => set.remove(&card),
            None => false,
        }
    }

    pub fn notify(&mut self, event: &Event) {
        match event {
            Event::Discarded { seat, card } => {
                if let Some(set) = self.known_held.get_mut(seat) {
                    set.remove(card);
                }
                self.last_discarded = Some(*card);
            }
            Event::Drew { seat, source } => {
                match source {
                    DrawSource::TopDiscard => {
                        if let Some(card) = self.last_discarded
                            && let Some(set) = self.known_held.get_mut(seat)
                        {
                            set.insert(card);
                        }
                    }
                    DrawSource::Stock => {}
                }
                // A draw consumes the "freshly discarded" signal: whatever
                // was on top is either now in `seat`'s hand (TopDiscard) or
                // irrelevant going forward (Stock).
                self.last_discarded = None;
            }
            Event::LaidMeld { seat, meld } => {
                if let Some(set) = self.known_held.get_mut(seat) {
                    for card in meld.cards.iter() {
                        set.remove(&card);
                    }
                }
            }
            Event::AddedToMeld { seat, card, .. } => {
                if let Some(set) = self.known_held.get_mut(seat) {
                    set.remove(card);
                }
            }
            Event::Closed { seat, card } => {
                if let Some(set) = self.known_held.get_mut(seat) {
                    set.remove(card);
                }
            }
            Event::HandEnded(_) => {
                for set in self.known_held.values_mut() {
                    set.clear();
                }
                self.last_discarded = None;
            }
            Event::Shuffled(_) | Event::Dealt { .. } | Event::Passed { .. } => {}
        }
    }

    /// Probability, from this seat's vantage point, that `seat` holds
    /// `card` in `obs`. `1.0` for cards we have inferred, `0.0` for cards
    /// already seen or known to live elsewhere, and a uniform
    /// `(uncertain_slots_for_seat) / (uncertain_pool_size)` otherwise.
    pub fn prob_holds(&self, card: Card, seat: Seat, obs: &Observation) -> f64 {
        if seat == self.own_seat {
            return if obs.own_hand.contains(card) {
                1.0
            } else {
                0.0
            };
        }

        let seen = seen_cards(obs);
        if seen.contains(card) {
            return 0.0;
        }
        for (s, set) in &self.known_held {
            if set.contains(&card) {
                return if *s == seat { 1.0 } else { 0.0 };
            }
        }

        let total_known: usize = self.known_held.values().map(|s| s.len()).sum();
        let unseen_total = 40usize.saturating_sub(seen.len() as usize);
        let uncertain_pool = unseen_total.saturating_sub(total_known);
        if uncertain_pool == 0 {
            return 0.0;
        }

        let hand_size = obs.hand_sizes[seat.index() as usize] as usize;
        let known_for_seat = self.known_held.get(&seat).map(|s| s.len()).unwrap_or(0);
        let uncertain_slots = hand_size.saturating_sub(known_for_seat);
        if uncertain_slots == 0 {
            return 0.0;
        }
        uncertain_slots as f64 / uncertain_pool as f64
    }

    /// A full `State` consistent with `obs` and with every `known_held`
    /// constraint. Each opponent receives their `known_held` cards first;
    /// the remainder of each hand and the stock is drawn uniformly from
    /// the uncertain pool via Fisher-Yates.
    pub fn sample_state(
        &self,
        rng: &mut dyn Rng,
        obs: &Observation,
        rules: &StandardConga,
    ) -> State {
        assert!(
            matches!(obs.phase, Phase::Draw | Phase::Discard),
            "BeliefTracker::sample_state only supports Draw/Discard phases (got {:?})",
            obs.phase
        );
        assert_eq!(
            self.own_seat, obs.own_seat,
            "tracker initialised for seat {:?} but given an observation for seat {:?}",
            self.own_seat, obs.own_seat
        );
        let seats_count = self.seats_count;
        assert_eq!(obs.hand_sizes.len() as u8, seats_count);

        let seen = seen_cards(obs);
        let mut known_mask = Hand::EMPTY;
        for set in self.known_held.values() {
            for &c in set {
                known_mask.insert(c);
            }
        }

        // Uncertain pool: deck minus seen minus already-inferred.
        let mut pool: Vec<Card> = rules
            .deck()
            .all_cards()
            .filter(|c| !seen.contains(*c) && !known_mask.contains(*c))
            .collect();
        let n = pool.len();
        for i in (1..n).rev() {
            let j = (rng.next_u32() as usize) % (i + 1);
            pool.swap(i, j);
        }

        let mut hands: Vec<Hand> = vec![Hand::EMPTY; seats_count as usize];
        hands[obs.own_seat.index() as usize] = obs.own_hand;

        let mut cursor = 0usize;
        for seat_idx in 0..seats_count as usize {
            if seat_idx == obs.own_seat.index() as usize {
                continue;
            }
            let seat = Seat::new(seat_idx as u8);
            let mut h = Hand::EMPTY;
            if let Some(known) = self.known_held.get(&seat) {
                for &c in known {
                    h.insert(c);
                }
            }
            let known_count = h.len() as usize;
            let hand_size = obs.hand_sizes[seat_idx] as usize;
            assert!(
                known_count <= hand_size,
                "known_held for {:?} ({} cards) exceeds hand_sizes ({} cards)",
                seat,
                known_count,
                hand_size
            );
            let needed = hand_size - known_count;
            for k in 0..needed {
                h.insert(pool[cursor + k]);
            }
            cursor += needed;
            hands[seat_idx] = h;
        }

        let stock = pool[cursor..].to_vec();
        let dealer = Seat::new((obs.current_seat.index() + seats_count - 1) % seats_count);

        State {
            seats_count,
            hands,
            stock,
            discard_pile: obs.discard_history.clone(),
            scores: obs.scores.clone(),
            dealer,
            current_seat: obs.current_seat,
            phase: obs.phase,
            melds_on_table: obs.melds_on_table.clone(),
            first_turn_done: vec![true; seats_count as usize],
            must_not_discard: None,
            closer: None,
            closer_hand_at_close: None,
            closer_has_opened: false,
            showdown_consecutive_passes: 0,
            face_down_close: None,
            reshuffles_this_hand: 0,
            stalemate_pending: false,
        }
    }
}

fn seen_cards(obs: &Observation) -> Hand {
    let mut seen = obs.own_hand;
    for &c in &obs.discard_history {
        seen.insert(c);
    }
    for meld in &obs.melds_on_table {
        seen = seen.union(meld.cards);
    }
    seen
}

#[cfg(test)]
mod tests {
    use super::*;
    use conga_core::{Card, Meld, MeldKind, Rank, RuleSet, Suit};
    use rand_chacha::ChaCha8Rng;
    use rand_core::SeedableRng;

    fn c(suit: Suit, rank: u8) -> Card {
        Card::new(suit, Rank::new(rank).unwrap())
    }

    fn seat(i: u8) -> Seat {
        Seat::new(i)
    }

    #[test]
    fn new_initialises_opponents_only() {
        let t = BeliefTracker::new(seat(0), 2);
        assert!(t.known_held(seat(0)).is_none());
        assert_eq!(t.known_held(seat(1)).unwrap().len(), 0);
    }

    #[test]
    fn discard_then_top_discard_pickup_adds_card_to_known_held() {
        let mut t = BeliefTracker::new(seat(0), 2);
        t.notify(&Event::Discarded {
            seat: seat(0),
            card: c(Suit::Clubs, 7),
        });
        t.notify(&Event::Drew {
            seat: seat(1),
            source: DrawSource::TopDiscard,
        });
        assert!(t.known_held(seat(1)).unwrap().contains(&c(Suit::Clubs, 7)));
    }

    #[test]
    fn opponent_discarding_a_known_card_removes_it() {
        let mut t = BeliefTracker::new(seat(0), 2);
        t.notify(&Event::Discarded {
            seat: seat(0),
            card: c(Suit::Clubs, 7),
        });
        t.notify(&Event::Drew {
            seat: seat(1),
            source: DrawSource::TopDiscard,
        });
        t.notify(&Event::Discarded {
            seat: seat(1),
            card: c(Suit::Clubs, 7),
        });
        assert!(!t.known_held(seat(1)).unwrap().contains(&c(Suit::Clubs, 7)));
    }

    #[test]
    fn drew_from_stock_does_not_add_known_held() {
        let mut t = BeliefTracker::new(seat(0), 2);
        t.notify(&Event::Discarded {
            seat: seat(0),
            card: c(Suit::Clubs, 7),
        });
        t.notify(&Event::Drew {
            seat: seat(1),
            source: DrawSource::Stock,
        });
        assert!(t.known_held(seat(1)).unwrap().is_empty());
    }

    #[test]
    fn lay_meld_removes_meld_cards() {
        let mut t = BeliefTracker::new(seat(0), 2);
        // Seed known_held via Discarded + TopDiscard three times.
        for rank in 1u8..=3 {
            t.notify(&Event::Discarded {
                seat: seat(0),
                card: c(Suit::Clubs, rank),
            });
            t.notify(&Event::Drew {
                seat: seat(1),
                source: DrawSource::TopDiscard,
            });
        }
        assert_eq!(t.known_held(seat(1)).unwrap().len(), 3);
        let meld = Meld {
            kind: MeldKind::Run,
            cards: [c(Suit::Clubs, 1), c(Suit::Clubs, 2), c(Suit::Clubs, 3)]
                .as_slice()
                .into(),
        };
        t.notify(&Event::LaidMeld {
            seat: seat(1),
            meld,
        });
        assert!(t.known_held(seat(1)).unwrap().is_empty());
    }

    #[test]
    fn add_to_meld_removes_card() {
        let mut t = BeliefTracker::new(seat(0), 2);
        t.notify(&Event::Discarded {
            seat: seat(0),
            card: c(Suit::Clubs, 7),
        });
        t.notify(&Event::Drew {
            seat: seat(1),
            source: DrawSource::TopDiscard,
        });
        t.notify(&Event::AddedToMeld {
            seat: seat(1),
            card: c(Suit::Clubs, 7),
            meld_index: 0,
        });
        assert!(t.known_held(seat(1)).unwrap().is_empty());
    }

    #[test]
    fn closed_removes_close_card() {
        let mut t = BeliefTracker::new(seat(0), 2);
        t.notify(&Event::Discarded {
            seat: seat(0),
            card: c(Suit::Hearts, 5),
        });
        t.notify(&Event::Drew {
            seat: seat(1),
            source: DrawSource::TopDiscard,
        });
        t.notify(&Event::Closed {
            seat: seat(1),
            card: c(Suit::Hearts, 5),
        });
        assert!(t.known_held(seat(1)).unwrap().is_empty());
    }

    #[test]
    fn hand_ended_clears_state() {
        let mut t = BeliefTracker::new(seat(0), 2);
        t.notify(&Event::Discarded {
            seat: seat(0),
            card: c(Suit::Clubs, 7),
        });
        t.notify(&Event::Drew {
            seat: seat(1),
            source: DrawSource::TopDiscard,
        });
        assert!(!t.known_held(seat(1)).unwrap().is_empty());
        t.notify(&Event::HandEnded(conga_core::HandResult {
            points: vec![0, 0],
            closer: None,
            bonus: None,
            terminal: conga_core::HandTerminal::Stalemate,
        }));
        assert!(t.known_held(seat(1)).unwrap().is_empty());
    }

    fn mid_hand_observation_two_seats() -> Observation {
        // Seat-0 vantage, mid-Discard phase. Opponent has 7 cards; stock
        // and discard sizes are consistent with 40 - 7 - 7 - 4 = 22.
        let own: Hand = [
            c(Suit::Clubs, 1),
            c(Suit::Clubs, 2),
            c(Suit::Clubs, 3),
            c(Suit::Hearts, 7),
            c(Suit::Hearts, 8),
            c(Suit::Hearts, 9),
            c(Suit::Spades, 10),
        ]
        .as_slice()
        .into();
        let discard = vec![
            c(Suit::Clubs, 10),
            c(Suit::Diamonds, 10),
            c(Suit::Hearts, 10),
            c(Suit::Spades, 4),
        ];
        Observation {
            own_seat: seat(0),
            current_seat: seat(0),
            phase: Phase::Discard,
            own_hand: own,
            top_discard: Some(c(Suit::Spades, 4)),
            discard_history: discard,
            stock_size: 22,
            melds_on_table: Vec::new(),
            scores: vec![0, 0],
            hand_sizes: vec![7, 7],
        }
    }

    #[test]
    fn prob_holds_is_one_for_inferred_and_zero_for_seen() {
        // Build an event history that places 2D into seat 1's known_held
        // without leaving 2D on the discard pile (the pickup consumed it).
        // The observation's discard_history reflects the real pile, which
        // does not contain 2D.
        let mut t = BeliefTracker::new(seat(0), 2);
        t.notify(&Event::Discarded {
            seat: seat(0),
            card: c(Suit::Diamonds, 2),
        });
        t.notify(&Event::Drew {
            seat: seat(1),
            source: DrawSource::TopDiscard,
        });
        let obs = mid_hand_observation_two_seats();

        // 1C is in own_hand: seen => 0.0.
        assert_eq!(t.prob_holds(c(Suit::Clubs, 1), seat(1), &obs), 0.0);
        // 10C is in discard_history: seen => 0.0.
        assert_eq!(t.prob_holds(c(Suit::Clubs, 10), seat(1), &obs), 0.0);
        // 2D is in known_held[seat 1]: inferred => 1.0.
        assert_eq!(t.prob_holds(c(Suit::Diamonds, 2), seat(1), &obs), 1.0);
        // 5S is unseen and uninferred: uniform probability.
        let p = t.prob_holds(c(Suit::Spades, 5), seat(1), &obs);
        assert!(
            p > 0.0 && p < 1.0,
            "unseen prob should be fractional, got {p}"
        );
    }

    #[test]
    fn sample_state_respects_known_held() {
        let rules = StandardConga::new();
        let mut t = BeliefTracker::new(seat(0), 2);
        // Opponent picks up 3 cards from the discard over time. Each pickup
        // consumes the top of the pile, so these cards do not appear in
        // the observation's discard_history.
        for card in [
            c(Suit::Diamonds, 1),
            c(Suit::Diamonds, 2),
            c(Suit::Diamonds, 3),
        ] {
            t.notify(&Event::Discarded {
                seat: seat(0),
                card,
            });
            t.notify(&Event::Drew {
                seat: seat(1),
                source: DrawSource::TopDiscard,
            });
        }
        let obs = mid_hand_observation_two_seats();

        let mut rng = ChaCha8Rng::seed_from_u64(42);
        for _ in 0..20 {
            let sample = t.sample_state(&mut rng, &obs, &rules);
            assert_eq!(rules.observe(&sample, seat(0)), obs);
            for &known in t.known_held(seat(1)).unwrap() {
                assert!(
                    sample.hands[1].contains(known),
                    "sample missing known-held card {known}"
                );
            }
        }
    }

    #[test]
    fn sample_state_falls_back_to_uniform_when_no_constraints() {
        let rules = StandardConga::new();
        let t = BeliefTracker::new(seat(0), 2);
        let obs = mid_hand_observation_two_seats();
        let mut rng = ChaCha8Rng::seed_from_u64(0);
        let sample = t.sample_state(&mut rng, &obs, &rules);
        assert_eq!(rules.observe(&sample, seat(0)), obs);
    }
}
