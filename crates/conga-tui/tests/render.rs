//! Pure renderer tests. The driver's event loop is manual-play
//! territory; here we only assert on the data the renderer produces
//! for a fixed observation.

use conga_core::{Action, Card, Hand, Observation, Phase, Rank, Seat, Suit};
use conga_tui::render::{action_label, hand_summary_string, render_hand, render_legal_lines};

fn c(suit: Suit, rank: u8) -> Card {
    Card::new(suit, Rank::new(rank).unwrap())
}

fn line_text(line: &ratatui::text::Line<'_>) -> String {
    line.spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect::<String>()
}

fn obs_with_hand(cards: &[Card]) -> Observation {
    Observation {
        own_seat: Seat::new(0),
        current_seat: Seat::new(0),
        phase: Phase::Discard,
        own_hand: Hand::from(cards),
        top_discard: Some(c(Suit::Spades, 5)),
        discard_history: vec![c(Suit::Spades, 5)],
        stock_size: 25,
        melds_on_table: Vec::new(),
        scores: vec![0, 0],
        hand_sizes: vec![cards.len() as u8, 7],
    }
}

#[test]
fn render_hand_shows_meld_brackets_then_unmatched_with_total() {
    // Two sets of 3 + one unmatched 7S, totaling 7.
    let hand = [
        c(Suit::Clubs, 1),
        c(Suit::Diamonds, 1),
        c(Suit::Hearts, 1),
        c(Suit::Clubs, 2),
        c(Suit::Diamonds, 2),
        c(Suit::Hearts, 2),
        c(Suit::Spades, 7),
    ];
    let obs = obs_with_hand(&hand);
    let line = render_hand(&obs);
    let text = line_text(&line);
    // The meld solver may order the two sets of three either way, but
    // the unmatched 7S(7) must be last after "| ".
    assert!(
        text.contains("| 7S(7)"),
        "expected unmatched 7S with total: {text}"
    );
    assert!(
        text.matches('[').count() == 2 && text.matches(']').count() == 2,
        "expected exactly two meld brackets: {text}"
    );
    // CLOSE OK must not appear (can_close=false for this hand).
    assert!(!text.contains("CLOSE OK"), "hand cannot close: {text}");
}

#[test]
fn render_hand_marks_can_close() {
    // Two sets of 3 + one 3S (value 3 <= 5) closes.
    let hand = [
        c(Suit::Clubs, 1),
        c(Suit::Diamonds, 1),
        c(Suit::Hearts, 1),
        c(Suit::Clubs, 2),
        c(Suit::Diamonds, 2),
        c(Suit::Hearts, 2),
        c(Suit::Spades, 3),
    ];
    let obs = obs_with_hand(&hand);
    let line = render_hand(&obs);
    let text = line_text(&line);
    assert!(text.contains("CLOSE OK"), "expected CLOSE OK: {text}");
}

#[test]
fn render_hand_with_full_7_card_run_has_no_unmatched() {
    let hand: Vec<Card> = (1u8..=7).map(|r| c(Suit::Hearts, r)).collect();
    let obs = obs_with_hand(&hand);
    let line = render_hand(&obs);
    let text = line_text(&line);
    assert!(!text.contains('|'), "no unmatched expected: {text}");
    assert!(text.contains("CLOSE OK"));
}

#[test]
fn render_hand_colors_red_suits() {
    let obs = obs_with_hand(&[c(Suit::Hearts, 2), c(Suit::Clubs, 7)]);
    let line = render_hand(&obs);
    // Find the 2H span and check its style.
    let heart = line
        .spans
        .iter()
        .find(|s| s.content.as_ref() == "2H")
        .expect("2H span");
    assert_eq!(heart.style.fg, Some(ratatui::style::Color::Red));
    let club = line
        .spans
        .iter()
        .find(|s| s.content.as_ref() == "7C")
        .expect("7C span");
    assert_eq!(club.style.fg, None, "clubs stay default color");
}

#[test]
fn legal_lines_number_1_through_n() {
    let legal = vec![Action::DrawStock, Action::DrawDiscard, Action::Pass];
    let lines = render_legal_lines(&legal);
    assert_eq!(lines.len(), 3);
    assert!(line_text(&lines[0]).starts_with(" 1) "));
    assert!(line_text(&lines[1]).starts_with(" 2) "));
    assert!(line_text(&lines[2]).starts_with(" 3) "));
    assert!(line_text(&lines[0]).contains("Draw from stock"));
    assert!(line_text(&lines[1]).contains("Take the top discard"));
    assert!(line_text(&lines[2]).contains("Pass"));
}

#[test]
fn legal_lines_tenth_entry_is_keyed_zero() {
    let legal: Vec<Action> = (0..10).map(|_| Action::Pass).collect();
    let lines = render_legal_lines(&legal);
    assert_eq!(lines.len(), 10);
    // 1..9 then 0.
    assert!(line_text(&lines[8]).starts_with(" 9) "));
    assert!(line_text(&lines[9]).starts_with(" 0) "));
}

#[test]
fn legal_lines_caps_at_ten() {
    let legal: Vec<Action> = (0..20).map(|_| Action::Pass).collect();
    let lines = render_legal_lines(&legal);
    assert_eq!(lines.len(), 10);
}

#[test]
fn action_label_is_readable_for_every_variant() {
    assert_eq!(action_label(&Action::DrawStock), "Draw from stock");
    assert_eq!(action_label(&Action::DrawDiscard), "Take the top discard");
    assert_eq!(
        action_label(&Action::Discard(c(Suit::Clubs, 7))),
        "Discard 7C"
    );
    assert_eq!(
        action_label(&Action::Close(c(Suit::Hearts, 2))),
        "Close with 2H"
    );
    assert_eq!(
        action_label(&Action::AddToMeld {
            card: c(Suit::Spades, 7),
            meld_index: 0,
        }),
        "Add 7S to meld #0"
    );
    assert_eq!(action_label(&Action::Pass), "Pass");
}

#[test]
fn hand_summary_string_matches_m1_debug_cli_shape() {
    let obs = obs_with_hand(&[
        c(Suit::Clubs, 7),
        c(Suit::Diamonds, 7),
        c(Suit::Hearts, 7),
        c(Suit::Spades, 5),
        c(Suit::Spades, 6),
        c(Suit::Spades, 7),
        c(Suit::Spades, 8),
    ]);
    let s = hand_summary_string(&obs);
    assert!(s.contains("[7C 7D 7H]"), "set-of-3 of sevens: {s}");
    assert!(s.contains("[5S 6S 7S 8S]"), "4-card run of spades: {s}");
    assert!(!s.contains('|'), "no unmatched: {s}");
}
