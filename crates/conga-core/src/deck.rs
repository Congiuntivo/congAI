//! Deck specification. Every rule set declares its deck here.

use crate::card::Card;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeckSpec {
    pub cards: u8,
}

impl DeckSpec {
    pub fn all_cards(self) -> impl Iterator<Item = Card> {
        (0..self.cards).filter_map(Card::from_index)
    }
}

pub fn standard_40() -> DeckSpec {
    DeckSpec { cards: 40 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_40_has_40_cards() {
        let spec = standard_40();
        assert_eq!(spec.cards, 40);
        let cards: Vec<Card> = spec.all_cards().collect();
        assert_eq!(cards.len(), 40);
    }

    #[test]
    fn standard_40_covers_every_index_once() {
        let spec = standard_40();
        let mut seen = [false; 40];
        for card in spec.all_cards() {
            let i = card.index() as usize;
            assert!(i < 40);
            assert!(!seen[i], "duplicate card index {i}");
            seen[i] = true;
        }
        assert!(seen.iter().all(|&b| b));
    }
}
