use super::*;

#[test]
fn progress_requires_comparable_byte_or_object_identity() {
    let file = ToolFileIdentityV1 {
        kind: ToolFileKind::File,
        content_digest: Some(format!("sha256:{}", "a".repeat(64))),
        mode: None,
    };
    let mut change = ToolPathChangeV1 {
        path: "state.txt".into(),
        before: None,
        after: file.clone(),
    };
    assert!(
        !known_identity_changed(&change),
        "no invented before identity"
    );
    change.before = Some(file.clone());
    change.after.mode = Some(0o644);
    assert!(
        !known_identity_changed(&change),
        "new metadata is not a changed mode"
    );
    change.after.content_digest = Some(format!("sha256:{}", "b".repeat(64)));
    assert!(known_identity_changed(&change));
    change.after = file.clone();
    change.before.as_mut().unwrap().mode = Some(0o600);
    change.after.mode = Some(0o644);
    assert!(known_identity_changed(&change));
    change.after = ToolFileIdentityV1 {
        kind: ToolFileKind::Missing,
        content_digest: None,
        mode: None,
    };
    assert!(known_identity_changed(&change));
    change.before = Some(change.after.clone());
    change.after.mode = Some(0o755);
    assert!(
        !known_identity_changed(&change),
        "missing objects have no mode transition"
    );
}

#[test]
fn a_new_user_message_starts_a_fresh_duplicate_window() {
    let config = HarnessConfig::new(16, usize::MAX, 8, 1, vec![], vec![]).unwrap();
    let mut state = SchedulerState::new();
    state.messages = vec!["assistant\ntool read\n{\"file_path\":\"recovery.txt\"}".into()];
    state.duplicate_calls.insert(
        (
            "read".into(),
            tool_args_digest(&json!({"file_path":"recovery.txt"})).unwrap(),
        ),
        3,
    );
    state.duplicate_reset_pending = true;
    let fact = Fact {
        seq: 1,
        kind: "user.message".into(),
        key: "again".into(),
        cause: None,
        payload: json!({"text":"read recovery.txt again"}),
    };
    let state = step(state, &fact, &config);
    assert!(state.duplicate_calls.is_empty());
    assert!(!state.duplicate_reset_pending);
    assert!(state
        .messages
        .last()
        .is_some_and(|line| line == "user\nread recovery.txt again"));
    assert!(duplicate_nudge(
        &state.duplicate_calls,
        &ToolCall {
            id: "read-again".into(),
            name: "read".into(),
            args: json!({"file_path":"recovery.txt"}),
            needs_confirmation: false,
            text: None,
            reasoning: None,
        },
        Some(3),
    )
    .unwrap()
    .is_none());
}

#[test]
fn compaction_preserves_original_duplicate_counts_for_every_retained_suffix() {
    let config = HarnessConfig::new(16, usize::MAX, 8, 1, vec![], vec![]).unwrap();
    for kept in [vec!["c", "d"], vec!["unrelated"], vec![]] {
        let mut state = SchedulerState::new();
        state.messages = ["a", "b", "c", "d"].map(str::to_owned).to_vec();
        state
            .duplicate_calls
            .insert(("read".into(), tool_args_digest(&json!({})).unwrap()), 3);
        let original = state.duplicate_calls.clone();
        let fact = Fact {
            seq: 1,
            kind: "compaction.done".into(),
            key: "compact".into(),
            cause: None,
            payload: json!({"summary":"controlled summary","kept":kept}),
        };
        let state = step(state, &fact, &config);
        assert_eq!(state.duplicate_calls, original);
    }
}

#[test]
fn progress_waits_for_the_whole_model_tool_batch() {
    let mut state = SchedulerState::new();
    state.messages = vec!["assistant\ntool verify\n{}".into()];
    state
        .duplicate_calls
        .insert(("verify".into(), tool_args_digest(&json!({})).unwrap()), 3);
    state.duplicate_reset_pending = true;
    state.queued.push(ToolCall {
        id: "later-ask".into(),
        name: "verify".into(),
        args: json!({}),
        needs_confirmation: true,
        text: None,
        reasoning: None,
    });
    let state = promote_queued(state);
    assert_eq!(state.phase, CodingPhase::Confirm);
    assert_eq!(state.duplicate_calls.len(), 1);
    assert!(state.duplicate_reset_pending);
    let state = step(
        state,
        &Fact {
            seq: 1,
            kind: "confirmation.answered".into(),
            key: "last-answer".into(),
            cause: None,
            payload: json!({"tool_call_id":"later-ask","approved":false}),
        },
        &HarnessConfig::new(16, usize::MAX, 8, 1, vec![], vec![]).unwrap(),
    );
    assert_eq!(state.phase, CodingPhase::Done);
    assert!(state.duplicate_calls.is_empty());
    assert!(!state.duplicate_reset_pending);
}

