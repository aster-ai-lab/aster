#![cfg(test)]

use super::driver::LiveTui;
use super::tool_wave::{finish, start};
use std::time::{Duration, Instant};

#[test]
#[ignore = "requires the frozen native pair and original kimi/k3.1-pro ACL"]
fn kimi_voice_off_explicit_profile_hides_all_backend_and_voice_rows() {
    let case = "coverage-settings-voice-off";
    let mut tui = LiveTui::new(case);
    tui.set_env("GROK_VOICE_MODE", "0");
    tui.launch(&[]);
    tui.wait_for("Aster", 45);
    tui.session_id();
    tui.dismiss();
    let facts = tui.facts();
    let preferences = tui.preferences();
    tui.command("/settings");
    key(&mut tui, b"/");
    for setting in [
        "coding_data_sharing",
        "dashboard_preview",
        "combine_queued_prompts",
        "follow_up_behavior",
        "plan_mode",
        "contextual_hints.plan_mode",
        "toolset.ask_user_question.timeout_enabled",
        "fork_secondary_model",
        "prompt_suggestions",
        "auto_update",
        "remember_tool_approvals",
        "default_selected_permission",
        "default_model",
        "subagent_model_inheritance",
        "hunk_tracker_mode",
        "show_tips",
        "confirm_before_rewind",
        "contextual_hints.image_input",
        "voice_keybind_enabled",
        "voice_capture_mode",
        "voice_stt_language",
    ] {
        key(&mut tui, b"\x15");
        key(&mut tui, setting.as_bytes());
        visible(&mut tui, &format!("No matches for \"{setting}\""));
        tui.record(&format!("hidden-{setting}"), "exact setting key has no rendered actionable row with the explicit public GROK_VOICE_MODE=0 pin");
    }
    for (setting, label) in [
        ("vim_mode", "Vim scrollback navigation"),
        ("permission_mode", "Permission mode"),
    ] {
        key(&mut tui, b"\x15");
        key(&mut tui, setting.as_bytes());
        visible(&mut tui, label);
        assert!(!tui.physical_screen().contains("No matches for"));
    }
    key(&mut tui, b"\x1bOQ");
    assert_eq!(tui.preferences(), preferences);
    assert_eq!(tui.facts(), facts);
    tui.record("explicit-voice-off-hidden-settings", "all 18 backend-inapplicable rows and all three voice-gated rows are absent; two supported controls remain, and no preference or backend state changes");
    finish(tui, case);
}

#[test]
#[ignore = "requires the frozen native pair and original kimi/k3.1-pro ACL"]
fn kimi_theme_preview_cancel_commit_reset_compare_actual_background() {
    let case = "coverage-theme-background";
    let mut tui = start(case, &[]);
    tui.command("/theme groknight");
    tui.wait_preference("theme = \"groknight\"");
    tui.pump(6500);
    let facts = tui.facts();
    let night = tui.background();
    let preferences = tui.preferences();
    theme_picker(&mut tui);
    key(&mut tui, b"\x1b[B");
    assert_ne!(
        tui.background(),
        night,
        "Day preview did not render its background"
    );
    assert_eq!(tui.preferences(), preferences);
    key(&mut tui, b"\x1b");
    key(&mut tui, b"\x1bOQ");
    tui.pump(6500);
    assert_eq!(tui.background(), night);
    assert_eq!(tui.preferences(), preferences);
    tui.record("theme-background-preview-cancel", "Day changes the actual terminal background during preview, persists nothing, and Escape restores the Night background and original preferences");
    theme_picker(&mut tui);
    key(&mut tui, b"\x1b[B");
    key(&mut tui, b"\r");
    key(&mut tui, b"\x1bOQ");
    tui.wait_preference("theme = \"grokday\"");
    tui.pump(6500);
    let day = tui.background();
    assert_ne!(day, night);
    tui.record(
        "theme-background-commit",
        "Enter persists canonical grokday and the idle terminal background differs from Night",
    );
    theme_row(&mut tui);
    key(&mut tui, b"d");
    visible(&mut tui, "Reset 'Theme' to default");
    key(&mut tui, b"n");
    key(&mut tui, b"\x1bOQ");
    tui.pump(6500);
    assert_eq!(tui.background(), day);
    assert!(tui.preferences().contains("theme = \"grokday\""));
    theme_row(&mut tui);
    key(&mut tui, b"d");
    visible(&mut tui, "Reset 'Theme' to default");
    key(&mut tui, b"y");
    key(&mut tui, b"\x1bOQ");
    tui.pump(6500);
    assert_eq!(tui.background(), night);
    assert!(!tui.preferences().contains("theme = \"grokday\""));
    assert_eq!(tui.facts(), facts);
    tui.record("theme-background-reset", "No preserves Day; Yes restores default Night in the real renderer without creating backend turns");
    finish(tui, case);
}

#[test]
#[ignore = "requires the frozen native pair and original kimi/k3.1-pro ACL"]
fn kimi_markdown_last_match_selects_actual_answer_across_reasoning_variation() {
    let case = "coverage-markdown-last-match";
    let mut tui = start(case, &[]);
    let token = format!("RAW-{}", uuid::Uuid::new_v4());
    let markdown = format!("# Viewer heading\n\n**{token}**\n\n- alpha\n- beta");
    let prompt = format!("Reply with exactly the following Markdown without a code fence or commentary. Do not call tools.\n{markdown}");
    let facts = tui.turn(&prompt, None);
    let answer = tui.assistant_text(&facts);
    assert_eq!(answer.trim(), markdown);
    let baseline = tui.facts();
    tui.command(&format!("/find {token}"));
    visible(&mut tui, &format!("search: {token}"));
    key(&mut tui, b"\r");
    key(&mut tui, b"N");
    key(&mut tui, b"\x1b");
    key(&mut tui, b"\x06");
    visible(&mut tui, "raw");
    visible(&mut tui, &token);
    assert!(!tui.physical_screen().contains(&format!("**{token}**")));
    key(&mut tui, b"r");
    visible(&mut tui, &format!("**{token}**"));
    tui.record("markdown-last-match-raw", "the last real search match selects the terminal assistant answer even when Thinking adds a middle match; the actual Markdown viewer toggles into literal source");
    key(&mut tui, b"r");
    assert!(!tui.physical_screen().contains(&format!("**{token}**")));
    key(&mut tui, b"\x06");
    let path = tui.workspace.join("raw-export.txt");
    tui.literal_command(&format!("/copy {}", path.display()));
    let deadline = Instant::now() + Duration::from_secs(8);
    while !path.is_file() {
        tui.pump(80);
        assert!(Instant::now() < deadline, "missing assistant export");
    }
    assert_eq!(std::fs::read(path).unwrap(), answer.as_bytes());
    assert_eq!(tui.facts(), baseline);
    tui.record("markdown-last-match-exact-export", "formatting and raw toggles leave canonical assistant UTF-8 export bytes and backend facts unchanged");
    finish(tui, case);
}

fn theme_row(tui: &mut LiveTui) {
    tui.command("/settings");
    key(tui, b"/");
    key(tui, b"theme");
    key(tui, b"\r");
    visible(tui, "Theme");
}

fn theme_picker(tui: &mut LiveTui) {
    theme_row(tui);
    key(tui, b"\r");
    visible(tui, "revert");
    visible(tui, "select");
}

fn key(tui: &mut LiveTui, bytes: &[u8]) {
    tui.bytes(bytes);
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
        assert!(Instant::now() < deadline, "expected {needle:?}\n{screen}");
    }
}
