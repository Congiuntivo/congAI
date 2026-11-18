//! Pure rendering. `draw(frame, state)` composes the screen; the
//! per-widget helpers below are unit-testable without a terminal.

use conga_core::{
    Action, Card, CloseParams, DrawSource, Event, HandEval, MeldParams, Observation,
    best_meld_split, evaluate_hand, standard_40,
};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::app::{AppState, CursorPos, Mode};
use crate::suits::{SuitTheme, suit_color};

/// Human-visible string for a card: rank + the theme's suit glyph.
/// Example: `"7♣"` (French) or `"7✦"` (Spanish).
pub fn card_label(card: Card, theme: SuitTheme) -> String {
    format!("{}{}", card.rank(), theme.glyph(card.suit()))
}

/// Colored span for a card. Red for hearts / diamonds, default for
/// clubs / spades — same mapping in both themes.
fn card_span(card: Card, theme: SuitTheme) -> Span<'static> {
    let text = card_label(card, theme);
    let style = match suit_color(card.suit()) {
        Some(fg) => Style::default().fg(fg),
        None => Style::default(),
    };
    Span::styled(text, style)
}

/// Returns one `Line` grouping the acting seat's hand by the meld
/// solver's best split: meld cards first inside brackets, then `|` and
/// the unmatched cards followed by their point value in parens.
pub fn render_hand(obs: &Observation, theme: SuitTheme) -> Line<'static> {
    let eval = evaluate_hand(
        obs.own_hand,
        &MeldParams::standard(),
        &CloseParams::standard(),
        standard_40(),
    );
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (i, meld) in eval.splits.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" "));
        }
        spans.push(Span::raw("["));
        let cards: Vec<Card> = meld.cards.iter().collect();
        for (j, card) in cards.iter().enumerate() {
            if j > 0 {
                spans.push(Span::raw(" "));
            }
            spans.push(card_span(*card, theme));
        }
        spans.push(Span::raw("]"));
    }
    if !eval.unmatched.is_empty() {
        if !eval.splits.is_empty() {
            spans.push(Span::raw(" | "));
        }
        let unmatched: Vec<Card> = eval.unmatched.iter().collect();
        for (j, card) in unmatched.iter().enumerate() {
            if j > 0 {
                spans.push(Span::raw(" "));
            }
            spans.push(card_span(*card, theme));
        }
        spans.push(Span::raw(format!("({})", eval.unmatched_total)));
    }
    if eval.can_close {
        spans.push(Span::styled(
            "  CLOSE OK",
            Style::default().add_modifier(Modifier::BOLD),
        ));
    }
    Line::from(spans)
}

/// Human text for one action. Used in the numbered menu.
pub fn action_label(action: &Action, theme: SuitTheme) -> String {
    match action {
        Action::DrawStock => "Draw from stock".into(),
        Action::DrawDiscard => "Take the top discard".into(),
        Action::Discard(c) => format!("Discard {}", card_label(*c, theme)),
        Action::Close(c) => format!("Close with {}", card_label(*c, theme)),
        Action::LayMeld(m) => {
            let cards: Vec<Card> = m.cards.iter().collect();
            let joined = cards
                .iter()
                .map(|c| card_label(*c, theme))
                .collect::<Vec<_>>()
                .join(" ");
            format!("Lay meld [{joined}]")
        }
        Action::AddToMeld { card, meld_index } => {
            format!("Add {} to meld #{meld_index}", card_label(*card, theme))
        }
        Action::Pass => "Pass".into(),
    }
}

/// One Line per legal action, numbered 1..=10 (0 means 10).
pub fn render_legal_lines(legal: &[Action], theme: SuitTheme) -> Vec<Line<'static>> {
    legal
        .iter()
        .enumerate()
        .take(10)
        .map(|(i, a)| {
            let key = if i == 9 {
                "0".to_string()
            } else {
                (i + 1).to_string()
            };
            Line::from(vec![
                Span::styled(format!(" {key}) "), Style::default().fg(Color::Yellow)),
                Span::raw(action_label(a, theme)),
            ])
        })
        .collect()
}

