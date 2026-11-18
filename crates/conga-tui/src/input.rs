//! Translate keyboard input to an in-game intent.
//!
//! m2 slice 10: cursor model replaced the numbered action list.
//! m2 slice 11: `m` / `a` enter meld-cycle modes; `Esc` means "cancel
//! the current mode" rather than always quit (the driver falls back to
//! Quit when the mode is already `Normal`).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyIntent {
    MoveLeft,
    MoveRight,
    /// Shift+←: swap the hovered hand card with its left neighbor.
    SwapLeft,
    /// Shift+→: swap the hovered hand card with its right neighbor.
    SwapRight,
    /// Enter: commit — Normal: act on the hovered tile; Cycle: commit
    /// the currently-pointed meld variant.
    Activate,
    /// `c`: close with the hovered hand card.
    CloseSelected,
    /// `p`: pass (legal in showdown).
    Pass,
    /// `m`: enter LayMeld cycle mode.
    MeldMode,
    /// `a`: enter AddToMeld cycle mode.
    AddMode,
    /// `Esc`: cancel the current mode, or quit if already in Normal.
    Cancel,
    /// `q`: quit the TUI unconditionally.
    Quit,
    /// A key with no in-game meaning; the driver ignores it.
    Ignored,
}

pub fn key_to_intent(key: &KeyEvent) -> KeyIntent {
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    match key.code {
        KeyCode::Left if shift => KeyIntent::SwapLeft,
        KeyCode::Right if shift => KeyIntent::SwapRight,
        KeyCode::Left => KeyIntent::MoveLeft,
        KeyCode::Right => KeyIntent::MoveRight,
        KeyCode::Enter => KeyIntent::Activate,
        KeyCode::Esc => KeyIntent::Cancel,
        KeyCode::Char(c) => char_to_intent(c),
        _ => KeyIntent::Ignored,
    }
}

fn char_to_intent(c: char) -> KeyIntent {
    match c {
        'q' | 'Q' => KeyIntent::Quit,
        'c' | 'C' => KeyIntent::CloseSelected,
        'p' | 'P' => KeyIntent::Pass,
        'm' | 'M' => KeyIntent::MeldMode,
        'a' | 'A' => KeyIntent::AddMode,
        _ => KeyIntent::Ignored,
    }
}
