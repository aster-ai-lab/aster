#![cfg(test)]

use super::test_support::*;
use super::*;
use aster_runtime::contract::{RuntimeInspection, RuntimeUnitState};
use aster_runtime::{RuntimeClock, SystemRuntimeClock};
use aster_use_core::{PackageSurfaceKind, ToolWorkloadContract};
use std::sync::atomic::Ordering;
use std::sync::Arc;

fn plan(startup_ms: u64) -> RuntimeSurfacePlan {
    let mut descriptor = service_descriptor();
    let ToolWorkloadContract::Service {
        startup_timeout_ms, ..
    } = &mut descriptor.workload
    else {
        panic!("the fixture must declare a Service");
    };
    *startup_timeout_ms = startup_ms;
    plan_tool_service_release(
        context(PackageSurfaceKind::Tool, "index"),
        &service_surface(),
        &descriptor,
        artifact(&descriptor.artifact.digest, &descriptor.artifact.media_type),
        policy(),
    )
    .unwrap()
}

#[tokio::test]
async fn service_startup_waits_for_health_without_reapplying() {
    let plan = plan(60_000);
    assert_eq!(plan.spec().health.as_ref().unwrap().start_period_ms, 0);
    assert_ne!(
        plan.spec().digest().unwrap(),
        self::plan(30_000).spec().digest().unwrap()
    );
    let caps = capabilities(&plan);
    let provider = evidence(&plan, &caps);
    let runtime = Arc::new(FakeRuntime::new(caps, true).with_starting_service());
    let activation = PluginRuntimeClient::new(runtime.clone())
        .apply_service(&plan, &provider, "startup", None)
        .await
        .unwrap();
    assert!(activation.observation().converges(plan.spec()));
    assert_eq!(runtime.apply_count.load(Ordering::SeqCst), 1);
    assert!(runtime.inspect_count.load(Ordering::SeqCst) > 0);
}

#[tokio::test]
async fn service_startup_refuses_changed_provider_or_generation_and_disappearance() {
    for mismatch in ["provider", "generation", "missing", "failed"] {
        let plan = plan(60_000);
        let caps = capabilities(&plan);
        let provider = evidence(&plan, &caps);
        let runtime = Arc::new(FakeRuntime::new(caps, true).with_starting_service());
        let baseline = PluginRuntimeClient::new(runtime.clone())
            .apply_service(&plan, &provider, "baseline", None)
            .await
            .unwrap();
        let mut observation = baseline.observation().clone();
        match mismatch {
            "provider" => observation.provider_build = Some("other-build".into()),
            "generation" => observation.generation += 1,
            "failed" => {
                observation.state = RuntimeUnitState::Failed;
                observation.health = None;
                observation.liveness = None;
                observation.clear_service_endpoints();
            }
            _ => {}
        }
        *runtime.inspection_override.lock().unwrap() = Some(if mismatch == "missing" {
            RuntimeInspection::NotFound {
                schema: RuntimeInspection::SCHEMA.into(),
                unit_id: plan.spec().unit_id.clone(),
                last_generation: Some(plan.spec().generation),
            }
        } else {
            RuntimeInspection::Found {
                schema: RuntimeInspection::SCHEMA.into(),
                observation: Box::new(observation),
            }
        });
        let error = PluginRuntimeClient::new(runtime.clone())
            .apply_service(&plan, &provider, "refused", None)
            .await
            .unwrap_err();
        assert!(
            matches!(
                error.code.as_str(),
                "use.plugin.runtime.contract_invalid"
                    | "use.plugin.runtime.observation_evidence_mismatch"
                    | "use.plugin.runtime.not_converged"
            ),
            "{mismatch}: {error:?}"
        );
        assert_eq!(runtime.apply_count.load(Ordering::SeqCst), 2);
    }
}

#[tokio::test]
async fn service_startup_is_bounded_by_release_and_callers_deadlines() {
    for (startup_ms, deadline) in [(60_000, Some(SystemRuntimeClock.now_ms() + 30)), (30, None)] {
        let plan = plan(startup_ms);
        let caps = capabilities(&plan);
        let provider = evidence(&plan, &caps);
        let runtime = Arc::new(FakeRuntime::new(caps, false));
        let error = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            PluginRuntimeClient::new(runtime.clone())
                .apply_service(&plan, &provider, "bounded", deadline),
        )
        .await
        .expect("a pending Service must not wait indefinitely")
        .unwrap_err();
        assert_eq!(error.code, "use.plugin.runtime.not_converged");
        assert_eq!(runtime.apply_count.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn service_startup_bounds_an_unfinished_apply_without_guessing_cleanup() {
    let plan = plan(30);
    let caps = capabilities(&plan);
    let provider = evidence(&plan, &caps);
    let runtime = Arc::new(FakeRuntime::new(caps, true).with_apply_gate(
        Arc::new(tokio::sync::Notify::new()),
        Arc::new(tokio::sync::Notify::new()),
    ));
    let error = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        PluginRuntimeClient::new(runtime.clone()).apply_service(
            &plan,
            &provider,
            "slow-apply",
            None,
        ),
    )
    .await
    .expect("the release startup bound includes apply")
    .unwrap_err();
    assert_eq!(error.code, "use.plugin.runtime.not_converged");
    assert_eq!(runtime.apply_count.load(Ordering::SeqCst), 1);
    assert_eq!(runtime.stop_count.load(Ordering::SeqCst), 0);
    assert_eq!(runtime.remove_count.load(Ordering::SeqCst), 0);
}
