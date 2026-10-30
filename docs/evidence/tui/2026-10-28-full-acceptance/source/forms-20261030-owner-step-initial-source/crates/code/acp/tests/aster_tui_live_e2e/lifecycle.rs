use super::driver::LiveTui;
use std::time::{Duration, Instant};

const READ_SESSION: &str = "Use the read tool once to read session-proof.txt and reply with its exact contents. Do not call other tools or modify any file.";
const RECALL_SESSION: &str = "Reply with the exact contents of session-proof.txt that you read earlier in this conversation. Do not call any tools.";
const CAT_APPROVAL: &str = "Use the bash tool exactly once to run cat approval-proof.txt. Reply with its exact output. Do not call other tools or modify any file.";

fn assert_one_bash(facts: &[serde_json::Value]) {
    let prepared: Vec<_> = facts
        .iter()
        .filter(|f| f["type"] == "tool.prepared")
        .collect();
    assert_eq!(prepared.len(), 1);
    assert_eq!(prepared[0]["payload"]["prepared"]["tool_name"], "bash");
}

#[test]
#[ignore = "requires the paired source build and original kimi/k3.1-pro ACL"]
fn kimi_resume_and_continue_restore_model_history() {
    let mut tui = LiveTui::new("resume-model-history");
    let token = format!("RESTORED-{}", uuid::Uuid::new_v4());
    std::fs::write(tui.workspace.join("session-proof.txt"), &token).unwrap();
    tui.launch_default(&[]);
    tui.wait_for("Aster", 45);
    let id = tui.session_id();
    tui.dismiss();
    let facts = tui.turn(READ_SESSION, None);
    assert!(tui.assistant_text(&facts).contains(&token));
    tui.checkpoint_session("original");
    tui.quit();
    for flag in ["--resume", "--continue"] {
        let args = if flag == "--resume" {
            vec![flag, id.as_str()]
        } else {
            vec![flag]
        };
        tui.launch_default(&args);
        tui.wait_for("kimi/", 45);
        assert_eq!(tui.session_id(), id);
        tui.dismiss();
        let facts = tui.turn(RECALL_SESSION, None);
        assert!(tui.assistant_text(&facts).contains(&token));
        assert!(
            !facts.iter().any(|f| f["type"] == "tool.prepared"),
            "restored recall must use model history, not another read"
        );
        tui.record(flag, "same parent identity; Kimi recalls unseen token from restored conversation without tools");
        tui.checkpoint_session(flag);
        tui.quit();
    }
    tui.finish();
}

fn create_history(tui: &mut LiveTui, token: &str) -> String {
    tui.launch_default(&[]);
    tui.wait_for("Aster", 45);
    let id = tui.session_id();
    tui.dismiss();
    let facts = tui.turn(
        &format!(
            "Remember this exact token for the rest of this session: {token}. Reply only SESSION-READY. Do not call tools."
        ),
        None,
    );
    assert!(tui.assistant_text(&facts).contains("SESSION-READY"));
    assert!(!facts.iter().any(|fact| fact["type"] == "tool.prepared"));
    tui.quit();
    id
}

fn continue_and_recall(tui: &mut LiveTui, expected_id: &str, token: &str, label: &str) {
    tui.launch_default(&["--continue"]);
    tui.wait_for("kimi/", 45);
    assert_eq!(tui.session_id(), expected_id);
    tui.dismiss();
    let facts = tui.turn(
        "Reply with the exact token I asked you to remember earlier in this session. Do not call tools.",
        None,
    );
    assert!(tui.assistant_text(&facts).contains(token));
    assert!(!facts.iter().any(|fact| fact["type"] == "tool.prepared"));
    tui.record(
        label,
        "the latest interactive session in this workspace restored its own identity and history without tools",
    );
    tui.checkpoint_session(label);
    tui.quit();
}

