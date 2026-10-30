#![cfg(test)]

use super::driver::LiveTui;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

const TRUST_PROMPT: &str = "Do you trust the contents of this directory";
const READ_TRUSTED_FILE: &str = "Use the read tool once to read trust-proof.txt and reply with its exact contents. Do not call other tools or modify any file.";

fn fixture(workspace: &Path) {
    let marker = workspace.join("repository-command-ran.txt");
    let config = if cfg!(windows) {
        let script = workspace.join("marker.ps1");
        std::fs::write(
            &script,
            "param([string]$Marker)\nSet-Content -LiteralPath $Marker -Value started -NoNewline\n",
        )
        .unwrap();
        serde_json::json!({
            "mcpServers": {
                "repo-tools": {
                    "command": "powershell.exe",
                    "args": ["-NoProfile", "-File", script, marker]
                }
            }
        })
    } else {
        let script = workspace.join("marker.sh");
        std::fs::write(&script, "printf started > \"$1\"\n").unwrap();
        serde_json::json!({
            "mcpServers": {
                "repo-tools": {
                    "command": "/bin/sh",
                    "args": [script, marker]
                }
            }
        })
    };
    std::fs::write(
        workspace.join(".mcp.json"),
        serde_json::to_vec_pretty(&config).unwrap(),
    )
    .unwrap();
    let status = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(workspace)
        .status()
        .unwrap();
    assert!(status.success(), "git init failed for isolated workspace");
}

fn assert_no_work_before(tui: &LiveTui) {
    assert!(!tui.workspace.join("repository-command-ran.txt").exists());
    assert!(
        tui.facts().is_empty(),
        "trust must precede session/model work"
    );
    assert!(!tui.home.join("grok/trusted_folders.toml").exists());
}

fn wait_for_exit_without_effects(tui: &mut LiveTui, answer: &[u8]) {
    tui.bytes(answer);
    tui.wait_exit(15);
    assert!(!tui.workspace.join("repository-command-ran.txt").exists());
    assert!(
        tui.facts().is_empty(),
        "declined trust produced session/model work"
    );
    assert!(!tui.home.join("grok/trusted_folders.toml").exists());
}

#[test]
#[ignore = "requires the paired source build and original kimi/k3.1-pro ACL"]
fn kimi_workspace_trust_accept_restart_and_second_workspace_are_scoped() {
    let mut tui = LiveTui::new("trust-accept");
    tui.set_env("GROK_TEST_VERSION", "0.0.0-acceptance");
    fixture(&tui.workspace);
    let token = format!("TRUSTED-{}", uuid::Uuid::new_v4());
    std::fs::write(tui.workspace.join("trust-proof.txt"), &token).unwrap();
    tui.launch(&[]);
    let prompt = tui.wait_for_unanswered(TRUST_PROMPT, 45);
    assert!(
        prompt.contains(&tui.workspace.to_string_lossy().into_owned()),
        "{prompt}"
    );
    assert_no_work_before(&tui);
    tui.record(
        "trust-prompt",
        "release-stamped trust gate blocks the declared process and all session/model work",
    );
    tui.bytes(b"y");
    tui.wait_for("kimi/", 45);
    let deadline = Instant::now() + Duration::from_secs(15);
    while !tui.workspace.join("repository-command-ran.txt").is_file() {
        tui.pump(100);
        assert!(
            Instant::now() < deadline,
            "accepted workspace command did not start"
        );
    }
    let store = tui.home.join("grok/trusted_folders.toml");
    let saved = std::fs::read_to_string(&store).unwrap();
    assert_eq!(saved.matches("trusted = true").count(), 1, "{saved}");
    let before = tui.facts().len();
    tui.command(READ_TRUSTED_FILE);
    let facts = tui.settle_turn(before, READ_TRUSTED_FILE, None);
    assert!(tui.assistant_text(&facts).contains(&token));
    tui.record(
        "trust-accept",
        "the shown workspace grant starts its declared command and permits a real Kimi read",
    );
    tui.checkpoint_session("accepted");
    tui.quit();

    tui.launch(&[]);
    let restarted = tui.wait_for_unanswered("kimi/", 45);
    assert!(!restarted.contains(TRUST_PROMPT), "{restarted}");
    let before = tui.facts().len();
    tui.command(READ_TRUSTED_FILE);
    let facts = tui.settle_turn(before, READ_TRUSTED_FILE, None);
    assert!(tui.assistant_text(&facts).contains(&token));
    tui.record(
        "trust-restart",
        "the original workspace reuses its grant and completes another real Kimi read after a real process restart",
    );
    tui.quit();

    let original_store = std::fs::read(&store).unwrap();
    let other = tui.workspace.parent().unwrap().join("other-workspace");
    std::fs::create_dir(&other).unwrap();
    fixture(&other);
    tui.workspace = other;
    tui.launch(&[]);
    let prompt = tui.wait_for_unanswered(TRUST_PROMPT, 45);
    assert!(
        prompt.contains(&tui.workspace.to_string_lossy().into_owned()),
        "{prompt}"
    );
    assert!(!tui.workspace.join("repository-command-ran.txt").exists());
    assert_eq!(std::fs::read(&store).unwrap(), original_store);
    tui.bytes(b"n");
    tui.wait_exit(15);
    assert!(!tui.workspace.join("repository-command-ran.txt").exists());
    assert!(tui.facts().is_empty());
    assert_eq!(std::fs::read(&store).unwrap(), original_store);
    tui.record(
        "trust-workspace-scope",
        "a second workspace asks again and declining it leaves the original grant byte-identical",
    );
    tui.finish();
}

