#![cfg(test)]

use super::*;
use a3s_effect::{
    Fact, ToolAdmissionIdentityV1, ToolEffectObservationV1, ToolExecutionOutcome,
    ToolExecutionResultV1, ToolFileIdentityV1, ToolFileKind, ToolInvocationIdentityV1,
    ToolPathChangeV1, ToolPreparedV1, ToolRecoveryPolicy, TOOL_PREPARED_SCHEMA, TOOL_RESULT_SCHEMA,
};
use serde_json::json;

fn observation() -> ToolEffectObservationV1 {
    ToolEffectObservationV1::complete(vec![ToolPathChangeV1 {
        path: "source.txt".into(),
        before: Some(ToolFileIdentityV1 {
            kind: ToolFileKind::Missing,
            content_digest: None,
            mode: None,
        }),
        after: ToolFileIdentityV1 {
            kind: ToolFileKind::File,
            content_digest: Some(format!("sha256:{}", sha256::digest("new bytes"))),
            mode: Some(0o644),
        },
    }])
}

fn fact(seq: u64, kind: &str, key: &str, payload: serde_json::Value) -> Fact {
    Fact {
        seq,
        kind: kind.into(),
        key: key.into(),
        cause: None,
        payload,
    }
}

fn receipt_pair(effects: ToolEffectObservationV1) -> Vec<Fact> {
    let args = a3s_effect::tool_args_digest(&json!({"command":"fixture"})).unwrap();
    let prepared = ToolPreparedV1 {
        schema: TOOL_PREPARED_SCHEMA.into(),
        identity: ToolInvocationIdentityV1 {
            turn: 1,
            cycle: 0,
            tool_call_id: "current-tool".into(),
            batch_index: 0,
            admission: Some(ToolAdmissionIdentityV1 {
                run_id: "current-run".into(),
                thread_id: "current-session".into(),
                capability_binding_digest: args.clone(),
            }),
        },
        tool_name: "write".into(),
        original_args_digest: args.clone(),
        effective_args_digest: args,
        recovery: ToolRecoveryPolicy::NeverReplay,
        baseline: None,
    };
    let result = ToolExecutionResultV1 {
        schema: TOOL_RESULT_SCHEMA.into(),
        identity: prepared.identity.clone(),
        prepared_digest: prepared.digest().unwrap(),
        outcome: ToolExecutionOutcome::Succeeded,
        exit_code: Some(0),
        output: json!("done"),
        effects,
    };
    vec![
        fact(1, "a3s.goal.slice.admitted", "slice", json!({})),
        fact(2, "tool.prepared", "prepared", json!({"prepared":prepared})),
        fact(
            3,
            "tool.result",
            "result",
            json!({"receipt":result,"toolCallId":"current-tool","ok":true,"output":"done"}),
        ),
    ]
}

fn progress(facts: &[Fact]) -> bool {
    goal_slice_made_progress(facts, 1, "current-session", "current-run")
}

#[test]
fn only_complete_known_workspace_identity_changes_are_progress() {
    assert!(progress(&receipt_pair(observation())));
    let mut same = observation();
    same.changes[0].before = Some(same.changes[0].after.clone());
    assert!(!progress(&receipt_pair(same)));
    let mut unknown = observation();
    unknown.changes[0].before = None;
    assert!(!progress(&receipt_pair(unknown)));
    assert!(!progress(&receipt_pair(ToolEffectObservationV1::complete(
        vec![]
    ))));
    assert!(!progress(&receipt_pair(
        ToolEffectObservationV1::incomplete("scan unavailable")
    )));
    let mut harness = observation();
    harness.changes[0].path = ".a3s/session.json".into();
    assert!(!progress(&receipt_pair(harness)));
}

