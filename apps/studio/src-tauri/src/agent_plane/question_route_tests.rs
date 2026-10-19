#![cfg(test)]

use super::tests::{harness, message, until_done};
use super::*;

#[test]
fn a_boolean_approval_never_selects_an_unknown_or_question_option() {
    assert_eq!(
        permission_outcome(&[json!({"optionId":"question-0"})], true),
        json!({"outcome":{"outcome":"cancelled"}})
    );
    assert_eq!(
        permission_outcome(&[json!({"optionId":"unknown"})], true),
        json!({"outcome":{"outcome":"cancelled"}})
    );
    assert_eq!(
        permission_outcome(&[json!({"optionId":"allow-once"})], true)["outcome"]["optionId"],
        "allow-once"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn choices_and_free_text_round_trip_without_permission_approval_even_in_bypass_mode() {
    for (name, values, expected) in [
        (
            "native-question-choice",
            json!({"choice":"question-1","answer":""}),
            "selected B.docx",
        ),
        (
            "native-question-text",
            json!({"choice":"","answer":"仅审查 B.docx"}),
            "typed: 仅审查 B.docx",
        ),
    ] {
        let h = harness(name, "bypassPermissions").expect("node fixture");
        let mut events = h.hub.subscribe();
        h.turns.open(&h.session_id, &message(name)).unwrap();
        let mut answered = false;
        let seen = until_done(&mut events, |event| {
            let chunk = &event["payload"]["chunk"];
            if chunk["type"] != "data-form" || chunk["data"]["state"] != "form-requested" {
                return;
            }
            assert_eq!(chunk["data"]["format"], "question");
            let binding: FormBinding =
                serde_json::from_value(chunk["data"]["binding"].clone()).unwrap();
            assert!(!h.turns.respond_approval(&binding.request_id, true));
            assert!(!h.turns.submit_form(binding.clone(), FormValues::new()));
            assert!(
                h.turns
                    .submit_form(binding, serde_json::from_value(values.clone()).unwrap())
            );
            answered = true;
        })
        .await;
        assert!(answered);
        assert!(
            !seen
                .iter()
                .any(|event| event["payload"]["chunk"]["type"] == "tool-approval-request")
        );
        assert_eq!(seen.last().unwrap()["payload"]["status"], "success");
        assert!(h.store.messages(&h.session_id).iter().any(
            |message| message.role == "assistant" && message.searchable_text.contains(expected)
        ));
        h.turns.close_test_runtimes().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn aborting_a_question_rejects_the_stale_answer_and_preserves_paused_status() {
    let h = harness("native-question-cancel", "default").expect("node fixture");
    let mut events = h.hub.subscribe();
    h.turns
        .open(&h.session_id, &message("native-question-choice"))
        .unwrap();
    let mut asked = None;
    let seen = until_done(&mut events, |event| {
        let chunk = &event["payload"]["chunk"];
        if chunk["type"] == "data-form" && chunk["data"]["state"] == "form-requested" {
            let binding: FormBinding =
                serde_json::from_value(chunk["data"]["binding"].clone()).unwrap();
            h.turns.abort(&h.session_id);
            assert!(!h.turns.submit_form(
                binding.clone(),
                serde_json::from_value(json!({"choice":"question-1"})).unwrap()
            ));
            asked = Some(binding);
        }
    })
    .await;
    assert!(asked.is_some());
    assert_eq!(seen.last().unwrap()["payload"]["status"], "paused");
    let rows = h.store.messages(&h.session_id);
    let card = rows
        .last()
        .unwrap()
        .parts()
        .iter()
        .find(|part| part["type"] == "data-form")
        .unwrap();
    assert_eq!(card["data"]["state"], "closed");
    assert!(card["data"].get("values").is_none());
    h.turns.close_test_runtimes().await;
}
