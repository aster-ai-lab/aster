#![cfg(test)]

use super::driver::LiveTui;
use std::time::{Duration, Instant};

pub(super) fn permissions() {
    for (case, answer) in [
        ("permission-reject", b"2".as_slice()),
        ("permission-cancel", b"\x03".as_slice()),
    ] {
        let mut tui = LiveTui::new(case);
        tui.launch(&[]);
        tui.wait_for("Aster", 45);
        tui.command("Use the write tool once to create side-effect.txt containing 'permission-test'. If the request is rejected or cancelled, do not retry and do not use another tool; just report that it was not written.");
        tui.wait_for("Yes, proceed", 120);
        assert!(
            !tui.workspace.join("side-effect.txt").exists(),
            "write ran before consent"
        );
        tui.bytes(answer);
        let deadline = Instant::now() + Duration::from_secs(60);
        while tui.screen().contains("Yes, proceed") {
            tui.pump(80);
            assert!(Instant::now() < deadline, "permission card did not close");
        }
        tui.pump(1200);
        assert!(
            !tui.workspace.join("side-effect.txt").exists(),
            "refused write had a side effect"
        );
        tui.record(
            case,
            "real permission card dismissed; file absent before and after rejection/cancellation",
        );
        tui.quit();
        tui.finish();
    }
}

pub(super) fn write_and_verify(yolo: bool) {
    let mut tui = LiveTui::new(if yolo { "write-verify" } else { "allow-once" });
    let token = format!("KIMI-WRITE-{}", uuid::Uuid::new_v4());
    std::fs::write(tui.workspace.join("source.txt"), &token).unwrap();
    tui.launch(&[]);
    tui.wait_for("Aster", 45);
    if yolo {
        tui.command("/yolo");
        tui.wait_for("yolo", 8);
    }
    let prompt = if cfg!(windows) {
        "Read source.txt. Use the write tool to create result.txt with exactly the same bytes. Then run Get-Item -LiteralPath result.txt -ErrorAction Stop using the bash tool to verify the output exists. Read result.txt back and reply with its exact contents. Do not modify source.txt or any other file."
    } else {
        "Read source.txt. Use the write tool to create result.txt with exactly the same bytes. Then run test -f result.txt using the bash tool to verify the output exists. Read result.txt back and reply with its exact contents. Do not modify source.txt or any other file."
    };
    let facts = tui.turn(prompt, if yolo { None } else { Some(b"1") });
    assert_eq!(
        std::fs::read_to_string(tui.workspace.join("result.txt")).unwrap(),
        token
    );
    assert_eq!(
        std::fs::read_to_string(tui.workspace.join("source.txt")).unwrap(),
        token
    );
    assert!(tui.assistant_text(&facts).contains(&token));
    for tool in ["read", "write", "bash"] {
        assert!(
            facts
                .iter()
                .any(|f| f["type"] == "tool.prepared"
                    && f["payload"]["prepared"]["tool_name"] == tool),
            "missing actual {tool} invocation"
        );
    }
    assert!(
        facts
            .iter()
            .any(|f| f["type"] == "turn.complete" && f["payload"]["terminal"] == "verified"),
        "mutating run did not finish with a bound verified terminal"
    );
    tui.record(
        "live-write-verify",
        "real read/write/shell receipts, exact output bytes and verified terminal",
    );
    tui.quit();
    tui.finish();
}

#[cfg(any(windows, unix))]
pub(super) fn interrupt() {
    let mut tui = LiveTui::new("interrupt");
    tui.launch(&[]);
    tui.wait_for("Aster", 45);
    tui.command("/yolo");
    tui.wait_for("yolo", 8);
    let before = tui.facts().len();
    let prompt = if cfg!(windows) {
        "Use the bash tool to run Start-Sleep -Seconds 6; Set-Content -LiteralPath late.txt -Value LATE -NoNewline. Do not run another command. This test will interrupt that operation."
    } else {
        "Use the bash tool to run sleep 6; printf LATE > late.txt. Do not run another command. This test will interrupt that operation."
    };
    tui.command(prompt);
    let prepared = tui.wait_fact("tool.prepared", before);
    assert_eq!(prepared["payload"]["prepared"]["tool_name"], "bash");
    tui.pump(500);
    tui.bytes(b"\x03");
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let screen = tui.pump(100);
        if !screen.contains("[stop]") && !screen.contains("Ctrl+c:cancel") {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "cancelled tool did not settle\n{screen}"
        );
    }
    tui.pump(8000);
    assert!(
        !tui.workspace.join("late.txt").exists(),
        "cancelled tool wrote after interruption"
    );
    tui.record(
        "interrupt",
        "Ctrl+C settles an admitted bash call; no delayed write after eight seconds",
    );
    tui.quit();
    tui.finish();
}

