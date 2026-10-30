#![cfg(test)]

use super::*;
use crossterm_winapi::InputRecord;
use winapi::um::wincon::{INPUT_RECORD, KEY_EVENT};

fn native_key(virtual_key: i32, down: bool, character: u16) -> KeyEventRecord {
    // INPUT_RECORD and KEY_EVENT_RECORD contain only integer/union POD fields.
    let mut native: INPUT_RECORD = unsafe { std::mem::zeroed() };
    native.EventType = KEY_EVENT;
    let key = unsafe { native.Event.KeyEvent_mut() };
    key.bKeyDown = i32::from(down);
    key.wRepeatCount = 1;
    key.wVirtualKeyCode = virtual_key as u16;
    key.wVirtualScanCode = 56;
    unsafe { *key.uChar.UnicodeChar_mut() = character };
    let InputRecord::KeyEvent(record) = InputRecord::from(native) else {
        panic!("native key record");
    };
    record
}

#[test]
fn committed_alt_code_characters_are_text_presses() {
    for character in [0x0301, 0x200d, 0x20ac] {
        let event = handle_key_event(native_key(VK_MENU, false, character), &mut None);
        assert_eq!(
            event,
            Some(Event::Key(KeyEvent::new(
                KeyCode::Char(char::from_u32(u32::from(character)).unwrap()),
                KeyModifiers::NONE,
            )))
        );
    }
}

#[test]
fn ordinary_alt_release_does_not_insert_text() {
    assert_eq!(
        handle_key_event(native_key(VK_MENU, false, 0), &mut None),
        None
    );
}

#[test]
fn ordinary_character_release_remains_a_release() {
    assert_eq!(
        handle_key_event(native_key(0x41, false, u16::from(b'a')), &mut None),
        Some(Event::Key(KeyEvent::new_with_kind(
            KeyCode::Char('a'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
        ))),
    );
}

#[test]
fn committed_alt_code_surrogates_still_form_one_text_press() {
    let mut pending = None;
    assert_eq!(
        handle_key_event(native_key(VK_MENU, false, 0xd83d), &mut pending),
        None
    );
    assert_eq!(
        handle_key_event(native_key(VK_MENU, false, 0xdc69), &mut pending),
        Some(Event::Key(KeyEvent::new(
            KeyCode::Char('\u{1f469}'),
            KeyModifiers::NONE
        )))
    );
    assert!(pending.is_none());
}
