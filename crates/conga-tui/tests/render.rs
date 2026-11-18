//! Pure renderer tests. The driver's event loop is manual-play
//! territory; here we only assert on the data the renderer produces
//! for a fixed observation.

use conga_core::{Action, Card, Hand, Observation, Phase, Rank, Seat, Suit};
use conga_tui::app::CursorPos;
use conga_tui::render::{
    action_label, hand_summary_string, render_card_tile, render_discard_tile, render_hand,
    render_hand_strip, render_legal_lines, render_stock_tile,
};
use conga_tui::suits::SuitTheme;

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
    let line = render_hand(&obs, SuitTheme::French);
    let text = line_text(&line);
    // The meld solver may order the two sets of three either way, but
    // the unmatched 7♠(7) must be last after "| ".
    assert!(
        text.contains("| 7\u{2660}(7)"),
        "expected unmatched 7♠ with total: {text}"
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
    let line = render_hand(&obs, SuitTheme::French);
    let text = line_text(&line);
    assert!(text.contains("CLOSE OK"), "expected CLOSE OK: {text}");
}

#[test]
fn render_hand_with_full_7_card_run_has_no_unmatched() {
    let hand: Vec<Card> = (1u8..=7).map(|r| c(Suit::Hearts, r)).collect();
    let obs = obs_with_hand(&hand);
    let line = render_hand(&obs, SuitTheme::French);
    let text = line_text(&line);
    assert!(!text.contains('|'), "no unmatched expected: {text}");
    assert!(text.contains("CLOSE OK"));
}

#[test]
fn render_hand_colors_red_suits() {
    let obs = obs_with_hand(&[c(Suit::Hearts, 2), c(Suit::Clubs, 7)]);
    let line = render_hand(&obs, SuitTheme::French);
    // Find the 2♥ span and check its style.
    let heart = line
        .spans
        .iter()
        .find(|s| s.content.as_ref() == "2\u{2665}")
        .expect("2♥ span");
    assert_eq!(heart.style.fg, Some(ratatui::style::Color::Red));
    let club = line
        .spans
        .iter()
        .find(|s| s.content.as_ref() == "7\u{2663}")
        .expect("7♣ span");
    assert_eq!(club.style.fg, None, "clubs stay default color");
}

#[test]
fn render_hand_honors_spanish_theme() {
    let obs = obs_with_hand(&[c(Suit::Hearts, 2), c(Suit::Spades, 7)]);
    let line = render_hand(&obs, SuitTheme::Spanish);
    let text = line_text(&line);
    // Spanish hearts = ♡ (copas), Spanish spades = ✚ (espadas). The
    // French glyphs must not appear under the Spanish theme.
    assert!(text.contains('\u{2661}'), "expected copa ♡: {text}");
    assert!(text.contains('\u{271A}'), "expected espada ✚: {text}");
    assert!(!text.contains('\u{2665}'), "no french heart: {text}");
    assert!(!text.contains('\u{2660}'), "no french spade: {text}");
}

#[test]
fn legal_lines_number_1_through_n() {
    let legal = vec![Action::DrawStock, Action::DrawDiscard, Action::Pass];
    let lines = render_legal_lines(&legal, SuitTheme::French);
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
    let lines = render_legal_lines(&legal, SuitTheme::French);
    assert_eq!(lines.len(), 10);
    // 1..9 then 0.
    assert!(line_text(&lines[8]).starts_with(" 9) "));
    assert!(line_text(&lines[9]).starts_with(" 0) "));
}

#[test]
fn legal_lines_caps_at_ten() {
    let legal: Vec<Action> = (0..20).map(|_| Action::Pass).collect();
    let lines = render_legal_lines(&legal, SuitTheme::French);
    assert_eq!(lines.len(), 10);
}

#[test]
fn action_label_is_readable_for_every_variant() {
    let th = SuitTheme::French;
    assert_eq!(action_label(&Action::DrawStock, th), "Draw from stock");
    assert_eq!(
        action_label(&Action::DrawDiscard, th),
        "Take the top discard"
    );
    assert_eq!(
        action_label(&Action::Discard(c(Suit::Clubs, 7)), th),
        "Discard 7\u{2663}"
    );
    assert_eq!(
        action_label(&Action::Close(c(Suit::Hearts, 2)), th),
        "Close with 2\u{2665}"
    );
    assert_eq!(
        action_label(
            &Action::AddToMeld {
                card: c(Suit::Spades, 7),
                meld_index: 0,
            },
            th,
        ),
        "Add 7\u{2660} to meld #0"
    );
    assert_eq!(action_label(&Action::Pass, th), "Pass");
}

