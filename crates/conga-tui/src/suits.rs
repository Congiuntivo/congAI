//! Suit theming: map a `conga_core::Suit` to a user-visible glyph and
//! color. The internal deck is always `C/D/H/S` (see D24); the theme is
//! a display-only concern.
//!
//! Spanish glyphs are intentionally single-cell so the slice-9 card
//! tiles stay aligned regardless of the terminal font.

use conga_core::Suit;
use ratatui::style::Color;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SuitTheme {
    #[default]
    French,
    Spanish,
}

impl SuitTheme {
    pub fn glyph(self, suit: Suit) -> char {
        match (self, suit) {
            (SuitTheme::French, Suit::Clubs) => '\u{2663}',
            (SuitTheme::French, Suit::Diamonds) => '\u{2666}',
            (SuitTheme::French, Suit::Hearts) => '\u{2665}',
            (SuitTheme::French, Suit::Spades) => '\u{2660}',
            (SuitTheme::Spanish, Suit::Clubs) => '\u{2726}',
            (SuitTheme::Spanish, Suit::Diamonds) => '\u{25CF}',
            (SuitTheme::Spanish, Suit::Hearts) => '\u{2661}',
            (SuitTheme::Spanish, Suit::Spades) => '\u{271A}',
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "french" | "f" => Some(SuitTheme::French),
            "spanish" | "s" => Some(SuitTheme::Spanish),
            _ => None,
        }
    }
}

/// Red for hearts and diamonds, default color for clubs and spades.
/// Same mapping for both themes so a player switching themes keeps the
/// same color grouping.
pub fn suit_color(suit: Suit) -> Option<Color> {
    match suit {
        Suit::Hearts | Suit::Diamonds => Some(Color::Red),
        Suit::Clubs | Suit::Spades => None,
    }
}