pub(super) fn goals() {
    let mut tui = LiveTui::new("goal");
    tui.launch(&[]);
    tui.wait_for("Aster", 45);
    let objective = "Wait for the user to provide an approval token in their next message. Do not call tools or finish the goal; ask them to provide the token.";
    let before = tui.facts().len();
    tui.command(&format!("/goal {objective}"));
    let created = tui.wait_fact("a3s.goal.created", before);
    assert!(created.to_string().contains(objective));
    tui.record(
        "goal-create",
        "actual Core goal.created fact preserves the objective",
    );
    for (command, fact) in [
        ("/goal pause", "a3s.goal.paused"),
        ("/goal resume", "a3s.goal.resumed"),
        ("/goal clear", "a3s.goal.cancelled"),
    ] {
        let before = tui.facts().len();
        tui.command(command);
        tui.wait_fact(fact, before);
        tui.record(command, fact);
    }
    assert!(tui.facts().iter().any(|f| f["type"] == "a3s.goal.created"));
    tui.quit();
    tui.finish();
}

pub(super) fn expand() {
    let mut tui = LiveTui::new("expand");
    let token = format!("EXPANDED-{}", uuid::Uuid::new_v4());
    let mut body = (0..160)
        .map(|n| format!("fixture row {n:03}\n"))
        .collect::<String>();
    body.push_str(&token);
    std::fs::write(tui.workspace.join("long.txt"), body).unwrap();
    tui.launch(&[]);
    tui.wait_for("Aster", 45);
    let facts = tui.turn("Use the read tool once to read long.txt. Reply only 'Read completed'. Do not echo the file contents and do not modify any file.", None);
    assert!(!tui.assistant_text(&facts).contains(&token));
    tui.command("/minimal");
    tui.wait_for("Switched to minimal mode", 10);
    assert!(
        !tui.screen().contains(&token),
        "tail was already expanded before /expand"
    );
    tui.command("/expand");
    tui.wait_for(&token, 15);
    tui.record(
        "expand-content",
        "unseen last tool-output line appears only after /expand",
    );
    tui.command("/fullscreen");
    tui.wait_for("Switched to fullscreen mode", 10);
    tui.quit();
    tui.finish();
}