fn event_label(event: &Event, theme: SuitTheme) -> String {
    match event {
        Event::Shuffled(_) => "shuffled the stock".into(),
        Event::Dealt { seat, hand_size } => {
            format!("seat {} dealt {} cards", seat.index(), hand_size)
        }
        Event::Drew { seat, source } => {
            let s = match source {
                DrawSource::Stock => "stock",
                DrawSource::TopDiscard => "discard",
            };
            format!("seat {} drew from {}", seat.index(), s)
        }
        Event::Discarded { seat, card } => {
            format!(
                "seat {} discarded {}",
                seat.index(),
                card_label(*card, theme)
            )
        }
        Event::Closed { seat, card } => {
            format!(
                "seat {} CLOSED with {}",
                seat.index(),
                card_label(*card, theme)
            )
        }
        Event::LaidMeld { seat, meld } => {
            let cards: Vec<Card> = meld.cards.iter().collect();
            let joined = cards
                .iter()
                .map(|c| card_label(*c, theme))
                .collect::<Vec<_>>()
                .join(" ");
            format!("seat {} laid [{joined}]", seat.index())
        }
        Event::AddedToMeld {
            seat,
            card,
            meld_index,
        } => format!(
            "seat {} added {} to meld #{}",
            seat.index(),
            card_label(*card, theme),
            meld_index
        ),
        Event::Passed { seat } => format!("seat {} passed", seat.index()),
        Event::HandEnded(result) => format!("hand ended: {:?}", result.points),
    }
}

pub fn render_events(events: &[Event], theme: SuitTheme) -> Vec<Line<'static>> {
    events
        .iter()
        .map(|e| Line::from(Span::raw(event_label(e, theme))))
        .collect()
}

pub fn render_header(state: &AppState) -> Vec<Line<'static>> {
    let obs = match state.observation.as_ref() {
        Some(o) => o,
        None => {
            return vec![Line::from(Span::raw(
                "waiting for the first observation...".to_string(),
            ))];
        }
    };
    let phase = format!("{:?}", obs.phase);
    let scores: Vec<String> = obs
        .scores
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let marker = if (i as u8) == state.own_seat {
                "you"
            } else {
                "AI"
            };
            format!("seat {i} ({marker}): {s}")
        })
        .collect();
    vec![
        Line::from(Span::raw(format!(
            "Hand {}  Phase: {phase}  Current seat: {}",
            state.hand_index,
            obs.current_seat.index()
        ))),
        Line::from(Span::raw(format!("Scores: {}", scores.join("  |  ")))),
    ]
}

pub fn render_table(obs: &Observation, theme: SuitTheme) -> Vec<Line<'static>> {
    let melds_line = if obs.melds_on_table.is_empty() {
        "Melds on table: (none)".to_string()
    } else {
        let parts: Vec<String> = obs
            .melds_on_table
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let cards: Vec<Card> = m.cards.iter().collect();
                let joined = cards
                    .iter()
                    .map(|c| card_label(*c, theme))
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("#{i} [{joined}]")
            })
            .collect();
        format!("Melds on table: {}", parts.join("  "))
    };
    vec![Line::from(Span::raw(melds_line))]
}

const TILE_INNER: usize = 3;

fn tile_chars(selected: bool) -> (char, char, char, char, char, char) {
    if selected {
        (
            '\u{2554}', '\u{2550}', '\u{2557}', '\u{2551}', '\u{255A}', '\u{255D}',
        )
    } else {
        (
            '\u{250C}', '\u{2500}', '\u{2510}', '\u{2502}', '\u{2514}', '\u{2518}',
        )
    }
}

/// 4 Lines describing one card tile. Width is always `TILE_INNER + 2`
/// cells (default 5). Selected tiles use a double border.
pub fn render_card_tile(card: Card, selected: bool, theme: SuitTheme) -> Vec<Line<'static>> {
    let (tl, h, tr, v, bl, br) = tile_chars(selected);
    let horiz: String = std::iter::repeat_n(h, TILE_INNER).collect();
    let top = format!("{tl}{horiz}{tr}");
    let bot = format!("{bl}{horiz}{br}");
    let rank_text = format!("{:>2}", card.rank().as_u8());
    let rank_line = format!("{v}{rank_text} {v}");
    let glyph = theme.glyph(card.suit()).to_string();
    let glyph_style = match suit_color(card.suit()) {
        Some(fg) => Style::default().fg(fg),
        None => Style::default(),
    };
    vec![
        Line::from(Span::raw(top)),
        Line::from(Span::raw(rank_line)),
        Line::from(vec![
            Span::raw(format!("{v} ")),
            Span::styled(glyph, glyph_style),
            Span::raw(format!(" {v}")),
        ]),
        Line::from(Span::raw(bot)),
    ]
}

/// 4 Lines describing the stock pile as a face-down tile.
pub fn render_stock_tile(selected: bool) -> Vec<Line<'static>> {
    let (tl, h, tr, v, bl, br) = tile_chars(selected);
    let horiz: String = std::iter::repeat_n(h, TILE_INNER).collect();
    let top = format!("{tl}{horiz}{tr}");
    let bot = format!("{bl}{horiz}{br}");
    let fill: String = std::iter::repeat_n('\u{2592}', TILE_INNER).collect();
    let body = format!("{v}{fill}{v}");
    vec![
        Line::from(Span::raw(top)),
        Line::from(Span::raw(body.clone())),
        Line::from(Span::raw(body)),
        Line::from(Span::raw(bot)),
    ]
}

