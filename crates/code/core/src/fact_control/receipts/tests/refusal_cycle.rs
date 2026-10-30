#![cfg(test)]

use super::*;

fn refusal_cycle(facts: &[Fact], session: &str, run: &str, binding: &str) -> Option<String> {
    match super::super::execution_cycle(facts, session, run, binding, 0)? {
        super::super::ExecutionCycle::Refused(tool) => Some(tool),
        _ => None,
    }
}

fn child_refusal_cycle(facts: &[Fact]) -> Option<String> {
    match super::super::child_execution_cycle(facts, 0)? {
        super::super::ExecutionCycle::Refused(tool) => Some(tool),
        _ => None,
    }
}

fn binding() -> String {
    format!("sha256:{}", "a".repeat(64))
}

fn push(facts: &mut Vec<Fact>, run: &str, path: &str, outcome: ToolExecutionOutcome, text: &str) {
    let mut prepared = preparation(&format!("refused-{}", facts.len()));
    prepared.identity.admission = Some(ToolAdmissionIdentityV1 {
        run_id: run.into(),
        thread_id: "session".into(),
        capability_binding_digest: binding(),
    });
    prepared.original_args_digest = a3s_effect::tool_args_digest(&json!({"path":path})).unwrap();
    prepared.effective_args_digest = prepared.original_args_digest.clone();
    let mut receipt = result_without_execution(&prepared, outcome, text.into()).unwrap();
    if outcome == ToolExecutionOutcome::Succeeded {
        receipt.exit_code = Some(0);
    }
    for mut fact in [prepared_fact(&prepared), result_fact(&receipt)] {
        fact.seq = facts.len() as u64 + 1;
        fact.key = format!("fixture-{}", fact.seq);
        facts.push(fact);
    }
}

#[test]
fn typed_refusals_ignore_different_call_ids_and_rendered_diagnostics() {
    let mut facts = Vec::new();
    push(
        &mut facts,
        "current",
        "a",
        ToolExecutionOutcome::Denied,
        "first diagnostic",
    );
    assert!(refusal_cycle(&facts, "session", "current", &binding()).is_none());
    push(
        &mut facts,
        "current",
        "a",
        ToolExecutionOutcome::Denied,
        "different localized diagnostic",
    );
    assert_eq!(
        refusal_cycle(&facts, "session", "current", &binding()),
        Some("bash".into())
    );
    assert!(refusal_cycle(&facts, "other-session", "current", &binding()).is_none());
    assert!(refusal_cycle(&facts, "session", "new-run", &binding()).is_none());
    assert!(refusal_cycle(
        &facts,
        "session",
        "current",
        &format!("sha256:{}", "b".repeat(64))
    )
    .is_none());
}

#[test]
fn fresh_original_information_and_new_user_input_supersede_an_earlier_cycle() {
    let mut facts = Vec::new();
    for _ in 0..2 {
        push(
            &mut facts,
            "current",
            "a",
            ToolExecutionOutcome::Denied,
            "denied",
        );
    }
    assert!(refusal_cycle(&facts, "session", "current", &binding()).is_some());
    push(
        &mut facts,
        "current",
        "b",
        ToolExecutionOutcome::Succeeded,
        "new observation",
    );
    push(
        &mut facts,
        "current",
        "a",
        ToolExecutionOutcome::Denied,
        "denied",
    );
    assert!(refusal_cycle(&facts, "session", "current", &binding()).is_none());
    push(
        &mut facts,
        "current",
        "a",
        ToolExecutionOutcome::Denied,
        "denied",
    );
    assert!(refusal_cycle(&facts, "session", "current", &binding()).is_some());
    let mut input = fact(
        "user.message",
        "new-input",
        json!({"text":"explicit host input"}),
    );
    input.seq = facts.len() as u64 + 1;
    facts.push(input);
    assert!(refusal_cycle(&facts, "session", "current", &binding()).is_none());
    push(
        &mut facts,
        "current",
        "a",
        ToolExecutionOutcome::Denied,
        "denied",
    );
    assert!(refusal_cycle(&facts, "session", "current", &binding()).is_none());
}

