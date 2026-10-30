#![cfg(test)]

use super::driver::LiveTui;

#[test]
#[ignore = "requires the paired source build and original kimi/k3.1-pro ACL"]
fn kimi_file_completion_selects_real_file_and_line_range() {
    let mut tui = LiveTui::new("file-ref-selection");
    std::fs::write(
        tui.workspace.join("ref-target.txt"),
        b"FIRST\nSECOND\nTHIRD\n",
    )
    .unwrap();
    tui.launch(&[]);
    tui.wait_for("Aster", 45);
    select_file(&mut tui, "ref-target", "ref-target.txt");
    tui.bytes(b"\x0c");
    tui.wait_for("FIRST", 15);
    tui.wait_for("SECOND", 15);
    tui.pump(300);
    tui.bytes(b"v");
    tui.pump(300);
    tui.bytes(b"\x1b[B");
    tui.pump(300);
    tui.bytes(b"\r");
    tui.wait_for("@ref-target.txt:1-2", 15);
    assert_no_model_work(&tui);
    tui.record(
        "file-ref-selection",
        "the real completion opens the selected file and confirms exact lines 1-2",
    );
    tui.quit();
    tui.finish();
}

#[test]
#[ignore = "requires the paired source build and original kimi/k3.1-pro ACL"]
fn kimi_missing_file_reference_is_refused_without_model_work() {
    let mut tui = LiveTui::new("file-ref-missing");
    std::fs::write(tui.workspace.join("ref-target.txt"), b"REFERENCE\n").unwrap();
    tui.launch(&[]);
    tui.wait_for("Aster", 45);
    select_file(&mut tui, "ref-target", "ref-target.txt");
    std::fs::remove_file(tui.workspace.join("ref-target.txt")).unwrap();
    tui.bytes(b"\x0c");
    tui.wait_for("File not found or unreadable", 5);
    assert_no_model_work(&tui);
    tui.record(
        "file-ref-missing",
        "a selected reference whose file disappeared is explicitly refused without model work",
    );
    tui.dismiss();
    tui.quit();
    tui.finish();
}

#[test]
#[ignore = "requires the paired source build and original kimi/k3.1-pro ACL"]
fn kimi_outside_file_reference_is_refused_without_model_work() {
    let mut tui = LiveTui::new("file-ref-outside");
    let linked = tui.workspace.join("linked");
    std::fs::create_dir(&linked).unwrap();
    std::fs::write(linked.join("ref-target.txt"), b"REFERENCE\n").unwrap();
    let outside = tui.workspace.parent().unwrap().join("outside-directory");
    std::fs::create_dir(&outside).unwrap();
    let secret = format!("OUTSIDE-{}", uuid::Uuid::new_v4());
    std::fs::write(outside.join("ref-target.txt"), &secret).unwrap();
    tui.launch(&[]);
    tui.wait_for("Aster", 45);
    select_file(&mut tui, "ref-target", "ref-target.txt");
    replace_directory_with_link(&linked, &outside);
    tui.bytes(b"\x0c");
    let screen = tui.wait_for("File reference is outside the workspace", 5);
    assert!(!screen.contains(&secret), "{screen}");
    assert_no_model_work(&tui);
    tui.record(
        "file-ref-outside",
        "a selected reference redirected outside the workspace is explicitly refused without exposing bytes",
    );
    tui.dismiss();
    tui.quit();
    tui.finish();
}

fn select_file(tui: &mut LiveTui, query: &str, completed: &str) {
    let _ = tui.session_id();
    tui.dismiss();
    tui.bytes(format!("@{query}").as_bytes());
    tui.wait_for(completed, 15);
    tui.record("file-ref-completion", "the actual fuzzy result is visible");
    tui.bytes(b"\t");
    tui.pump(300);
    tui.bytes(b"\x1b[D");
}

fn assert_no_model_work(tui: &LiveTui) {
    let work = tui
        .facts()
        .iter()
        .filter(|fact| {
            matches!(
                fact["type"].as_str(),
                Some("user.message" | "model.turn" | "turn.complete" | "tool.prepared")
            )
        })
        .count();
    assert_eq!(
        work, 0,
        "local file reference interaction started model work"
    );
}

fn replace_directory_with_link(link: &std::path::Path, outside: &std::path::Path) {
    std::fs::remove_dir_all(link).unwrap();
    #[cfg(windows)]
    {
        let status = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(link)
            .arg(outside)
            .status()
            .unwrap();
        assert!(status.success(), "failed to create an isolated junction");
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(outside, link).unwrap();
}