/// 4 Lines describing the top of the discard pile. Blank frame if the
/// discard is empty (start of a hand or just after the closer's close).
pub fn render_discard_tile(
    top: Option<Card>,
    selected: bool,
    theme: SuitTheme,
) -> Vec<Line<'static>> {
    match top {
        Some(card) => render_card_tile(card, selected, theme),
        None => {
            let (tl, h, tr, v, bl, br) = tile_chars(selected);
            let horiz: String = std::iter::repeat_n(h, TILE_INNER).collect();
            let top_line = format!("{tl}{horiz}{tr}");
            let bot_line = format!("{bl}{horiz}{br}");
            let blank: String = std::iter::repeat_n(' ', TILE_INNER).collect();
            let body = format!("{v}{blank}{v}");
            vec![
                Line::from(Span::raw(top_line)),
                Line::from(Span::raw(body.clone())),
                Line::from(Span::raw(body)),
                Line::from(Span::raw(bot_line)),
            ]
        }
    }
}

/// Compose stock + discard + hand into 4 side-by-side tile rows, with a
/// 5th hint line below (stock count and best-meld split). `cursor`
/// highlights one tile with a double border; `highlighted_cards`
/// additionally highlights any hand card in the set (used by cycle
/// mode to show which cards a meld variant covers). `hand_order`
/// drives the tile order; the caller is responsible for keeping it
/// in sync with `obs.own_hand`.
pub fn render_hand_strip(
    obs: &Observation,
    hand_order: &[Card],
    cursor: Option<CursorPos>,
    highlighted_cards: &[Card],
    theme: SuitTheme,
) -> Vec<Line<'static>> {
    let stock = render_stock_tile(cursor == Some(CursorPos::Stock));
    let discard = render_discard_tile(obs.top_discard, cursor == Some(CursorPos::Discard), theme);

    let hand: Vec<Vec<Line<'static>>> = hand_order
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let selected = cursor == Some(CursorPos::Hand(i)) || highlighted_cards.contains(c);
            render_card_tile(*c, selected, theme)
        })
        .collect();

    let mut out = Vec::with_capacity(5);
    for row in 0..4 {
        let mut spans: Vec<Span<'static>> = Vec::new();
        spans.extend(stock[row].spans.iter().cloned());
        spans.push(Span::raw("  "));
        spans.extend(discard[row].spans.iter().cloned());
        spans.push(Span::raw("   "));
        for (i, tile) in hand.iter().enumerate() {
            if i > 0 {
                spans.push(Span::raw(" "));
            }
            spans.extend(tile[row].spans.iter().cloned());
        }
        out.push(Line::from(spans));
    }

    let summary = hand_summary_string(obs, theme);
    out.push(Line::from(Span::raw(format!(
        "stock {} cards   {summary}",
        obs.stock_size
    ))));
    out
}

/// Full-frame draw. Called from `terminal.draw(|f| draw(f, &state))`.
pub fn draw(frame: &mut Frame, state: &AppState) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(7),
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(area);

    // Header.
    frame.render_widget(
        Paragraph::new(render_header(state))
            .block(Block::default().borders(Borders::ALL).title(" conga ")),
        chunks[0],
    );

    // Your hand: stock + discard + card tiles with the hint line below.
    // Cursor is visible only in Normal mode; cycle modes highlight the
    // cards of the currently-pointed meld variant instead.
    let (cursor, highlighted) = match state.mode {
        Mode::Normal => (state.is_my_turn().then_some(state.cursor), Vec::new()),
        Mode::LayMeldCycle { .. } | Mode::AddToMeldCycle { .. } => {
            (None, state.current_cycle_cards())
        }
    };
    let hand_lines = state
        .observation
        .as_ref()
        .map(|o| render_hand_strip(o, &state.hand_order, cursor, &highlighted, state.theme))
        .unwrap_or_else(|| vec![Line::from(Span::raw("(no observation yet)".to_string()))]);
    frame.render_widget(
        Paragraph::new(hand_lines)
            .block(Block::default().borders(Borders::ALL).title(" your hand ")),
        chunks[1],
    );

    // Table: melds laid by any seat.
    let table_lines = state
        .observation
        .as_ref()
        .map(|o| render_table(o, state.theme))
        .unwrap_or_else(|| vec![Line::from(Span::raw(""))]);
    frame.render_widget(
        Paragraph::new(table_lines)
            .block(Block::default().borders(Borders::ALL).title(" table "))
            .wrap(Wrap { trim: true }),
        chunks[2],
    );

    // Recent events, full width.
    frame.render_widget(
        Paragraph::new(render_events(&state.recent_events, state.theme))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" recent events "),
            )
            .wrap(Wrap { trim: true }),
        chunks[3],
    );

    // Bindings footer: greys out illegal keys.
    frame.render_widget(
        Paragraph::new(vec![render_bindings_footer(state)])
            .block(Block::default().borders(Borders::ALL).title(" bindings ")),
        chunks[4],
    );

    // Status line.
    frame.render_widget(
        Paragraph::new(state.status_line.clone())
            .block(Block::default().borders(Borders::ALL).title(" status ")),
        chunks[5],
    );
}