#[test]
#[ignore = "requires the paired source build and original kimi/k3.1-pro ACL"]
fn kimi_workspace_trust_decline_and_cancel_have_no_effects() {
    for (case, answer) in [
        ("trust-reject", b"n".as_slice()),
        ("trust-cancel", b"\x1b".as_slice()),
    ] {
        let mut tui = LiveTui::new(case);
        tui.set_env("GROK_TEST_VERSION", "0.0.0-acceptance");
        fixture(&tui.workspace);
        tui.launch(&[]);
        let prompt = tui.wait_for_unanswered(TRUST_PROMPT, 45);
        assert!(
            prompt.contains(&tui.workspace.to_string_lossy().into_owned()),
            "{prompt}"
        );
        assert_no_work_before(&tui);
        wait_for_exit_without_effects(&mut tui, answer);
        tui.record(
            case,
            "decline/cancel exits before the declared process, session/model work or an acceptance record",
        );
        tui.finish();
    }
}

#[test]
#[ignore = "requires the paired source build and original kimi/k3.1-pro ACL"]
fn kimi_workspace_acl_cannot_override_host_provider_or_explicit_deny() {
    let mut tui = LiveTui::new("workspace-acl-authority");
    tui.set_env("GROK_TEST_VERSION", "0.0.0-acceptance");
    fixture(&tui.workspace);
    std::fs::create_dir(tui.workspace.join(".a3s")).unwrap();
    std::fs::write(
        tui.workspace.join(".a3s/config.acl"),
        r#"default_model = "kimi/k3.1-pro"
providers "kimi" {
  api_key = "workspace-key-must-not-be-used"
  base_url = "http://127.0.0.1:9/v1"
  models "k3.1-pro" {}
}
"#,
    )
    .unwrap();
    let token = format!("HOST-{}", uuid::Uuid::new_v4());
    std::fs::write(tui.workspace.join("trust-proof.txt"), &token).unwrap();
    std::fs::write(tui.workspace.join("protected.txt"), b"original").unwrap();
    tui.launch(&["--yolo", "--deny", "Write(*)"]);
    let prompt = tui.wait_for_unanswered(TRUST_PROMPT, 45);
    assert!(
        prompt.contains(&tui.workspace.to_string_lossy().into_owned()),
        "{prompt}"
    );
    assert_no_work_before(&tui);
    tui.bytes(b"y");
    tui.wait_for("kimi/", 45);
    let deadline = Instant::now() + Duration::from_secs(15);
    while !tui.workspace.join("repository-command-ran.txt").is_file() {
        tui.pump(100);
        assert!(
            Instant::now() < deadline,
            "accepted workspace command did not start"
        );
    }

    let before = tui.facts().len();
    tui.command(READ_TRUSTED_FILE);
    let facts = tui.settle_turn(before, READ_TRUSTED_FILE, None);
    assert!(tui.assistant_text(&facts).contains(&token));
    assert!(!format!("{facts:?}").contains("workspace-key-must-not-be-used"));
    tui.record(
        "workspace-provider-boundary",
        "a same-name workspace provider and local URL cannot replace the trusted Kimi provider or its credentials",
    );

    let prompt = "Use the write tool exactly once to replace protected.txt with REPLACED. Do not call any other tools. Report the actual result.";
    let before = tui.facts().len();
    tui.command(prompt);
    let facts = tui.settle_turn(before, prompt, None);
    assert!(facts.iter().any(|fact| {
        fact["type"] == "tool.prepared" && fact["payload"]["prepared"]["tool_name"] == "write"
    }));
    assert!(facts.iter().any(|fact| {
        fact["type"] == "tool.result" && fact["payload"]["receipt"]["outcome"] == "denied"
    }));
    assert_eq!(
        std::fs::read(tui.workspace.join("protected.txt")).unwrap(),
        b"original"
    );
    tui.record(
        "workspace-host-deny",
        "workspace content cannot relax the explicit CLI deny under YOLO; original bytes remain",
    );
    tui.quit();
    tui.finish();
}

