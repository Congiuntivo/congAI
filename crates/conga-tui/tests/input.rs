//! `key_to_intent` tests. All deterministic unit tests, no terminal.

use conga_tui::{KeyIntent, key_to_intent};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn arrow_keys_move_the_cursor() {
    assert_eq!(key_to_intent(&key(KeyCode::Left)), KeyIntent::MoveLeft);
    assert_eq!(key_to_intent(&key(KeyCode::Right)), KeyIntent::MoveRight);
}

#[test]
fn enter_activates_the_hovered_tile() {
    assert_eq!(key_to_intent(&key(KeyCode::Enter)), KeyIntent::Activate);
}

#[test]
fn d_c_p_bind_discard_close_pass() {
    assert_eq!(
        key_to_intent(&key(KeyCode::Char('d'))),
        KeyIntent::DiscardSelected
    );
    assert_eq!(
        key_to_intent(&key(KeyCode::Char('D'))),
        KeyIntent::DiscardSelected
    );
    assert_eq!(
        key_to_intent(&key(KeyCode::Char('c'))),
        KeyIntent::CloseSelected
    );
    assert_eq!(
        key_to_intent(&key(KeyCode::Char('C'))),
        KeyIntent::CloseSelected
    );
    assert_eq!(key_to_intent(&key(KeyCode::Char('p'))), KeyIntent::Pass);
    assert_eq!(key_to_intent(&key(KeyCode::Char('P'))), KeyIntent::Pass);
}

#[test]
fn q_and_esc_quit() {
    assert_eq!(key_to_intent(&key(KeyCode::Char('q'))), KeyIntent::Quit);
    assert_eq!(key_to_intent(&key(KeyCode::Char('Q'))), KeyIntent::Quit);
    assert_eq!(key_to_intent(&key(KeyCode::Esc)), KeyIntent::Quit);
}

#[test]
fn digits_are_no_longer_meaningful() {
    for d in b'0'..=b'9' {
        let got = key_to_intent(&key(KeyCode::Char(d as char)));
        assert_eq!(
            got,
            KeyIntent::Ignored,
            "digit {} should be ignored",
            d as char
        );
    }
}

#[test]
fn other_keys_are_ignored() {
    assert_eq!(key_to_intent(&key(KeyCode::Char('a'))), KeyIntent::Ignored);
    assert_eq!(key_to_intent(&key(KeyCode::Up)), KeyIntent::Ignored);
    assert_eq!(key_to_intent(&key(KeyCode::Down)), KeyIntent::Ignored);
    assert_eq!(key_to_intent(&key(KeyCode::Tab)), KeyIntent::Ignored);
}
