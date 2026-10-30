#![cfg(test)]

use super::driver::LiveTui;
use std::time::{Duration, Instant};

fn key(tui: &mut LiveTui, sequence: &[u8]) {
    tui.bytes(sequence);
    tui.pump(200);
}

#[test]
#[ignore = "requires the paired source build and original kimi/k3.1-pro ACL"]
fn kimi_queued_prompts_edit_delete_and_drain_once_in_order() {
    let mut tui = LiveTui::new("queue-edit-delete-drain");
    let token = format!("QUEUE-PROOF-{}", uuid::Uuid::new_v4());
    std::fs::write(tui.workspace.join("queue-proof.txt"), &token).unwrap();
    tui.launch_default(&[]);
    tui.wait_for("Aster", 45);
    tui.session_id();
    tui.dismiss();
    tui.command("/yolo");
    tui.wait_for("yolo", 8);
    let initial = if cfg!(windows) {
        "Use the bash tool exactly once to run Start-Sleep -Seconds 20; Get-Content -LiteralPath queue-proof.txt -Raw. Reply with its exact output. Do not call other tools or modify any file."
    } else {
        "Use the bash tool exactly once to run sleep 20; cat queue-proof.txt. Reply with its exact output. Do not call other tools or modify any file."
    };
    let first = "Reply only QUEUED-FIRST-OK. Do not call any tools.";
    let second = "Reply only QUEUED-SECOND-ORIGINAL. Do not call any tools.";
    let edited = "Reply only QUEUED-SECOND-EDITED. Do not call any tools.";
    let removed = "Reply only QUEUED-REMOVED-MUST-NOT-RUN. Do not call any tools.";
    let before = tui.facts().len();
    tui.command(initial);
    let prepared = tui.wait_fact("tool.prepared", before);
    assert_eq!(prepared["payload"]["prepared"]["tool_name"], "bash");
    for (index, prompt) in [first, second, removed].into_iter().enumerate() {
        tui.command(prompt);
        tui.wait_for(&format!("#{} {prompt}", index + 1), 8);
    }
    key(&mut tui, b"\x1b[A");
    key(&mut tui, b"x");
    key(&mut tui, b"e");
    tui.wait_for("QUEUED-SECOND-ORIGINAL", 8);
    key(&mut tui, b"\x05\x15");
    tui.bytes(b"\x1b[200~");
    tui.bytes(edited.as_bytes());
    tui.bytes(b"\x1b[201~\x1b[F");
    tui.wait_for("QUEUED-SECOND-EDITED", 8);
    key(&mut tui, b"\r");

    let deadline = Instant::now() + Duration::from_secs(420);
    let observed = loop {
        let screen = tui.pump(100);
        assert!(
            !screen.contains("Turn failed"),
            "queued turn failed\n{screen}"
        );
        let facts = tui.facts();
        let delta = facts.get(before..).unwrap_or_default();
        if delta
            .iter()
            .filter(|fact| fact["type"] == "turn.complete")
            .count()
            == 3
        {
            break delta.to_vec();
        }
        assert!(
            Instant::now() < deadline,
            "queued work did not drain\n{screen}"
        );
    };
    let submitted = observed
        .iter()
        .filter(|fact| fact["type"] == "user.message")
        .map(|fact| fact["payload"]["text"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(submitted, vec![initial, first, edited]);
    let boundaries = observed
        .iter()
        .enumerate()
        .filter(|(_, fact)| fact["type"] == "user.message")
        .map(|(index, _)| index)
        .chain(std::iter::once(observed.len()))
        .collect::<Vec<_>>();
    for (range, expected) in
        boundaries
            .windows(2)
            .zip([token.as_str(), "QUEUED-FIRST-OK", "QUEUED-SECOND-EDITED"])
    {
        let turn = &observed[range[0]..range[1]];
        assert!(tui.assistant_text(turn).contains(expected));
        assert_eq!(
            turn.iter()
                .filter(|fact| fact["type"] == "turn.complete")
                .count(),
            1
        );
    }
    assert_eq!(
        observed
            .iter()
            .filter(|fact| fact["type"] == "tool.prepared")
            .count(),
        1
    );
    assert_eq!(
        std::fs::read(tui.workspace.join("queue-proof.txt")).unwrap(),
        token.as_bytes()
    );
    tui.record("queue-effects", "three queued rows: delete the last, edit the second, then exact user messages and settled replies drain once in the intended order");
    tui.quit();
    tui.finish();
}

#[test]
#[ignore = "requires the paired source build and original kimi/k3.1-pro ACL"]
fn kimi_direct_shell_is_executed_or_explicitly_refused_without_model_routing() {
    let mut tui = LiveTui::new("direct-shell-capability");
    let token = format!("DIRECT-SHELL-{}", uuid::Uuid::new_v4());
    tui.launch_default(&[]);
    tui.wait_for("Aster", 45);
    tui.session_id();
    tui.dismiss();
    tui.command("/yolo");
    tui.wait_for("yolo", 8);
    let before = tui.facts().len();
    key(&mut tui, b"!");
    let command = if cfg!(windows) {
        format!("[IO.File]::WriteAllText('direct-shell.txt', '{token}'); Write-Output '{token}'")
    } else {
        format!("printf '%s' '{token}' > direct-shell.txt; printf '%s' '{token}'")
    };
    tui.command(&command);
    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        let screen = tui.pump(100);
        let facts = tui.facts();
        let delta = facts.get(before..).unwrap_or_default();
        assert!(
            !delta.iter().any(|fact| fact["type"] == "model.turn"),
            "direct shell metadata was silently routed into model inference"
        );
        if screen.contains("Direct shell commands are unavailable with the Code backend.") {
            assert_eq!(facts.len(), before);
            assert!(!tui.workspace.join("direct-shell.txt").exists());
            tui.record("direct-shell-refusal", "backend explicitly refuses the direct-command surface before any model request or file effect");
            break;
        }
        if delta.iter().any(|fact| fact["type"] == "turn.complete") {
            assert_eq!(
                std::fs::read(tui.workspace.join("direct-shell.txt")).unwrap(),
                token.as_bytes()
            );
            assert!(screen.contains(&token));
            assert!(delta.iter().any(|fact| fact["type"] == "tool.prepared"
                && fact["payload"]["prepared"]["tool_name"] == "bash"));
            tui.record("direct-shell-execution", "typed direct shell runs without model inference and produces exact workspace bytes with a real receipt");
            break;
        }
        assert!(
            Instant::now() < deadline,
            "direct shell neither executed nor explicitly refused\n{screen}"
        );
    }
    key(&mut tui, b"\x15");
    key(&mut tui, b"\x15");
    tui.quit();
    tui.finish();
}