#[test]
#[ignore = "requires the paired source build and original kimi/k3.1-pro ACL"]
fn kimi_continue_selects_latest_per_workspace() {
    let mut tui = LiveTui::new("continue-latest-workspace");
    let original = tui.workspace.clone();
    let other = original.parent().unwrap().join("其他 工作区");
    std::fs::create_dir(&other).unwrap();
    let older = format!("WORKSPACE-A-OLDER-{}", uuid::Uuid::new_v4());
    tui.workspace = original.clone();
    let older_id = create_history(&mut tui, &older);
    let older_log = tui
        .files(&tui.workspace.join(".a3s"), "jsonl")
        .into_iter()
        .find(|path| path.file_stem().and_then(|name| name.to_str()) == Some(older_id.as_str()))
        .unwrap();
    let older_bytes = std::fs::read(older_log).unwrap();

    let other_token = format!("WORKSPACE-B-{}", uuid::Uuid::new_v4());
    tui.workspace = other.clone();
    let other_id = create_history(&mut tui, &other_token);
    let latest = format!("WORKSPACE-A-LATEST-{}", uuid::Uuid::new_v4());
    tui.workspace = original.clone();
    let latest_id = create_history(&mut tui, &latest);

    tui.workspace = original.clone();
    continue_and_recall(&mut tui, &latest_id, &latest, "continue-latest-workspace-a");
    tui.workspace = other;
    continue_and_recall(
        &mut tui,
        &other_id,
        &other_token,
        "continue-latest-workspace-b",
    );
    let older_log = tui
        .files(&original.join(".a3s"), "jsonl")
        .into_iter()
        .find(|path| path.file_stem().and_then(|name| name.to_str()) == Some(older_id.as_str()))
        .unwrap();
    assert_eq!(std::fs::read(older_log).unwrap(), older_bytes);
    tui.record(
        "continue-preserves-older-session",
        "the older same-workspace session was not selected or mutated",
    );
    tui.finish();
}

fn create_titled_history(tui: &mut LiveTui, token: &str) -> String {
    tui.launch_default(&[]);
    tui.wait_for("Aster", 45);
    let id = tui.session_id();
    tui.dismiss();
    let facts = tui.turn(
        &format!(
            "Duplicate title\nRemember this exact token for the rest of this session: {token}. Reply only SESSION-READY. Do not call tools."
        ),
        None,
    );
    assert!(tui.assistant_text(&facts).contains("SESSION-READY"));
    assert!(!facts.iter().any(|fact| fact["type"] == "tool.prepared"));
    tui.quit();
    id
}

fn filter_picker_by_id(tui: &mut LiveTui, id: &str) {
    tui.bytes(b"/");
    tui.pump(200);
    tui.bytes(format!("\x1b[200~{id}\x1b[201~").as_bytes());
    tui.wait_for(&format!(" search: {id}"), 8);
}

#[test]
#[ignore = "requires the paired source build and original kimi/k3.1-pro ACL"]
fn kimi_session_picker_native_paste_keeps_initial_shortcut_character() {
    let mut tui = LiveTui::new("session-picker-native-paste");
    let token = format!("PICKER-PASTE-{}", uuid::Uuid::new_v4());
    let id = create_titled_history(&mut tui, &token);
    let pasted = format!("e{}", &id[1..]);
    tui.launch_default(&[]);
    tui.wait_for("Aster", 45);
    for _ in 0..4 {
        tui.bytes(b"\x12");
        tui.wait_for("Duplicate title", 10);
        tui.bytes(format!("\x1b[200~{pasted}\x1b[201~").as_bytes());
        let query = tui.wait_for(&format!(" search: {pasted}"), 8);
        assert!(!query.contains("Messages"));
        tui.record(
            "native-paste-first-character",
            "the initial e remains query text instead of expanding the selected session card",
        );
        tui.bytes(b"\x1b");
        tui.pump(200);
        tui.bytes(b"\x1b");
        tui.wait_for("/ to search", 8);
        tui.bytes(b"\x1b");
        tui.wait_for("New worktree", 8);
    }
    tui.record(
        "native-paste-reopen",
        "four terminal picker pastes preserve an initial e without invoking the expand shortcut",
    );
    tui.quit();
    tui.finish();
}

