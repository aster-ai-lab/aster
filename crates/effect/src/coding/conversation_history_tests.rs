#![cfg(test)]

use super::*;
use serde_json::json;

fn fact(seq: u64, kind: &str, payload: Value) -> Fact {
    Fact {
        seq,
        kind: kind.into(),
        key: format!("history:{seq}"),
        cause: None,
        payload,
    }
}
fn fold(facts: &[Fact]) -> SchedulerState {
    let config = HarnessConfig::new(16, 1_000_000, 8, 1, vec![], vec![]).unwrap();
    facts.iter().fold(SchedulerState::new(), |state, fact| {
        step(state, fact, &config)
    })
}

#[test]
fn ordinary_assistant_reply_survives_the_next_user_turn() {
    let facts = [
        fact(0, "user.message", json!({"text":"first request"})),
        fact(
            1,
            "model.turn",
            json!({"kind":"text","text":"first response"}),
        ),
        fact(2, "user.message", json!({"text":"next request"})),
    ];
    let state = fold(&facts);
    assert_eq!(
        state.messages,
        [
            "user\nfirst request",
            "assistant\nfirst response",
            "user\nnext request"
        ]
    );
    assert_eq!(state.assistant, None, "the current answer must be fresh");
    assert_eq!(state.phase, CodingPhase::Infer);
    assert_eq!(
        fold(&facts).messages,
        state.messages,
        "immutable replay agrees"
    );
}

#[test]
fn model_question_survives_the_original_answer_fact() {
    let facts = [
        fact(0, "user.message", json!({"text":"choose a target"})),
        fact(
            1,
            "model.turn",
            json!({"kind":"question","question_id":"q1","question":"Which target?","allow_free_text":true,"options":[]}),
        ),
        fact(2, "question.answered", json!({"text":"staging"})),
    ];
    assert_eq!(
        fold(&facts).messages,
        [
            "user\nchoose a target",
            "assistant\nWhich target?",
            "user\nstaging"
        ]
    );
}

#[test]
fn compaction_prefix_contains_the_actual_assistant_fact() {
    let facts = [
        fact(0, "user.message", json!({"text":"inspect the source"})),
        fact(
            1,
            "model.turn",
            json!({"kind":"text","text":"observed answer 7319"}),
        ),
        fact(2, "user.message", json!({"text":"continue"})),
    ];
    let state = fold(&facts);
    let (prefix, kept) = split_compact_tail(&state.messages, 1);
    assert!(prefix
        .iter()
        .any(|line| line == "assistant\nobserved answer 7319"));
    assert_eq!(kept, ["user\ncontinue"]);
}

#[test]
fn compact_tail_keeps_a_closed_tool_batch_together() {
    let messages = vec![
        "user\nolder".to_string(),
        "assistant\ntool read\n{\"file_path\":\"a.rs\"}".to_string(),
        "assistant\ntool read\n{\"file_path\":\"b.rs\"}".to_string(),
        "tool\na".to_string(),
        "tool-error\nb".to_string(),
        "user\nnext".to_string(),
    ];
    let batch: usize = messages[1..5]
        .iter()
        .map(|line| estimate_tokens(line))
        .sum();
    let next = estimate_tokens(&messages[5]);
    let (prefix, kept) = split_compact_tail(&messages, batch + next);
    assert_eq!(prefix, vec!["user\nolder".to_string()]);
    assert_eq!(kept, messages[1..].to_vec());
}

#[test]
fn compact_tail_drops_a_closed_batch_that_exceeds_the_budget() {
    let messages = vec![
        "assistant\ntool read\n{\"file_path\":\"a.rs\"}".to_string(),
        "assistant\ntool read\n{\"file_path\":\"b.rs\"}".to_string(),
        "tool\na".to_string(),
        "tool\nb".to_string(),
        "user\nnext".to_string(),
    ];
    let next = estimate_tokens(&messages[4]);
    let (prefix, kept) = split_compact_tail(&messages, next);
    assert_eq!(kept, vec!["user\nnext".to_string()]);
    assert_eq!(prefix, messages[..4].to_vec());
}

#[test]
fn compact_tail_keeps_one_adjacent_pair_and_drops_the_earlier_pair() {
    let earlier = [
        "assistant\ntool read\n{\"file_path\":\"a.rs\"}".to_string(),
        "tool\na".to_string(),
    ];
    let later = [
        "assistant\ntool read\n{\"file_path\":\"b.rs\"}".to_string(),
        "tool\nb".to_string(),
    ];
    let messages = [earlier.clone(), later.clone()].concat();
    let later_tokens: usize = later.iter().map(|line| estimate_tokens(line)).sum();
    let (prefix, kept) = split_compact_tail(&messages, later_tokens);
    assert_eq!(prefix, earlier);
    assert_eq!(kept, later);
}

