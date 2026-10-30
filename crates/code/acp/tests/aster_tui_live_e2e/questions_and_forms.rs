#![cfg(test)]

use super::driver::LiveTui;
use serde_json::{json, Value};

fn latest_answer(tui: &LiveTui, session: &str, expected: &str) -> String {
    let thread = a3s_code_core::fact_control::thread_for_session(session);
    let facts = a3s_code_core::fact_control::read_workspace_facts(&tui.workspace, &thread).unwrap();
    let answered = facts
        .iter()
        .rev()
        .find(|fact| fact.kind == "question.answered")
        .unwrap();
    let answer: Value = serde_json::from_str(answered.payload["text"].as_str().unwrap()).unwrap();
    assert_eq!(answer["schema"], "a3s.code.user-answer.v1");
    assert_eq!(answer["status"], "answered");
    assert_eq!(answer["answer"], expected);
    assert_eq!(answer["permission_grant"], false);
    assert_eq!(answer["wrote_files"], false);
    let id = answer["question_id"].as_str().unwrap();
    assert!(facts.iter().any(|fact| {
        fact.kind == "model.turn"
            && fact.payload["kind"] == "question"
            && fact.payload["question_id"].as_str() == Some(id)
    }));
    id.to_owned()
}

#[test]
#[ignore = "requires the frozen native Aster/ACP pair and original kimi/k3.1-pro ACL"]
fn kimi_ask_user_choices_and_unseen_free_text_match_the_real_question() {
    let mut tui = LiveTui::new("question-choice-free-text");
    tui.launch(&[]);
    tui.wait_for("Aster", 45);
    let session = tui.session_id();
    tui.dismiss();
    let prompt = "Call ask_user exactly once with question 'Choose a route', options ['ALPHA-ROUTE','BETA-ROUTE'] and allow_free_text=false. After the user answers, reply with the exact selected answer. Do not call other tools or modify files.";
    let before = tui.facts().len();
    tui.command(prompt);
    let card = tui.wait_for("Tab:next option", 120);
    assert!(card.contains("ALPHA-ROUTE") && card.contains("BETA-ROUTE"));
    tui.bytes(b"ignoredletters");
    assert!(!tui.pump(200).contains("ignoredletters"));
    tui.bytes(b"2");
    let facts = tui.settle_turn(before, prompt, None);
    assert!(tui.assistant_text(&facts).contains("BETA-ROUTE"));
    let first = latest_answer(&tui, &session, "BETA-ROUTE");
    tui.record("question-selected-answer", "the second real choice is persisted under its exact pending question id and reaches the next model response without granting permission");

    let prompt = "Call ask_user exactly once with question 'Enter a route note', options ['USE-DEFAULT','USE-ALTERNATE'] and allow_free_text=true. After the user answers, reply with the exact answer text. Do not call other tools or modify files.";
    let before = tui.facts().len();
    tui.command(prompt);
    tui.wait_for("Tab:next option", 120);
    let note = format!("HOST-NOTE-{}-中文", uuid::Uuid::new_v4());
    tui.bytes(b"t");
    tui.pump(150);
    tui.bytes(b"\x15");
    tui.bytes(format!("\x1b[200~{note}\x1b[201~").as_bytes());
    tui.wait_for(&note, 8);
    tui.bytes(b"\r");
    let facts = tui.settle_turn(before, prompt, None);
    assert!(tui.assistant_text(&facts).contains(&note));
    let second = latest_answer(&tui, &session, &note);
    assert_ne!(first, second);
    tui.record("question-unseen-free-text", "the host-only Unicode answer is persisted for a distinct question id and returned by the real model; the selected choice row is not substituted for the text");
    tui.checkpoint_session("question-choice-free-text");
    tui.quit();
    tui.finish();
}

#[test]
#[ignore = "requires the frozen native Aster/ACP pair and original kimi/k3.1-pro ACL"]
fn kimi_ask_user_cancel_has_no_default_answer_and_session_recovers() {
    let mut tui = LiveTui::new("question-cancel-recovery");
    tui.launch(&[]);
    tui.wait_for("Aster", 45);
    let session = tui.session_id();
    tui.dismiss();
    let prompt = "Call ask_user exactly once with question 'Cancel this question', options ['KEEP-DEFAULT','CHANGE-DEFAULT'] and allow_free_text=true. If the question is cancelled, report cancellation without retrying. Do not call other tools or modify files.";
    let before = tui.facts().len();
    tui.command(prompt);
    tui.wait_for("Tab:next option", 120);
    tui.bytes(b"\x03");
    tui.wait_fact("turn.complete", before);
    let thread = a3s_code_core::fact_control::thread_for_session(&session);
    let facts = a3s_code_core::fact_control::read_workspace_facts(&tui.workspace, &thread).unwrap();
    assert!(!facts.iter().any(|fact| fact.kind == "question.answered"));
    tui.record(
        "question-cancel-no-answer",
        "cancelling the real question ends the turn without persisting any selected/default answer",
    );
    let facts = tui.turn(
        "Do not call tools. Reply exactly QUESTION-CANCEL-RECOVERED.",
        None,
    );
    assert!(tui
        .assistant_text(&facts)
        .contains("QUESTION-CANCEL-RECOVERED"));
    assert!(!facts.iter().any(|fact| fact["type"] == "tool.prepared"));
    tui.record("question-cancel-recovery", "the same session completes a fresh real model turn after cancellation without reopening the previous question");
    tui.checkpoint_session("question-cancel-recovery");
    tui.quit();
    tui.finish();
}