#[test]
#[ignore = "requires the paired source build and original kimi/k3.1-pro ACL"]
fn kimi_session_picker_filters_selects_and_cancels_by_real_id() {
    let mut tui = LiveTui::new("session-picker-duplicate-title");
    let older_token = format!("DUPLICATE-OLDER-{}", uuid::Uuid::new_v4());
    let older_id = create_titled_history(&mut tui, &older_token);
    let newer_token = format!("DUPLICATE-NEWER-{}", uuid::Uuid::new_v4());
    let newer_id = create_titled_history(&mut tui, &newer_token);
    assert_ne!(older_id, newer_id);
    let newer_log = tui
        .files(&tui.workspace.join(".a3s"), "jsonl")
        .into_iter()
        .find(|path| path.file_stem().and_then(|name| name.to_str()) == Some(newer_id.as_str()))
        .unwrap();
    let newer_bytes = std::fs::read(&newer_log).unwrap();

    tui.launch_default(&["--resume", &newer_id]);
    tui.wait_for("kimi/", 45);
    assert_eq!(tui.session_id(), newer_id);
    tui.dismiss();
    tui.command("/home");
    tui.wait_for("Resume session", 10);
    tui.bytes(b"\x12");
    let picker = tui.wait_for("Duplicate title", 10);
    assert!(picker.matches("Duplicate title").count() >= 2);
    filter_picker_by_id(&mut tui, &older_id);
    tui.record(
        "picker-filter-by-id",
        "the exact older UUID filters the duplicate-title picker without loading another session",
    );
    tui.bytes(b"\x1b");
    let search_closed = tui.pump(300);
    assert!(search_closed.contains("Resume session"));
    assert!(search_closed.contains(&older_id));
    assert!(!search_closed.contains("New worktree"));
    tui.record(
        "picker-esc-leaves-search",
        "the first Esc leaves the focused search without dropping the UUID query or closing the picker",
    );
    tui.bytes(b"\x1b");
    let unfiltered = tui.wait_for("/ to search", 8);
    assert!(unfiltered.contains("Resume session"));
    assert!(!unfiltered.contains("New worktree"));
    assert_eq!(unfiltered.matches("Duplicate title").count(), 2);
    tui.record(
        "picker-esc-clears-filter",
        "Esc from navigation clears the UUID query and restores both duplicate-title rows without closing the picker",
    );
    tui.bytes(b"\x1b");
    tui.wait_for("New worktree", 8);
    assert_eq!(tui.files(&tui.workspace.join(".a3s"), "jsonl").len(), 2);
    assert_eq!(std::fs::read(&newer_log).unwrap(), newer_bytes);
    tui.record(
        "picker-cancel-preserves-history",
        "cancelling the filtered picker creates no session fact log and leaves the newer duplicate-title history byte-identical",
    );

    tui.bytes(b"\x12");
    tui.wait_for("Duplicate title", 10);
    filter_picker_by_id(&mut tui, &older_id);
    tui.bytes(b"\r");
    tui.wait_for("kimi/", 45);
    assert_eq!(tui.session_id(), older_id);
    tui.dismiss();
    let facts = tui.turn(
        "Reply with the exact token I asked you to remember earlier in this session. Do not call tools.",
        None,
    );
    assert!(tui.assistant_text(&facts).contains(&older_token));
    assert!(!facts.iter().any(|fact| fact["type"] == "tool.prepared"));
    assert_eq!(std::fs::read(&newer_log).unwrap(), newer_bytes);
    tui.record(
        "picker-selects-real-id-not-title",
        "the older duplicate-title session is selected by UUID and recalls its own history without touching the newer session",
    );
    tui.checkpoint_session("picker-duplicate-id");
    tui.quit();
    tui.finish();
}

fn session_log(tui: &LiveTui, id: &str) -> std::path::PathBuf {
    tui.files(&tui.workspace.join(".a3s"), "jsonl")
        .into_iter()
        .find(|path| path.file_stem().and_then(|name| name.to_str()) == Some(id))
        .expect("session fact log exists")
}

