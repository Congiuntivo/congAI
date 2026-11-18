//! Translate keyboard input to an in-game intent.
//!
//! m2 slice 10: the TUI uses a tile cursor instead of a numbered-action
//! list. Keys split into movement (`←/→`), binding keys for the hovered
//! tile (`Enter`, `d`, `c`, `p`) and session control (`q`, `Esc`). The
//! driver translates these intents into actions against the current
//! `legal` set and the current cursor position.

use crossterm::event::{KeyCode, KeyEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyIntent {
    MoveLeft,
    MoveRight,
    /// Enter: act on the hovered tile (Stock → DrawStock, Discard →
    /// DrawDiscard, Hand(i) → Discard(hand[i])).
    Activate,
    /// `d`: discard the hovered hand card.
    DiscardSelected,
    /// `c`: close with the hovered hand card.
    CloseSelected,
    /// `p`: pass (legal in showdown).
    Pass,
    Quit,
    /// A key with no in-game meaning; the driver ignores it.
    Ignored,
}

pub fn key_to_intent(key: &KeyEvent) -> KeyIntent {
    match key.code {
        KeyCode::Left => KeyIntent::MoveLeft,
        KeyCode::Right => KeyIntent::MoveRight,
        KeyCode::Enter => KeyIntent::Activate,
        KeyCode::Esc => KeyIntent::Quit,
        KeyCode::Char(c) => char_to_intent(c),
        _ => KeyIntent::Ignored,
    }
}

fn char_to_intent(c: char) -> KeyIntent {
    match c {
        'q' | 'Q' => KeyIntent::Quit,
        'd' | 'D' => KeyIntent::DiscardSelected,
        'c' | 'C' => KeyIntent::CloseSelected,
        'p' | 'P' => KeyIntent::Pass,
        _ => KeyIntent::Ignored,
    }
}