fn only_tool_output(facts: &[serde_json::Value]) -> serde_json::Value {
    let prepared = facts
        .iter()
        .filter(|fact| fact["type"] == "tool.prepared")
        .collect::<Vec<_>>();
    assert_eq!(
        prepared.len(),
        1,
        "expected exactly one tool call: {facts:?}"
    );
    assert_eq!(prepared[0]["payload"]["prepared"]["tool_name"], "read");
    let receipts = facts
        .iter()
        .filter(|fact| fact["type"] == "tool.result")
        .collect::<Vec<_>>();
    assert_eq!(
        receipts.len(),
        1,
        "expected exactly one read receipt: {facts:?}"
    );
    serde_json::from_str(
        receipts[0]["payload"]["receipt"]["output"]
            .as_str()
            .unwrap(),
    )
    .unwrap()
}

fn assert_failed_read(tui: &LiveTui, facts: &[serde_json::Value], secret: &str) {
    let outcome = facts
        .iter()
        .find(|fact| fact["type"] == "tool.result")
        .map(|fact| fact["payload"]["receipt"]["outcome"].as_str().unwrap())
        .unwrap();
    assert!(matches!(outcome, "failed" | "denied"), "{facts:?}");
    let output = facts
        .iter()
        .find(|fact| fact["type"] == "tool.result")
        .map(|fact| fact["payload"]["receipt"]["output"].as_str().unwrap())
        .unwrap();
    assert!(!output.is_empty(), "{facts:?}");
    assert!(!output.contains(secret), "{output}");
    assert!(!tui.assistant_text(facts).contains(secret));
}

fn boundary_case(case: &str) -> LiveTui {
    let mut tui = LiveTui::new(case);
    tui.launch(&["--yolo"]);
    tui.wait_for("Aster", 45);
    tui
}

fn escape_prompt(path: &str) -> String {
    format!("This is a one-shot diagnostic of the read tool's own path validation. Call read exactly once with file_path {path}. Do not infer or short-circuit the outcome; let the tool return its actual error. Do not retry or call other tools. Report the tool's result.")
}