#[test]
fn novel_information_clears_prior_duplicate_counts_while_repeats_preserve_them() {
    let config = HarnessConfig::new(16, usize::MAX, 8, 1, vec![], vec![])
        .unwrap()
        .with_duplicate_threshold(3);
    let args_a = json!({"file_path":"a.txt"});
    let args_b = json!({"file_path":"b.txt"});
    let args_c = json!({"file_path":"c.txt"});
    let digest_a = tool_args_digest(&args_a).unwrap();
    let digest_b = tool_args_digest(&args_b).unwrap();
    let digest_c = tool_args_digest(&args_c).unwrap();
    let info_a = format!("sha256:{}", "a".repeat(64));
    let info_b = format!("sha256:{}", "b".repeat(64));
    let info_c = format!("sha256:{}", "c".repeat(64));

    let make_step_receipt =
        |state: &mut SchedulerState, id: &str, args: &Value, output: &str, info: &str| {
            let digest = tool_args_digest(args).unwrap();
            let identity = ToolInvocationIdentityV1 {
                turn: state.turn,
                cycle: state.cycle,
                tool_call_id: id.into(),
                batch_index: 0,
                admission: None,
            };
            let prepared = ToolPreparedV1 {
                schema: TOOL_PREPARED_SCHEMA.into(),
                identity: identity.clone(),
                tool_name: "read".into(),
                original_args_digest: digest.clone(),
                effective_args_digest: digest,
                recovery: ToolRecoveryPolicy::NeverReplay,
                baseline: None,
            };
            let receipt = ToolExecutionResultV1 {
                schema: TOOL_RESULT_SCHEMA.into(),
                identity,
                prepared_digest: prepared.digest().unwrap(),
                outcome: ToolExecutionOutcome::Succeeded,
                exit_code: Some(0),
                output: json!(output),
                effects: ToolEffectObservationV1::complete(Vec::new()),
            };
            state.phase = CodingPhase::Tool;
            state.pending_call = Some(ToolCall {
                id: id.into(),
                name: "read".into(),
                args: args.clone(),
                needs_confirmation: false,
                text: None,
                reasoning: None,
            });
            state.preparations.push(prepared);
            Fact {
                seq: 1,
                kind: "tool.result".into(),
                key: format!("res-{id}"),
                cause: None,
                payload: json!({
                    "toolCallId": id,
                    "ok": true,
                    "output": "",
                    "receipt": receipt,
                    "informationIdentity": info,
                }),
            }
        };

    let mut state = SchedulerState::new();
    state.turn = 1;
    state
        .duplicate_calls
        .insert(("read".into(), digest_a.clone()), 3);
    state
        .duplicate_calls
        .insert(("read".into(), digest_b.clone()), 2);
    state.seen_information.insert(info_a);
    state.seen_information.insert(info_b.clone());

    let fact_c = make_step_receipt(&mut state, "call-c", &args_c, "content c", &info_c);
    let mut state = step(state, &fact_c, &config);
    assert_eq!(state.schema_error, None);
    assert_eq!(
        state
            .duplicate_calls
            .get(&("read".into(), digest_c.clone())),
        Some(&1)
    );
    assert_eq!(
        state
            .duplicate_calls
            .get(&("read".into(), digest_a.clone())),
        Some(&3),
        "already-exhausted duplicate action must remain blocked until workspace mutation or fresh self-observation"
    );
    assert!(
        !state
            .duplicate_calls
            .contains_key(&("read".into(), digest_b.clone())),
        "non-exhausted duplicate count must clear on novel information"
    );

    let fact_b = make_step_receipt(&mut state, "call-b-again", &args_b, "content b", &info_b);
    let mut state = step(state, &fact_b, &config);
    assert_eq!(state.schema_error, None);
    assert_eq!(
        state
            .duplicate_calls
            .get(&("read".into(), digest_b.clone())),
        Some(&1)
    );
    assert_eq!(
        state
            .duplicate_calls
            .get(&("read".into(), digest_c.clone())),
        Some(&1),
        "repeat with already-seen information must not clear other non-exhausted counts"
    );

    let fact_c2 = make_step_receipt(&mut state, "call-c-again", &args_c, "content c", &info_c);
    let state = step(state, &fact_c2, &config);
    assert_eq!(state.schema_error, None);
    assert_eq!(
        state.duplicate_calls.get(&("read".into(), digest_c)),
        Some(&2)
    );
    assert_eq!(
        state.duplicate_calls.get(&("read".into(), digest_b)),
        Some(&1)
    );
    assert_eq!(
        state.duplicate_calls.get(&("read".into(), digest_a)),
        Some(&3)
    );
}

#[test]
fn question_answered_and_background_output_reset_relevant_duplicate_counts() {
    let config = HarnessConfig::new(16, usize::MAX, 8, 1, vec![], vec![])
        .unwrap()
        .with_duplicate_threshold(3);
    let bg_args = json!({ "action": "tail", "name": "dev" });
    let read_args = json!({ "file_path": "a.txt" });
    let bg_digest = tool_args_digest(&bg_args).unwrap();
    let read_digest = tool_args_digest(&read_args).unwrap();

    let mut state = SchedulerState::new();
    state.turn = 1;
    state
        .duplicate_calls
        .insert(("background".into(), bg_digest.clone()), 3);
    state
        .duplicate_calls
        .insert(("read".into(), read_digest.clone()), 2);

    let bg_fact = Fact {
        seq: 1,
        kind: "background.output".into(),
        key: "bg:task-1:1".into(),
        cause: None,
        payload: json!({
            "name": "dev",
            "line": "listening on 3000"
        }),
    };
    let state = step(state, &bg_fact, &config);
    assert_eq!(state.schema_error, None);
    assert!(
        !state
            .duplicate_calls
            .contains_key(&("background".into(), bg_digest)),
        "live background.output must clear background polling duplicate counts"
    );
    assert_eq!(
        state.duplicate_calls.get(&("read".into(), read_digest)),
        Some(&2),
        "background output must preserve unrelated tool duplicate counts"
    );

    let answer_fact = Fact {
        seq: 2,
        kind: "question.answered".into(),
        key: "qa:1".into(),
        cause: None,
        payload: json!({ "text": "use staging target" }),
    };
    let state = step(state, &answer_fact, &config);
    assert_eq!(state.schema_error, None);
    assert!(
        state.duplicate_calls.is_empty(),
        "explicit host answer via ask_user must clear duplicate counts like user.message"
    );
}
