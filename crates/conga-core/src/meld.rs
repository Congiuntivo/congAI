//! Melds (sets and runs) and the hand-evaluation algorithm.
//!
//! A set is three or four cards of one rank. A run is three or more consecutive
//! cards of one suit, with no wrap from the top rank down to the bottom. Each
//! card sits in at most one meld. These rules are configurable through
//! [`MeldParams`] so another rule set can override any of them.
//!
//! The solver enumerates melds anchored on the lowest card still in play and
//! memoises splits by the hand bitmask, which keeps the work tiny for the
//! 7- or 8-card hands the engine ever asks about.

use std::collections::HashMap;

use crate::card::{Card, Rank};
use crate::deck::DeckSpec;
use crate::hand::Hand;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MeldKind {
    Set,
    Run,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Meld {
    pub kind: MeldKind,
    pub cards: Hand,
}

impl Meld {
    pub fn is_valid_for(&self, params: &MeldParams) -> bool {
        let count = self.cards.len();
        match self.kind {
            MeldKind::Set => {
                if count < params.min_set as u32 || count > params.max_set as u32 {
                    return false;
                }
                let mut rank: Option<u8> = None;
                for c in self.cards.iter() {
                    match rank {
                        None => rank = Some(c.rank().as_u8()),
                        Some(r) if r == c.rank().as_u8() => {}
                        Some(_) => return false,
                    }
                }
                true
            }
            MeldKind::Run => {
                if count < params.min_run as u32 {
                    return false;
                }
                let mut prev: Option<Card> = None;
                for c in self.cards.iter() {
                    if let Some(p) = prev
                        && (p.suit() != c.suit() || p.rank().as_u8() + 1 != c.rank().as_u8())
                    {
                        return false;
                    }
                    prev = Some(c);
                }
                true
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeldParams {
    pub min_set: u8,
    pub max_set: u8,
    pub min_run: u8,
    pub no_wrap: bool,
}

impl MeldParams {
    pub const fn standard() -> MeldParams {
        MeldParams {
            min_set: 3,
            max_set: 4,
            min_run: 3,
            no_wrap: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CloseParams {
    pub max_unmatched: u32,
    pub max_sum: u32,
}

impl CloseParams {
    pub const fn standard() -> CloseParams {
        CloseParams {
            max_unmatched: 2,
            max_sum: 5,
        }
    }

    pub fn can_close(&self, unmatched: Hand) -> bool {
        if unmatched.len() > self.max_unmatched {
            return false;
        }
        let sum: u32 = unmatched.iter().map(|c| c.rank().point_value()).sum();
        sum <= self.max_sum
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MeldSplit {
    pub splits: Vec<Meld>,
    pub unmatched: Hand,
    pub unmatched_total: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandEval {
    pub splits: Vec<Meld>,
    pub unmatched: Hand,
    pub unmatched_total: u32,
    pub can_close: bool,
    pub improvers: Hand,
}

/// Best split of the hand under `params`. Minimises the sum of card values left
/// unmatched. Cards sit in at most one meld.
pub fn best_meld_split(hand: Hand, params: &MeldParams) -> MeldSplit {
    let mut memo: HashMap<u64, MeldSplit> = HashMap::new();
    solve(hand, params, &mut memo)
}

/// Full hand evaluation: best split plus whether this hand already meets the
/// closing condition, plus the set of unseen cards that, if drawn, would let a
/// non-closing hand reach a close after a suitable discard.
pub fn evaluate_hand(
    hand: Hand,
    meld: &MeldParams,
    close: &CloseParams,
    deck: DeckSpec,
) -> HandEval {
    let split = best_meld_split(hand, meld);
    let can_close = close.can_close(split.unmatched);
    let improvers = if can_close {
        Hand::EMPTY
    } else {
        compute_improvers(hand, meld, close, deck)
    };
    HandEval {
        splits: split.splits,
        unmatched: split.unmatched,
        unmatched_total: split.unmatched_total,
        can_close,
        improvers,
    }
}

fn solve(hand: Hand, params: &MeldParams, memo: &mut HashMap<u64, MeldSplit>) -> MeldSplit {
    if hand.is_empty() {
        return MeldSplit::default();
    }
    if let Some(cached) = memo.get(&hand.bits()) {
        return cached.clone();
    }

    let Some(lowest) = hand.iter().next() else {
        return MeldSplit::default();
    };

    // Baseline: leave the lowest card unmatched.
    let mut rest = hand;
    rest.remove(lowest);
    let base = solve(rest, params, memo);
    let mut base_unmatched = base.unmatched;
    base_unmatched.insert(lowest);
    let mut best = MeldSplit {
        splits: base.splits,
        unmatched: base_unmatched,
        unmatched_total: base.unmatched_total + lowest.rank().point_value(),
    };

    for meld in melds_anchored_on(hand, lowest, params) {
        let rest_meld = hand.difference(meld.cards);
        let rest_eval = solve(rest_meld, params, memo);
        let new_split_count = rest_eval.splits.len() + 1;
        let strictly_fewer_unmatched = rest_eval.unmatched_total < best.unmatched_total;
        let same_unmatched_fewer_melds = rest_eval.unmatched_total == best.unmatched_total
            && new_split_count < best.splits.len();
        if strictly_fewer_unmatched || same_unmatched_fewer_melds {
            let mut splits = rest_eval.splits.clone();
            splits.insert(0, meld);
            best = MeldSplit {
                splits,
                unmatched: rest_eval.unmatched,
                unmatched_total: rest_eval.unmatched_total,
            };
        }
    }

    memo.insert(hand.bits(), best.clone());
    best
}

/// All valid melds that contain `anchor` and in which `anchor` is the lowest
/// card. Anchoring on the lowest card avoids enumerating the same meld under
/// multiple orderings.
fn melds_anchored_on(hand: Hand, anchor: Card, params: &MeldParams) -> Vec<Meld> {
    let mut out = Vec::new();

    // Sets: anchor + (k-1) other cards of the same rank with a higher index.
    let same_rank_higher: Vec<Card> = hand
        .iter()
        .filter(|c| c.rank() == anchor.rank() && c.index() > anchor.index())
        .collect();
    let n = same_rank_higher.len();
    // k = 3 ..= max_set
    for size in params.min_set..=params.max_set {
        let needed = size as usize - 1;
        if needed > n {
            break;
        }
        for combo in combinations(&same_rank_higher, needed) {
            let mut cards = Hand::EMPTY;
            cards.insert(anchor);
            for c in &combo {
                cards.insert(*c);
            }
            out.push(Meld {
                kind: MeldKind::Set,
                cards,
            });
        }
    }

    // Runs: anchor plus consecutive-rank cards of the same suit still in hand.
    // The run extends upward; "no wrap" is enforced because Rank tops out at
    // Rank::MAX and we never wrap back to 1.
    let mut run_cards = vec![anchor];
    let mut want_rank = anchor.rank().as_u8() + 1;
    while want_rank <= Rank::MAX {
        let Some(next_rank) = Rank::new(want_rank) else {
            break;
        };
        let next = Card::new(anchor.suit(), next_rank);
        if !hand.contains(next) {
            break;
        }
        run_cards.push(next);
        if run_cards.len() >= params.min_run as usize {
            out.push(Meld {
                kind: MeldKind::Run,
                cards: Hand::from(run_cards.as_slice()),
            });
        }
        want_rank += 1;
    }

    out
}

fn combinations<T: Copy>(items: &[T], k: usize) -> Vec<Vec<T>> {
    let mut out = Vec::new();
    let mut buf = Vec::with_capacity(k);
    combinations_inner(items, k, 0, &mut buf, &mut out);
    out
}

fn combinations_inner<T: Copy>(
    items: &[T],
    k: usize,
    start: usize,
    buf: &mut Vec<T>,
    out: &mut Vec<Vec<T>>,
) {
    if buf.len() == k {
        out.push(buf.clone());
        return;
    }
    let remaining = k - buf.len();
    if items.len() < start + remaining {
        return;
    }
    for i in start..items.len() {
        buf.push(items[i]);
        combinations_inner(items, k, i + 1, buf, out);
        buf.pop();
    }
}

fn compute_improvers(hand: Hand, meld: &MeldParams, close: &CloseParams, deck: DeckSpec) -> Hand {
    let mut improvers = Hand::EMPTY;
    for candidate in deck.all_cards() {
        if hand.contains(candidate) {
            continue;
        }
        // For each possible discard from (hand + candidate), check whether the
        // resulting hand would close. A discard back of the candidate is
        // allowed here: the "cannot discard what you just drew" rule only
        // applies when the card was taken from the discard pile, not from the
        // stock, and the engine enforces that at the action layer.
        let mut extended = hand;
        extended.insert(candidate);
        for discard in extended.iter() {
            let mut trial = extended;
            trial.remove(discard);
            let split = best_meld_split(trial, meld);
            if close.can_close(split.unmatched) {
                improvers.insert(candidate);
                break;
            }
        }
    }
    improvers
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{Rank, Suit};
    use crate::deck::standard_40;

    fn c(suit: Suit, rank: u8) -> Card {
        Card::new(suit, Rank::new(rank).unwrap())
    }

    fn hand(cards: &[Card]) -> Hand {
        Hand::from(cards)
    }

    // --- Meld validity ---

    #[test]
    fn valid_set_of_three_same_rank() {
        let m = Meld {
            kind: MeldKind::Set,
            cards: hand(&[c(Suit::Clubs, 7), c(Suit::Diamonds, 7), c(Suit::Hearts, 7)]),
        };
        assert!(m.is_valid_for(&MeldParams::standard()));
    }

    #[test]
    fn valid_set_of_four_same_rank() {
        let m = Meld {
            kind: MeldKind::Set,
            cards: hand(&[
                c(Suit::Clubs, 7),
                c(Suit::Diamonds, 7),
                c(Suit::Hearts, 7),
                c(Suit::Spades, 7),
            ]),
        };
        assert!(m.is_valid_for(&MeldParams::standard()));
    }

    #[test]
    fn invalid_set_mixed_ranks() {
        let m = Meld {
            kind: MeldKind::Set,
            cards: hand(&[c(Suit::Clubs, 7), c(Suit::Diamonds, 8), c(Suit::Hearts, 7)]),
        };
        assert!(!m.is_valid_for(&MeldParams::standard()));
    }

    #[test]
    fn valid_run_of_three_same_suit() {
        let m = Meld {
            kind: MeldKind::Run,
            cards: hand(&[c(Suit::Spades, 5), c(Suit::Spades, 6), c(Suit::Spades, 7)]),
        };
        assert!(m.is_valid_for(&MeldParams::standard()));
    }

    #[test]
    fn invalid_run_mixed_suits() {
        let m = Meld {
            kind: MeldKind::Run,
            cards: hand(&[c(Suit::Spades, 5), c(Suit::Clubs, 6), c(Suit::Spades, 7)]),
        };
        assert!(!m.is_valid_for(&MeldParams::standard()));
    }

    #[test]
    fn invalid_run_gap() {
        let m = Meld {
            kind: MeldKind::Run,
            cards: hand(&[c(Suit::Spades, 5), c(Suit::Spades, 7), c(Suit::Spades, 8)]),
        };
        assert!(!m.is_valid_for(&MeldParams::standard()));
    }

    // --- best_meld_split shapes from the design doc ---

    #[test]
    fn seven_card_run_in_one_suit_splits_fully() {
        let h = hand(&(1u8..=7).map(|r| c(Suit::Hearts, r)).collect::<Vec<_>>());
        let split = best_meld_split(h, &MeldParams::standard());
        assert_eq!(split.unmatched_total, 0);
        assert_eq!(split.unmatched, Hand::EMPTY);
        assert_eq!(split.splits.len(), 1);
        assert_eq!(split.splits[0].kind, MeldKind::Run);
        assert_eq!(split.splits[0].cards.len(), 7);
    }

    #[test]
    fn meld_of_three_plus_meld_of_four() {
        // Set of 3 ones + 4-card run in spades.
        let h = hand(&[
            c(Suit::Clubs, 1),
            c(Suit::Diamonds, 1),
            c(Suit::Hearts, 1),
            c(Suit::Spades, 5),
            c(Suit::Spades, 6),
            c(Suit::Spades, 7),
            c(Suit::Spades, 8),
        ]);
        let split = best_meld_split(h, &MeldParams::standard());
        assert_eq!(split.unmatched_total, 0);
        assert_eq!(split.unmatched, Hand::EMPTY);
        assert_eq!(split.splits.len(), 2);
    }

    #[test]
    fn two_sets_of_three_plus_low_card() {
        let h = hand(&[
            c(Suit::Clubs, 1),
            c(Suit::Diamonds, 1),
            c(Suit::Hearts, 1),
            c(Suit::Clubs, 2),
            c(Suit::Diamonds, 2),
            c(Suit::Hearts, 2),
            c(Suit::Spades, 3),
        ]);
        let split = best_meld_split(h, &MeldParams::standard());
        assert_eq!(split.unmatched.len(), 1);
        assert_eq!(split.unmatched_total, 3);
        assert_eq!(split.splits.len(), 2);
    }

    #[test]
    fn two_sets_of_three_plus_high_card_still_minimises_unmatched() {
        let h = hand(&[
            c(Suit::Clubs, 1),
            c(Suit::Diamonds, 1),
            c(Suit::Hearts, 1),
            c(Suit::Clubs, 2),
            c(Suit::Diamonds, 2),
            c(Suit::Hearts, 2),
            c(Suit::Spades, 7),
        ]);
        let split = best_meld_split(h, &MeldParams::standard());
        assert_eq!(split.unmatched.len(), 1);
        assert_eq!(split.unmatched_total, 7);
    }

    #[test]
    fn five_card_run_plus_two_low_cards() {
        let h = hand(&[
            c(Suit::Spades, 1),
            c(Suit::Spades, 2),
            c(Suit::Spades, 3),
            c(Suit::Spades, 4),
            c(Suit::Spades, 5),
            c(Suit::Clubs, 1),
            c(Suit::Clubs, 2),
        ]);
        let split = best_meld_split(h, &MeldParams::standard());
        assert_eq!(split.unmatched.len(), 2);
        assert_eq!(split.unmatched_total, 3);
    }

    #[test]
    fn scattered_hand_has_no_melds() {
        let h = hand(&[
            c(Suit::Clubs, 1),
            c(Suit::Diamonds, 3),
            c(Suit::Hearts, 5),
            c(Suit::Spades, 7),
            c(Suit::Clubs, 9),
            c(Suit::Diamonds, 10),
            c(Suit::Hearts, 2),
        ]);
        let split = best_meld_split(h, &MeldParams::standard());
        assert_eq!(split.splits.len(), 0);
        assert_eq!(split.unmatched_total, 1 + 3 + 5 + 7 + 9 + 10 + 2);
    }

    #[test]
    fn solver_prefers_splitting_overlapping_cards_to_minimise_unmatched() {
        // 7-8-9 of spades plus 7 of clubs and 7 of hearts. 7S can go in the run
        // or the set; the solver picks whichever minimises unmatched.
        let h = hand(&[
            c(Suit::Spades, 7),
            c(Suit::Spades, 8),
            c(Suit::Spades, 9),
            c(Suit::Clubs, 7),
            c(Suit::Hearts, 7),
            c(Suit::Clubs, 10),
            c(Suit::Diamonds, 10),
        ]);
        let split = best_meld_split(h, &MeldParams::standard());
        // The best is the 3-card run in spades, leaving 7C, 7H, 10C, 10D unmatched
        // (totals 7+7+10+10 = 34). Sticking 7S in a set would need 3 sevens but
        // that would break the run; we have only 7C and 7H leaving 8S, 9S stranded.
        // Either way the result is a single meld plus 4 unmatched, total 34.
        assert_eq!(split.unmatched.len(), 4);
        assert_eq!(split.unmatched_total, 34);
        assert_eq!(split.splits.len(), 1);
    }

    // --- evaluate_hand: can_close and improvers ---

    #[test]
    fn evaluate_full_run_can_close() {
        let h = hand(&(1u8..=7).map(|r| c(Suit::Hearts, r)).collect::<Vec<_>>());
        let e = evaluate_hand(
            h,
            &MeldParams::standard(),
            &CloseParams::standard(),
            standard_40(),
        );
        assert!(e.can_close);
        assert_eq!(e.improvers, Hand::EMPTY);
    }

    #[test]
    fn evaluate_two_sets_plus_low_card_can_close() {
        let h = hand(&[
            c(Suit::Clubs, 1),
            c(Suit::Diamonds, 1),
            c(Suit::Hearts, 1),
            c(Suit::Clubs, 2),
            c(Suit::Diamonds, 2),
            c(Suit::Hearts, 2),
            c(Suit::Spades, 3),
        ]);
        let e = evaluate_hand(
            h,
            &MeldParams::standard(),
            &CloseParams::standard(),
            standard_40(),
        );
        assert!(e.can_close);
    }

    #[test]
    fn evaluate_high_card_cannot_close_but_has_improvers() {
        let h = hand(&[
            c(Suit::Clubs, 1),
            c(Suit::Diamonds, 1),
            c(Suit::Hearts, 1),
            c(Suit::Clubs, 2),
            c(Suit::Diamonds, 2),
            c(Suit::Hearts, 2),
            c(Suit::Spades, 7),
        ]);
        let e = evaluate_hand(
            h,
            &MeldParams::standard(),
            &CloseParams::standard(),
            standard_40(),
        );
        assert!(!e.can_close);
        // Drawing a 3, 4 or 5 of any suit (that is not in hand) and discarding
        // the 7S would close. There are 3 suits each of 3, 4 and 5 = 9 cards.
        assert!(!e.improvers.is_empty());
        assert!(e.improvers.contains(c(Suit::Clubs, 3)));
        assert!(e.improvers.contains(c(Suit::Spades, 5)));
        // The 7S is in hand so not an improver.
        assert!(!e.improvers.contains(c(Suit::Spades, 7)));
    }

    #[test]
    fn evaluate_close_already_has_empty_improvers() {
        let h = hand(&[
            c(Suit::Clubs, 1),
            c(Suit::Diamonds, 1),
            c(Suit::Hearts, 1),
            c(Suit::Clubs, 2),
            c(Suit::Diamonds, 2),
            c(Suit::Hearts, 2),
            c(Suit::Spades, 3),
        ]);
        let e = evaluate_hand(
            h,
            &MeldParams::standard(),
            &CloseParams::standard(),
            standard_40(),
        );
        assert!(e.can_close);
        assert_eq!(e.improvers, Hand::EMPTY);
    }

    #[test]
    fn can_close_predicate_respects_count_and_sum() {
        let close = CloseParams::standard();
        assert!(close.can_close(Hand::EMPTY));
        // Three unmatched cards of value 1 sum to 3, but exceed count 2.
        let three_ones = hand(&[c(Suit::Clubs, 1), c(Suit::Diamonds, 1), c(Suit::Hearts, 1)]);
        assert!(!close.can_close(three_ones));
        // Two unmatched cards summing to 6 fail the sum check.
        let two_summing_six = hand(&[c(Suit::Clubs, 1), c(Suit::Diamonds, 5)]);
        assert!(!close.can_close(two_summing_six));
        // Two cards summing to exactly 5 pass.
        let two_summing_five = hand(&[c(Suit::Clubs, 2), c(Suit::Diamonds, 3)]);
        assert!(close.can_close(two_summing_five));
    }
}
