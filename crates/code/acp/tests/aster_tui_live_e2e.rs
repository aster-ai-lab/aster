//! Real-provider acceptance through the built Aster TUI and its sibling ACP.
//!
//! ASTER_CONFIG_FILE names the operator-owned ACL; it is never copied or edited.
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

#[path = "aster_tui_live_e2e/ui_qualified_wave.rs"]
mod ui_qualified_wave;

#[path = "aster_tui_live_e2e/network_tools_wave.rs"]
mod network_tools_wave;

#[path = "aster_tui_live_e2e/workspace_trust.rs"]
mod workspace_trust;

#[path = "aster_tui_live_e2e/file_references.rs"]
mod file_references;

use driver::LiveTui;

#[test]
#[ignore = "requires the built Aster/ACP pair and operator kimi-01/k3.1-pro"]
fn kimi_01_tui_compacts_and_plans_then_reopens_the_same_goal() {
    let model = "kimi-01/k3.1-pro";
    let mut tui = LiveTui::new_with_model("kimi-01-operation-lifecycle", Some(model));
    tui.launch(&["--fullscreen", "--trust", "--model", model]);
    tui.wait_for("Aster", 45);
    tui.wait_for("kimi-01/", 45);
    let session = tui.session_id();
    tui.dismiss();
    let marker = format!("EMBER-{}", uuid::Uuid::new_v4());
    let first = tui.turn(
        &format!("Remember the synthetic marker {marker}. Repeat the complete marker verbatim. Do not use tools."),
        None,
    );
    assert!(tui.assistant_text(&first).contains(&marker));
    let before = tui.facts().len();
    tui.command("/compact");
    tui.wait_fact("compaction.done", before);
    tui.wait_for("Compaction completed", 30);
    let reply = tui.turn(
        "Repeat the full synthetic marker you were asked to remember. Do not use tools.",
        None,
    );
    assert!(tui.assistant_text(&reply).contains(&marker));

    let objective = "Ask the user for a verification token and wait. Do not call tools.";
    let before = tui.facts().len();
    tui.command(&format!("/goal {objective}"));
    let goal = tui.wait_fact("aster.goal.created", before);
    assert_eq!(goal["payload"]["body"]["goal"]["description"], objective);
    tui.wait_fact("aster.goal.plan.accepted", before);
    tui.wait_for("Goal", 90);
    tui.record(
        "goal-plan",
        "model proposal accepted for the exact objective",
    );
    tui.quit();

    tui.launch(&["--fullscreen", "--model", model, "--resume", &session]);
    tui.wait_for("kimi-01/", 45);
    assert_eq!(tui.session_id(), session);
    tui.dismiss();
    let before = tui.facts().len();
    tui.command("/goal pause");
    tui.wait_fact("aster.goal.paused", before);
    tui.record(
        "kimi-01",
        "paired TUI and ACP preserved recall across compaction and reopened the accepted Goal",
    );
    tui.quit();
    tui.finish();
}

