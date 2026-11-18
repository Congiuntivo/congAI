//! Card, suit and rank for the standard 40-card Conga deck.
//!
//! Card indices 0..40 encode (suit, rank) as `suit * 10 + (rank - 1)`.
//! Cards 0..9 are Clubs 1..10, 10..19 are Diamonds 1..10, and so on.
//! If another rule set ever uses a different deck layout, this mapping
//! moves to a per-deck interpreter.

use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum Suit {
    Clubs = 0,
    Diamonds = 1,
    Hearts = 2,
    Spades = 3,
}

impl Suit {
    pub const ALL: [Suit; 4] = [Suit::Clubs, Suit::Diamonds, Suit::Hearts, Suit::Spades];

    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn letter(self) -> char {
        match self {
            Suit::Clubs => 'C',
            Suit::Diamonds => 'D',
            Suit::Hearts => 'H',
            Suit::Spades => 'S',
        }
    }
}

impl fmt::Display for Suit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.letter())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct Rank(u8);

impl Rank {
    pub const MIN: u8 = 1;
    pub const MAX: u8 = 10;

    pub fn new(value: u8) -> Option<Rank> {
        if (Self::MIN..=Self::MAX).contains(&value) {
            Some(Rank(value))
        } else {
            None
        }
    }

    pub fn as_u8(self) -> u8 {
        self.0
    }

    pub fn point_value(self) -> u32 {
        self.0 as u32
    }
}

impl fmt::Display for Rank {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Card(u8);

impl Card {
    pub const COUNT: u8 = 40;

    pub fn new(suit: Suit, rank: Rank) -> Card {
        Card(suit.as_u8() * Rank::MAX + (rank.as_u8() - 1))
    }

    pub fn from_index(index: u8) -> Option<Card> {
        if index < Self::COUNT {
            Some(Card(index))
        } else {
            None
        }
    }

    pub fn index(self) -> u8 {
        self.0
    }

    pub fn suit(self) -> Suit {
        match self.0 / Rank::MAX {
            0 => Suit::Clubs,
            1 => Suit::Diamonds,
            2 => Suit::Hearts,
            _ => Suit::Spades,
        }
    }

    pub fn rank(self) -> Rank {
        Rank(self.0 % Rank::MAX + 1)
    }

    /// Parse the `Display` form of a card: a decimal rank 1..=10 followed by
    /// one suit letter in `CDHS`. Returns `None` for any other input.
    pub fn parse(s: &str) -> Option<Card> {
        if s.len() < 2 || !s.is_ascii() {
            return None;
        }
        let (rank_part, suit_part) = s.split_at(s.len() - 1);
        let rank_num: u8 = rank_part.parse().ok()?;
        let rank = Rank::new(rank_num)?;
        let suit = match suit_part {
            "C" => Suit::Clubs,
            "D" => Suit::Diamonds,
            "H" => Suit::Hearts,
            "S" => Suit::Spades,
            _ => return None,
        };
        Some(Card::new(suit, rank))
    }
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.rank(), self.suit())
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Card {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Card {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Card, D::Error> {
        use serde::de::Error;
        let s = String::deserialize(deserializer)?;
        Card::parse(&s).ok_or_else(|| D::Error::custom(format!("invalid card: {s:?}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rank_rejects_out_of_range() {
        assert!(Rank::new(0).is_none());
        assert!(Rank::new(11).is_none());
        assert_eq!(Rank::new(1).unwrap().as_u8(), 1);
        assert_eq!(Rank::new(10).unwrap().as_u8(), 10);
    }

    #[test]
    fn card_round_trips_through_index() {
        for suit in Suit::ALL {
            for r in 1..=10u8 {
                let rank = Rank::new(r).unwrap();
                let card = Card::new(suit, rank);
                assert_eq!(card.suit(), suit);
                assert_eq!(card.rank(), rank);
                assert_eq!(Card::from_index(card.index()).unwrap(), card);
            }
        }
    }

    #[test]
    fn card_indices_are_dense_0_to_39() {
        let mut seen = [false; 40];
        for suit in Suit::ALL {
            for r in 1..=10u8 {
                let card = Card::new(suit, Rank::new(r).unwrap());
                let idx = card.index() as usize;
                assert!(idx < 40, "index out of range: {idx}");
                assert!(!seen[idx], "duplicate index {idx}");
                seen[idx] = true;
            }
        }
        assert!(seen.iter().all(|&b| b));
    }

    #[test]
    fn card_from_index_rejects_out_of_range() {
        assert!(Card::from_index(40).is_none());
        assert!(Card::from_index(63).is_none());
        assert!(Card::from_index(0).is_some());
        assert!(Card::from_index(39).is_some());
    }

    #[test]
    fn display_format() {
        let c = Card::new(Suit::Clubs, Rank::new(7).unwrap());
        assert_eq!(c.to_string(), "7C");
        let c = Card::new(Suit::Hearts, Rank::new(10).unwrap());
        assert_eq!(c.to_string(), "10H");
        let c = Card::new(Suit::Spades, Rank::new(1).unwrap());
        assert_eq!(c.to_string(), "1S");
    }

    #[test]
    fn point_value_matches_rank() {
        let rank = Rank::new(7).unwrap();
        assert_eq!(rank.point_value(), 7);
    }
}