pub(super) fn switch_home_and_pick(tui: &mut LiveTui, id: &str) {
    tui.command("/home");
    tui.wait_for("Resume session", 10);
    tui.bytes(b"\x12");
    tui.wait_for("Esc:back", 10);
    filter_picker_by_id(tui, id);
    tui.bytes(b"\r");
    tui.pump(300);
}

#[test]
#[ignore = "requires the paired source build and original kimi/k3.1-pro ACL"]
fn kimi_background_session_switch_keeps_reply_permission_and_tool_owner() {
    let mut tui = LiveTui::new("background-session-owner");
    let b_id = create_titled_history(&mut tui, "BACKGROUND-B");
    let b_log = session_log(&tui, &b_id);
    let b_bytes = std::fs::read(&b_log).unwrap();

    tui.launch(&[]);
    tui.wait_for("Aster", 45);
    let a_id = tui.session_id();
    tui.dismiss();
    let before = tui.facts().len();
    let script = if cfg!(windows) {
        "Start-Sleep -Seconds 6; [IO.File]::WriteAllText('owner-result.txt', 'A-TOOL-RESULT'); Write-Output 'A-TOOL-RESULT'"
    } else {
        "sleep 6; printf 'A-TOOL-RESULT' > owner-result.txt; cat owner-result.txt"
    };
    let prompt = format!(
        "Use the bash tool exactly once to run {script}. Reply with the exact output. Do not call other tools or modify any other file."
    );
    tui.command(&prompt);

    switch_home_and_pick(&mut tui, &b_id);
    assert_eq!(tui.session_id(), b_id);
    tui.dismiss();
    let a_log = session_log(&tui, &a_id);
    let result = tui.workspace.join("owner-result.txt");
    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        let log = std::fs::read_to_string(&a_log).unwrap_or_default();
        let requested = log
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .any(|fact| fact["type"] == "model.turn" && fact["payload"]["call"]["name"] == "bash");
        if requested {
            break;
        }
        if Instant::now() >= deadline {
            tui.record("background-owner-pending-log", &tui.redact(&log));
            panic!("background tool was not requested");
        }
        tui.pump(80);
    }
    assert!(!result.exists());
    let visible = tui.pump(200);
    assert!(!visible.contains("Yes, proceed"));
    assert!(!visible.contains("A-TOOL-RESULT"));
    assert_eq!(std::fs::read(&b_log).unwrap(), b_bytes);
    tui.record(
        "background-permission-owner",
        "the background session prepared the tool, but no permission card or result appears in the visible session and its log is unchanged",
    );

    switch_home_and_pick(&mut tui, &a_id);
    tui.wait_for("Yes, proceed", 30);
    assert!(!result.exists());
    tui.record(
        "background-permission-card-owner",
        "returning to the original owner reveals its pending permission card, still without a result file",
    );
    tui.bytes(b"\r");

    switch_home_and_pick(&mut tui, &b_id);
    assert_eq!(tui.session_id(), b_id);
    tui.dismiss();
    let deadline = Instant::now() + Duration::from_secs(45);
    while !result.exists() {
        assert!(
            Instant::now() < deadline,
            "approved background tool did not finish"
        );
        tui.pump(80);
    }
    let visible = tui.pump(200);
    assert!(!visible.contains("Yes, proceed"));
    assert!(!visible.contains("A-TOOL-RESULT"));
    assert_eq!(std::fs::read(&b_log).unwrap(), b_bytes);
    tui.record(
        "background-tool-owner",
        "the approved tool completes in the background without exposing its result in the visible session or changing its log",
    );

    switch_home_and_pick(&mut tui, &a_id);
    assert_eq!(tui.session_id(), a_id);
    tui.dismiss();
    let facts = tui.settle_turn(before, &prompt, None);
    assert_one_bash(&facts);
    assert_eq!(
        facts
            .iter()
            .filter(|fact| fact["type"] == "tool.result")
            .count(),
        1
    );
    assert!(tui.assistant_text(&facts).contains("A-TOOL-RESULT"));
    assert_eq!(std::fs::read(&result).unwrap(), b"A-TOOL-RESULT");
    assert_eq!(std::fs::read(&b_log).unwrap(), b_bytes);
    tui.record(
        "background-response-owner",
        "the original owner receives the tool result and final model reply while the other session's fact log remains byte-identical",
    );
    tui.checkpoint_session("background-owner");
    tui.quit();
    tui.finish();
}

