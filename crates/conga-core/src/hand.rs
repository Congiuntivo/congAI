//! A hand stored as a 64-bit mask. Bit `i` is set when card `i` is in the hand.

use crate::card::Card;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Hand(u64);

impl Hand {
    pub const EMPTY: Hand = Hand(0);

    pub fn new() -> Hand {
        Hand::EMPTY
    }

    pub fn insert(&mut self, card: Card) -> bool {
        let bit = 1u64 << card.index();
        let newly_inserted = self.0 & bit == 0;
        self.0 |= bit;
        newly_inserted
    }

    pub fn remove(&mut self, card: Card) -> bool {
        let bit = 1u64 << card.index();
        let was_present = self.0 & bit != 0;
        self.0 &= !bit;
        was_present
    }

    pub fn contains(&self, card: Card) -> bool {
        self.0 & (1u64 << card.index()) != 0
    }

    pub fn len(&self) -> u32 {
        self.0.count_ones()
    }

    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }

    pub fn iter(&self) -> HandIter {
        HandIter { bits: self.0 }
    }

    pub fn union(self, other: Hand) -> Hand {
        Hand(self.0 | other.0)
    }

    pub fn intersection(self, other: Hand) -> Hand {
        Hand(self.0 & other.0)
    }

    pub fn difference(self, other: Hand) -> Hand {
        Hand(self.0 & !other.0)
    }

    pub fn bits(&self) -> u64 {
        self.0
    }
}

pub struct HandIter {
    bits: u64,
}

impl Iterator for HandIter {
    type Item = Card;

    fn next(&mut self) -> Option<Card> {
        if self.bits == 0 {
            return None;
        }
        let bit_idx = self.bits.trailing_zeros() as u8;
        self.bits &= self.bits - 1;
        Card::from_index(bit_idx)
    }
}

impl From<&[Card]> for Hand {
    fn from(cards: &[Card]) -> Hand {
        let mut h = Hand::EMPTY;
        for &c in cards {
            h.insert(c);
        }
        h
    }
}

impl FromIterator<Card> for Hand {
    fn from_iter<I: IntoIterator<Item = Card>>(iter: I) -> Hand {
        let mut h = Hand::EMPTY;
        for c in iter {
            h.insert(c);
        }
        h
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{Rank, Suit};

    fn c(suit: Suit, rank: u8) -> Card {
        Card::new(suit, Rank::new(rank).unwrap())
    }

    #[test]
    fn empty_hand_has_len_zero() {
        let h = Hand::EMPTY;
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
        assert_eq!(h.iter().count(), 0);
    }

    #[test]
    fn insert_contains_remove() {
        let mut h = Hand::new();
        let seven_c = c(Suit::Clubs, 7);
        assert!(!h.contains(seven_c));
        assert!(h.insert(seven_c));
        assert!(h.contains(seven_c));
        assert_eq!(h.len(), 1);
        assert!(!h.insert(seven_c));
        assert_eq!(h.len(), 1);
        assert!(h.remove(seven_c));
        assert!(!h.contains(seven_c));
        assert!(!h.remove(seven_c));
    }

    #[test]
    fn iter_yields_ascending_indices_once_each() {
        let cards = [
            c(Suit::Clubs, 7),
            c(Suit::Hearts, 2),
            c(Suit::Spades, 10),
            c(Suit::Diamonds, 1),
        ];
        let h: Hand = cards.iter().copied().collect();
        assert_eq!(h.len(), 4);
        let collected: Vec<Card> = h.iter().collect();
        assert_eq!(collected.len(), 4);
        let indices: Vec<u8> = collected.iter().map(|c| c.index()).collect();
        let mut sorted = indices.clone();
        sorted.sort_unstable();
        assert_eq!(indices, sorted);
    }

    #[test]
    fn set_operations_are_consistent() {
        let a: Hand = [c(Suit::Clubs, 1), c(Suit::Clubs, 2), c(Suit::Clubs, 3)]
            .as_slice()
            .into();
        let b: Hand = [c(Suit::Clubs, 2), c(Suit::Clubs, 3), c(Suit::Clubs, 4)]
            .as_slice()
            .into();
        assert_eq!(a.union(b).len(), 4);
        assert_eq!(a.intersection(b).len(), 2);
        assert_eq!(a.difference(b).len(), 1);
        assert_eq!(b.difference(a).len(), 1);
        assert_eq!(a.union(b), b.union(a));
        assert_eq!(a.intersection(b), b.intersection(a));
    }

    #[test]
    fn from_slice_and_from_iter_agree() {
        let cards = [c(Suit::Hearts, 5), c(Suit::Spades, 9)];
        let from_slice: Hand = cards.as_slice().into();
        let from_iter: Hand = cards.iter().copied().collect();
        assert_eq!(from_slice, from_iter);
    }
}
