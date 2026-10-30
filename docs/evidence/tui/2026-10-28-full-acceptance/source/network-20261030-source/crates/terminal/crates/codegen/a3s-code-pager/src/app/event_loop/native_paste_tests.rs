use super::*;

fn key(code: KeyCode, kind: KeyEventKind, modifiers: KeyModifiers) -> TimedInputEvent {
    TimedInputEvent::now(Event::Key(KeyEvent::new_with_kind(code, modifiers, kind)))
}

#[test]
fn native_space_releases_do_not_split_a_large_unicode_paste() {
    let text = format!(
        "中文\n{}\nEND",
        "中文 λ e\u{301} payload 0123456789 <> & \" ' / \\ ".repeat(512)
    );
    let events = text
        .chars()
        .flat_map(|character| {
            let code = if character == '\n' {
                KeyCode::Enter
            } else {
                KeyCode::Char(character)
            };
            [
                key(code, KeyEventKind::Press, KeyModifiers::NONE),
                key(code, KeyEventKind::Release, KeyModifiers::NONE),
            ]
        })
        .collect();
    let result = coalesce_live_keys(events, false, false);
    assert_eq!(
        result.len(),
        1,
        "one native paste must not become thousands of textarea edits"
    );
    assert_eq!(result[0].event, Event::Paste(text));
}

#[test]
fn ordinary_single_line_keys_keep_their_edit_semantics() {
    let text = "two words";
    let events = text
        .chars()
        .flat_map(|character| {
            [
                key(
                    KeyCode::Char(character),
                    KeyEventKind::Press,
                    KeyModifiers::NONE,
                ),
                key(
                    KeyCode::Char(character),
                    KeyEventKind::Release,
                    KeyModifiers::NONE,
                ),
            ]
        })
        .collect();
    let result = coalesce_live_keys(events, false, false);
    let chars = result
        .iter()
        .map(|event| match event.event {
            Event::Key(KeyEvent {
                code: KeyCode::Char(character),
                kind: KeyEventKind::Press,
                ..
            }) => character,
            _ => panic!("ordinary typing must remain key presses"),
        })
        .collect::<String>();
    assert_eq!(chars, text);
}

#[test]
fn owned_voice_hold_keeps_its_release() {
    let events = vec![
        key(
            KeyCode::Char(' '),
            KeyEventKind::Release,
            KeyModifiers::NONE,
        ),
        key(KeyCode::Char('a'), KeyEventKind::Press, KeyModifiers::NONE),
        key(KeyCode::Char('b'), KeyEventKind::Press, KeyModifiers::NONE),
    ];
    assert_eq!(coalesce_live_keys(events.clone(), false, true), events);
}

#[test]
fn voice_press_and_release_in_one_batch_stay_paired() {
    for (code, modifiers) in [
        (KeyCode::Char(' '), KeyModifiers::CONTROL),
        (KeyCode::F(8), KeyModifiers::NONE),
    ] {
        let events = vec![
            key(code, KeyEventKind::Press, modifiers),
            key(code, KeyEventKind::Release, KeyModifiers::NONE),
            key(KeyCode::Char('a'), KeyEventKind::Press, KeyModifiers::NONE),
        ];
        assert_eq!(coalesce_live_keys(events.clone(), false, false), events);
    }
}

#[test]
fn voice_chord_is_preserved_beside_an_actual_paste() {
    let events = vec![
        TimedInputEvent::now(Event::Paste("payload".into())),
        key(
            KeyCode::Char(' '),
            KeyEventKind::Press,
            KeyModifiers::CONTROL,
        ),
        key(
            KeyCode::Char(' '),
            KeyEventKind::Release,
            KeyModifiers::NONE,
        ),
        key(KeyCode::Char('a'), KeyEventKind::Press, KeyModifiers::NONE),
    ];
    assert_eq!(coalesce_live_keys(events.clone(), false, false), events);
}

#[test]
fn card_and_game_contexts_keep_all_original_keys() {
    let events = vec![
        key(KeyCode::Char(' '), KeyEventKind::Press, KeyModifiers::NONE),
        key(
            KeyCode::Char(' '),
            KeyEventKind::Release,
            KeyModifiers::NONE,
        ),
        key(KeyCode::Char('a'), KeyEventKind::Press, KeyModifiers::NONE),
    ];
    assert_eq!(coalesce_live_keys(events.clone(), true, false), events);
}

#[test]
fn unowned_voice_release_does_not_create_a_voice_operation() {
    let events = vec![
        key(KeyCode::F(8), KeyEventKind::Release, KeyModifiers::NONE),
        key(KeyCode::Char('a'), KeyEventKind::Press, KeyModifiers::NONE),
        key(KeyCode::Char('b'), KeyEventKind::Press, KeyModifiers::NONE),
    ];
    let expected = events[1..].to_vec();
    assert_eq!(coalesce_live_keys(events, false, false), expected);
}

#[test]
fn native_end_fence_keeps_submit_out_of_the_paste() {
    let text = "中文\ncomplete native paste";
    let mut events = text
        .chars()
        .map(|character| {
            let code = if character == '\n' {
                KeyCode::Enter
            } else {
                KeyCode::Char(character)
            };
            key(code, KeyEventKind::Press, KeyModifiers::NONE)
        })
        .collect::<Vec<_>>();
    let end = key(KeyCode::End, KeyEventKind::Press, KeyModifiers::NONE);
    let submit = key(KeyCode::Enter, KeyEventKind::Press, KeyModifiers::NONE);
    events.extend([end.clone(), submit.clone()]);
    let result = coalesce_live_keys(events, false, false);
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].event, Event::Paste(text.into()));
    assert_eq!(result[1], end);
    assert_eq!(result[2], submit);
}

#[test]
fn paste_fragments_preserve_a_following_esc_press() {
    let paste = TimedInputEvent::now(Event::Paste("session-id".into()));
    let trailing_text = key(KeyCode::Char('a'), KeyEventKind::Press, KeyModifiers::NONE);
    let esc = key(KeyCode::Esc, KeyEventKind::Press, KeyModifiers::NONE);
    let result = coalesce_live_keys(vec![paste, trailing_text, esc.clone()], false, false);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].event, Event::Paste("session-ida".into()));
    assert_eq!(result[1], esc);
}

#[test]
fn native_single_line_paste_stays_text_in_picker() {
    let text = "e1234567-1234-1234-1234-123456789abc".to_string();
    let events = text
        .chars()
        .flat_map(|character| {
            [
                key(
                    KeyCode::Char(character),
                    KeyEventKind::Press,
                    KeyModifiers::NONE,
                ),
                key(
                    KeyCode::Char(character),
                    KeyEventKind::Release,
                    KeyModifiers::NONE,
                ),
            ]
        })
        .collect();
    let result = coalesce_live_keys_with_single_line(events, false, false, true);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].event, Event::Paste(text));
}

#[tokio::test(start_paused = true)]
async fn native_paste_detection_tolerates_input_record_gap() {
    let (tx, mut input_rx) = tokio::sync::mpsc::unbounded_channel();
    tx.send(key(
        KeyCode::Char('e'),
        KeyEventKind::Release,
        KeyModifiers::NONE,
    ))
    .unwrap();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(5)).await;
        tx.send(key(
            KeyCode::Char('1'),
            KeyEventKind::Press,
            KeyModifiers::NONE,
        ))
        .unwrap();
    });
    let mut batch = vec![key(
        KeyCode::Char('e'),
        KeyEventKind::Press,
        KeyModifiers::NONE,
    )];
    assert!(detect_paste(&mut batch, &mut input_rx).await);
    assert_eq!(batch.len(), 3);
}
