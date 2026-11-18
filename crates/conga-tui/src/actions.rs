//! Translate a `KeyIntent` plus cursor + legal actions into a concrete
//! `Action`, or an error message the status line can display. Pure
//! function so it's testable without a terminal.

use conga_core::{Action, Card, Observation};

use crate::app::CursorPos;
use crate::input::KeyIntent;
use crate::render::card_label;
use crate::suits::SuitTheme;

/// Result of resolving a key intent against the current state. `Accept`
/// commits the action; `Reject` keeps the cursor in place and feeds the
/// message to the status line; `Movement` means the driver should
/// update the cursor (left/right) and re-draw without acting.
pub enum Resolved {
    Accept(Action),
    Reject(String),
    MoveLeft,
    MoveRight,
    Quit,
    /// Ignored keys produce no visible effect.
    NoOp,
}

pub fn resolve(
    intent: KeyIntent,
    cursor: CursorPos,
    obs: &Observation,
    legal: &[Action],
    theme: SuitTheme,
) -> Resolved {
    match intent {
        KeyIntent::MoveLeft => Resolved::MoveLeft,
        KeyIntent::MoveRight => Resolved::MoveRight,
        KeyIntent::Quit => Resolved::Quit,
        KeyIntent::Ignored => Resolved::NoOp,
        KeyIntent::Activate => resolve_activate(cursor, obs, legal, theme),
        KeyIntent::CloseSelected => resolve_close(cursor, obs, legal, theme),
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
    obs: &Observation,
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
        CursorPos::Hand(i) => match hand_card(obs, i) {
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
    obs: &Observation,
    legal: &[Action],
    theme: SuitTheme,
) -> Resolved {
    match cursor {
        CursorPos::Hand(i) => match hand_card(obs, i) {
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

fn hand_card(obs: &Observation, index: usize) -> Option<Card> {
    obs.own_hand.iter().nth(index)
}
