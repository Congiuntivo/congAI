//! Translate keyboard input to an in-game intent.
//!
//! Digits `1..9, 0` select an action by its menu index (1-based, with
//! `0` meaning 10). `q` quits. Everything else is ignored.

use crossterm::event::{KeyCode, KeyEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyIntent {
    /// Pick the action at menu index `0..legal.len()`. The caller must
    /// range-check: `index < legal.len()`.
    PickAction(usize),
    Quit,
    /// A key with no in-game meaning; the driver ignores it.
    Ignored,
}

pub fn key_to_intent(key: &KeyEvent) -> KeyIntent {
    match key.code {
        KeyCode::Char(c) => char_to_intent(c),
        KeyCode::Esc => KeyIntent::Quit,
        _ => KeyIntent::Ignored,
    }
}

fn char_to_intent(c: char) -> KeyIntent {
    match c {
        'q' | 'Q' => KeyIntent::Quit,
        '1'..='9' => KeyIntent::PickAction((c as u8 - b'1') as usize),
        '0' => KeyIntent::PickAction(9),
        _ => KeyIntent::Ignored,
    }
}