#[test]
fn action_label_switches_glyphs_under_spanish_theme() {
    let th = SuitTheme::Spanish;
    assert_eq!(
        action_label(&Action::Discard(c(Suit::Clubs, 7)), th),
        "Discard 7\u{2726}"
    );
    assert_eq!(
        action_label(&Action::Close(c(Suit::Hearts, 2)), th),
        "Close with 2\u{2661}"
    );
}

#[test]
fn card_tile_shape_is_four_lines_wide_five() {
    let tile = render_card_tile(c(Suit::Clubs, 10), false, SuitTheme::French);
    assert_eq!(tile.len(), 4);
    for line in &tile {
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        let width = text.chars().count();
        assert_eq!(width, 5, "tile line should be 5 cells wide: {text:?}");
    }
    // Rank row shows "10" right-aligned, then a space.
    let rank_row: String = tile[1].spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(rank_row.contains("10 "), "rank row: {rank_row:?}");
    // Suit row shows the glyph centered between pipes.
    let suit_row: String = tile[2].spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(suit_row.contains('\u{2663}'), "suit row: {suit_row:?}");
}

#[test]
fn card_tile_rank_1_through_9_is_space_padded() {
    let tile = render_card_tile(c(Suit::Hearts, 3), false, SuitTheme::French);
    let rank_row: String = tile[1].spans.iter().map(|s| s.content.as_ref()).collect();
    // Format: "│ 3 │" — pipe, space, rank, space, pipe.
    assert_eq!(rank_row, "\u{2502} 3 \u{2502}", "rank row: {rank_row:?}");
}

#[test]
fn card_tile_selected_uses_double_border() {
    let tile = render_card_tile(c(Suit::Spades, 7), true, SuitTheme::French);
    let top: String = tile[0].spans.iter().map(|s| s.content.as_ref()).collect();
    let bot: String = tile[3].spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(top, "\u{2554}\u{2550}\u{2550}\u{2550}\u{2557}");
    assert_eq!(bot, "\u{255A}\u{2550}\u{2550}\u{2550}\u{255D}");
    // Side walls use ║.
    let side: String = tile[1].spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(side.starts_with('\u{2551}') && side.ends_with('\u{2551}'));
}

#[test]
fn card_tile_unselected_uses_light_border() {
    let tile = render_card_tile(c(Suit::Diamonds, 1), false, SuitTheme::French);
    let top: String = tile[0].spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(top, "\u{250C}\u{2500}\u{2500}\u{2500}\u{2510}");
}

#[test]
fn card_tile_colors_the_suit_glyph_for_red_suits() {
    let tile = render_card_tile(c(Suit::Hearts, 4), false, SuitTheme::French);
    // The glyph lives on line 2 as its own span.
    let glyph_span = tile[2]
        .spans
        .iter()
        .find(|s| s.content.as_ref() == "\u{2665}")
        .expect("heart glyph span");
    assert_eq!(glyph_span.style.fg, Some(ratatui::style::Color::Red));

    let tile_black = render_card_tile(c(Suit::Clubs, 4), false, SuitTheme::French);
    let glyph_span = tile_black[2]
        .spans
        .iter()
        .find(|s| s.content.as_ref() == "\u{2663}")
        .expect("club glyph span");
    assert_eq!(glyph_span.style.fg, None);
}

#[test]
fn card_tile_uses_spanish_glyph_under_spanish_theme() {
    let tile = render_card_tile(c(Suit::Spades, 7), false, SuitTheme::Spanish);
    let glyph_span = tile[2]
        .spans
        .iter()
        .find(|s| s.content.as_ref() == "\u{271A}")
        .expect("espada glyph span");
    assert_eq!(glyph_span.style.fg, None);
}

#[test]
fn stock_tile_is_pattern_filled() {
    let tile = render_stock_tile(false);
    assert_eq!(tile.len(), 4);
    let body: String = tile[1].spans.iter().map(|s| s.content.as_ref()).collect();
    // Pipes around three light-shade blocks.
    assert_eq!(body, "\u{2502}\u{2592}\u{2592}\u{2592}\u{2502}");
}

