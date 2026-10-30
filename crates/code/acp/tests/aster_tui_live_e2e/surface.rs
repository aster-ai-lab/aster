#![cfg(test)]

use super::driver::LiveTui;
use std::time::{Duration, Instant};

const OFFERED: &[(&str, &str)] = &[
    ("settings", "Open the settings modal"),
    ("config", "Add and configure providers and models"),
    ("new", "Start a new session"),
    ("effort", "Set reasoning effort"),
    ("goal", "durable goal"),
    ("compact", "Compact conversation history"),
    ("model", "Switch the active model"),
    ("context", "View context usage"),
    ("jump", "Jump to a turn"),
    ("edit-prompt", "external editor"),
    ("session-info", "Show session info"),
    ("export", "Export the current conversation"),
    ("copy", "Copy last response"),
    ("find", "Search the conversation scrollback"),
    ("theme", "Switch the color theme"),
    ("yolo", "skip all permission prompts"),
    ("vim-mode", "vim-style scrollback"),
    ("multiline", "multiline input mode"),
    ("compact-mode", "less padding, more content"),
    ("timestamps", "message timestamps"),
    ("toggle-mouse-reporting", "terminal mouse reporting"),
    ("minimal", "scrollback-native"),
    ("timeline", "timeline sidebar"),
    ("doctor", "show available fixes"),
    ("home", "welcome screen"),
    ("delete", "Delete this session"),
    ("help", "Browse commands and keyboard shortcuts"),
    ("quit", "Quit the application"),
];

