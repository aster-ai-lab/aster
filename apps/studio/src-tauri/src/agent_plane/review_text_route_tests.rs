#![cfg(test)]

use super::tests::{harness, message, until_done};
use super::*;
use sha2::{Digest, Sha256};

#[tokio::test(flavor = "multi_thread")]
async fn pasted_text_is_segmented_and_never_revives_the_previous_word_document() {
    let h = harness("text-review-segments", "default").expect("node fixture");
    super::super::reference_tasks_live_tests::publish_reference_review(&h.dir);
    h.turns
        .store_review_memory(&h.session_id, "/tmp/previous.docx", "old finding");
    let text = format!("{}中段{}原文尾部", "正文".repeat(4000), "内容".repeat(4000));
    let revision = format!("{:x}", Sha256::digest(text.as_bytes()));
    let task = "/review 只做基础校核，保留白名单中的 ACME。";
    let mut body = message(task);
    body["turnContext"] =
        json!({"agentId":"legal-expert","reviewSource":{"kind":"text","text":text}});
    let mut events = h.hub.subscribe();
    h.turns.open(&h.session_id, &body).unwrap();
    let seen = until_done(&mut events, |_| {}).await;
    assert_eq!(seen.last().unwrap()["payload"]["status"], "success");
    let rows = h.store.messages(&h.session_id);
    let source = rows[0]
        .parts()
        .into_iter()
        .find(|part| part["type"] == "data-review-source")
        .unwrap();
    assert_eq!(source["data"]["text"], text);
    assert_eq!(source["data"]["sha256"], revision);
    let memory = h.turns.load_review_memory(&h.session_id).unwrap();
    assert_eq!(memory.document, "");
    assert_eq!(memory.text_revision.as_deref(), Some(revision.as_str()));
    assert!(!memory.findings.is_empty());
    // A reload must retain the text identity instead of recovering an older Word path.
    h.turns.lock_review().clear();
    let mut events = h.hub.subscribe();
    h.turns
        .open(&h.session_id, &message("请解释这些建议。"))
        .unwrap();
    until_done(&mut events, |_| {}).await;
    let trace: Value =
        serde_json::from_slice(&std::fs::read(h.dir.join("turn-trace.json")).unwrap()).unwrap();
    let prompts: Vec<_> = trace["prompts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().unwrap())
        .collect();
    let segments: Vec<_> = prompts
        .iter()
        .filter(|p| p.contains("Document segment"))
        .collect();
    assert!(segments.len() > 2);
    assert!(segments.iter().any(|p| p.contains("原文尾部")));
    assert!(segments.iter().all(|p| {
        p.split_once("\n\nUser task:\n")
            .unwrap()
            .1
            .starts_with(task)
            && !p.contains(&text)
    }));
    let follow = prompts.last().unwrap();
    assert!(follow.contains(&revision));
    assert!(!follow.contains("previous.docx"));
    assert!(!follow.contains("原文尾部"));
    h.turns.close_test_runtimes().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn invalid_text_source_is_rejected_before_a_turn_or_runtime_is_created() {
    let h = harness("invalid-text-review", "default").expect("node fixture");
    for value in [
        json!({"kind":"text","text":" "}),
        json!({"kind":"text","text":"正文","task":"extra"}),
        json!({"kind":"file","text":"正文"}),
    ] {
        let mut body = message("/review 检查错字");
        body["turnContext"] = json!({"reviewSource":value});
        assert!(h.turns.open(&h.session_id, &body).is_err());
        assert!(h.store.messages(&h.session_id).is_empty());
    }
    assert!(!h.dir.join("turn-trace.json").exists());
}
