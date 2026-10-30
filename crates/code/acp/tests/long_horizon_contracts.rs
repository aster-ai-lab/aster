#![cfg(test)]

#[path = "long_horizon/artifact.rs"]
mod artifact;
#[path = "long_horizon/candidate.rs"]
mod candidate;
#[path = "long_horizon/config.rs"]
mod config;
#[cfg(unix)]
#[path = "long_horizon/driver.rs"]
mod driver;
#[path = "long_horizon/fixture.rs"]
mod fixture;
#[path = "long_horizon/native_fixture.rs"]
mod native_fixture;
#[path = "long_horizon/relay.rs"]
mod relay;
#[path = "long_horizon/report.rs"]
mod report;
#[cfg(unix)]
#[path = "long_horizon/runtime.rs"]
mod runtime;
#[path = "long_horizon/upstream.rs"]
mod upstream;

use report::{Check, Definition, FailureLayer, Record, Status, Summary};
use std::collections::BTreeMap;

const CAMPAIGN: &str = "fixture-frozen-campaign";

fn definitions() -> Vec<Definition<'static>> {
    vec![Definition {
        case: "M08",
        variant: "default",
        required: &[Check::CurrentArtifact, Check::FaultBarrier],
    }]
}

fn passing() -> Record {
    Record {
        campaign: CAMPAIGN.into(),
        case: "M08".into(),
        variant: "default".into(),
        seed: 1,
        attempt: 1,
        started: true,
        status: Status::Pass,
        failure_layer: None,
        checks: [
            Check::Identity,
            Check::Oracle,
            Check::Terminal,
            Check::Cleanup,
            Check::CurrentArtifact,
            Check::FaultBarrier,
        ]
        .into_iter()
        .map(|check| (check, true))
        .collect(),
        tokens: Some(10),
    }
}

#[test]
fn m08_each_missing_or_failed_required_fact_refuses_pass() {
    let valid = passing();
    assert_eq!(
        report::summarize(CAMPAIGN, &definitions(), &[valid.clone()])
            .unwrap()
            .passed,
        1
    );
    for check in valid.checks.keys() {
        let mut missing = valid.clone();
        missing.checks.remove(check);
        assert!(report::summarize(CAMPAIGN, &definitions(), &[missing]).is_err());
        let mut failed = valid.clone();
        failed.checks.insert(*check, false);
        assert!(report::summarize(CAMPAIGN, &definitions(), &[failed]).is_err());
    }
    let mut extra_failure = valid;
    extra_failure.checks.insert(Check::SideEffects, false);
    assert!(report::summarize(CAMPAIGN, &definitions(), &[extra_failure]).is_err());
}

#[test]
fn m07_a_retry_keeps_the_first_failure_and_all_usage() {
    let mut first = passing();
    first.status = Status::Fail;
    first.failure_layer = Some(FailureLayer::Undetermined);
    first.tokens = None;
    let mut retry = passing();
    retry.attempt = 2;
    let result = report::summarize(CAMPAIGN, &definitions(), &[first, retry]).unwrap();
    assert_eq!(result.started_attempts, 2);
    assert_eq!(result.first_attempts, 1);
    assert_eq!(result.first_passed, 0);
    assert_eq!(result.failed, 1);
    assert_eq!(result.passed, 1);
    assert_eq!(result.known_tokens, 10);
    assert_eq!(result.unknown_usage_attempts, 1);
}

#[test]
fn m07_a_retry_without_its_earlier_attempt_is_not_a_complete_report() {
    let mut retry = passing();
    retry.attempt = 2;
    assert!(report::summarize(CAMPAIGN, &definitions(), &[retry.clone()]).is_err());
    retry.attempt = 3;
    assert!(report::summarize(CAMPAIGN, &definitions(), &[passing(), retry]).is_err());
}

#[test]
fn m10_duplicates_unknown_cases_and_foreign_campaigns_are_rejected() {
    let valid = passing();
    assert!(report::summarize(CAMPAIGN, &definitions(), &[valid.clone(), valid.clone()]).is_err());
    for (case, variant, campaign) in [
        ("UNKNOWN", "default", CAMPAIGN),
        ("M08", "unknown", CAMPAIGN),
        ("M08", "default", "different-source-model-profile"),
    ] {
        let mut invalid = valid.clone();
        invalid.case = case.into();
        invalid.variant = variant.into();
        invalid.campaign = campaign.into();
        assert!(report::summarize(CAMPAIGN, &definitions(), &[invalid]).is_err());
    }
    assert!(report::summarize(
        CAMPAIGN,
        &[definitions().remove(0), definitions().remove(0)],
        &[valid]
    )
    .is_err());
}

