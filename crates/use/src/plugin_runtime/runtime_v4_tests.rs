#![cfg(test)]

use super::test_support::*;
use super::*;
use aster_runtime::contract::{
    HealthProbe, RuntimeFeature, RuntimeHealthCheck, RuntimeHostService, RuntimeServiceLifecycle,
    SecretReference, SecretTarget,
};
use aster_use_core::{PackageSurfaceKind, ToolReleaseDescriptor, UseResult};
use std::sync::atomic::Ordering;
use std::sync::Arc;

fn plan(policy: RuntimeWorkloadPolicy) -> UseResult<RuntimeSurfacePlan> {
    let descriptor = service_descriptor();
    plan_tool_service_release(
        context(PackageSurfaceKind::Tool, "index"),
        &service_surface(),
        &descriptor,
        artifact(&descriptor.artifact.digest, &descriptor.artifact.media_type),
        policy,
    )
}

fn authorized_policy() -> RuntimeWorkloadPolicy {
    let mut policy = policy();
    policy.host_services = vec![RuntimeHostService {
        reference: "os.gateway".into(),
        vsock_port: 61000,
    }];
    policy.secrets = vec![SecretReference {
        name: "gateway-ocr".into(),
        reference: "secret://fixture/gateway-ocr".into(),
        target: SecretTarget::Environment {
            variable: "ASTER_GATEWAY_KEY".into(),
        },
    }];
    policy.service_lifecycle = Some(RuntimeServiceLifecycle {
        shutdown_grace_seconds: 30,
        liveness: RuntimeHealthCheck {
            probe: HealthProbe::Http {
                port: "http".into(),
                path: "/live".into(),
                expected_statuses: vec![200],
            },
            interval_ms: 10_000,
            timeout_ms: 2_000,
            start_period_ms: 60_000,
            success_threshold: 1,
            failure_threshold: 3,
        },
    });
    policy
}

#[tokio::test]
async fn runtime_v4_forwards_explicit_transport_and_distinct_liveness_through_capability_admission()
{
    let plan = plan(authorized_policy()).unwrap();
    assert_eq!(plan.spec().schema, "aster.runtime.unit-spec.v4");
    assert_eq!(plan.spec().network.host_services[0].reference, "os.gateway");
    assert_eq!(plan.spec().network.host_services[0].vsock_port, 61000);
    assert_eq!(
        plan.spec().secrets[0].reference,
        "secret://fixture/gateway-ocr"
    );
    assert!(matches!(&plan.spec().health.as_ref().unwrap().probe,
        HealthProbe::Http { path, .. } if path == "/healthz"));
    assert!(
        matches!(&plan.spec().service_lifecycle.as_ref().unwrap().liveness.probe,
        HealthProbe::Http { path, .. } if path == "/live")
    );
    let missing = capabilities(&plan);
    let runtime = Arc::new(FakeRuntime::new(missing.clone(), true));
    let error = PluginRuntimeClient::new(runtime.clone())
        .apply_service(&plan, &evidence(&plan, &missing), "denied-apply", None)
        .await
        .unwrap_err();
    assert_eq!(error.code, "use.plugin.runtime.capability_missing");
    assert_eq!(
        error.details["missing"],
        serde_json::json!([
            "feature:HostServices",
            "feature:SecretReferences",
            "feature:ServiceLifecycle"
        ])
    );
    assert_eq!(runtime.apply_count.load(Ordering::SeqCst), 0);
    let mut supported = missing;
    supported.features.extend([
        RuntimeFeature::HostServices,
        RuntimeFeature::SecretReferences,
        RuntimeFeature::ServiceLifecycle,
    ]);
    let provider = evidence(&plan, &supported);
    let runtime = Arc::new(FakeRuntime::new(supported, true));
    let client = PluginRuntimeClient::new(runtime);
    let activation = client
        .apply_service(&plan, &provider, "approved-apply", None)
        .await
        .unwrap();
    assert_eq!(
        activation.observation().spec_digest,
        plan.spec().digest().unwrap()
    );
    assert!(activation.observation().liveness.is_some());
    let receipt = activation
        .into_tool_service_receipt(RuntimeEndpointRef::parse("gateway:workspace-01/index").unwrap())
        .unwrap();
    let binding = RuntimeBindingReceipt::Service(receipt.clone());
    assert_eq!(
        client.observe_binding(&binding).await.unwrap().state,
        RuntimeBindingObservedState::Healthy
    );
    client
        .drain_remove_service(&receipt, "stop-approved", "remove-approved", None)
        .await
        .unwrap();
    assert_eq!(
        client.observe_binding(&binding).await.unwrap().state,
        RuntimeBindingObservedState::Missing
    );
}