#[test]
fn a_foreign_or_later_slice_cannot_refresh_this_slice() {
    let facts = receipt_pair(observation());
    assert!(!goal_slice_made_progress(
        &facts,
        1,
        "other-session",
        "current-run"
    ));
    assert!(!goal_slice_made_progress(
        &facts,
        1,
        "current-session",
        "other-run"
    ));
    let mut current = receipt_pair(ToolEffectObservationV1::complete(vec![]));
    current.push(fact(4, "a3s.goal.slice.admitted", "later-slice", json!({})));
    current.extend(facts.into_iter().skip(1).map(|mut fact| {
        fact.seq += 3;
        fact
    }));
    assert!(!progress(&current));
}

#[test]
fn mixed_capability_receipts_and_unsettled_outcomes_do_not_refresh_progress() {
    let mut facts = receipt_pair(ToolEffectObservationV1::complete(vec![]));
    let mut extra = receipt_pair(observation());
    let mut prepared: ToolPreparedV1 =
        serde_json::from_value(extra[1].payload["prepared"].clone()).unwrap();
    let mut result: ToolExecutionResultV1 =
        serde_json::from_value(extra[2].payload["receipt"].clone()).unwrap();
    prepared.identity.tool_call_id = "second-tool".into();
    prepared.identity.turn = 2;
    prepared
        .identity
        .admission
        .as_mut()
        .unwrap()
        .capability_binding_digest =
        a3s_effect::tool_args_digest(&json!({"other_capability":true})).unwrap();
    result.identity = prepared.identity.clone();
    result.prepared_digest = prepared.digest().unwrap();
    extra[1] = fact(
        4,
        "tool.prepared",
        "prepared-2",
        json!({"prepared":prepared}),
    );
    extra[2] = fact(
        5,
        "tool.result",
        "result-2",
        json!({"receipt":result,"toolCallId":"second-tool","ok":true,"output":"done"}),
    );
    facts.extend(extra.into_iter().skip(1));
    assert!(!progress(&facts));
    let mut uncertain = receipt_pair(observation());
    uncertain.pop();
    assert!(!progress(&uncertain));
    let mut failed = receipt_pair(observation());
    failed[2].payload["receipt"]["outcome"] = json!("failed");
    failed[2].payload["receipt"]["exit_code"] = json!(7);
    assert!(
        progress(&failed),
        "an observed mutation does not depend on success status"
    );
}

#[test]
fn a_settled_background_mutation_must_belong_to_the_current_run() {
    let mut facts = receipt_pair(ToolEffectObservationV1::complete(vec![]));
    let mut result: ToolExecutionResultV1 =
        serde_json::from_value(facts[2].payload["receipt"].clone()).unwrap();
    result
        .effects
        .unresolved_external_operations
        .push("workspace-child:child-one".into());
    result.output = json!(serde_json::json!({
        "kind":"a3s.fact_tool_result", "text":"accepted", "is_error":false,
        "images":[], "workspace_child":"child-one",
        "metadata":{"workspace_child":"child-one", "source_run":"current-run", "run":"effect-run"}
    })
    .to_string());
    facts[2].payload["receipt"] = serde_json::to_value(result).unwrap();
    facts.push(fact(
        4,
        "background.output",
        "bg:child-one:2",
        json!({
            "event":"exited", "task_id":"child-one", "workspace_child":"child-one",
            "changed_paths":["source.txt"], "observation_incomplete":false,
            "output_complete":true, "exit_code":0, "cancelled":false, "timed_out":false,
            "disposition":"completed", "process_settled":true,
            "execution_future_released":true, "output_facts_retained":true,
            "process_settlement_basis":"execution_returned", "source_run":"current-run",
            "run":"effect-run", "effects":observation()
        }),
    ));
    assert!(progress(&facts));
    facts[3].payload["source_run"] = json!("other-run");
    assert!(!progress(&facts));
    facts[3].payload["source_run"] = json!("current-run");
    facts[3].payload["process_settled"] = json!(false);
    assert!(!progress(&facts));
}