#[test]
fn m10_unexecuted_and_blocked_targets_remain_in_their_own_counts() {
    let records = [Status::Planned, Status::NotRun, Status::Blocked]
        .into_iter()
        .enumerate()
        .map(|(index, status)| {
            let mut record = passing();
            record.seed = index as u64;
            record.status = status;
            record.started = false;
            record.tokens = None;
            record.checks = BTreeMap::new();
            record.failure_layer =
                (status == Status::Blocked).then_some(FailureLayer::Infrastructure);
            record
        })
        .collect::<Vec<_>>();
    let result = report::summarize(CAMPAIGN, &definitions(), &records).unwrap();
    assert_eq!(
        result,
        Summary {
            planned: 1,
            not_run: 1,
            blocked: 1,
            ..Summary::default()
        }
    );
    let serialized = serde_json::to_string(&result).unwrap();
    assert!(!serialized.contains("coverage"));
}

#[test]
fn m10_usage_cannot_overflow_or_be_charged_to_an_unstarted_target() {
    let mut first = passing();
    first.tokens = Some(u64::MAX);
    let mut second = passing();
    second.seed = 2;
    assert!(report::summarize(CAMPAIGN, &definitions(), &[first, second]).is_err());
    let mut not_started = passing();
    not_started.started = false;
    not_started.status = Status::NotRun;
    assert!(report::summarize(CAMPAIGN, &definitions(), &[not_started]).is_err());
}

#[test]
fn outcome_vocabularies_round_trip_and_refuse_unknown_values() {
    for value in ["planned", "not_run", "blocked", "pass", "fail", "cancelled"] {
        let status = Status::parse(value).unwrap();
        assert_eq!(status.as_str(), value);
        assert_eq!(serde_json::to_value(status).unwrap(), value);
    }
    for value in [
        "product",
        "model",
        "provider",
        "test_spec",
        "infrastructure",
        "mixed",
        "undetermined",
    ] {
        let layer = FailureLayer::parse(value).unwrap();
        assert_eq!(layer.as_str(), value);
        assert_eq!(serde_json::to_value(layer).unwrap(), value);
    }
    for value in [
        "identity",
        "oracle",
        "terminal",
        "cleanup",
        "current_artifact",
        "fault_barrier",
        "side_effects",
    ] {
        let check = Check::parse(value).unwrap();
        assert_eq!(check.as_str(), value);
        assert_eq!(serde_json::to_value(check).unwrap(), value);
    }
    assert!(Status::parse("success").is_err());
    assert!(FailureLayer::parse("model_ceiling").is_err());
    assert!(Check::parse("probably_verified").is_err());
    let mut json = serde_json::to_value(passing()).unwrap();
    json["status"] = "success".into();
    assert!(serde_json::from_value::<Record>(json).is_err());
    let mut json = serde_json::to_value(passing()).unwrap();
    json["unexpected"] = true.into();
    assert!(serde_json::from_value::<Record>(json).is_err());
}

#[test]
fn isolated_child_acl_preserves_capabilities_without_upstream_or_user_state() {
    use a3s_code_core::config::CodeConfig;
    let original = CodeConfig::from_acl(
        r#"
default_model = "fixture/model"
import_accounts = ["codex"]
providers "fixture" {
  api_key = "parent-key-sentinel"
  base_url = "https://parent.invalid/private-sentinel"
  models "model" {
    api_key = "model-key-sentinel"
    reasoning = true
    limit = { context = 8192, output = 1024 }
  }
}
"#,
    )
    .unwrap();
    let mut provider = original.providers[0].clone();
    provider
        .headers
        .insert("X-Private".into(), "header-sentinel".into());
    provider.models[0]
        .headers
        .insert("X-Model-Private".into(), "model-header-sentinel".into());
    let root = tempfile::tempdir().unwrap();
    let source = config::isolated_acl(
        &provider,
        &provider.models[0],
        "http://127.0.0.1:12345/v1/chat/completions",
        root.path(),
        0,
    )
    .unwrap();
    assert!(!source.contains("sentinel"));
    let child = CodeConfig::from_acl(&source).unwrap();
    assert_eq!(child.default_model.as_deref(), Some("fixture/model"));
    assert!(child.import_accounts.is_empty());
    assert!(child.skill_dirs.is_empty());
    assert!(child.agent_dirs.is_empty());
    assert_eq!(child.memory_dir, Some(root.path().join("memory")));
    assert_eq!(child.sessions_dir, Some(root.path().join("sessions")));
    assert_eq!(
        child.user_instructions_dir,
        Some(root.path().join("instructions"))
    );
    assert_eq!(child.max_tool_rounds, Some(0));
    let provider = &child.providers[0];
    assert!(source.contains(config::RELAY_KEY_ENV));
    assert!(provider.api_key == std::env::var(config::RELAY_KEY_ENV).ok());
    assert!(provider.headers.is_empty());
    assert!(provider.models[0].headers.is_empty());
    assert!(provider.models[0].api_key.is_none());
    assert!(provider.models[0].reasoning);
    assert_eq!(provider.models[0].limit.context, 8192);
    assert_eq!(provider.models[0].limit.output, 1024);
}

