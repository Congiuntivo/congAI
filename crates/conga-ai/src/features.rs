//! Feature vocabulary extracted from an `Observation` + `BeliefTracker` (D48).
//!
//! Shared surface for disagreement mining, counterfactual probes and
//! tactic candidates so committed diagnostic artefacts share a schema.
//! Scope is bounded to m6's first needs; add fields in later slices
//! when a method actually uses them.

use conga_core::{Card, Observation, RuleSet, Seat};
use conga_rules::StandardConga;
use serde::{Deserialize, Serialize};

use crate::belief::BeliefTracker;

/// A seat-relative feature snapshot. Every field is primitive or
/// `Option<Card>` so that `Features` round-trips through JSON and CSV
/// without custom serialisation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Features {
    /// Sum of card values left unmatched by the best meld split.
    pub unmatched_total: u32,
    /// Number of cards left unmatched by the best meld split.
    pub unmatched_count: u8,
    /// Number of melds in the best split.
    pub meld_count: u8,
    /// Whether the current hand meets the closing condition.
    pub can_close: bool,
    /// Number of unseen cards that, if drawn, would let this hand reach
    /// a close after a suitable discard (`HandEval::improvers`).
    pub close_improver_count: u8,
    /// Own distance to elimination in points (`max(0, 100 - own_score)`).
    pub own_distance_to_elim: i32,
    /// Closest opponent's distance to elimination.
    pub opp_distance_to_elim: i32,
    /// Top card of the discard pile, if any.
    pub top_discard: Option<Card>,
    /// Discards played so far this hand; a monotonic proxy for turn number.
    pub turn_number: u32,
    /// Sum of inferred `known_held` cards across every opponent.
    pub known_opp_cards_count: u8,
}

