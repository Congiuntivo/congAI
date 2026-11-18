//! Suit-theme table and parser.

use conga_core::Suit;
use conga_tui::suits::{SuitTheme, suit_color};
use ratatui::style::Color;

#[test]
fn french_glyphs_match_unicode_playing_card_suits() {
    assert_eq!(SuitTheme::French.glyph(Suit::Clubs), '\u{2663}');
    assert_eq!(SuitTheme::French.glyph(Suit::Diamonds), '\u{2666}');
    assert_eq!(SuitTheme::French.glyph(Suit::Hearts), '\u{2665}');
    assert_eq!(SuitTheme::French.glyph(Suit::Spades), '\u{2660}');
}

#[test]
fn spanish_glyphs_are_distinct_from_french_and_from_each_other() {
    let spanish: Vec<char> = Suit::ALL
        .iter()
        .map(|s| SuitTheme::Spanish.glyph(*s))
        .collect();
    let french: Vec<char> = Suit::ALL
        .iter()
        .map(|s| SuitTheme::French.glyph(*s))
        .collect();

    // No overlap between the two themes (else the theme switch is silent).
    for s in &spanish {
        assert!(
            !french.contains(s),
            "spanish glyph {s:?} overlaps a french glyph"
        );
    }
    // All four spanish glyphs are distinct.
    let mut sorted = spanish.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        4,
        "spanish glyphs must be distinct: {spanish:?}"
    );
}

#[test]
fn parse_accepts_canonical_names_case_insensitive() {
    assert_eq!(SuitTheme::parse("french"), Some(SuitTheme::French));
    assert_eq!(SuitTheme::parse("French"), Some(SuitTheme::French));
    assert_eq!(SuitTheme::parse("FRENCH"), Some(SuitTheme::French));
    assert_eq!(SuitTheme::parse("spanish"), Some(SuitTheme::Spanish));
    assert_eq!(SuitTheme::parse("Spanish"), Some(SuitTheme::Spanish));
}

#[test]
fn parse_accepts_single_letter_shortcuts() {
    assert_eq!(SuitTheme::parse("f"), Some(SuitTheme::French));
    assert_eq!(SuitTheme::parse("s"), Some(SuitTheme::Spanish));
}

#[test]
fn parse_trims_surrounding_whitespace() {
    assert_eq!(SuitTheme::parse("  french  "), Some(SuitTheme::French));
}

#[test]
fn parse_rejects_garbage() {
    assert_eq!(SuitTheme::parse(""), None);
    assert_eq!(SuitTheme::parse("italian"), None);
    assert_eq!(SuitTheme::parse("fr"), None);
}

#[test]
fn default_theme_is_french() {
    assert_eq!(SuitTheme::default(), SuitTheme::French);
}

#[test]
fn suit_color_matches_the_red_black_convention() {
    assert_eq!(suit_color(Suit::Hearts), Some(Color::Red));
    assert_eq!(suit_color(Suit::Diamonds), Some(Color::Red));
    assert_eq!(suit_color(Suit::Clubs), None);
    assert_eq!(suit_color(Suit::Spades), None);
}
