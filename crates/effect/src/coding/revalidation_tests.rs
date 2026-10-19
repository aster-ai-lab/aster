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
fn compaction_translates_only_a_known_retained_suffix() {
    let config = HarnessConfig::new(16, usize::MAX, 8, 1, vec![], vec![]).unwrap();
    for (kept, expected) in [(vec!["c", "d"], 1), (vec!["unrelated"], 0), (vec![], 0)] {
        let mut state = SchedulerState::new();
        state.messages = ["a", "b", "c", "d"].map(str::to_owned).to_vec();
        state.duplicate_start = 3;
        let fact = Fact {
            seq: 1,
            kind: "compaction.done".into(),
            key: "compact".into(),
            cause: None,
            payload: json!({"summary":"controlled summary","kept":kept}),
        };
        let state = step(state, &fact, &config);
        assert_eq!(state.duplicate_start, expected);
    }
}

#[test]
fn progress_waits_for_the_whole_model_tool_batch() {
    let mut state = SchedulerState::new();
    state.messages = vec!["assistant\ntool verify\n{}".into()];
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
    assert_eq!(state.duplicate_start, 0);
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
    assert_eq!(state.duplicate_start, state.messages.len());
    assert!(!state.duplicate_reset_pending);
}
