#![cfg(test)]

use super::driver::LiveTui;
use std::time::{Duration, Instant};

fn key(tui: &mut LiveTui, sequence: &[u8]) {
    tui.bytes(sequence);
    tui.pump(200);
}

fn visible(tui: &mut LiveTui, needle: &str) {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        tui.pump(80);
        let screen = tui.physical_screen();
        if screen.contains(needle) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "expected visible {needle:?}\n{screen}"
        );
    }
}

#[test]
#[ignore = "requires the paired source build and original kimi/k3.1-pro ACL"]
fn kimi_find_match_positions_jump_cancel_and_vim_navigation() {
    let mut tui = LiveTui::new("reading-navigation");
    let token = format!("NAVMATCH-{}", uuid::Uuid::new_v4());
    let body = (0..90)
        .map(|row| {
            if [5, 35, 65].contains(&row) {
                format!("ROW-{row:03} {token}\n")
            } else {
                format!("fixture filler row {row:03}\n")
            }
        })
        .collect::<String>();
    std::fs::write(tui.workspace.join("reading-fixture.txt"), &body).unwrap();
    tui.launch_default(&[]);
    tui.wait_for("Aster", 45);
    let facts = tui.turn("Use the read tool exactly once to read reading-fixture.txt. Reply only READING-FIRST-DONE. Do not echo any file contents, modify files or call other tools.", None);
    assert!(tui.assistant_text(&facts).contains("READING-FIRST-DONE"));
    assert!(!tui.assistant_text(&facts).contains(&token));
    assert_eq!(
        facts
            .iter()
            .filter(|fact| fact["type"] == "tool.prepared")
            .count(),
        1
    );
    let facts = tui.turn(
        "Reply only READING-SECOND-TURN. Do not call any tools.",
        None,
    );
    assert!(tui.assistant_text(&facts).contains("READING-SECOND-TURN"));
    let baseline = tui.facts();

    tui.command(&format!("/find {token}"));
    tui.wait_for("1/3", 8);
    key(&mut tui, b"\r");
    visible(&mut tui, "ROW-005");
    for (sequence, counter, row) in [
        (b"n".as_slice(), "2/3", "ROW-035"),
        (b"n".as_slice(), "3/3", "ROW-065"),
        (b"N".as_slice(), "2/3", "ROW-035"),
        (b"\x1b[A".as_slice(), "1/3", "ROW-005"),
        (b"\x1b[B".as_slice(), "2/3", "ROW-035"),
        (b"n".as_slice(), "3/3", "ROW-065"),
        (b"n".as_slice(), "1/3", "ROW-005"),
    ] {
        key(&mut tui, sequence);
        tui.wait_for(counter, 8);
        visible(&mut tui, row);
    }
    tui.record("find-positions", "all three independently placed tool-output matches become visible in order, reverse order and wraparound");
    tui.dismiss();
    tui.command("/find NAVIGATION-NO-SUCH-MATCH");
    tui.wait_for("no matches", 8);
    tui.dismiss();

    tui.command("/jump");
    tui.wait_for("Jump to which turn?", 8);
    key(&mut tui, b"\x1b[A");
    key(&mut tui, b"\r");
    visible(&mut tui, "Use the read tool exactly once");
    assert!(!tui.physical_screen().contains("READING-SECOND-TURN"));
    tui.record("jump-first", "selecting the first actual turn moves the viewport to its prompt, away from the second turn");
    tui.command("/jump");
    tui.wait_for("Jump to which turn?", 8);
    key(&mut tui, b"\x1b[B");
    key(&mut tui, b"\x1b");
    visible(&mut tui, "Use the read tool exactly once");
    assert!(!tui.physical_screen().contains("READING-SECOND-TURN"));
    tui.record(
        "jump-cancel",
        "previewing the other turn and cancelling restores the original first-turn viewport",
    );

    tui.command("/vim-mode");
    tui.wait_preference("vim_mode = true");
    tui.dismiss();
    key(&mut tui, b"\t");
    key(&mut tui, b"g");
    visible(&mut tui, "Use the read tool exactly once");
    key(&mut tui, b"G");
    visible(&mut tui, "READING-SECOND-TURN");
    for (rows, cols) in [(16, 58), (26, 90), (40, 120)] {
        tui.resize(rows, cols);
        key(&mut tui, b"G");
        visible(&mut tui, "READING-SECOND-TURN");
    }
    tui.record("vim-and-resize", "Vim top/bottom navigation changes actual transcript positions and remains reachable across three PTY sizes");
    assert_eq!(
        tui.facts(),
        baseline,
        "local navigation unexpectedly reached the backend"
    );
    assert_eq!(
        std::fs::read_to_string(tui.workspace.join("reading-fixture.txt")).unwrap(),
        body
    );
    tui.quit();
    tui.finish();
}