#[test]
fn old_information_foreign_results_and_compaction_do_not_reset_the_refusal_cycle() {
    let mut facts = Vec::new();
    push(
        &mut facts,
        "old",
        "read",
        ToolExecutionOutcome::Succeeded,
        "unchanged source",
    );
    push(
        &mut facts,
        "current",
        "a",
        ToolExecutionOutcome::Denied,
        "denied",
    );
    push(
        &mut facts,
        "current",
        "read",
        ToolExecutionOutcome::Succeeded,
        "unchanged source",
    );
    push(
        &mut facts,
        "foreign-run",
        "new",
        ToolExecutionOutcome::Succeeded,
        "new foreign result",
    );
    let mut compacted = fact(
        "compaction.done",
        "summary",
        json!({"summary":"derived history", "kept":[]}),
    );
    compacted.seq = facts.len() as u64 + 1;
    facts.push(compacted);
    push(
        &mut facts,
        "current",
        "b",
        ToolExecutionOutcome::Denied,
        "other refused action",
    );
    push(
        &mut facts,
        "current",
        "a",
        ToolExecutionOutcome::Denied,
        "denied again",
    );
    assert!(refusal_cycle(&facts, "session", "current", &binding()).is_some());
}

#[test]
fn known_workspace_changes_allow_a_new_attempt_after_a_refusal() {
    let mut facts = Vec::new();
    push(
        &mut facts,
        "old",
        "write",
        ToolExecutionOutcome::Succeeded,
        "same acknowledged output",
    );
    push(
        &mut facts,
        "current",
        "a",
        ToolExecutionOutcome::Denied,
        "denied",
    );
    push(
        &mut facts,
        "current",
        "write",
        ToolExecutionOutcome::Succeeded,
        "same acknowledged output",
    );
    let mut receipt: ToolExecutionResultV1 =
        serde_json::from_value(facts.last().unwrap().payload["receipt"].clone()).unwrap();
    receipt.effects = ToolEffectObservationV1::complete(vec![ToolPathChangeV1 {
        path: "changed.txt".into(),
        before: Some(ToolFileIdentityV1 {
            kind: ToolFileKind::Missing,
            content_digest: None,
            mode: None,
        }),
        after: ToolFileIdentityV1 {
            kind: ToolFileKind::File,
            content_digest: Some(crate::content_digest::digest_bytes("test", b"changed")),
            mode: Some(0o644),
        },
    }]);
    facts.last_mut().unwrap().payload["receipt"] = json!(receipt);
    push(
        &mut facts,
        "current",
        "a",
        ToolExecutionOutcome::Denied,
        "denied",
    );
    assert!(refusal_cycle(&facts, "session", "current", &binding()).is_none());
}

#[test]
fn unsupported_or_unprepared_receipts_cannot_manufacture_a_cycle() {
    for unsupported in [false, true] {
        let mut facts = Vec::new();
        for _ in 0..2 {
            push(
                &mut facts,
                "current",
                "a",
                ToolExecutionOutcome::Denied,
                "denied",
            );
        }
        if unsupported {
            facts.last_mut().unwrap().payload["receipt"]["schema"] = json!("unsupported");
        } else {
            facts.remove(0);
        }
        assert!(refusal_cycle(&facts, "session", "current", &binding()).is_none());
    }
}

fn push_child(facts: &mut Vec<Fact>, path: &str, outcome: ToolExecutionOutcome) {
    let mut prepared = preparation(&format!("child-{}", facts.len()));
    prepared.original_args_digest = a3s_effect::tool_args_digest(&json!({"path":path})).unwrap();
    prepared.effective_args_digest = prepared.original_args_digest.clone();
    let receipt =
        result_without_execution(&prepared, outcome, "original observation".into()).unwrap();
    for mut fact in [prepared_fact(&prepared), result_fact(&receipt)] {
        fact.seq = facts.len() as u64 + 1;
        fact.key = format!("child-fixture-{}", fact.seq);
        facts.push(fact);
    }
}