/// One line listing the key bindings. The Enter label is contextual
/// (`draw` on Stock / Discard, `discard` on a hand card) and bindings
/// that aren't legal right now are greyed out. In cycle modes the
/// bindings switch to the cycle verbs.
pub fn render_bindings_footer(state: &AppState) -> Line<'static> {
    match state.mode {
        Mode::Normal => normal_bindings_footer(state),
        Mode::LayMeldCycle { .. } | Mode::AddToMeldCycle { .. } => cycle_bindings_footer(),
    }
}

fn normal_bindings_footer(state: &AppState) -> Line<'static> {
    let legal_active = state.is_my_turn();
    let hand_card = state.hand_card_at_cursor();

    let (enter_label, enter_legal) = match state.cursor {
        CursorPos::Stock => ("draw", state.legal.contains(&Action::DrawStock)),
        CursorPos::Discard => ("draw", state.legal.contains(&Action::DrawDiscard)),
        CursorPos::Hand(_) => (
            "discard",
            hand_card.is_some_and(|c| state.legal.contains(&Action::Discard(c))),
        ),
    };
    let legal_close =
        legal_active && hand_card.is_some_and(|c| state.legal.contains(&Action::Close(c)));
    let legal_pass = legal_active && state.legal.contains(&Action::Pass);
    let legal_meld = legal_active && state.legal.iter().any(|a| matches!(a, Action::LayMeld(_)));
    let legal_add = legal_active
        && state
            .legal
            .iter()
            .any(|a| matches!(a, Action::AddToMeld { .. }));

    let spans = vec![
        binding_span("←→ move", legal_active),
        Span::raw("  "),
        binding_span(&format!("Enter {enter_label}"), legal_active && enter_legal),
        Span::raw("  "),
        binding_span("c close", legal_close),
        Span::raw("  "),
        binding_span("m meld", legal_meld),
        Span::raw("  "),
        binding_span("a add", legal_add),
        Span::raw("  "),
        binding_span("p pass", legal_pass),
        Span::raw("  "),
        binding_span("q quit", true),
    ];
    Line::from(spans)
}

fn cycle_bindings_footer() -> Line<'static> {
    let spans = vec![
        binding_span("←→ cycle", true),
        Span::raw("  "),
        binding_span("Enter commit", true),
        Span::raw("  "),
        binding_span("Esc cancel", true),
        Span::raw("  "),
        binding_span("q quit", true),
    ];
    Line::from(spans)
}

/// Short human-readable label for the currently-cycled meld. Used in
/// the status line while a cycle mode is active.
pub fn cycle_status_line(state: &AppState) -> Option<String> {
    let (kind, variants_len, index) = match &state.mode {
        Mode::LayMeldCycle { variants, index } => ("lay meld", variants.len(), *index),
        Mode::AddToMeldCycle { variants, index } => ("add to meld", variants.len(), *index),
        Mode::Normal => return None,
    };
    let action = state.current_cycle_action()?;
    Some(format!(
        "{kind} {}/{variants_len}: {}",
        index + 1,
        action_label(&action, state.theme)
    ))
}

fn binding_span(text: &str, legal: bool) -> Span<'static> {
    let style = if legal {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    Span::styled(text.to_string(), style)
}

/// Convenience: a bare `HandEval` for tests that want to assert on the
/// solver output alongside render output.
pub fn hand_eval(obs: &Observation) -> HandEval {
    evaluate_hand(
        obs.own_hand,
        &MeldParams::standard(),
        &CloseParams::standard(),
        standard_40(),
    )
}

/// Short best-split summary used by replay in slice 7; kept here so
/// both the live TUI and the replay CLI render hands the same way.
pub fn hand_summary_string(obs: &Observation, theme: SuitTheme) -> String {
    let split = best_meld_split(obs.own_hand, &MeldParams::standard());
    let mut parts: Vec<String> = Vec::new();
    for meld in &split.splits {
        let cards: Vec<Card> = meld.cards.iter().collect();
        parts.push(format!(
            "[{}]",
            cards
                .iter()
                .map(|c| card_label(*c, theme))
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    if !split.unmatched.is_empty() {
        let cards: Vec<Card> = split.unmatched.iter().collect();
        let joined = cards
            .iter()
            .map(|c| card_label(*c, theme))
            .collect::<Vec<_>>()
            .join(" ");
        parts.push(format!("| {joined}({})", split.unmatched_total));
    }
    parts.join(" ")
}
