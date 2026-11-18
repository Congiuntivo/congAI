//! Events the engine emits when applying actions.
//!
//! The event log stores the ruleset id, every shuffle result and the action
//! list. Those three pieces together reproduce every state in a hand.

use crate::action::DrawSource;
use crate::card::Card;
use crate::meld::Meld;
use crate::seat::Seat;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Bonus {
    /// Closing with 0 unmatched cards.
    MinusTen,
    /// Closing with a 7-card run in one suit.
    MinusFifty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HandTerminal {
    /// A player closed; residuals and bonus scored normally.
    Closed,
    /// The hand stalemated (D31, C4): the reshuffle cap of 3 was hit
    /// without any player closing, or a `DrawStock` found both the
    /// stock and the reshufflable discard portion empty. Every seat
    /// scores 0, no bonus, dealer rotates normally.
    Stalemate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HandResult {
    /// Points scored this hand per seat, in seat order. Includes bonuses.
    pub points: Vec<i32>,
    /// The seat that closed, or `None` if the hand ended without a close.
    pub closer: Option<Seat>,
    /// The bonus the closer earned, if any.
    pub bonus: Option<Bonus>,
    /// How the hand ended. `Stalemate` implies `closer: None`, `bonus:
    /// None` and `points` all zero.
    pub terminal: HandTerminal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Event {
    /// The result of a shuffle, in the order cards will be dealt / drawn. The
    /// engine logs every shuffle so a replay can reproduce it exactly.
    Shuffled(Vec<Card>),
    Dealt {
        seat: Seat,
        hand_size: u8,
    },
    Drew {
        seat: Seat,
        source: DrawSource,
    },
    Discarded {
        seat: Seat,
        card: Card,
    },
    Closed {
        seat: Seat,
        card: Card,
    },
    LaidMeld {
        seat: Seat,
        meld: Meld,
    },
    AddedToMeld {
        seat: Seat,
        card: Card,
        meld_index: u8,
    },
    Passed {
        seat: Seat,
    },
    HandEnded(HandResult),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{Rank, Suit};
    use crate::hand::Hand;
    use crate::meld::MeldKind;

    fn c(suit: Suit, rank: u8) -> Card {
        Card::new(suit, Rank::new(rank).unwrap())
    }

    #[test]
    fn hand_result_construction() {
        let r = HandResult {
            points: vec![-10, 12, 8],
            closer: Some(Seat::new(0)),
            bonus: Some(Bonus::MinusTen),
            terminal: HandTerminal::Closed,
        };
        assert_eq!(r.points.len(), 3);
        assert_eq!(r.closer, Some(Seat::new(0)));
        assert_eq!(r.bonus, Some(Bonus::MinusTen));
        assert_eq!(r.terminal, HandTerminal::Closed);
    }

    #[test]
    fn stalemate_hand_result_shape() {
        let r = HandResult {
            points: vec![0, 0],
            closer: None,
            bonus: None,
            terminal: HandTerminal::Stalemate,
        };
        assert_eq!(r.terminal, HandTerminal::Stalemate);
        assert!(r.closer.is_none() && r.bonus.is_none());
        assert!(r.points.iter().all(|&p| p == 0));
    }

    #[test]
    fn bonuses_are_distinct() {
        assert_ne!(Bonus::MinusTen, Bonus::MinusFifty);
    }

    #[test]
    fn event_variants_round_trip_through_debug() {
        let events = vec![
            Event::Shuffled(vec![c(Suit::Clubs, 1), c(Suit::Spades, 10)]),
            Event::Dealt {
                seat: Seat::new(0),
                hand_size: 7,
            },
            Event::Drew {
                seat: Seat::new(1),
                source: DrawSource::Stock,
            },
            Event::Discarded {
                seat: Seat::new(0),
                card: c(Suit::Hearts, 5),
            },
            Event::Closed {
                seat: Seat::new(1),
                card: c(Suit::Spades, 2),
            },
            Event::LaidMeld {
                seat: Seat::new(0),
                meld: Meld {
                    kind: MeldKind::Set,
                    cards: Hand::from(
                        [c(Suit::Clubs, 7), c(Suit::Diamonds, 7), c(Suit::Hearts, 7)].as_slice(),
                    ),
                },
            },
            Event::AddedToMeld {
                seat: Seat::new(1),
                card: c(Suit::Spades, 7),
                meld_index: 0,
            },
            Event::Passed { seat: Seat::new(0) },
            Event::HandEnded(HandResult {
                points: vec![0, 0],
                closer: None,
                bonus: None,
                terminal: HandTerminal::Stalemate,
            }),
        ];
        // Each event's debug representation is non-empty and distinct.
        let mut seen = std::collections::HashSet::new();
        for e in &events {
            let s = format!("{e:?}");
            assert!(!s.is_empty());
            assert!(seen.insert(s), "duplicate debug repr");
        }
    }

    #[test]
    fn two_hand_results_with_same_fields_are_equal() {
        let r1 = HandResult {
            points: vec![3, 7],
            closer: Some(Seat::new(0)),
            bonus: None,
            terminal: HandTerminal::Closed,
        };
        let r2 = HandResult {
            points: vec![3, 7],
            closer: Some(Seat::new(0)),
            bonus: None,
            terminal: HandTerminal::Closed,
        };
        assert_eq!(r1, r2);
    }
}
