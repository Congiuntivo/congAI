//! `key_to_intent` tests. All deterministic unit tests, no terminal.

use conga_tui::{KeyIntent, key_to_intent};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn shift(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::SHIFT)
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
fn c_and_p_bind_close_and_pass() {
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
fn d_is_no_longer_bound() {
    assert_eq!(key_to_intent(&key(KeyCode::Char('d'))), KeyIntent::Ignored);
    assert_eq!(key_to_intent(&key(KeyCode::Char('D'))), KeyIntent::Ignored);
}

#[test]
fn q_quits_and_esc_cancels() {
    assert_eq!(key_to_intent(&key(KeyCode::Char('q'))), KeyIntent::Quit);
    assert_eq!(key_to_intent(&key(KeyCode::Char('Q'))), KeyIntent::Quit);
    assert_eq!(key_to_intent(&key(KeyCode::Esc)), KeyIntent::Cancel);
}

#[test]
fn shift_arrows_swap() {
    assert_eq!(key_to_intent(&shift(KeyCode::Left)), KeyIntent::SwapLeft);
    assert_eq!(key_to_intent(&shift(KeyCode::Right)), KeyIntent::SwapRight);
}

#[test]
fn m_and_a_enter_meld_modes() {
    assert_eq!(key_to_intent(&key(KeyCode::Char('m'))), KeyIntent::MeldMode);
    assert_eq!(key_to_intent(&key(KeyCode::Char('M'))), KeyIntent::MeldMode);
    assert_eq!(key_to_intent(&key(KeyCode::Char('a'))), KeyIntent::AddMode);
    assert_eq!(key_to_intent(&key(KeyCode::Char('A'))), KeyIntent::AddMode);
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
    assert_eq!(key_to_intent(&key(KeyCode::Char('x'))), KeyIntent::Ignored);
    assert_eq!(key_to_intent(&key(KeyCode::Up)), KeyIntent::Ignored);
    assert_eq!(key_to_intent(&key(KeyCode::Down)), KeyIntent::Ignored);
    assert_eq!(key_to_intent(&key(KeyCode::Tab)), KeyIntent::Ignored);
}
