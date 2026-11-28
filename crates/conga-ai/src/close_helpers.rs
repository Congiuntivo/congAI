//! Shared close-decision helpers used by the diagnostic modules and
//! by candidate tactics. Factored out of `ScoreAwarePlayer`'s private
//! impl to avoid three-way duplication (score_aware / disagreement /
//! counterfactual / candidates).
//!
//! The `ScoreAwarePlayer` keeps its own locally-identical copies to
//! stay self-contained; a later milestone can rebase it onto this
//! module when score_aware is next touched.

use conga_core::{Card, Hand, MeldParams, best_meld_split};

/// Closer's score delta under StandardConga, given the hand that
/// remains *after* placing the close card face-down: the best split's
/// `unmatched_total`, minus 10 for a clean split (empty unmatched) or
/// minus 50 for a 7-card single-suit run (`Conga`).
pub fn own_close_delta(hand_after_close: Hand) -> i32 {
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

/// `true` when `hand` is a 7-card single-suit run of consecutive
/// ranks (the StandardConga `-50` bonus trigger).
pub fn is_conga(hand: Hand) -> bool {
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
    use conga_core::{Rank, Suit};

    fn c(suit: Suit, rank: u8) -> Card {
        Card::new(suit, Rank::new(rank).unwrap())
    }

    #[test]
    fn clean_split_pays_minus_ten() {
        // Three 1s + three 2s: two 3-card sets, 0 unmatched.
        let mut h = Hand::new();
        for suit in [Suit::Hearts, Suit::Diamonds, Suit::Clubs] {
            h.insert(c(suit, 1));
            h.insert(c(suit, 2));
        }
        assert_eq!(own_close_delta(h), -10);
    }

    #[test]
    fn conga_pays_minus_fifty() {
        let mut h = Hand::new();
        for r in 1..=7 {
            h.insert(c(Suit::Hearts, r));
        }
        assert_eq!(own_close_delta(h), -50);
        assert!(is_conga(h));
    }

    #[test]
    fn non_empty_residual_pays_its_rank_sum() {
        let mut h = Hand::new();
        for suit in [Suit::Hearts, Suit::Diamonds, Suit::Clubs] {
            h.insert(c(suit, 1));
        }
        h.insert(c(Suit::Spades, 5));
        assert_eq!(own_close_delta(h), 5);
    }

    #[test]
    fn mixed_suit_run_is_not_conga() {
        let mut h = Hand::new();
        for r in 1..=6 {
            h.insert(c(Suit::Hearts, r));
        }
        h.insert(c(Suit::Spades, 7));
        assert!(!is_conga(h));
    }
}
