//! Seat-relative view of the game state.
//!
//! The engine derives an Observation from the full state and a seat. This is
//! the only place hidden information is filtered: everything a seat can see
//! during play lives here, nothing a seat should not see does.

use crate::card::Card;
use crate::hand::Hand;
use crate::meld::Meld;
use crate::phase::Phase;
use crate::seat::Seat;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Observation {
    /// The seat this observation is for.
    pub own_seat: Seat,
    /// The seat whose turn it currently is.
    pub current_seat: Seat,
    /// The current phase of the hand.
    pub phase: Phase,
    /// The acting seat's own hand.
    pub own_hand: Hand,
    /// The top card of the discard pile, if any.
    pub top_discard: Option<Card>,
    /// Every card discarded so far in this hand, in order. Public knowledge
    /// per rule 15.
    pub discard_history: Vec<Card>,
    /// Number of cards left in the stock.
    pub stock_size: u8,
    /// Melds laid on the table during the showdown.
    pub melds_on_table: Vec<Meld>,
    /// Match scores so far, in seat order.
    pub scores: Vec<i32>,
    /// Number of cards each opponent holds, in seat order. The acting seat's
    /// own entry is the size of `own_hand`.
    pub hand_sizes: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{Rank, Suit};

    fn c(suit: Suit, rank: u8) -> Card {
        Card::new(suit, Rank::new(rank).unwrap())
    }

    #[test]
    fn observation_construction_and_equality() {
        let obs = Observation {
            own_seat: Seat::new(0),
            current_seat: Seat::new(0),
            phase: Phase::Draw,
            own_hand: Hand::from([c(Suit::Clubs, 1), c(Suit::Hearts, 7)].as_slice()),
            top_discard: Some(c(Suit::Spades, 5)),
            discard_history: vec![c(Suit::Spades, 5)],
            stock_size: 25,
            melds_on_table: Vec::new(),
            scores: vec![0, 0],
            hand_sizes: vec![2, 7],
        };
        let same = obs.clone();
        assert_eq!(obs, same);
    }

    #[test]
    fn top_discard_none_means_empty_pile() {
        let obs = Observation {
            own_seat: Seat::new(0),
            current_seat: Seat::new(1),
            phase: Phase::Draw,
            own_hand: Hand::EMPTY,
            top_discard: None,
            discard_history: vec![],
            stock_size: 0,
            melds_on_table: Vec::new(),
            scores: vec![0, 0],
            hand_sizes: vec![0, 0],
        };
        assert!(obs.top_discard.is_none());
        assert_eq!(obs.stock_size, 0);
    }
}