#[cfg(windows)]
pub(super) fn coding() {
    let mut tui = LiveTui::new("coding");
    let source = "function Get-Total($a, $b) { return $a - $b }\nfunction Get-Tax($amount) { return $amount + 3 }\n";
    let token = format!("CHECKED-{}", uuid::Uuid::new_v4());
    let checker = format!(". \"$PSScriptRoot/calc.ps1\"\nif ((Get-Total 7 5) -ne 12 -or (Get-Tax 4) -ne 8) {{ throw 'CHECK-FAILED' }}\nWrite-Output '{token}'\n");
    std::fs::write(tui.workspace.join("calc.ps1"), source).unwrap();
    std::fs::write(tui.workspace.join("check.ps1"), &checker).unwrap();
    tui.launch(&[]);
    tui.wait_for("Aster", 45);
    tui.command("/yolo");
    tui.wait_for("yolo", 8);
    let diagnosis = tui.turn("Read calc.ps1 and check.ps1. Explain both defects. Do not edit any file or run shell commands yet.", None);
    let text = tui.assistant_text(&diagnosis);
    assert!(text.contains("Get-Total") && text.contains("Get-Tax"));
    assert_eq!(
        std::fs::read_to_string(tui.workspace.join("calc.ps1")).unwrap(),
        source
    );
    tui.record(
        "coding-diagnose",
        "both functions diagnosed, source bytes unchanged",
    );
    let repaired = tui.turn("Use the edit tool to fix both diagnosed defects in calc.ps1. Keep check.ps1 unchanged. Run ./check.ps1 and quote its output. Then run Get-Item -LiteralPath calc.ps1 -ErrorAction Stop to verify the changed file. Do not modify any other file.", None);
    assert!(repaired
        .iter()
        .any(|f| f["type"] == "tool.prepared" && f["payload"]["prepared"]["tool_name"] == "edit"));
    assert!(repaired
        .iter()
        .any(|f| f["type"] == "turn.complete" && f["payload"]["terminal"] == "verified"));
    assert!(tui.assistant_text(&repaired).contains(&token));
    assert_eq!(
        std::fs::read_to_string(tui.workspace.join("check.ps1")).unwrap(),
        checker
    );
    let shell = std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap())
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let checked = std::process::Command::new(shell)
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(tui.workspace.join("check.ps1"))
        .current_dir(&tui.workspace)
        .output()
        .unwrap();
    assert!(checked.status.success(), "independent checker failed");
    assert_eq!(String::from_utf8(checked.stdout).unwrap().trim(), token);
    tui.record(
        "coding-repair",
        "real edit receipt, verified terminal, unchanged checker and independent passing execution",
    );
    tui.quit();
    tui.finish();
}

#[cfg(unix)]
pub(super) fn coding() {
    let mut tui = LiveTui::new("coding");
    let source = "get_total() { printf '%s\\n' \"$(($1 - $2))\"; }\nget_tax() { printf '%s\\n' \"$(($1 + 3))\"; }\n";
    let token = format!("CHECKED-{}", uuid::Uuid::new_v4());
    let checker = format!(". \"$(dirname \"$0\")/calc.sh\"\nif [ \"$(get_total 7 5)\" != 12 ] || [ \"$(get_tax 4)\" != 8 ]; then\n  printf '%s\\n' CHECK-FAILED >&2\n  exit 1\nfi\nprintf '%s\\n' '{token}'\n");
    std::fs::write(tui.workspace.join("calc.sh"), source).unwrap();
    std::fs::write(tui.workspace.join("check.sh"), &checker).unwrap();
    tui.launch(&[]);
    tui.wait_for("Aster", 45);
    tui.command("/yolo");
    tui.wait_for("yolo", 8);
    let diagnosis = tui.turn("Read calc.sh and check.sh. Explain both defects. Do not edit any file or run shell commands yet.", None);
    let text = tui.assistant_text(&diagnosis);
    assert!(text.contains("get_total") && text.contains("get_tax"));
    assert_eq!(
        std::fs::read_to_string(tui.workspace.join("calc.sh")).unwrap(),
        source
    );
    tui.record(
        "coding-diagnose",
        "both functions diagnosed, source bytes unchanged",
    );
    let repaired = tui.turn("Use the edit tool to fix both diagnosed defects in calc.sh. Keep check.sh unchanged. Run /bin/sh ./check.sh and quote its output. Then run test -f calc.sh to verify the changed file. Do not modify any other file.", None);
    assert!(repaired
        .iter()
        .any(|f| f["type"] == "tool.prepared" && f["payload"]["prepared"]["tool_name"] == "edit"));
    assert!(repaired
        .iter()
        .any(|f| f["type"] == "turn.complete" && f["payload"]["terminal"] == "verified"));
    assert!(tui.assistant_text(&repaired).contains(&token));
    assert_eq!(
        std::fs::read_to_string(tui.workspace.join("check.sh")).unwrap(),
        checker
    );
    let checked = std::process::Command::new("/bin/sh")
        .arg(tui.workspace.join("check.sh"))
        .current_dir(&tui.workspace)
        .output()
        .unwrap();
    assert!(checked.status.success(), "independent checker failed");
    assert_eq!(String::from_utf8(checked.stdout).unwrap().trim(), token);
    tui.record(
        "coding-repair",
        "real edit receipt, verified terminal, unchanged checker and independent passing execution",
    );
    tui.quit();
    tui.finish();
}
