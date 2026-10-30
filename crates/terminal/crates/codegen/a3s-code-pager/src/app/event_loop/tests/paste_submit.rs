use super::*;

#[test]
fn merge_paste_and_key_fragments() {
    let events = vec![
        TimedInputEvent::now(Event::Paste("hello\nwor".into())),
        press(KeyCode::Char('l')),
        press(KeyCode::Char('d')),
    ];
    let result = coalesce_rapid_keys(events);
    assert_eq!(result.len(), 1);
    assert_eq!(
        nth(&result, 0).event,
        Event::Paste("hello\nworld".to_string())
    );
}
#[test]
fn resize_and_bracketed_paste_keep_the_trailing_submit() {
    let events = vec![
        TimedInputEvent::now(Event::Resize(100, 30)),
        TimedInputEvent::now(Event::Paste("wide".into())),
        press(KeyCode::Enter),
    ];
    let expected = events
        .iter()
        .map(|event| event.event.clone())
        .collect::<Vec<_>>();
    let result = coalesce_rapid_keys(events);
    assert_eq!(
        result
            .into_iter()
            .map(|event| event.event)
            .collect::<Vec<_>>(),
        expected
    );
}
#[test]
fn bracketed_paste_submit_survives_a_following_release_and_resize() {
    let events = vec![
        TimedInputEvent::now(Event::Paste("wide".into())),
        press(KeyCode::Enter),
        release(KeyCode::Enter),
        TimedInputEvent::now(Event::Resize(120, 40)),
    ];
    let result = coalesce_rapid_keys(events);
    assert_eq!(result.len(), 3);
    assert_eq!(nth(&result, 0).event, Event::Paste("wide".into()));
    assert!(matches!(nth(&result, 1).event, Event::Key(key) if key.code == KeyCode::Enter));
    assert!(matches!(nth(&result, 2).event, Event::Resize(120, 40)));
}