#[test]
fn owned_child_refusals_use_only_unbound_original_receipts() {
    let mut facts = Vec::new();
    push_child(&mut facts, "a", ToolExecutionOutcome::Denied);
    push(
        &mut facts,
        "foreign",
        "new",
        ToolExecutionOutcome::Succeeded,
        "foreign information",
    );
    push_child(&mut facts, "a", ToolExecutionOutcome::Denied);
    assert_eq!(child_refusal_cycle(&facts), Some("bash".into()));
    assert!(refusal_cycle(&facts, "session", "current", &binding()).is_none());
    facts.last_mut().unwrap().payload["receipt"]["schema"] = json!("unsupported");
    assert!(child_refusal_cycle(&facts).is_none());
}

#[test]
fn new_original_child_information_or_input_supersedes_refusals() {
    let mut facts = Vec::new();
    push_child(&mut facts, "a", ToolExecutionOutcome::Denied);
    push_child(&mut facts, "a", ToolExecutionOutcome::Denied);
    assert!(child_refusal_cycle(&facts).is_some());
    push_child(&mut facts, "fresh", ToolExecutionOutcome::Succeeded);
    push_child(&mut facts, "a", ToolExecutionOutcome::Denied);
    assert!(child_refusal_cycle(&facts).is_none());
    push_child(&mut facts, "a", ToolExecutionOutcome::Denied);
    assert!(child_refusal_cycle(&facts).is_some());
    let mut input = fact(
        "user.message",
        "new-input",
        json!({"text":"new host input"}),
    );
    input.seq = facts.len() as u64 + 1;
    facts.push(input);
    assert!(child_refusal_cycle(&facts).is_none());
}

fn file_state(value: &str) -> ToolFileIdentityV1 {
    ToolFileIdentityV1 {
        kind: ToolFileKind::File,
        content_digest: Some(crate::content_digest::digest_bytes(
            "state-test",
            value.as_bytes(),
        )),
        mode: Some(0o644),
    }
}

fn push_state(facts: &mut Vec<Fact>, args: &str, before: &str, after: &str) {
    push(
        facts,
        "current",
        args,
        ToolExecutionOutcome::Succeeded,
        "Source updated.",
    );
    let mut receipt: ToolExecutionResultV1 =
        serde_json::from_value(facts.last().unwrap().payload["receipt"].clone()).unwrap();
    receipt.effects = ToolEffectObservationV1::complete(vec![ToolPathChangeV1 {
        path: "source.txt".into(),
        before: Some(file_state(before)),
        after: file_state(after),
    }]);
    facts.last_mut().unwrap().payload["receipt"] = json!(receipt);
}

fn state_facts() -> Vec<Fact> {
    let mut facts = Vec::new();
    for i in 0..8 {
        let (before, after) = if i % 2 == 0 { ("A", "B") } else { ("B", "A") };
        push_state(&mut facts, after, before, after);
    }
    facts
}

fn state_cycle(facts: &[Fact], threshold: u32) -> Option<super::super::ExecutionCycle> {
    super::super::execution_cycle(facts, "session", "current", &binding(), threshold)
}

#[test]
fn repeated_original_workspace_states_consume_only_the_existing_allowance() {
    let facts = state_facts();
    assert!(state_cycle(&facts[..14], 3).is_none());
    assert!(
        matches!(state_cycle(&facts, 3), Some(super::super::ExecutionCycle::ObservedState(tool)) if tool == "bash")
    );
    assert!(state_cycle(&facts, 8).is_none());
    assert!(state_cycle(&facts, 0).is_none());
    assert!(super::super::execution_cycle(&facts, "other", "current", &binding(), 3).is_none());
    assert!(super::super::execution_cycle(&facts, "session", "other", &binding(), 3).is_none());
}

#[test]
fn fresh_net_workspace_states_reset_identical_invocation_counts() {
    let mut facts = Vec::new();
    for i in 0..12 {
        push_state(
            &mut facts,
            "same-action",
            &i.to_string(),
            &(i + 1).to_string(),
        );
    }
    assert!(state_cycle(&facts, 3).is_none());
}

