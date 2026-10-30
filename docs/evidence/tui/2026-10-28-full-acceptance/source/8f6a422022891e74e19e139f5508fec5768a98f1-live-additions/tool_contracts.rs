#![cfg(test)]

use super::driver::LiveTui;
use serde_json::Value;
use std::time::{Duration, Instant};

fn one_result(facts: &[Value], name: &str) -> Value {
    let calls = facts
        .iter()
        .filter(|fact| fact["type"] == "tool.prepared")
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1, "expected exactly one actual tool call");
    assert_eq!(calls[0]["payload"]["prepared"]["tool_name"], name);
    let results = facts
        .iter()
        .filter(|fact| fact["type"] == "tool.result")
        .collect::<Vec<_>>();
    assert_eq!(results.len(), 1);
    assert_eq!(
        calls[0]["payload"]["prepared"]["identity"],
        results[0]["payload"]["receipt"]["identity"]
    );
    serde_json::from_str(results[0]["payload"]["receipt"]["output"].as_str().unwrap()).unwrap()
}

#[test]
#[ignore = "requires the paired source build and original kimi/k3.1-pro ACL"]
fn kimi_ls_and_search_use_actual_scoped_bounded_workspace_results() {
    let mut tui = LiveTui::new("ls-search-contracts");
    let dir = tui.workspace.join("中文 目录");
    std::fs::create_dir_all(dir.join("00-subdirectory")).unwrap();
    let token = format!("SEARCH-PROOF-{}", uuid::Uuid::new_v4());
    let later = format!("LATER-PROOF-{}", uuid::Uuid::new_v4());
    let first = format!("SEARCH-ANCHOR\n{token}\n");
    let second = format!("SEARCH-ANCHOR\n{later}\n");
    std::fs::write(dir.join("01-first.txt"), &first).unwrap();
    std::fs::write(dir.join("02-second.txt"), &second).unwrap();
    std::fs::write(dir.join("03-last.txt"), "unmatched\n").unwrap();
    let outside = format!("OUTSIDE-PROOF-{}", uuid::Uuid::new_v4());
    std::fs::write(tui.workspace.join("outside.txt"), &outside).unwrap();
    tui.launch_default(&[]);
    tui.wait_for("Aster", 45);
    let listing = tui.turn("Call ls exactly once with path='中文 目录' and limit=2. Reply with the exact returned directory listing. Do not call any other tool or modify files.", None);
    let listed = one_result(&listing, "ls");
    assert_eq!(listed["exit_code"], 0);
    let text = listed["text"].as_str().unwrap();
    assert!(text.contains("00-subdirectory") && text.contains("01-first.txt"));
    assert!(text.find("00-subdirectory").unwrap() < text.find("01-first.txt").unwrap());
    assert!(!text.contains("02-second.txt") && !text.contains("03-last.txt"));
    assert!(!text.contains(&outside));
    assert!(text.contains("2 of 4 entries") && text.contains("More entries available"));
    assert!(tui.assistant_text(&listing).contains("01-first.txt"));
    tui.record("ls-bounded-order", "one actual ls call returns the directory before the first file and omits entries beyond limit=2");

    let searched = tui.turn("Call search exactly once with mode='grep', query='SEARCH-ANCHOR', path='中文 目录', context=1 and limit=1. Reply with the exact match and its following context line. Do not call any other tool or modify files.", None);
    let found = one_result(&searched, "search");
    assert_eq!(found["exit_code"], 0);
    let text = found["text"].as_str().unwrap();
    assert!(text.contains("01-first.txt") && text.contains(&token));
    assert!(!text.contains(&later) && !text.contains(&outside));
    assert!(tui.assistant_text(&searched).contains(&token));
    tui.record("search-context-bound", "one scoped grep search returns the unknown first context token and excludes later matches and outside content");

    let missing = format!("NO-MATCH-{}", uuid::Uuid::new_v4());
    let prompt = format!("Call search exactly once with mode='grep', query='{missing}', path='中文 目录', context=0 and limit=1. Report its actual no-match result. Do not retry, call other tools or modify files.");
    let facts = tui.turn(&prompt, None);
    let empty = one_result(&facts, "search");
    assert_eq!(empty["exit_code"], 0);
    let text = empty["text"].as_str().unwrap();
    assert!(text.contains("No matches found"));
    assert!(!text.contains(&token) && !text.contains(&later) && !text.contains(&outside));
    assert_eq!(
        std::fs::read(dir.join("01-first.txt")).unwrap(),
        first.as_bytes()
    );
    assert_eq!(
        std::fs::read(dir.join("02-second.txt")).unwrap(),
        second.as_bytes()
    );
    assert_eq!(
        std::fs::read(tui.workspace.join("outside.txt")).unwrap(),
        outside.as_bytes()
    );
    tui.record("search-no-match", "the actual scoped no-match receipt succeeds without returning fixture contents or changing their bytes");
    tui.quit();
    tui.finish();
}