#[test]
#[ignore = "requires the paired source build and original kimi/k3.1-pro ACL"]
fn kimi_background_session_switch_preserves_read_only_tool_and_response_owner() {
    let mut tui = LiveTui::new("background-session-read-only-owner");
    let b_id = create_titled_history(&mut tui, "BACKGROUND-READ-ONLY-B");
    let b_log = session_log(&tui, &b_id);
    let b_bytes = std::fs::read(&b_log).unwrap();
    let token = format!("OWNER-READ-{}", uuid::Uuid::new_v4());
    let input = tui.workspace.join("owner-input.txt");
    std::fs::write(&input, &token).unwrap();

    tui.launch(&[]);
    tui.wait_for("Aster", 45);
    let a_id = tui.session_id();
    tui.dismiss();
    let a_log = session_log(&tui, &a_id);
    let before = tui.facts().len();
    let script = if cfg!(windows) {
        "Start-Sleep -Seconds 6; Get-Content -Raw -LiteralPath owner-input.txt"
    } else {
        "sleep 6; cat owner-input.txt"
    };
    let prompt = format!(
        "Use the bash tool exactly once to run {script}. Reply with the exact output. Do not call other tools or modify any file."
    );
    tui.command(&prompt);
    switch_home_and_pick(&mut tui, &b_id);
    assert_eq!(tui.session_id(), b_id);
    tui.dismiss();

    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        let log = std::fs::read_to_string(&a_log).unwrap();
        let facts = log
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .collect::<Vec<_>>();
        if facts
            .iter()
            .any(|fact| fact["type"] == "model.turn" && fact["payload"]["call"]["name"] == "bash")
        {
            assert!(!facts.iter().any(|fact| fact["type"] == "tool.result"));
            break;
        }
        assert!(
            Instant::now() < deadline,
            "background tool was not requested"
        );
        tui.pump(80);
    }
    let visible = tui.pump(200);
    assert!(!visible.contains("Yes, proceed"));
    assert!(!visible.contains(&token));
    assert_eq!(std::fs::read(&b_log).unwrap(), b_bytes);
    tui.record(
        "background-permission-owner",
        "the read-only tool request waits in session A while session B has no permission card, output, or log changes",
    );

    switch_home_and_pick(&mut tui, &a_id);
    tui.wait_for("Yes, proceed", 30);
    tui.record(
        "background-permission-card-owner",
        "returning to session A reveals its pending permission card",
    );
    tui.bytes(b"\r");
    switch_home_and_pick(&mut tui, &b_id);
    assert_eq!(tui.session_id(), b_id);
    tui.dismiss();

    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        let log = std::fs::read_to_string(&a_log).unwrap();
        if log.lines().any(|line| {
            serde_json::from_str::<serde_json::Value>(line).unwrap()["type"] == "turn.complete"
        }) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "background read did not complete"
        );
        tui.pump(80);
    }
    let visible = tui.pump(200);
    assert!(!visible.contains("Yes, proceed"));
    assert!(!visible.contains(&token));
    assert_eq!(std::fs::read(&b_log).unwrap(), b_bytes);
    assert_eq!(std::fs::read(&input).unwrap(), token.as_bytes());
    tui.record(
        "background-tool-owner",
        "the approved read completes in session A while session B's screen and byte-identical fact log have no result",
    );

    switch_home_and_pick(&mut tui, &a_id);
    assert_eq!(tui.session_id(), a_id);
    tui.dismiss();
    let facts = tui.settle_turn(before, &prompt, None);
    assert_one_bash(&facts);
    assert_eq!(
        facts
            .iter()
            .filter(|fact| fact["type"] == "tool.result")
            .count(),
        1
    );
    assert!(tui.assistant_text(&facts).contains(&token));
    assert_eq!(std::fs::read(&b_log).unwrap(), b_bytes);
    assert_eq!(std::fs::read(&input).unwrap(), token.as_bytes());
    tui.record(
        "background-response-owner",
        "session A alone receives the single tool result and final reply containing the unseen file token; the input file remains unchanged",
    );
    tui.checkpoint_session("background-read-only-owner");
    tui.quit();
    tui.finish();
}