#[test]
fn original_information_and_user_input_supersede_state_cycles() {
    let mut facts = state_facts();
    push(
        &mut facts,
        "current",
        "read-new",
        ToolExecutionOutcome::Succeeded,
        "Fresh original source",
    );
    assert!(state_cycle(&facts, 3).is_none());
    let mut facts = state_facts();
    let mut input = fact(
        "user.message",
        "new-input",
        json!({"text":"new host input"}),
    );
    input.seq = facts.len() as u64 + 1;
    facts.push(input);
    assert!(state_cycle(&facts, 3).is_none());
}

#[test]
fn harness_files_do_not_reset_a_detected_workspace_cycle() {
    let mut facts = state_facts();
    push_state(&mut facts, "A", "B", "A");
    let receipt = &mut facts.last_mut().unwrap().payload["receipt"];
    receipt["effects"]["changes"][0]["path"] = json!(".a3s/internal-journal");
    assert!(matches!(
        state_cycle(&facts, 3),
        Some(super::super::ExecutionCycle::ObservedState(_))
    ));
}

#[test]
fn incoherent_receipt_endpoints_cannot_prove_a_state_cycle() {
    let mut facts = state_facts();
    push_state(&mut facts, "A", "B", "A");
    let mut receipt: ToolExecutionResultV1 =
        serde_json::from_value(facts.last().unwrap().payload["receipt"].clone()).unwrap();
    receipt.effects.changes.push(ToolPathChangeV1 {
        path: "source.txt".into(),
        before: Some(file_state("B")),
        after: file_state("C"),
    });
    facts.last_mut().unwrap().payload["receipt"] = json!(receipt);
    assert!(state_cycle(&facts, 3).is_none());
}

#[test]
fn private_child_state_cycles_preserve_their_own_log_authority() {
    let mut facts = state_facts();
    for pair in facts.chunks_exact_mut(2) {
        let mut prepared: ToolPreparedV1 =
            serde_json::from_value(pair[0].payload["prepared"].clone()).unwrap();
        prepared.identity.admission = None;
        let mut receipt: ToolExecutionResultV1 =
            serde_json::from_value(pair[1].payload["receipt"].clone()).unwrap();
        receipt.identity = prepared.identity.clone();
        receipt.prepared_digest = prepared.digest().unwrap();
        pair[0].payload["prepared"] = json!(prepared);
        pair[1].payload["receipt"] = json!(receipt);
    }
    assert!(matches!(
        super::super::child_execution_cycle(&facts, 3),
        Some(super::super::ExecutionCycle::ObservedState(_))
    ));
    assert!(state_cycle(&facts, 3).is_none());
}

#[test]
fn an_external_before_state_breaks_a_previously_closed_workspace_cycle() {
    let mut facts = state_facts();
    push_state(&mut facts, "A", "external-input", "A");
    assert!(state_cycle(&facts, 3).is_none());
}

#[test]
fn state_cycle_detection_resumes_after_original_continuity_recovers() {
    let mut facts = Vec::new();
    push_state(&mut facts, "B", "A", "B");
    push_state(&mut facts, "A", "external", "A");
    assert!(state_cycle(&facts, 3).is_none());
    for _ in 0..3 {
        push_state(&mut facts, "B", "A", "B");
        push_state(&mut facts, "A", "B", "A");
    }
    assert!(state_cycle(&facts, 3).is_none());
    push_state(&mut facts, "B", "A", "B");
    assert!(matches!(
        state_cycle(&facts, 3),
        Some(super::super::ExecutionCycle::ObservedState(_))
    ));
}

#[test]
fn a_gap_clears_an_old_cycle_before_recounting_fresh_original_actions() {
    let mut facts = state_facts();
    assert!(state_cycle(&facts, 3).is_some());
    push_state(&mut facts, "A", "external", "A");
    push(
        &mut facts,
        "current",
        "unchanged-read",
        ToolExecutionOutcome::Succeeded,
        "Source updated.",
    );
    assert!(state_cycle(&facts, 3).is_none());
    for _ in 0..3 {
        push_state(&mut facts, "B", "A", "B");
        push_state(&mut facts, "A", "B", "A");
    }
    assert!(state_cycle(&facts, 3).is_none());
    push_state(&mut facts, "B", "A", "B");
    assert!(state_cycle(&facts, 3).is_some());
}
