//! Translate a `KeyIntent` plus cursor + hand display order + legal
//! actions into a concrete `Action`, or an error message the status
//! line can display. Pure functions so they're testable without a
//! terminal.

use conga_core::{Action, Card};

use crate::app::CursorPos;
use crate::input::KeyIntent;
use crate::render::card_label;
use crate::suits::SuitTheme;

/// Result of resolving a key intent against Normal-mode state.
/// `Accept` commits the action; `Reject` feeds the message to the
/// status line; `MoveLeft` / `MoveRight` ask the driver to move the
/// cursor; `SwapLeft` / `SwapRight` ask it to swap the hovered hand
/// card with its neighbor in the display order; `EnterLayMeld` /
/// `EnterAddMeld` switch to the matching cycle mode; `Cancel` is
/// `Esc` in Normal (driver treats it as quit). Cycle-mode keys are
/// resolved by [`resolve_cycle`] instead.
pub enum Resolved {
    Accept(Action),
    Reject(String),
    MoveLeft,
    MoveRight,
    SwapLeft,
    SwapRight,
    EnterLayMeld,
    EnterAddMeld,
    Cancel,
    Quit,
    NoOp,
}

/// Result of resolving a key intent in cycle mode. `Commit` returns
/// the Action the driver should send and switch back to Normal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CycleResolved {
    Prev,
    Next,
    Commit,
    Cancel,
    Quit,
    Ignore,
}

/// Cycle-mode key resolution: movement rotates the index, Enter
/// commits, Esc cancels, q quits. Shift+arrows (reorder) are ignored
/// in cycle mode. Everything else is ignored so stray keystrokes do
/// not eject the user from the mode.
pub fn resolve_cycle(intent: KeyIntent) -> CycleResolved {
    match intent {
        KeyIntent::MoveLeft => CycleResolved::Prev,
        KeyIntent::MoveRight => CycleResolved::Next,
        KeyIntent::Activate => CycleResolved::Commit,
        KeyIntent::Cancel => CycleResolved::Cancel,
        KeyIntent::Quit => CycleResolved::Quit,
        _ => CycleResolved::Ignore,
    }
}

pub fn resolve(
    intent: KeyIntent,
    cursor: CursorPos,
    hand_order: &[Card],
    legal: &[Action],
    theme: SuitTheme,
) -> Resolved {
    match intent {
        KeyIntent::MoveLeft => Resolved::MoveLeft,
        KeyIntent::MoveRight => Resolved::MoveRight,
        KeyIntent::SwapLeft => Resolved::SwapLeft,
        KeyIntent::SwapRight => Resolved::SwapRight,
        KeyIntent::MeldMode => Resolved::EnterLayMeld,
        KeyIntent::AddMode => Resolved::EnterAddMeld,
        KeyIntent::Cancel => Resolved::Cancel,
        KeyIntent::Quit => Resolved::Quit,
        KeyIntent::Ignored => Resolved::NoOp,
        KeyIntent::Activate => resolve_activate(cursor, hand_order, legal, theme),
        KeyIntent::CloseSelected => resolve_close(cursor, hand_order, legal, theme),
        KeyIntent::Pass => {
            if legal.contains(&Action::Pass) {
                Resolved::Accept(Action::Pass)
            } else {
                Resolved::Reject("pass is not legal right now".into())
            }
        }
    }
}

fn resolve_activate(
    cursor: CursorPos,
    hand_order: &[Card],
    legal: &[Action],
    theme: SuitTheme,
) -> Resolved {
    match cursor {
        CursorPos::Stock => {
            if legal.contains(&Action::DrawStock) {
                Resolved::Accept(Action::DrawStock)
            } else {
                Resolved::Reject("drawing from stock is not legal right now".into())
            }
        }
        CursorPos::Discard => {
            if legal.contains(&Action::DrawDiscard) {
                Resolved::Accept(Action::DrawDiscard)
            } else {
                Resolved::Reject("drawing from discard is not legal right now".into())
            }
        }
        CursorPos::Hand(i) => match hand_order.get(i).copied() {
            Some(card) => {
                let act = Action::Discard(card);
                if legal.contains(&act) {
                    Resolved::Accept(act)
                } else {
                    Resolved::Reject(format!(
                        "{} is not a legal discard right now",
                        card_label(card, theme)
                    ))
                }
            }
            None => Resolved::Reject("cursor is past the end of the hand".into()),
        },
    }
}

fn resolve_close(
    cursor: CursorPos,
    hand_order: &[Card],
    legal: &[Action],
    theme: SuitTheme,
) -> Resolved {
    match cursor {
        CursorPos::Hand(i) => match hand_order.get(i).copied() {
            Some(card) => {
                let act = Action::Close(card);
                if legal.contains(&act) {
                    Resolved::Accept(act)
                } else {
                    Resolved::Reject(format!(
                        "closing with {} is not legal right now",
                        card_label(card, theme)
                    ))
                }
            }
            None => Resolved::Reject("cursor is past the end of the hand".into()),
        },
        _ => Resolved::Reject("move the cursor onto a hand card first".into()),
    }
}