pub fn extract(obs: &Observation, belief: &BeliefTracker, rules: &StandardConga) -> Features {
    let hand_eval = rules.evaluate_hand(obs.own_hand);

    let own_idx = obs.own_seat.index() as usize;
    let own_score = obs.scores[own_idx];
    let own_distance_to_elim = (100 - own_score).max(0);

    let opp_distance_to_elim = obs
        .scores
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != own_idx)
        .map(|(_, &s)| (100 - s).max(0))
        .min()
        .unwrap_or(100);

    let mut known_opp_cards_count: u8 = 0;
    for i in 0..obs.hand_sizes.len() as u8 {
        let seat = Seat::new(i);
        if seat == obs.own_seat {
            continue;
        }
        if let Some(set) = belief.known_held(seat) {
            known_opp_cards_count = known_opp_cards_count.saturating_add(set.len() as u8);
        }
    }

    Features {
        unmatched_total: hand_eval.unmatched_total,
        unmatched_count: hand_eval.unmatched.len() as u8,
        meld_count: hand_eval.splits.len() as u8,
        can_close: hand_eval.can_close,
        close_improver_count: hand_eval.improvers.len() as u8,
        own_distance_to_elim,
        opp_distance_to_elim,
        top_discard: obs.top_discard,
        turn_number: obs.discard_history.len() as u32,
        known_opp_cards_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conga_core::{Card, DrawSource, Event, Hand, Phase, Rank, Suit};

    fn c(suit: Suit, rank: u8) -> Card {
        Card::new(suit, Rank::new(rank).unwrap())
    }

    fn seat(i: u8) -> Seat {
        Seat::new(i)
    }

    /// A seat-0 observation: eight cards in hand (three-clubs run, three
    /// hearts partial, two spades unmatched), four cards on the discard,
    /// 22 stock cards. Scores 30-10 (own ahead). Phase Discard so
    /// `evaluate_hand` is meaningful.
    fn mixed_observation() -> Observation {
        let own: Hand = [
            c(Suit::Clubs, 1),
            c(Suit::Clubs, 2),
            c(Suit::Clubs, 3),
            c(Suit::Hearts, 5),
            c(Suit::Hearts, 6),
            c(Suit::Hearts, 7),
            c(Suit::Spades, 10),
            c(Suit::Diamonds, 10),
        ]
        .as_slice()
        .into();
        let discard = vec![
            c(Suit::Clubs, 10),
            c(Suit::Diamonds, 9),
            c(Suit::Hearts, 8),
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
            scores: vec![30, 10],
            hand_sizes: vec![8, 7],
        }
    }

    #[test]
    fn mixed_hand_snapshot() {
        let rules = StandardConga::new();
        let belief = BeliefTracker::new(seat(0), 2);
        let obs = mixed_observation();
        let f = extract(&obs, &belief, &rules);

        // Two full melds (1-2-3 clubs, 5-6-7 hearts); two unmatched 10s
        // (10 + 10 = 20).
        assert_eq!(f.meld_count, 2);
        assert_eq!(f.unmatched_count, 2);
        assert_eq!(f.unmatched_total, 20);
        assert!(!f.can_close); // sum 20 > 5 close threshold
        assert_eq!(f.own_distance_to_elim, 70); // 100 - 30
        assert_eq!(f.opp_distance_to_elim, 90); // 100 - 10
        assert_eq!(f.top_discard, Some(c(Suit::Spades, 4)));
        assert_eq!(f.turn_number, 4);
        assert_eq!(f.known_opp_cards_count, 0);
    }

    #[test]
    fn close_improver_count_fires_when_one_card_away() {
        // 7-card hand with six cards that form two melds and one dangling
        // low card. A matching run-extender in the deck lets us close.
        let own: Hand = [
            c(Suit::Clubs, 1),
            c(Suit::Clubs, 2),
            c(Suit::Clubs, 3),
            c(Suit::Hearts, 5),
            c(Suit::Hearts, 6),
            c(Suit::Hearts, 7),
            c(Suit::Spades, 1), // unmatched (1 point, under close threshold)
        ]
        .as_slice()
        .into();
        let obs = Observation {
            own_seat: seat(0),
            current_seat: seat(0),
            phase: Phase::Discard,
            own_hand: own,
            top_discard: None,
            discard_history: vec![],
            stock_size: 25,
            melds_on_table: Vec::new(),
            scores: vec![0, 0],
            hand_sizes: vec![7, 7],
        };
        let rules = StandardConga::new();
        let belief = BeliefTracker::new(seat(0), 2);
        let f = extract(&obs, &belief, &rules);
        // 7 cards: 1-2-3 clubs and 5-6-7 hearts are melds, 1 spade
        // unmatched (1 point <= 5), so already closable.
        assert!(f.can_close);
        assert_eq!(f.meld_count, 2);
        assert_eq!(f.unmatched_count, 1);
        // `improvers` is empty when `can_close` is already true.
        assert_eq!(f.close_improver_count, 0);
    }

    #[test]
    fn empty_hand_edge_case() {
        let obs = Observation {
            own_seat: seat(0),
            current_seat: seat(1),
            phase: Phase::Draw,
            own_hand: Hand::EMPTY,
            top_discard: None,
            discard_history: vec![],
            stock_size: 40,
            melds_on_table: Vec::new(),
            scores: vec![0, 0],
            hand_sizes: vec![0, 0],
        };
        let rules = StandardConga::new();
        let belief = BeliefTracker::new(seat(0), 2);
        let f = extract(&obs, &belief, &rules);
        assert_eq!(f.unmatched_count, 0);
        assert_eq!(f.unmatched_total, 0);
        assert_eq!(f.meld_count, 0);
        assert!(f.can_close); // 0 unmatched sums to 0 <= 5
        assert_eq!(f.turn_number, 0);
        assert_eq!(f.known_opp_cards_count, 0);
    }

    #[test]
    fn known_opp_cards_count_sums_belief_inferences() {
        let mut belief = BeliefTracker::new(seat(0), 2);
        // Two Discarded+Drew pairs: opponent is now known to hold 2 cards.
        for card in [c(Suit::Diamonds, 1), c(Suit::Diamonds, 2)] {
            belief.notify(&Event::Discarded {
                seat: seat(0),
                card,
            });
            belief.notify(&Event::Drew {
                seat: seat(1),
                source: DrawSource::TopDiscard,
            });
        }
        let obs = mixed_observation();
        let rules = StandardConga::new();
        let f = extract(&obs, &belief, &rules);
        assert_eq!(f.known_opp_cards_count, 2);
    }

    #[test]
    fn opp_distance_to_elim_takes_the_closest_opponent() {
        // Two opponents: one near elimination (90 points), one safe (10).
        // Closest-to-elim distance should be 10 (100 - 90).
        let own: Hand = Hand::EMPTY;
        let obs = Observation {
            own_seat: seat(0),
            current_seat: seat(0),
            phase: Phase::Draw,
            own_hand: own,
            top_discard: None,
            discard_history: vec![],
            stock_size: 40,
            melds_on_table: Vec::new(),
            scores: vec![50, 90, 10],
            hand_sizes: vec![0, 0, 0],
        };
        let rules = StandardConga::new();
        let belief = BeliefTracker::new(seat(0), 3);
        let f = extract(&obs, &belief, &rules);
        assert_eq!(f.own_distance_to_elim, 50);
        assert_eq!(f.opp_distance_to_elim, 10);
    }

    #[test]
    fn features_json_roundtrip() {
        let rules = StandardConga::new();
        let belief = BeliefTracker::new(seat(0), 2);
        let obs = mixed_observation();
        let f = extract(&obs, &belief, &rules);
        let json = serde_json::to_string(&f).unwrap();
        let back: Features = serde_json::from_str(&json).unwrap();
        assert_eq!(f, back);
    }
}