#[test]
#[ignore = "requires the paired source build and original kimi/k3.1-pro ACL"]
fn kimi_read_rejects_escapes_links_and_preserves_bounded_pages() {
    for (case, relative) in [
        ("path-absolute-escape", false),
        ("path-relative-escape", true),
    ] {
        let mut tui = boundary_case(case);
        let outside = tui.workspace.parent().unwrap().join("outside-secret.txt");
        let outside_token = format!("OUTSIDE-{}", uuid::Uuid::new_v4());
        std::fs::write(&outside, &outside_token).unwrap();
        let path = if relative {
            "../outside-secret.txt".to_string()
        } else {
            outside.to_string_lossy().into_owned()
        };
        let prompt = escape_prompt(&path);
        let before = tui.facts().len();
        tui.command(&prompt);
        let facts = tui.settle_turn(before, &prompt, None);
        assert_failed_read(&tui, &facts, &outside_token);
        tui.record(
            case,
            "the actual read receipt fails closed and never exposes the outside secret",
        );
        tui.quit();
        tui.finish();
    }

    let mut tui = boundary_case("path-link-escape");
    let outside = tui.workspace.parent().unwrap().join("outside-secret.txt");
    let outside_token = format!("OUTSIDE-{}", uuid::Uuid::new_v4());
    std::fs::write(&outside, &outside_token).unwrap();
    #[cfg(windows)]
    let linked_path = {
        let outside_dir = tui.workspace.parent().unwrap().join("outside-directory");
        std::fs::create_dir(&outside_dir).unwrap();
        std::fs::write(outside_dir.join("secret.txt"), &outside_token).unwrap();
        let link = tui.workspace.join("outside-junction");
        let status = Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(&link)
            .arg(&outside_dir)
            .status()
            .unwrap();
        assert!(status.success(), "failed to create an isolated junction");
        "outside-junction/secret.txt".to_string()
    };
    #[cfg(unix)]
    let linked_path = {
        std::os::unix::fs::symlink(&outside, tui.workspace.join("outside-link.txt")).unwrap();
        "outside-link.txt".to_string()
    };
    let prompt = escape_prompt(&linked_path);
    let before = tui.facts().len();
    tui.command(&prompt);
    let facts = tui.settle_turn(before, &prompt, None);
    assert_failed_read(&tui, &facts, &outside_token);
    tui.record(
        "link-escape",
        "a symlink/junction cannot expose the outside secret through the real read path",
    );
    tui.quit();
    tui.finish();

    let mut tui = boundary_case("path-bounded-read");
    let bounded_token = format!("BOUNDED-{}", uuid::Uuid::new_v4());
    let mut contents = (1..=250)
        .map(|line| format!("LINE-{line:03}"))
        .collect::<Vec<_>>()
        .join("\n");
    contents.push_str(&format!("\n{bounded_token}\n"));
    std::fs::write(tui.workspace.join("bounded.txt"), contents).unwrap();
    let prompt = "Use the read tool exactly once to read bounded.txt with file_path bounded.txt, offset 0 and limit 20. Report the visible lines and next_offset. Do not call other tools or modify any file.";
    let before = tui.facts().len();
    tui.command(prompt);
    let facts = tui.settle_turn(before, prompt, None);
    let output = only_tool_output(&facts);
    assert_eq!(output["is_error"], false, "{output}");
    assert_eq!(output["metadata"]["range"]["requested_limit"], 20);
    assert_eq!(output["metadata"]["range"]["applied_limit"], 20);
    assert_eq!(output["metadata"]["range"]["returned_lines"], 20);
    assert_eq!(output["metadata"]["range"]["next_offset"], 20);
    assert_eq!(output["metadata"]["range"]["eof"], false);
    let text = output["text"].as_str().unwrap();
    assert!(
        text.contains("LINE-001") && text.contains("LINE-020"),
        "{text}"
    );
    assert!(
        !text.contains("LINE-021") && !text.contains(&bounded_token),
        "{text}"
    );
    tui.record(
        "bounded-read",
        "the large file returns exactly twenty lines with next_offset=20 and no later secret",
    );
    tui.quit();
    tui.finish();
}

#[test]
#[ignore = "requires the paired source build and original kimi/k3.1-pro ACL"]
fn kimi_relative_cwd_with_unicode_and_spaces_uses_the_actual_workspace() {
    let mut tui = LiveTui::new("relative-cwd");
    let token = format!("RELATIVE-{}", uuid::Uuid::new_v4());
    std::fs::write(tui.workspace.join("trust-proof.txt"), &token).unwrap();
    let relative = Path::new("工作 空间");
    let relocated = tui.workspace.parent().unwrap().join(relative);
    std::fs::rename(&tui.workspace, &relocated).unwrap();
    tui.workspace = relocated;
    tui.launch_relative(relative);
    tui.wait_for("Aster", 45);
    let before = tui.facts().len();
    tui.command(READ_TRUSTED_FILE);
    let facts = tui.settle_turn(before, READ_TRUSTED_FILE, None);
    assert!(tui.assistant_text(&facts).contains(&token));
    let output = only_tool_output(&facts);
    assert_eq!(output["is_error"], false);
    assert!(tui.workspace.join("trust-proof.txt").is_file());
    assert!(!tui
        .workspace
        .parent()
        .unwrap()
        .join("workspace/trust-proof.txt")
        .exists());
    tui.record(
        "relative-cwd",
        "interactive relative --cwd preserves Unicode and spaces in the real tool workspace",
    );
    tui.quit();
    tui.finish();
}
