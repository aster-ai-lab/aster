#![cfg(test)]

use super::*;

#[test]
fn private_observation_meter_does_not_change_the_model_tool_message() {
    let digest = crate::content_digest::digest_bytes("test", b"original");
    let plain = runner_result_value("original", false, &[], 0, None, None);
    let measured = runner_result_value("original", false, &[], 0, None, Some(&digest));
    assert_eq!(
        serde_json::to_value(message_from_folded_tool(
            "call",
            plain.as_str().unwrap(),
            false
        ))
        .unwrap(),
        serde_json::to_value(message_from_folded_tool(
            "call",
            measured.as_str().unwrap(),
            false
        ))
        .unwrap(),
    );
    assert_eq!(
        decode_fact_tool_result(measured.as_str().unwrap())
            .unwrap()
            .observation_digest,
        Some(digest)
    );
}

fn append_observation(
    facts: &mut Vec<Fact>,
    run: &str,
    path: &str,
    body: &str,
    metadata: Option<&serde_json::Value>,
    observation: Option<&str>,
) {
    let mut prepared = preparation(&format!("observation-{}", facts.len()));
    prepared.identity.admission = Some(ToolAdmissionIdentityV1 {
        run_id: run.into(),
        thread_id: "session".into(),
        capability_binding_digest: format!("sha256:{}", "a".repeat(64)),
    });
    prepared.original_args_digest = a3s_effect::tool_args_digest(&json!({"path":path})).unwrap();
    prepared.effective_args_digest = prepared.original_args_digest.clone();
    let mut receipt =
        result_without_execution(&prepared, ToolExecutionOutcome::Succeeded, body.into()).unwrap();
    receipt.exit_code = Some(0);
    receipt.output = runner_result_value(body, false, &[], 0, metadata, observation);
    for mut fact in [prepared_fact(&prepared), result_fact(&receipt)] {
        fact.seq = facts.len() as u64 + 1;
        fact.key = format!("fixture-{}", fact.seq);
        facts.push(fact);
    }
}

#[test]
fn original_observations_ignore_run_identity_host_hints_and_tool_supplied_digest_metadata() {
    let mut facts = Vec::new();
    append_observation(&mut facts, "old", "a", "same", None, None);
    let observation = crate::content_digest::digest_json(
        "a3s.code.tool-observation.v1",
        &crate::fact_control::tool_results::message_from_tool_observation(
            "observation",
            "same",
            false,
            &[],
            Some(0),
        ),
    )
    .unwrap();
    append_observation(
        &mut facts,
        "current",
        "a",
        "same[host repeat hint 2]",
        Some(&json!({"observation_digest":"tool-supplied-fake", "duration_ms":2})),
        Some(&observation),
    );
    append_observation(
        &mut facts,
        "current",
        "a",
        "same[host repeat hint 3]",
        Some(&json!({"observation_digest":"different-fake", "duration_ms":3})),
        Some(&observation),
    );
    assert_eq!(novel_information(&facts, "session", "current"), 0);
    append_observation(&mut facts, "current", "b", "same", None, None);
    append_observation(&mut facts, "current", "a", "changed", None, None);
    assert_eq!(novel_information(&facts, "session", "current"), 2);
}

#[test]
fn missing_preparation_malformed_digest_and_denied_results_never_earn_information_credit() {
    for fault in ["preparation", "digest", "denied", "foreign"] {
        let mut facts = Vec::new();
        append_observation(&mut facts, "current", "a", "new", None, None);
        let mut receipt: ToolExecutionResultV1 =
            serde_json::from_value(facts[1].payload["receipt"].clone()).unwrap();
        match fault {
            "preparation" => {
                facts.remove(0);
            }
            "digest" => {
                receipt.output =
                    runner_result_value("new", false, &[], 0, None, Some("invalid-digest"));
                facts[1].payload["receipt"] = json!(receipt);
            }
            "denied" => {
                receipt.outcome = ToolExecutionOutcome::Denied;
                receipt.exit_code = None;
                facts[1].payload["receipt"] = json!(receipt);
            }
            "foreign" => {
                let mut prepared: ToolPreparedV1 =
                    serde_json::from_value(facts[0].payload["prepared"].clone()).unwrap();
                prepared.identity.admission.as_mut().unwrap().thread_id = "foreign".into();
                receipt.identity = prepared.identity.clone();
                receipt.prepared_digest = prepared.digest().unwrap();
                facts[0].payload["prepared"] = json!(prepared);
                facts[1].payload["receipt"] = json!(receipt);
            }
            _ => unreachable!(),
        }
        assert_eq!(
            novel_information(&facts, "session", "current"),
            0,
            "{fault}"
        );
    }
}