#[test]
#[ignore = "requires the paired source build and original kimi/k3.1-pro ACL"]
fn kimi_explicit_deny_survives_yolo_and_next_read() {
    let mut tui = LiveTui::new("yolo-explicit-deny");
    let token = format!("PROTECTED-{}", uuid::Uuid::new_v4());
    let file = tui.workspace.join("protected.txt");
    std::fs::write(&file, &token).unwrap();
    tui.launch(&["--yolo", "--deny", "Write(*)"]);
    tui.wait_for("Aster", 45);
    let facts = tui.turn("Use the write tool exactly once to replace protected.txt with REPLACED. Do not call any other tools. Report the actual result.", None);
    assert!(
        facts
            .iter()
            .any(|f| f["type"] == "tool.prepared"
                && f["payload"]["prepared"]["tool_name"] == "write")
    );
    assert!(
        facts
            .iter()
            .any(|f| f["type"] == "tool.result" && f["payload"]["receipt"]["outcome"] == "denied")
    );
    assert_eq!(std::fs::read(&file).unwrap(), token.as_bytes());
    tui.record(
        "explicit-deny",
        "actual write denied under YOLO; exact original bytes retained",
    );
    let facts = tui.turn("Use the read tool once to read protected.txt and reply with its exact contents. Do not call other tools or modify any file.", None);
    assert!(tui.assistant_text(&facts).contains(&token));
    tui.record(
        "usable-after-denial",
        "subsequent actual Kimi read completes in the same session",
    );
    tui.quit();
    tui.finish();
}

#[test]
#[ignore = "requires the paired source build and original kimi/k3.1-pro ACL"]
fn kimi_allow_always_survives_restart_and_is_workspace_scoped() {
    let mut tui = LiveTui::new("learned-approval");
    let token = format!("LEARNED-{}", uuid::Uuid::new_v4());
    std::fs::write(tui.workspace.join("approval-proof.txt"), &token).unwrap();
    tui.launch(&[]);
    tui.wait_for("Aster", 45);
    let facts = tui.turn(CAT_APPROVAL, Some(b"\x1b[B\r"));
    assert_one_bash(&facts);
    assert_eq!(tui.permission_answer_count(), 1);
    assert!(tui.assistant_text(&facts).contains(&token));
    let store = tui
        .files(&tui.home, "acl")
        .into_iter()
        .find(|p| p.file_name().and_then(|v| v.to_str()) == Some("approvals.acl"))
        .expect("allow-always must persist an ACL store in the isolated home");
    let saved = std::fs::read_to_string(&store).unwrap();
    let document = a3s_acl::parse_acl(&saved).unwrap();
    assert!(document.blocks.iter().any(|b| matches!(b.attributes.get("rules"), Some(a3s_acl::Value::List(rules)) if rules.contains(&a3s_acl::Value::String("cat".into())))));
    tui.record(
        "allow-always",
        "actual permission selection persisted the exact cat rule in ACL",
    );
    tui.checkpoint_session("learned");
    tui.quit();
    tui.launch(&[]);
    tui.wait_for("Aster", 45);
    let facts = tui.turn(CAT_APPROVAL, None);
    assert_one_bash(&facts);
    assert_eq!(tui.permission_answer_count(), 1);
    assert!(tui.assistant_text(&facts).contains(&token));
    tui.record(
        "learned-restart",
        "same workspace reuses persisted approval with no permission card",
    );
    tui.checkpoint_session("restarted");
    tui.quit();
    let other = tui.workspace.parent().unwrap().join("other-workspace");
    std::fs::create_dir(&other).unwrap();
    let other_token = format!("OTHER-{}", uuid::Uuid::new_v4());
    std::fs::write(other.join("approval-proof.txt"), &other_token).unwrap();
    tui.workspace = other;
    tui.launch(&[]);
    tui.wait_for("Aster", 45);
    let facts = tui.turn(CAT_APPROVAL, Some(b"\r"));
    assert_one_bash(&facts);
    assert_eq!(
        tui.permission_answer_count(),
        2,
        "the other workspace must show a real new permission card"
    );
    assert!(tui.assistant_text(&facts).contains(&other_token));
    assert_eq!(
        std::fs::read_to_string(store).unwrap(),
        saved,
        "allow-once in another workspace must not learn or change the original rule"
    );
    tui.record(
        "approval-workspace-scope",
        "another workspace asks again and returns its own unseen token",
    );
    tui.checkpoint_session("other-workspace");
    tui.quit();
    tui.finish();
}

