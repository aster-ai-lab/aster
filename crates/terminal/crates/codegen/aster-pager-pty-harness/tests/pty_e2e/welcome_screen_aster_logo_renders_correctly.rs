// Per-test-case module for the `pty_e2e` integration test crate.
#[allow(unused_imports)]
use super::common::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn welcome_screen_aster_logo_renders_correctly() {
    let content = ContentController::start().await.expect("start content");
    let binary = pager_binary().expect("resolve pager binary");
    let mut harness =
        PtyHarness::spawn_with_content(&binary, DEFAULT_ROWS, DEFAULT_COLS, &content, &[])
            .expect("spawn pager");
    harness
        .wait_for_text(WELCOME_SCREEN_SENTINEL, WELCOME_TIMEOUT)
        .expect("welcome text");
    let screen = harness.screen_contents();
    assert!(
        screen
            .as_bytes()
            .windows(4)
            .any(|run| run.iter().all(|byte| matches!(byte, b'#' | b'@' | b'*'))),
        "Aster ASCII star missing:\n{screen}"
    );
    assert!(
        !screen
            .chars()
            .any(|ch| ('\u{2800}'..='\u{28ff}').contains(&ch)),
        "welcome logo must use ASCII glyphs"
    );
    harness.quit().expect("clean quit");
}