pub(super) fn tour() {
    let mut tui = LiveTui::new("surface");
    std::fs::write(
        tui.home.join("grok/config.toml"),
        "[ui]\nmouse_reporting_toggle = true\n",
    )
    .unwrap();
    let editor = tui.workspace.join(if cfg!(windows) {
        "editor.ps1"
    } else {
        "editor.sh"
    });
    if cfg!(windows) {
        std::fs::write(&editor, "[IO.File]::WriteAllText((Join-Path $PSScriptRoot 'editor-ran.txt'), 'edited'); [IO.File]::WriteAllText($args[0], 'EDITOR-DRAFT')").unwrap();
        let shell = std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        tui.editor = Some(format!(
            "\"{}\" -NoProfile -ExecutionPolicy Bypass -File \"{}\"",
            shell.display().to_string().replace('\\', "/"),
            editor.display().to_string().replace('\\', "/")
        ));
    } else {
        std::fs::write(
            &editor,
            "printf edited > \"$(dirname \"$0\")/editor-ran.txt\"\nprintf EDITOR-DRAFT > \"$1\"\n",
        )
        .unwrap();
        tui.editor = Some(format!("/bin/sh '{}'", editor.display()));
    }
    tui.launch(&[]);
    tui.wait_for("Aster", 45);
    for (rows, cols) in [(20, 60), (24, 80), (40, 120)] {
        tui.resize(rows, cols);
        tui.wait_for("Aster", 8);
        tui.record(
            &format!("welcome-{cols}x{rows}"),
            "visible Aster welcome after actual PTY resize",
        );
    }
    let original_id = tui.session_id();
    tui.dismiss();
    for (name, description) in OFFERED {
        tui.bytes(format!("/{name}").as_bytes());
        let screen = tui.pump(400);
        assert!(
            screen.lines().any(|row| row.contains(description)),
            "offered command /{name} has no palette row\n{screen}"
        );
        tui.record(&format!("palette-{name}"), description);
        tui.dismiss();
    }
    for (command, needle) in [
        ("/doctor", "terminal"),
        ("/goal status", "No durable goal is set."),
        ("/compact", "Nothing to compact yet."),
        ("/help", "Commands"),
        ("/settings", "Space toggle"),
        ("/config", "Add provider"),
    ] {
        let before = tui
            .facts()
            .iter()
            .filter(|f| f["type"] == "model.turn")
            .count();
        tui.command(command);
        tui.wait_for(needle, 15);
        assert_eq!(
            tui.facts()
                .iter()
                .filter(|f| f["type"] == "model.turn")
                .count(),
            before,
            "local UI command unexpectedly called the model"
        );
        tui.record(command, "actual command result; no model turn");
        tui.dismiss();
    }

    let token = format!("TUI-TRANSCRIPT-{}", uuid::Uuid::new_v4());
    std::fs::write(tui.workspace.join("transcript.txt"), &token).unwrap();
    let facts = tui.turn("Read transcript.txt with the read tool and reply with its exact contents. Do not modify files.", None);
    assert!(tui.assistant_text(&facts).contains(&token));
    tui.record(
        "live-transcript",
        "unseen file content in actual Kimi reply",
    );

    let export = tui.workspace.join("exported.md");
    tui.literal_command(&format!("/export {}", export.display()));
    let deadline = Instant::now() + Duration::from_secs(10);
    while !export.is_file() {
        tui.pump(80);
        assert!(
            Instant::now() < deadline,
            "export file absent\n{}",
            tui.screen()
        );
    }
    assert!(std::fs::read_to_string(&export).unwrap().contains(&token));
    tui.record("export", "actual transcript file contains Kimi reply");
    let copy = tui.workspace.join("copied.txt");
    tui.literal_command(&format!("/copy {}", copy.display()));
    let deadline = Instant::now() + Duration::from_secs(10);
    while !copy.is_file() {
        tui.pump(80);
        assert!(Instant::now() < deadline, "copy file absent");
    }
    assert!(std::fs::read_to_string(copy).unwrap().contains(&token));
    tui.record("copy", "actual copied reply bytes");

    assert_eq!(tui.session_id(), original_id);
    tui.record("session-info", "same persisted session after a live turn");
    tui.dismiss();
    tui.command("/context");
    let context = tui.wait_for("System prompt", 15);
    assert!(context.contains("Auto-compact"));
    tui.record("context", "actual context report after a live turn");
    tui.dismiss();

    tui.command(&format!("/find {token}"));
    let found = tui.wait_for("search:", 10);
    assert!(found.contains(&token));
    tui.record("find", "search locates actual assistant transcript token");
    tui.dismiss();
    let facts = tui.turn("Reply with one short sentence. Do not call tools.", None);
    assert!(!tui.assistant_text(&facts).is_empty());
    tui.command("/jump");
    tui.wait_for("Jump to which turn?", 10);
    tui.bytes(b"\r");
    tui.pump(400);
    tui.record("jump", "turn picker opened and selected a turn");
    tui.dismiss();

    for (command, preference) in [
        ("/theme grokday", "grokday"),
        ("/compact-mode", "compact_mode = true"),
        ("/timestamps", "show_timestamps = false"),
        ("/timeline", "show_timeline = true"),
        ("/vim-mode", "vim_mode = true"),
    ] {
        tui.command(command);
        tui.wait_preference(preference);
        tui.record(command, preference);
    }
    tui.command("/vim-mode");
    tui.wait_preference("vim_mode = false");
    tui.command("/multiline");
    tui.wait_for("Multiline:", 8);
    tui.record("multiline-on", "session toggle shown");
    tui.command("/multiline");
    tui.pump(500);
    tui.record("multiline-off", "toggle returned to ordinary input");

    tui.command("/model");
    tui.wait_for("K3.1 Pro", 10);
    tui.dismiss();
    // Change selection without submitting inference to another provider.
    tui.literal_command("/model gateway/prism-27b");
    tui.pump(600);
    tui.command("/session-info");
    let switched = tui.wait_for("Session ID", 15);
    assert!(switched.contains("gateway/prism-27b"));
    tui.dismiss();
    tui.literal_command("/model kimi/k3.1-pro");
    tui.pump(600);
    tui.command("/session-info");
    let restored = tui.wait_for("Session ID", 15);
    assert!(restored.contains("kimi/k3.1-pro"));
    tui.dismiss();
    tui.record(
        "model",
        "catalog selection changed and restored to Kimi before further inference",
    );
    tui.command("/effort low");
    tui.wait_for("(low)", 10);
    tui.record(
        "effort",
        "reasoning effort actually changed to low in model footer",
    );
    tui.command("/yolo");
    tui.wait_for("yolo", 8);
    tui.record("yolo", "permission posture flag enabled");
    tui.command("/yolo");
    tui.pump(400);

    let mouse_before = tui.mouse_modes();
    tui.command("/toggle-mouse-reporting");
    let mouse_screen = tui.pump(600);
    if cfg!(windows) {
        assert!(mouse_screen.contains("to enable mouse reporting and restore TUI features"));
    } else {
        assert!(
            tui.mouse_modes() > mouse_before,
            "mouse capture terminal modes did not change"
        );
    }
    tui.record(
        "toggle-mouse-reporting",
        "actual terminal mouse capture mode changed",
    );
    tui.command("/toggle-mouse-reporting");
    let restored = tui.pump(600);
    assert!(!restored.contains("to enable mouse reporting and restore TUI features"));
    tui.command("/edit-prompt");
    let deadline = Instant::now() + Duration::from_secs(15);
    while !tui.workspace.join("editor-ran.txt").exists() {
        tui.pump(80);
        assert!(
            Instant::now() < deadline,
            "external editor did not run\n{}",
            tui.screen()
        );
    }
    tui.wait_for("EDITOR-DRAFT", 8);
    tui.bytes(b"\x15");
    tui.record(
        "edit-prompt",
        "real editor process ran and returned edited composer bytes",
    );

    let before = tui.facts().len();
    tui.command("/compact");
    tui.wait_fact("compaction.done", before);
    tui.record(
        "compact",
        "actual model compaction appended compaction.done",
    );
    tui.dismiss();
    tui.quit();
    tui.launch(&["--continue"]);
    tui.wait_for("kimi/", 45);
    assert_eq!(tui.session_id(), original_id);
    tui.record("continue", "same source-built ACP session reconnected");
    tui.dismiss();
    tui.quit();
    tui.launch(&["--resume", &original_id]);
    tui.wait_for("kimi/", 45);
    assert_eq!(tui.session_id(), original_id);
    tui.record("resume", "explicit session identity restored");
    tui.dismiss();

    tui.command("/home");
    tui.wait_for("Resume session", 10);
    tui.record("home", "returned to welcome screen");
    tui.command("/new");
    tui.pump(1000);
    let new_id = tui.session_id();
    assert_ne!(new_id, original_id);
    tui.record("new", "new session identity");
    tui.dismiss();
    tui.command("/minimal");
    tui.wait_for("Switched to minimal mode", 10);
    tui.record("minimal", "switched renderer");
    tui.command("/expand");
    tui.wait_for("Nothing to expand", 10);
    tui.record("expand-empty", "actual empty result in minimal mode");
    tui.command("/fullscreen");
    tui.wait_for("Switched to fullscreen mode", 10);
    tui.record("fullscreen", "returned to fullscreen renderer");
    let deadline = Instant::now() + Duration::from_secs(10);
    let summary = loop {
        if let Some(path) = tui.files(&tui.home, "json").into_iter().find(|p| {
            p.file_name().and_then(|v| v.to_str()) == Some("summary.json")
                && p.parent()
                    .and_then(|v| v.file_name())
                    .and_then(|v| v.to_str())
                    == Some(new_id.as_str())
        }) {
            break path;
        }
        tui.pump(80);
        assert!(
            Instant::now() < deadline,
            "new session summary was not persisted"
        );
    };
    tui.command("/delete");
    tui.wait_for("Delete this session permanently?", 10);
    tui.pump(250);
    tui.bytes(b"1");
    let deadline = Instant::now() + Duration::from_secs(15);
    while summary.exists() {
        tui.pump(80);
        assert!(
            Instant::now() < deadline,
            "confirmed delete left the persisted summary\n{}",
            tui.screen()
        );
    }
    tui.pump(250);
    tui.wait_for("Resume session", 10);
    assert!(!summary.exists());
    tui.record(
        "delete",
        "persisted session summary removed and welcome screen restored",
    );
    tui.quit();
    tui.finish();
}
