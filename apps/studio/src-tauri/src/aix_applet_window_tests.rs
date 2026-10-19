#![cfg(test)]

use super::*;

#[test]
fn closing_a_window_releases_its_home_draft_without_touching_other_windows() {
    let _guard = HOME_TEST_LOCK.lock().unwrap();
    reset_home_store();
    let first = stage_home_seed("a3s-digital-twin", "editor", r#"{"content":"scene"}"#).unwrap();
    home_store()
        .unwrap()
        .slots
        .get_mut(&("a3s-digital-twin".into(), "editor".into()))
        .unwrap()
        .holds_work = true;
    let other = stage_home_seed("a3s-presenton", "workbench", r#"{"content":"slides"}"#).unwrap();
    let label = window_label("a3s-digital-twin", "editor");
    home_store().unwrap().recovery_windows.insert(label.clone());
    assert!(protects_recovery(&label));
    retire_home_window("main").unwrap();
    assert!(
        read_home_deck("main", "a3s-digital-twin", "editor")
            .unwrap()
            .unwrap()
            .holds_work
    );
    retire_home_window(&window_label("a3s-digital-twin", "editor")).unwrap();
    assert!(!protects_recovery(&label));
    assert!(
        read_home_deck("main", "a3s-digital-twin", "editor")
            .unwrap()
            .is_none()
    );
    assert_eq!(
        read_home_deck("main", "a3s-presenton", "workbench")
            .unwrap()
            .unwrap()
            .delivery,
        other
    );
    let next =
        stage_home_seed("a3s-digital-twin", "editor", r#"{"content":"next scene"}"#).unwrap();
    assert!(next > first);
    assert!(
        !read_home_deck("main", "a3s-digital-twin", "editor")
            .unwrap()
            .unwrap()
            .holds_work
    );
}