#[test]
#[ignore = "requires the paired source build and original kimi/k3.1-pro ACL"]
fn kimi_native_shell_preserves_nonzero_stdout_stderr_cwd_and_recovers() {
    let mut tui = LiveTui::new("shell-nonzero-contract");
    let stdout = format!("STDOUT-{}", uuid::Uuid::new_v4());
    let stderr = format!("STDERR-{}", uuid::Uuid::new_v4());
    let (name, script, command) = if cfg!(windows) {
        (
            "probe script.ps1",
            format!("[Console]::Out.WriteLine('{stdout}')\n[Console]::Error.WriteLine('{stderr}')\n[Console]::Out.WriteLine('CWD=' + (Get-Location).Path)\nexit 7\n"),
            "& './probe script.ps1'",
        )
    } else {
        (
            "probe script.sh",
            format!("printf '%s\\n' '{stdout}'\nprintf '%s\\n' '{stderr}' >&2\nprintf 'CWD=%s\\n' \"$PWD\"\nexit 7\n"),
            "/bin/sh './probe script.sh'",
        )
    };
    std::fs::write(tui.workspace.join(name), &script).unwrap();
    tui.launch_default(&[]);
    tui.wait_for("Aster", 45);
    tui.session_id();
    tui.dismiss();
    tui.command("/yolo");
    tui.wait_for("yolo", 8);
    let prompt = format!("Use bash exactly once to run this exact command: {command}. The script intentionally fails. Report the actual exit code and quote both output streams and its CWD line. Do not retry, repair files or call other tools.");
    let facts = tui.turn(&prompt, None);
    let result = one_result(&facts, "bash");
    assert_eq!(result["exit_code"], 7);
    assert_eq!(result["is_error"], true);
    let text = result["text"].as_str().unwrap();
    assert!(text.contains(&stdout) && text.contains(&stderr));
    let cwd = text
        .lines()
        .find_map(|line| line.strip_prefix("CWD="))
        .unwrap();
    assert_eq!(
        std::path::Path::new(cwd).canonicalize().unwrap(),
        tui.workspace.canonicalize().unwrap()
    );
    let reply = tui.assistant_text(&facts);
    assert!(reply.contains(&stdout) && reply.contains(&stderr) && reply.contains('7'));
    assert_eq!(
        std::fs::read(tui.workspace.join(name)).unwrap(),
        script.as_bytes()
    );
    tui.record("native-nonzero", "one real native command preserves exit 7, independent stdout/stderr tokens, exact working directory and unchanged script bytes");
    let next = tui.turn(
        "Reply only SHELL-FAILURE-RECOVERED. Do not call any tool.",
        None,
    );
    assert!(tui
        .assistant_text(&next)
        .contains("SHELL-FAILURE-RECOVERED"));
    assert!(!next.iter().any(|fact| fact["type"] == "tool.prepared"));
    tui.record("failed-tool-recovery", "the same session accepts and completes the next no-tool request after the real failed shell call");
    tui.quit();
    tui.finish();
}

#[test]
#[ignore = "requires the paired source build and original kimi/k3.1-pro ACL"]
fn kimi_send_now_cancels_running_tool_and_runs_new_prompt_once() {
    let mut tui = LiveTui::new("send-now-cancel-ownership");
    let script = if cfg!(windows) {
        "[IO.File]::WriteAllText('started.txt', 'started'); Start-Sleep -Seconds 8; [IO.File]::WriteAllText('late.txt', 'late')"
    } else {
        "printf started > started.txt; sleep 8; printf late > late.txt"
    };
    let initial = format!("Use bash exactly once to run this exact command: {script}. Then reply ORIGINAL-COMPLETED. Do not call any other tool.");
    let followup = "Reply only SEND-NOW-OWNED. Do not call any tool.";
    tui.launch_default(&[]);
    tui.wait_for("Aster", 45);
    tui.session_id();
    tui.dismiss();
    tui.command("/yolo");
    tui.wait_for("yolo", 8);
    let before = tui.facts().len();
    tui.command(&initial);
    tui.wait_fact("tool.prepared", before);
    let started = Instant::now() + Duration::from_secs(10);
    while !tui.workspace.join("started.txt").is_file() {
        tui.pump(50);
        assert!(Instant::now() < started, "the actual shell never started");
    }
    tui.submit_keys = if cfg!(windows) {
        b"\n".to_vec()
    } else {
        b"\x1b[13;5u".to_vec()
    };
    tui.command(followup);
    tui.submit_keys = b"\r".to_vec();
    let deadline = Instant::now() + Duration::from_secs(420);
    let observed = loop {
        let screen = tui.pump(80);
        assert!(!screen.contains("Turn failed"), "send-now failed\n{screen}");
        let facts = tui.facts();
        let delta = facts.get(before..).unwrap_or_default();
        if tui.assistant_text(delta).contains("SEND-NOW-OWNED")
            && delta.iter().any(|fact| fact["type"] == "turn.complete")
            && !screen.contains("[stop]")
            && !screen.contains("Ctrl+c:cancel")
        {
            break delta.to_vec();
        }
        assert!(
            Instant::now() < deadline,
            "send-now did not settle\n{screen}"
        );
    };
    let messages = observed
        .iter()
        .filter(|fact| fact["type"] == "user.message")
        .map(|fact| fact["payload"]["text"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(messages, vec![initial.as_str(), followup]);
    assert_eq!(
        observed
            .iter()
            .filter(|fact| fact["type"] == "tool.prepared")
            .count(),
        1
    );
    assert!(!tui.assistant_text(&observed).contains("ORIGINAL-COMPLETED"));
    tui.pump(9000);
    assert!(
        !tui.workspace.join("late.txt").exists(),
        "send-now left the old process running"
    );
    assert_eq!(
        std::fs::read(tui.workspace.join("started.txt")).unwrap(),
        b"started"
    );
    tui.record("send-now-owner", "the native shortcut submits exact text once, cancels the old running tool and completes the new prompt without a late write");
    tui.quit();
    tui.finish();
}
