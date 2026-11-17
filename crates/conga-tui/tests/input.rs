//! `key_to_intent` tests. All deterministic unit tests, no terminal.

use conga_tui::{KeyIntent, key_to_intent};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn digits_1_to_9_map_to_indices_0_to_8() {
    for i in 1..=9u8 {
        let c = char::from(b'0' + i);
        let got = key_to_intent(&key(KeyCode::Char(c)));
        assert_eq!(
            got,
            KeyIntent::PickAction((i - 1) as usize),
            "unexpected intent for {c}"
        );
    }
}

#[test]
fn digit_0_maps_to_index_9() {
    assert_eq!(
        key_to_intent(&key(KeyCode::Char('0'))),
        KeyIntent::PickAction(9)
    );
}

#[test]
fn q_and_esc_quit() {
    assert_eq!(key_to_intent(&key(KeyCode::Char('q'))), KeyIntent::Quit);
    assert_eq!(key_to_intent(&key(KeyCode::Char('Q'))), KeyIntent::Quit);
    assert_eq!(key_to_intent(&key(KeyCode::Esc)), KeyIntent::Quit);
}

#[test]
fn other_keys_are_ignored() {
    assert_eq!(key_to_intent(&key(KeyCode::Char('a'))), KeyIntent::Ignored);
    assert_eq!(key_to_intent(&key(KeyCode::Enter)), KeyIntent::Ignored);
    assert_eq!(key_to_intent(&key(KeyCode::Up)), KeyIntent::Ignored);
}