#[test]
fn discard_tile_without_top_renders_empty_frame() {
    let tile = render_discard_tile(None, false, SuitTheme::French);
    assert_eq!(tile.len(), 4);
    let body: String = tile[1].spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(body, "\u{2502}   \u{2502}");
}

#[test]
fn discard_tile_with_top_matches_card_tile() {
    let want = render_card_tile(c(Suit::Clubs, 5), false, SuitTheme::French);
    let got = render_discard_tile(Some(c(Suit::Clubs, 5)), false, SuitTheme::French);
    let w_text: String = want
        .iter()
        .flat_map(|l| l.spans.iter())
        .map(|s| s.content.as_ref())
        .collect();
    let g_text: String = got
        .iter()
        .flat_map(|l| l.spans.iter())
        .map(|s| s.content.as_ref())
        .collect();
    assert_eq!(w_text, g_text);
}

#[test]
fn hand_strip_produces_four_tile_rows_plus_hint() {
    // Hand of 7 cards; the strip should have 4 tile rows and one hint.
    let hand: Vec<Card> = (1u8..=7).map(|r| c(Suit::Hearts, r)).collect();
    let obs = obs_with_hand(&hand);
    let lines = render_hand_strip(&obs, None, &[], SuitTheme::French);
    assert_eq!(lines.len(), 5);
    // Each of the first 4 rows includes stock, discard and seven cards.
    for row in lines.iter().take(4) {
        let text: String = row.spans.iter().map(|s| s.content.as_ref()).collect();
        // Rough sanity: 5 (stock) + 2 (gap) + 5 (discard) + 3 (gap)
        //             + 7*5 (hand tiles) + 6 (inter-tile gaps) = 56 cells.
        let w = text.chars().count();
        assert_eq!(w, 56, "row width {w}: {text:?}");
    }
    // Hint line mentions "stock" count.
    let hint: String = lines[4].spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(hint.contains("stock 25 cards"), "hint: {hint:?}");
}

#[test]
fn hand_strip_highlights_only_the_cursor_tile() {
    let hand: Vec<Card> = (1u8..=3).map(|r| c(Suit::Clubs, r)).collect();
    let obs = obs_with_hand(&hand);
    // Cursor on Hand(1): only the second card tile should use double
    // corners; stock, discard and the other two hand tiles use light.
    let lines = render_hand_strip(&obs, Some(CursorPos::Hand(1)), &[], SuitTheme::French);
    let top_row: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
    let count_double = top_row.matches('\u{2554}').count();
    let count_single = top_row.matches('\u{250C}').count();
    assert_eq!(count_double, 1, "exactly one selected tile: {top_row:?}");
    // Stock + discard + 2 unselected hand tiles = 4 light-border tiles.
    assert_eq!(count_single, 4, "unselected light borders: {top_row:?}");
}

#[test]
fn hand_strip_highlights_stock_or_discard() {
    let hand = [c(Suit::Clubs, 1)];
    let obs = obs_with_hand(&hand);
    let lines = render_hand_strip(&obs, Some(CursorPos::Stock), &[], SuitTheme::French);
    let top_row: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
    // First tile corner should be double (stock selected).
    assert!(
        top_row.starts_with('\u{2554}'),
        "stock not selected: {top_row:?}"
    );

    let lines = render_hand_strip(&obs, Some(CursorPos::Discard), &[], SuitTheme::French);
    let top_row: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
    // Stock is single, discard (next tile) is double.
    assert!(top_row.starts_with('\u{250C}'));
    assert_eq!(
        top_row.matches('\u{2554}').count(),
        1,
        "exactly one double border (the discard): {top_row:?}"
    );
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
    let s = hand_summary_string(&obs, SuitTheme::French);
    assert!(
        s.contains("[7\u{2663} 7\u{2666} 7\u{2665}]"),
        "set-of-3 of sevens: {s}"
    );
    assert!(
        s.contains("[5\u{2660} 6\u{2660} 7\u{2660} 8\u{2660}]"),
        "4-card run of spades: {s}"
    );
    assert!(!s.contains('|'), "no unmatched: {s}");
}
