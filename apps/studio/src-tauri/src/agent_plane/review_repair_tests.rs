#![cfg(test)]

use super::tests::{harness, message, until_done};
use super::*;

#[tokio::test(flavor = "multi_thread")]
async fn a_clean_review_keeps_its_source_for_a_later_turn_without_replaying_the_text() {
    let h = harness("review-clean-memory", "default").expect("node fixture");
    super::super::reference_tasks_live_tests::publish_reference_review(&h.dir);
    let text = "项目已经完成，资料已归档。";
    let mut body = message("只做基础校核。review-clean-memory");
    body["turnContext"] =
        json!({"agentId":"legal-expert","reviewSource":{"kind":"text","text":text}});
    let mut events = h.hub.subscribe();
    h.turns.open(&h.session_id, &body).unwrap();
    until_done(&mut events, |_| {}).await;
    let memory = h.turns.load_review_memory(&h.session_id).unwrap();
    assert_eq!(memory.findings, "[]");
    h.turns.lock_review().clear();
    let mut events = h.hub.subscribe();
    h.turns
        .open(&h.session_id, &message("还有需要修改的吗？"))
        .unwrap();
    until_done(&mut events, |_| {}).await;
    let trace: Value =
        serde_json::from_slice(&std::fs::read(h.dir.join("turn-trace.json")).unwrap()).unwrap();
    let prompt = trace["prompts"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .as_str()
        .unwrap();
    assert!(prompt.contains("Findings:\n[]"));
    assert!(prompt.contains(memory.text_revision.as_deref().unwrap()));
    assert!(!prompt.contains(text));
    h.turns.close_test_runtimes().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn invalid_review_gets_one_visible_correction_and_only_grounded_findings_are_published() {
    for (scenario, expected) in [
        ("review-repair-format", "success"),
        ("review-repair-quote", "success"),
        ("review-repair-repeat", "error"),
    ] {
        let h = harness(scenario, "default").expect("node fixture");
        super::super::reference_tasks_live_tests::publish_reference_review(&h.dir);
        let text = "我们以经完成测试，请按排验收。";
        let mut body = message(&format!("只做基础校核。{scenario}"));
        body["turnContext"] =
            json!({"agentId":"legal-expert","reviewSource":{"kind":"text","text":text}});
        let mut events = h.hub.subscribe();
        h.turns.open(&h.session_id, &body).unwrap();
        let seen = until_done(&mut events, |_| {}).await;
        if expected == "success" {
            assert_eq!(seen.last().unwrap()["payload"]["status"], "success");
        } else {
            assert_eq!(seen.last().unwrap()["channel"], "ai.stream.error");
        }
        let retries: Vec<_> = seen
            .iter()
            .filter(|event| event["payload"]["chunk"]["type"] == "data-retry")
            .collect();
        assert_eq!(retries.len(), 2);
        assert_eq!(retries[0]["payload"]["chunk"]["data"]["state"], "retrying");
        assert_eq!(retries[0]["payload"]["chunk"]["data"]["attempt"], 2);
        assert_eq!(retries[1]["payload"]["chunk"]["data"]["state"], "settled");
        let rows = h.store.messages(&h.session_id);
        let answer = rows.iter().find(|row| row.role == "assistant").unwrap();
        assert!(!answer.searchable_text.contains("不在原文中的句子"));
        assert!(!answer.searchable_text.contains("解释不能混入结果"));
        assert!(
            !answer
                .parts()
                .iter()
                .any(|part| part["type"] == "data-retry")
        );
        if expected == "success" {
            assert!(accepted_finding_fences(&answer.searchable_text, text).is_ok());
            assert!(answer.searchable_text.contains("我们以经完成测试"));
        } else {
            assert_eq!(answer.status, "error");
            assert!(!answer.searchable_text.contains("a3s-review-finding"));
            assert_ne!(answer.searchable_text.trim(), "[]");
            assert!(
                h.turns
                    .load_review_memory(&h.session_id)
                    .unwrap()
                    .findings
                    .is_empty()
            );
        }
        let trace: Value =
            serde_json::from_slice(&std::fs::read(h.dir.join("turn-trace.json")).unwrap()).unwrap();
        let prompts = trace["prompts"].as_array().unwrap();
        assert_eq!(prompts.len(), 2, "one correction only");
        assert!(
            prompts
                .iter()
                .all(|prompt| prompt.as_str().unwrap().contains(text))
        );
        assert!(
            prompts[1]
                .as_str()
                .unwrap()
                .contains("Host validation rejected")
        );
        h.turns.close_test_runtimes().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn cancelling_the_visible_review_correction_keeps_the_turn_paused() {
    let h = harness("review-repair-abort", "default").expect("node fixture");
    super::super::reference_tasks_live_tests::publish_reference_review(&h.dir);
    let mut body = message("只做基础校核。review-repair-abort");
    body["turnContext"] = json!({"agentId":"legal-expert","reviewSource":{"kind":"text","text":"我们以经完成测试。"}});
    let mut events = h.hub.subscribe();
    h.turns.open(&h.session_id, &body).unwrap();
    let seen = until_done(&mut events, |event| {
        if event["payload"]["chunk"]["type"] == "data-retry"
            && event["payload"]["chunk"]["data"]["state"] == "retrying"
        {
            h.turns.abort(&h.session_id);
        }
    })
    .await;
    assert_eq!(seen.last().unwrap()["payload"]["status"], "paused");
    assert!(
        !h.store
            .messages(&h.session_id)
            .iter()
            .any(|row| row.searchable_text.contains("a3s-review-finding"))
    );
    h.turns.close_test_runtimes().await;
}