#[test]
#[ignore = "requires the frozen native Aster/ACP pair and original kimi/k3.1-pro ACL"]
fn kimi_cancelled_question_returns_cancelled_without_completion_and_recovers() {
    let mut tui = LiveTui::new("question-cancel-protocol-recovery");
    let proxy = std::path::PathBuf::from(
        std::env::var_os("A3S_CODE_ACP_TRACE_PROXY").expect("transparent ACP trace proxy"),
    );
    tui.trace_acp(&proxy);
    tui.launch(&[]);
    tui.wait_for("Aster", 45);
    let session = tui.session_id();
    tui.dismiss();
    let prompt = "Call ask_user exactly once with question 'Cancel this question', options ['KEEP-DEFAULT','CHANGE-DEFAULT'] and allow_free_text=true. If the question is cancelled, report cancellation without retrying. Do not call other tools or modify files.";
    let before = tui.facts().len();
    tui.command(prompt);
    tui.wait_for("Tab:next option", 120);
    tui.bytes(b"\x03");
    tui.wait_acp_stop("cancelled");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        let screen = tui.pump(80);
        if !screen.contains("[stop]") && !screen.contains("Ctrl+c:cancel") {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "cancelled question did not become idle\n{screen}"
        );
    }
    assert!(!tui.facts()[before..]
        .iter()
        .any(|fact| fact["type"] == "turn.complete"));
    let thread = a3s_code_core::fact_control::thread_for_session(&session);
    let facts = a3s_code_core::fact_control::read_workspace_facts(&tui.workspace, &thread).unwrap();
    assert!(!facts.iter().any(|fact| fact.kind == "question.answered"));
    tui.record("question-cancel-wire", "the actual ACP prompt response is cancelled, the UI becomes idle, and neither a default answer nor a successful completion fact is fabricated");
    let facts = tui.turn(
        "Do not call tools. Reply exactly QUESTION-CANCEL-RECOVERED.",
        None,
    );
    assert!(tui
        .assistant_text(&facts)
        .contains("QUESTION-CANCEL-RECOVERED"));
    assert!(!facts.iter().any(|fact| fact["type"] == "tool.prepared"));
    tui.record("question-cancel-recovery", "the same session completes a fresh real model turn after the cancelled question without reopening it");
    tui.checkpoint_session("question-cancel-protocol-recovery");
    tui.quit();
    tui.finish();
}

pub(super) fn named_tool_output(facts: &[Value], name: &str) -> Value {
    let calls = facts
        .iter()
        .filter(|fact| {
            fact["type"] == "tool.prepared" && fact["payload"]["prepared"]["tool_name"] == name
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1, "one actual {name} call required");
    let identity = &calls[0]["payload"]["prepared"]["identity"];
    let results = facts
        .iter()
        .filter(|fact| {
            fact["type"] == "tool.result" && &fact["payload"]["receipt"]["identity"] == identity
        })
        .collect::<Vec<_>>();
    assert_eq!(results.len(), 1);
    serde_json::from_str(results[0]["payload"]["receipt"]["output"].as_str().unwrap()).unwrap()
}

#[test]
#[ignore = "requires the frozen native Aster/ACP pair and original kimi/k3.1-pro ACL"]
fn kimi_unsupported_form_documents_return_real_errors_without_blank_cards() {
    let mut tui = LiveTui::new("form-schema-refusals");
    tui.launch(&[]);
    tui.wait_for("Aster", 45);
    tui.session_id();
    tui.dismiss();
    let cases = [
        (
            "unknown-component",
            json!({"root":"root","elements":{"root":{"type":"UnsupportedWidget","props":{}}}}),
            "unsupported component",
        ),
        (
            "unknown-schema",
            json!({"version":"future","root":"root","elements":{"root":{"type":"Input","props":{"label":"Name","value":{"$bindState":"/form/name"}}}},"state":{"form":{"name":""}}}),
            "expected root/elements/state",
        ),
        (
            "cyclic-document",
            json!({"root":"root","elements":{"root":{"type":"Card","props":{"title":"Cycle"},"children":["root"]}}}),
            "cyclic",
        ),
        (
            "no-editable-fields",
            json!({"root":"root","elements":{"root":{"type":"Metric","props":{"label":"Output","value":42}}}}),
            "form contains no fields",
        ),
    ];
    for (name, form, reason) in cases {
        let bytes = serde_json::to_vec(&form).unwrap();
        let path = tui.workspace.join("form-probe.json");
        std::fs::write(&path, &bytes).unwrap();
        let facts = tui.turn("Read form-probe.json once. Call ask_user_form exactly once with the exact JSON document from that file as form and prompt 'Schema validation probe'. Do not repair or replace the document. Report the actual tool error without retrying, modifying files or calling other tools.", None);
        let result = named_tool_output(&facts, "ask_user_form");
        assert_eq!(result["is_error"], true);
        assert!(
            result["text"].as_str().unwrap().contains(reason),
            "{name}: {result}"
        );
        assert!(!tui.screen().contains("MCP “Aster” requests your input"));
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        tui.record(name, &format!("the real ask_user_form receipt refuses {name} with its explicit reason, leaves no native form card, and preserves the source bytes"));
    }
    let facts = tui.turn(
        "Do not call tools. Reply exactly FORM-SCHEMA-RECOVERED.",
        None,
    );
    assert!(tui.assistant_text(&facts).contains("FORM-SCHEMA-RECOVERED"));
    tui.record(
        "form-refusal-recovery",
        "the same session completes a fresh real model turn after all four schema refusals",
    );
    tui.checkpoint_session("form-schema-refusals");
    tui.quit();
    tui.finish();
}