#[test]
#[cfg(unix)]
#[ignore = "requires the built Aster/ACP pair and operator kimi-01/k3.1-pro"]
fn kimi_01_tui_read_only_replay_safe_and_sidecar_crash_self_heal() {
    use std::time::{Duration, Instant};

    let model = "kimi-01/k3.1-pro";
    let mut tui = LiveTui::new_with_model("kimi-01-crash-self-heal", Some(model));
    let pids_log = tui.wrap_sidecar_pids();
    let token = format!("KIMI01-SELFHEAL-{}", uuid::Uuid::new_v4());
    std::fs::write(tui.workspace.join("read-proof.txt"), &token).unwrap();

    tui.launch(&["--fullscreen", "--trust", "--model", model]);
    tui.wait_for("Aster", 45);
    tui.wait_for("kimi-01/", 45);
    let _session = tui.session_id();
    tui.dismiss();

    // Turn 1: real read tool prepares with ReplaySafe recovery policy.
    let facts1 = tui.turn(
        "Use the read tool once to read read-proof.txt and reply with its exact contents. Do not modify any file or call other tools.",
        None,
    );
    assert!(
        tui.assistant_text(&facts1).contains(&token),
        "Turn 1 reply missed file token: {facts1:?}"
    );
    assert!(
        facts1.iter().any(|f| {
            f["type"] == "tool.prepared"
                && f["payload"]["prepared"]["tool_name"] == "read"
                && f["payload"]["prepared"]["recovery"] == "replay_safe"
        }),
        "expected read tool.prepared to carry recovery=replay_safe: {facts1:?}"
    );
    assert!(!tui.screen().contains("Turn failed"));
    tui.record(
        "turn1-replay-safe",
        "read tool prepared with replay_safe and returned real file token",
    );

    // Turn 2: SIGKILL sidecar gen 1, verify respawn + session restore + history continuity.
    let first_pid = std::fs::read_to_string(&pids_log)
        .expect("wrapper pid log")
        .lines()
        .next()
        .expect("first sidecar pid")
        .trim()
        .to_string();
    let kill_status = std::process::Command::new("/bin/kill")
        .args(["-KILL", &first_pid])
        .status()
        .expect("invoke /bin/kill");
    assert!(
        kill_status.success(),
        "failed to SIGKILL sidecar pid {first_pid}"
    );

    let respawn_deadline = Instant::now() + Duration::from_secs(30);
    let mut second_pid = None;
    while Instant::now() < respawn_deadline {
        let screen = tui.pump(120);
        if let Ok(contents) = std::fs::read_to_string(&pids_log) {
            let pids: Vec<&str> = contents
                .lines()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect();
            if pids.len() >= 2 && pids[1] != first_pid {
                second_pid = Some(pids[1].to_string());
                if screen.contains("Session restored") {
                    break;
                }
            }
        }
    }
    let second_pid = second_pid.expect("expected supervised sidecar respawn (gen 2)");

    let facts2 = tui.turn(
        "Reply with the exact KIMI01-SELFHEAL token from read-proof.txt that you read earlier in this conversation. Do not call any tools.",
        None,
    );
    assert!(
        tui.assistant_text(&facts2).contains(&token),
        "Turn 2 after respawn missed remembered token: {facts2:?}"
    );
    assert!(!tui.screen().contains("Turn failed"));
    tui.record(
        "turn2-respawn-continuity",
        "TUI survived sidecar SIGKILL, respawned gen 2, and preserved memory",
    );

    // Turn 3: SIGKILL sidecar gen 2 mid-turn after user.message lands, verify auto-resume without duplicate prompt.
    let before_turn3 = tui.facts().len();
    let midturn_token = format!("KIMI01-MIDTURN-{}", uuid::Uuid::new_v4());
    let prompt3 = format!(
        "Write a concise 2-sentence explanation of why append-only logs aid crash recovery, and end your reply with the exact token {midturn_token}. Do not call any tools."
    );
    tui.command(&prompt3);

    let user_fact_deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let screen = tui.pump(40);
        assert!(
            !screen.contains("Turn failed"),
            "unexpected Turn failed before mid-turn kill\n{screen}"
        );
        let facts = tui.facts();
        let delta = facts.get(before_turn3..).unwrap_or_default();
        if delta.iter().any(|f| f["type"] == "user.message") {
            assert!(
                !delta.iter().any(|f| f["type"] == "model.turn"),
                "Turn 3 model.turn completed before mid-turn SIGKILL could fire"
            );
            break;
        }
        assert!(
            Instant::now() < user_fact_deadline,
            "timed out waiting for Turn 3 user.message fact\n{screen}"
        );
    }

    let kill2_status = std::process::Command::new("/bin/kill")
        .args(["-KILL", &second_pid])
        .status()
        .expect("invoke /bin/kill for gen 2");
    assert!(
        kill2_status.success(),
        "failed to SIGKILL sidecar pid {second_pid}"
    );

    let gen3_deadline = Instant::now() + Duration::from_secs(30);
    let mut respawned_gen3 = false;
    while Instant::now() < gen3_deadline {
        let _ = tui.pump(100);
        if let Ok(contents) = std::fs::read_to_string(&pids_log) {
            let pids: Vec<&str> = contents
                .lines()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect();
            if pids.len() >= 3 && pids[2] != second_pid {
                respawned_gen3 = true;
                break;
            }
        }
    }
    assert!(
        respawned_gen3,
        "expected gen 3 sidecar respawn after mid-turn SIGKILL"
    );

    let complete_deadline = Instant::now() + Duration::from_secs(120);
    loop {
        let screen = tui.pump(80);
        assert!(
            !screen.contains("Turn failed"),
            "mid-turn crash auto-resume must suppress Turn failed\n{screen}"
        );
        let facts = tui.facts();
        let delta = facts.get(before_turn3..).unwrap_or_default();
        if delta.iter().any(|f| f["type"] == "turn.complete") {
            break;
        }
        assert!(
            Instant::now() < complete_deadline,
            "timed out waiting for Turn 3 auto-resumed completion\n{screen}"
        );
    }

    let idle_deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let screen = tui.pump(80);
        assert!(
            !screen.contains("Turn failed"),
            "Turn failed appeared while settling mid-turn auto-resume\n{screen}"
        );
        if !screen.contains("[stop]") && !screen.contains("Ctrl+c:cancel") {
            break;
        }
        assert!(
            Instant::now() < idle_deadline,
            "TUI did not return idle after Turn 3 auto-resume\n{screen}"
        );
    }

    let all_facts = tui.facts();
    let turn3_facts = all_facts.get(before_turn3..).unwrap_or_default();
    assert!(
        tui.assistant_text(turn3_facts).contains(&midturn_token),
        "Turn 3 auto-resumed reply missed {midturn_token}: {turn3_facts:?}"
    );
    assert_eq!(
        turn3_facts
            .iter()
            .filter(|f| f["type"] == "user.message")
            .count(),
        1,
        "mid-turn auto-resume must not duplicate user.message: {turn3_facts:?}"
    );
    tui.record(
        "turn3-midturn-sigkill-autoresume",
        "gen 2 mid-turn SIGKILL auto-respawned gen 3 and completed Turn 3 once without duplicating user.message",
    );

    tui.quit();
    tui.finish();
}

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
