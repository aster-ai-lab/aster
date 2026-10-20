#![cfg(test)]
use super::*;
#[test]
fn lost_batch_lifecycle_evidence_cannot_be_successful_execution() {
    let value = runner_result_value(
        "done",
        false,
        &[],
        0,
        Some(&json!({"evidence_incomplete":true})),
    );
    let envelope = decode_fact_tool_result(value.as_str().unwrap()).unwrap();
    assert_eq!(envelope.outcome, Some(ToolExecutionOutcome::Unknown));
}

#[test]
fn wrapped_reconciliation_failure_keeps_its_typed_event_and_identity() {
    let error = crate::harness_loop::ToolEffectUnresolved {
        obligation_ids: vec!["exact-1".into()],
    };
    let wrapped: anyhow::Error =
        crate::agent::AgentExecutionFailure::new(error.into(), Default::default(), 1).into();
    let event = crate::agent::AgentEvent::execution_error(&wrapped);
    assert!(
        matches!(&event, crate::agent::AgentEvent::ToolEffectUnresolved { error } if error.obligation_ids == ["exact-1"])
    );
    assert_eq!(event.event_type_v1(), "tool_effect_unresolved");
    let wire = crate::event_protocol::EventEnvelopeV1::try_from(&event).unwrap();
    assert!(serde_json::to_value(wire)
        .unwrap()
        .to_string()
        .contains("exact-1"));
}

#[test]
fn batched_background_identity_settles_only_with_its_matching_terminal() {
    for wrong_run in [false, true] {
        let mut facts = background_debt_fixture(false);
        let mut prepared: ToolPreparedV1 =
            serde_json::from_value(facts[0].payload["prepared"].clone()).unwrap();
        let mut receipt: ToolExecutionResultV1 =
            serde_json::from_value(facts[1].payload["receipt"].clone()).unwrap();
        prepared.tool_name = "batch".into();
        receipt.prepared_digest = prepared.digest().unwrap();
        let metadata = json!({"results":[{"tool":"background", "metadata":{
            "workspace_child":"one", "source_run":if wrong_run {"other-source"} else {"source-one"}, "run":"effect-one"
        }}]});
        receipt.output = runner_result_value("batch accepted", false, &[], 0, Some(&metadata));
        facts[0] = prepared_fact(&prepared);
        facts[1] = result_fact(&receipt);
        let live = Arc::new(Mutex::new(crate::harness_loop::MutationLedger::default()));
        let live_id = format!("batch-binding-fixture-{wrong_run}");
        let mut live_metadata = metadata.clone();
        live_metadata["results"][0]["metadata"]["workspace_child"] = json!(live_id);
        crate::porcelain::reserve_workspace_child(&live_id);
        observe_live_child(&live, Some(&live_metadata), "batch");
        assert!(live.lock().unwrap().has_open_children());
        crate::porcelain::settle_workspace_child_paths(&live_id, Vec::new());
        let _ = crate::porcelain::take_settled_workspace_child(&live_id);
        let debt = fold_debt(&facts);
        assert_eq!(debt.unresolved.contains("workspace-child:one"), wrong_run);
        assert!(debt.unresolved.contains("workspace-child:two"));
    }
}