#[test]
fn child_configuration_cannot_select_an_external_or_ambiguous_relay() {
    use a3s_code_core::config::CodeConfig;
    let original = CodeConfig::from_acl("providers \"fixture\" { models \"model\" {} }").unwrap();
    let provider = &original.providers[0];
    let root = tempfile::tempdir().unwrap();
    for endpoint in [
        "https://127.0.0.1/v1/chat/completions",
        "http://localhost/v1/chat/completions",
        "http://example.invalid/v1/chat/completions",
        "http://user:secret@127.0.0.1/v1/chat/completions",
        "http://127.0.0.1/v1/chat/completions?upstream=elsewhere",
        "http://127.0.0.1/other",
    ] {
        assert!(
            config::isolated_acl(provider, &provider.models[0], endpoint, root.path(), 20).is_err()
        );
    }
    assert!(config::isolated_acl(
        provider,
        &provider.models[0],
        "http://127.0.0.1:12345/v1/chat/completions",
        std::path::Path::new("relative-state"),
        20,
    )
    .is_err());
}

#[test]
fn isolated_paths_are_acl_data_and_cannot_inject_an_account_import() {
    use a3s_code_core::config::CodeConfig;
    let original = CodeConfig::from_acl("providers \"fixture\" { models \"model\" {} }").unwrap();
    let provider = &original.providers[0];
    let root = tempfile::tempdir().unwrap();
    let state = root.path().join("quoted \"\nimport_accounts = [\"codex\"]");
    let source = config::isolated_acl(
        provider,
        &provider.models[0],
        "http://127.0.0.1:12345/v1/chat/completions",
        &state,
        7,
    )
    .unwrap();
    let parsed = CodeConfig::from_acl(&source).unwrap();
    assert!(parsed.import_accounts.is_empty());
    assert_eq!(
        parsed.user_instructions_dir,
        Some(state.join("instructions"))
    );
    assert_eq!(parsed.max_tool_rounds, Some(7));
}

#[test]
fn rust_verification_is_exact_and_scoped_to_the_declared_workspace() {
    use a3s_code_core::config::CodeConfig;
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("quoted \" workspace");
    let other = root.path().join("other");
    std::fs::create_dir(&workspace).unwrap();
    std::fs::create_dir(&other).unwrap();
    let source = config::with_rust_verification("", &workspace).unwrap();
    let parsed = CodeConfig::from_acl(&source).unwrap();
    let contracts = parsed
        .verification_contracts_for_workspace(&workspace)
        .unwrap();
    assert_eq!(contracts.len(), 1);
    assert_eq!(contracts[0].command, fixture::LOCAL_CHECK);
    assert_eq!(
        contracts[0].covers,
        [
            "input.rs",
            "calc.rs",
            "display.rs",
            "CHANGELOG.md",
            "checks"
        ]
    );
    assert!(parsed
        .verification_contracts_for_workspace(&other)
        .unwrap()
        .is_empty());
    assert!(parsed.import_accounts.is_empty());
    assert!(config::with_rust_verification("", std::path::Path::new("relative")).is_err());
}

#[tokio::test]
async fn upstream_route_uses_the_existing_adapter_without_sending_a_request() {
    use a3s_code_core::llm::LlmConfig;
    let route = upstream::openai_route(
        LlmConfig::new("deepseek", "fixture-model", "host-memory-sentinel")
            .with_base_url("http://127.0.0.1:1/api/paas/v4")
            .with_headers(
                [("X-Fixture".into(), "header-sentinel".into())]
                    .into_iter()
                    .collect(),
            ),
    )
    .await
    .unwrap();
    assert_eq!(
        route.endpoint,
        "http://127.0.0.1:1/api/paas/v4/chat/completions"
    );
    assert!(route
        .headers
        .iter()
        .any(|(key, value)| key.eq_ignore_ascii_case("authorization")
            && value == "Bearer host-memory-sentinel"));
    assert!(route
        .headers
        .iter()
        .any(|(key, value)| key == "X-Fixture" && value == "header-sentinel"));
}

#[tokio::test]
async fn an_unsupported_provider_is_not_silently_reinterpreted_by_the_relay() {
    use a3s_code_core::llm::LlmConfig;
    assert!(
        upstream::openai_route(LlmConfig::new("anthropic", "fixture-model", "fixture"))
            .await
            .is_err()
    );
    let mut config =
        LlmConfig::new("deepseek", "fixture-model", "fixture").with_base_url("http://127.0.0.1:1");
    config.session_id_header = Some("X-Session".into());
    assert!(upstream::openai_route(config).await.is_err());
}
