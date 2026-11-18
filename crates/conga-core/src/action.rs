//! Actions a player can take.
//!
//! `Action` is the typed form the engine and server work with. The fixed
//! 333-index encoding required for the neural policy is a later-milestone
//! concern and is derived from this enum.

use crate::card::Card;
use crate::meld::Meld;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DrawSource {
    Stock,
    TopDiscard,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Action {
    /// Draw the top card of the stock.
    DrawStock,
    /// Take the top card of the discard pile.
    DrawDiscard,
    /// Discard a card from hand, keeping play open.
    Discard(Card),
    /// Close by placing this card face-down as the discard.
    Close(Card),
    /// Lay a meld from hand down on the table during the showdown.
    LayMeld(Meld),
    /// Add one card from hand to the meld at `meld_index` on the table.
    AddToMeld { card: Card, meld_index: u8 },
    /// Pass during the showdown.
    Pass,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{Rank, Suit};
    use crate::hand::Hand;
    use crate::meld::{Meld, MeldKind};

    fn c(suit: Suit, rank: u8) -> Card {
        Card::new(suit, Rank::new(rank).unwrap())
    }

    #[test]
    fn construction_of_every_variant() {
        let a1 = Action::DrawStock;
        let a2 = Action::DrawDiscard;
        let a3 = Action::Discard(c(Suit::Clubs, 7));
        let a4 = Action::Close(c(Suit::Hearts, 10));
        let a5 = Action::LayMeld(Meld {
            kind: MeldKind::Set,
            cards: Hand::from(
                [c(Suit::Clubs, 7), c(Suit::Diamonds, 7), c(Suit::Hearts, 7)].as_slice(),
            ),
        });
        let a6 = Action::AddToMeld {
            card: c(Suit::Spades, 9),
            meld_index: 2,
        };
        let a7 = Action::Pass;
        // Distinct variants are not equal to each other.
        for (left, right) in [(&a1, &a2), (&a1, &a3), (&a3, &a4), (&a5, &a7), (&a6, &a7)] {
            assert_ne!(left, right);
        }
    }

    #[test]
    fn discard_equality_compares_cards() {
        let seven_c = c(Suit::Clubs, 7);
        assert_eq!(Action::Discard(seven_c), Action::Discard(seven_c));
        assert_ne!(
            Action::Discard(c(Suit::Clubs, 7)),
            Action::Discard(c(Suit::Clubs, 8))
        );
    }

    #[test]
    fn debug_contains_variant_name() {
        assert!(format!("{:?}", Action::DrawStock).contains("DrawStock"));
        assert!(format!("{:?}", Action::Pass).contains("Pass"));
        assert!(
            format!(
                "{:?}",
                Action::AddToMeld {
                    card: c(Suit::Clubs, 1),
                    meld_index: 0
                }
            )
            .contains("AddToMeld")
        );
    }

    #[test]
    fn draw_source_variants() {
        assert_ne!(DrawSource::Stock, DrawSource::TopDiscard);
    }
}
