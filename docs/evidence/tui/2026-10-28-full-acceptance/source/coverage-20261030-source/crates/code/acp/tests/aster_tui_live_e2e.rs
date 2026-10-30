//! Real-provider acceptance through the built Aster TUI and its sibling ACP.
//!
//! A3S_CONFIG_FILE names the operator-owned ACL; it is never copied or edited.
//! Run explicitly with --ignored --test-threads=1 --nocapture.

#![cfg(test)]

#[path = "aster_tui_live_e2e/driver.rs"]
mod driver;

#[path = "aster_tui_live_e2e/surface.rs"]
mod surface;

#[path = "aster_tui_live_e2e/workflows.rs"]
mod workflows;

#[path = "aster_tui_live_e2e/forms.rs"]
mod forms;

#[path = "aster_tui_live_e2e/questions_and_forms.rs"]
mod questions_and_forms;

#[path = "aster_tui_live_e2e/forms_catalog.rs"]
mod forms_catalog;

#[path = "aster_tui_live_e2e/functional.rs"]
mod functional;

#[path = "aster_tui_live_e2e/lifecycle.rs"]
mod lifecycle;

#[path = "aster_tui_live_e2e/context_and_input.rs"]
mod context_and_input;

#[path = "aster_tui_live_e2e/input_controls.rs"]
mod input_controls;

#[path = "aster_tui_live_e2e/reading_controls.rs"]
mod reading_controls;

#[path = "aster_tui_live_e2e/stream_controls.rs"]
mod stream_controls;

#[path = "aster_tui_live_e2e/tool_contracts.rs"]
mod tool_contracts;

#[path = "aster_tui_live_e2e/tool_wave.rs"]
mod tool_wave;

#[path = "aster_tui_live_e2e/coverage_wave.rs"]
mod coverage_wave;

#[path = "aster_tui_live_e2e/workspace_trust.rs"]
mod workspace_trust;

#[path = "aster_tui_live_e2e/file_references.rs"]
mod file_references;

use driver::LiveTui;

#[test]
#[ignore = "requires built Aster/ACP and the explicitly selected live kimi/k3.1-pro ACL"]
fn kimi_tui_offered_commands_have_observable_effects() {
    surface::tour();
}

#[test]
#[ignore = "requires built Aster/ACP and the explicitly selected live kimi/k3.1-pro ACL"]
fn kimi_tui_permission_reject_and_cancel_leave_no_side_effect() {
    workflows::permissions();
}

#[test]
#[ignore = "requires built Aster/ACP and the explicitly selected live kimi/k3.1-pro ACL"]
fn kimi_tui_writes_and_verifies_actual_workspace_bytes() {
    workflows::write_and_verify(true);
}

#[test]
#[ignore = "requires built Aster/ACP and the explicitly selected live kimi/k3.1-pro ACL"]
fn kimi_tui_allow_once_writes_and_verifies_actual_workspace_bytes() {
    workflows::write_and_verify(false);
}

#[test]
#[cfg(any(windows, unix))]
#[ignore = "requires built Aster/ACP and the explicitly selected live kimi/k3.1-pro ACL"]
fn kimi_tui_interrupts_a_running_tool_without_late_side_effects() {
    workflows::interrupt();
}

#[test]
#[ignore = "requires built Aster/ACP and the explicitly selected live kimi/k3.1-pro ACL"]
fn kimi_tui_durable_goal_controls_persist_real_facts() {
    workflows::goals();
}

#[test]
#[ignore = "requires built Aster/ACP and the explicitly selected live kimi/k3.1-pro ACL"]
fn kimi_tui_native_form_validates_submits_and_cancels() {
    forms::run();
}

#[test]
#[ignore = "requires built Aster/ACP and the explicitly selected live kimi/k3.1-pro ACL"]
fn kimi_tui_expands_actual_folded_tool_output() {
    workflows::expand();
}

#[test]
#[cfg(any(windows, unix))]
#[ignore = "requires built Aster/ACP and the explicitly selected live kimi/k3.1-pro ACL"]
fn kimi_tui_multiturn_edits_and_runs_the_real_checker() {
    workflows::coding();
}

#[test]
#[ignore = "requires built Aster/ACP and the explicitly selected live kimi/k3.1-pro ACL"]
fn kimi_tui_reads_actual_workspace_bytes() {
    let mut tui = LiveTui::new("read");
    let token = format!("KIMI-READ-{}", uuid::Uuid::new_v4());
    std::fs::write(tui.workspace.join("read-proof.txt"), &token).unwrap();
    tui.launch(&[]);
    tui.wait_for("Aster", 45);
    tui.command("/session-info");
    let info = tui.wait_for("Session ID", 30);
    assert!(info.contains("kimi/k3.1-pro"), "ACL model was not selected");
    tui.dismiss();
    let facts = tui.turn(
        "Use the read tool to read read-proof.txt. Reply with its exact contents. Do not modify any file or call other tools.",
        None,
    );
    assert!(tui.assistant_text(&facts).contains(&token));
    assert!(facts
        .iter()
        .any(|f| f["type"] == "tool.prepared" && f["payload"]["prepared"]["tool_name"] == "read"));
    assert_eq!(
        std::fs::read_to_string(tui.workspace.join("read-proof.txt")).unwrap(),
        token
    );
    tui.record(
        "live-read",
        "actual read receipt and unseen file token in settled assistant reply",
    );
    tui.quit();
    tui.finish();
}