#[test]
fn runtime_v4_policy_changes_invalidate_plan_identity_and_reject_mismatched_shutdown_grace() {
    let original = plan(authorized_policy()).unwrap();
    let mut changed = authorized_policy();
    changed.host_services[0].reference = "another.authorized.reference".into();
    let changed = plan(changed).unwrap();
    assert_ne!(
        original.spec().digest().unwrap(),
        changed.spec().digest().unwrap()
    );
    assert_ne!(
        original.spec().semantics_profile_digest,
        changed.spec().semantics_profile_digest
    );
    let mut invalid = authorized_policy();
    invalid
        .service_lifecycle
        .as_mut()
        .unwrap()
        .shutdown_grace_seconds = 29;
    assert!(plan(invalid).is_err());
    let mut unrequested = policy();
    unrequested
        .non_secret_environment
        .insert("ASTER_GATEWAY_ORIGIN".into(), "vsock://2:61000".into());
    let default = plan(unrequested).unwrap();
    assert!(default.spec().network.host_services.is_empty());
    assert!(default.spec().service_lifecycle.is_none());
}

#[test]
fn runtime_v4_refuses_incompatible_release_metadata_before_runtime_selection() {
    let legacy = ToolReleaseDescriptor::from_json(include_bytes!(
        "../../core//fixtures/releases/tool-service-release-v1.json"
    ))
    .unwrap();
    let error = plan_tool_service_release(
        context(PackageSurfaceKind::Tool, "index"),
        &service_surface(),
        &legacy,
        artifact(&legacy.artifact.digest, &legacy.artifact.media_type),
        policy(),
    )
    .unwrap_err();
    assert_eq!(error.code, "use.release.incompatible");
    assert_eq!(error.details["component"], "aster-runtime");
    assert_eq!(error.details["actual"], "0.8.0");
    let mut unresolved = service_descriptor();
    unresolved
        .compatibility
        .push(aster_use_core::ReleaseCompatibility {
            component: "unconfigured-service".into(),
            version_requirement: ">=1.0.0".into(),
        });
    let error = plan_tool_service_release(
        context(PackageSurfaceKind::Tool, "index"),
        &service_surface(),
        &unresolved,
        artifact(&unresolved.artifact.digest, &unresolved.artifact.media_type),
        policy(),
    )
    .unwrap_err();
    assert_eq!(error.code, "use.release.compatibility_missing");
    let mut unresolved = service_descriptor();
    unresolved
        .dependencies
        .push(aster_use_core::ReleaseDependency {
            kind: aster_use_core::ReleaseKind::Skill,
            name: "aster/unresolved".into(),
            version: "1.0.0".into(),
            descriptor_digest: format!("sha256:{}", "a".repeat(64)),
        });
    let error = plan_tool_service_release(
        context(PackageSurfaceKind::Tool, "index"),
        &service_surface(),
        &unresolved,
        artifact(&unresolved.artifact.digest, &unresolved.artifact.media_type),
        policy(),
    )
    .unwrap_err();
    assert_eq!(error.code, "use.release.dependency_missing");
}

#[tokio::test]
async fn runtime_v4_rejects_service_only_task_policy_and_old_runtime_task_templates() {
    let package = aster_use_extension::inspect_release_bundle(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/runtime-v4/package"),
    )
    .await
    .unwrap();
    assert_eq!(
        format!("sha256:{}", package.package_sha256),
        include_str!("../../fixtures/runtime-v4/package.sha256").trim()
    );
    let descriptor = task_descriptor();
    let create = |policy| {
        plan_tool_task_release(
            context(PackageSurfaceKind::Tool, "convert"),
            &task_surface(),
            &descriptor,
            artifact(&descriptor.artifact.digest, &descriptor.artifact.media_type),
            RuntimeTaskInvocation::new("invoke-01", Vec::new()).unwrap(),
            policy,
            aster_runtime::contract::NetworkMode::None,
        )
    };
    assert!(create(authorized_policy()).is_err());
    let plan = create(policy()).unwrap();
    let caps = capabilities(&plan);
    let provider = evidence(&plan, &caps);
    let mut binding = PluginRuntimeClient::new(Arc::new(FakeRuntime::new(caps, true)))
        .prepare_task(&plan, &provider)
        .await
        .unwrap();
    binding.template_spec.schema = "aster.runtime.unit-spec.v2".into();
    assert!(binding.validate().is_err());
    let mut invalid = authorized_policy();
    invalid.host_services[0].vsock_port = 1023;
    assert!(self::plan(invalid).is_err());
}