#[test]
fn compact_tail_uses_remaining_budget_for_public_prose_across_a_large_tool_result() {
    let checkpoint = "assistant\nObserved opaque value ξ-7a93; verification remains pending.";
    let messages = vec![
        "user\nPreserve the original observations.".to_string(),
        checkpoint.to_string(),
        "assistant\ntool read\n{\"file_path\":\"earlier.txt\"}".to_string(),
        format!("tool\n{}", "large original observation\n".repeat(1_000)),
        "assistant\nInspect the remaining requirement.".to_string(),
        "assistant\ntool read\n{\"file_path\":\"requirements.txt\"}".to_string(),
        "tool\nOriginal requirements.".to_string(),
    ];
    let keep_tokens = messages[4..]
        .iter()
        .map(|line| estimate_tokens(line))
        .sum::<usize>()
        + estimate_tokens(checkpoint);
    let original = messages.clone();
    let (prefix, kept) = split_compact_tail(&messages, keep_tokens);
    assert!(kept.iter().any(|line| line == checkpoint));
    assert_eq!(kept, [&messages[1..2], &messages[4..]].concat());
    assert_eq!(prefix, [&messages[..1], &messages[2..4]].concat());
    assert!(kept.iter().map(|line| estimate_tokens(line)).sum::<usize>() <= keep_tokens);
    assert_eq!(messages, original);

    let (prefix, kept) = split_compact_tail(&messages, keep_tokens - estimate_tokens(checkpoint));
    assert_eq!(kept, messages[4..]);
    assert_eq!(prefix, messages[..4]);
}

#[test]
fn compact_tail_keeps_a_short_result_run_together() {
    let region = [
        "assistant\ntool read\n{\"file_path\":\"a.rs\"}".to_string(),
        "assistant\ntool read\n{\"file_path\":\"b.rs\"}".to_string(),
        "tool\nonly".to_string(),
    ];
    let mut messages = vec!["user\nolder".to_string()];
    messages.extend(region.iter().cloned());
    messages.push("user\nnext".to_string());
    let region_tokens: usize = region.iter().map(|line| estimate_tokens(line)).sum();
    let next = estimate_tokens(messages.last().unwrap());
    let (prefix, kept) = split_compact_tail(&messages, region_tokens + next);
    assert_eq!(prefix, vec!["user\nolder".to_string()]);
    assert_eq!(kept, messages[1..].to_vec());

    let (small_prefix, small_kept) = split_compact_tail(&messages, next);
    assert_eq!(small_kept, vec!["user\nnext".to_string()]);
    assert_eq!(small_prefix, messages[..messages.len() - 1].to_vec());
    assert!(!small_kept
        .iter()
        .any(|line| line.starts_with("assistant\ntool ")));
}

#[test]
fn compact_tail_does_not_keep_a_record_with_another_calls_result() {
    let messages = vec![
        "assistant\ntool read\n{\"file_path\":\"a.rs\"}".to_string(),
        "assistant\ntool read\n{\"file_path\":\"b.rs\"}".to_string(),
        "tool\na".to_string(),
        "tool-error\nb".to_string(),
    ];
    let (prefix, kept) = split_compact_tail(&messages, 0);
    assert_eq!(kept, vec!["tool-error\nb".to_string()]);
    assert!(prefix
        .iter()
        .any(|line| line.starts_with("assistant\ntool read")));
}

#[test]
fn tool_turn_public_progress_reaches_compaction_once() {
    let call = |id: &str, text: Option<&str>| ToolCall {
        id: id.into(),
        name: "read".into(),
        args: json!({"file_path": format!("{id}.txt")}),
        needs_confirmation: false,
        text: text.map(str::to_owned),
        reasoning: Some("synthetic-private-reasoning".into()),
    };
    let decision = ModelDecision::Tool {
        call: call("first", Some("Observed copper=83, lavender=17.")),
        also: vec![
            call("second", Some("Observed copper=83, lavender=17.")),
            call("third", Some("Next inspect the remaining source.")),
            call("fourth", Some("")),
            call("fifth", None),
        ],
    };
    let state = fold(&[fact(
        0,
        "model.turn",
        serde_json::to_value(decision).unwrap(),
    )]);
    assert!(state.schema_error.is_none());
    assert_eq!(state.phase, CodingPhase::Tool);
    let (prefix, _) = split_compact_tail(&state.messages, 0);
    let transcript = prefix.join("\n");
    assert_eq!(
        transcript
            .matches("Observed copper=83, lavender=17.")
            .count(),
        1
    );
    assert_eq!(
        transcript
            .matches("Next inspect the remaining source.")
            .count(),
        1
    );
    assert!(!transcript.contains("synthetic-private-reasoning"));
    let start = state
        .messages
        .iter()
        .position(|line| line.starts_with("assistant\ntool "))
        .unwrap();
    assert!(state.messages[start..]
        .iter()
        .all(|line| line.starts_with("assistant\ntool ")));
    assert_eq!(state.messages[start..].len(), 5);
}