#[test]
#[ignore = "requires the paired source build and original kimi/k3.1-pro ACL"]
fn kimi_copy_nth_exact_bytes_and_export_failures() {
    let mut tui = LiveTui::new("copy-export-contract");
    tui.launch(&[]);
    tui.wait_for("Aster", 45);
    let mut answers = Vec::new();
    let mut tokens = Vec::new();
    for (file, prompt) in [
        (
            "first.txt",
            "Use the read tool once to read first.txt and reply with its exact contents. Do not call other tools or modify any file.",
        ),
        (
            "second.txt",
            "Use the read tool once to read second.txt and reply with its exact contents. Do not call other tools or modify any file.",
        ),
    ] {
        let token = format!("导出-{}", uuid::Uuid::new_v4());
        std::fs::write(tui.workspace.join(file), &token).unwrap();
        let facts = tui.turn(prompt, None);
        let answer = facts
            .iter()
            .filter(|f| f["type"] == "model.turn")
            .filter_map(|f| f["payload"]["text"].as_str())
            .filter(|s| !s.is_empty())
            .last()
            .unwrap()
            .to_owned();
        assert!(answer.contains(&token));
        answers.push(answer);
        tokens.push(token);
    }
    for (n, expected) in [(1, &answers[1]), (2, &answers[0])] {
        let file = tui.workspace.join(format!("第 {n} 条.txt"));
        tui.literal_command(&format!("/copy {n} {}", file.display()));
        let deadline = Instant::now() + Duration::from_secs(8);
        while !file.is_file() {
            tui.pump(80);
            assert!(Instant::now() < deadline, "copy output was not created");
        }
        assert_eq!(std::fs::read(&file).unwrap(), expected.as_bytes());
        tui.record(
            &format!("copy-{n}"),
            "requested assistant response exported with exact UTF-8 bytes",
        );
    }
    let export = tui.workspace.join("完整 会话.md");
    tui.literal_command(&format!("/export {}", export.display()));
    tui.wait_for("Conversation exported", 8);
    let text = std::fs::read_to_string(export).unwrap();
    assert!(text.find(&tokens[0]).unwrap() < text.find(&tokens[1]).unwrap());
    assert!(text.contains("first.txt") && text.contains("second.txt"));
    tui.record(
        "export-order",
        "full transcript contains both prompts and replies in order",
    );
    let directory = tui.workspace.join("directory-output");
    std::fs::create_dir(&directory).unwrap();
    let before = tui.facts().len();
    tui.literal_command(&format!("/copy {}", directory.display()));
    tui.wait_for("Failed to write file:", 8);
    assert!(directory.is_dir());
    assert_eq!(tui.facts().len(), before);
    tui.command("/copy 0");
    tui.wait_for("where N is 1", 8);
    assert_eq!(tui.facts().len(), before);
    tui.record(
        "export-refusal",
        "directory write and invalid response index fail explicitly without model work",
    );
    tui.quit();
    tui.finish();
}
