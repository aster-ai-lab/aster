use std::sync::Arc;

use aster_runtime::contract::NetworkMode;
use aster_runtime::{
    ProviderId, RuntimeClient, RuntimeClientRegistry, RuntimeProviderFactory, RuntimeResult,
};
use aster_use_core::{PackageSurfaceKind, PlanScope, PlanScopeKind, PluginSurfaceRef};
use aster_use_extension::{ExtensionManifest, ToolWorkload};
use async_trait::async_trait;
use tempfile::TempDir;

use crate::surface_reconciler::{
    reconcile_with_runtime, PluginDesiredState, PluginObservedState, SurfaceObservations,
    SurfaceObservedState,
};

use super::test_support::*;
use super::*;

const OTHER_PACKAGE_DIGEST: &str =
    "sha256:9999999999999999999999999999999999999999999999999999999999999999";

struct StaticRuntimeFactory {
    provider_id: ProviderId,
    client: Arc<dyn RuntimeClient>,
}

#[async_trait]
impl RuntimeProviderFactory for StaticRuntimeFactory {
    fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }

    async fn create(&self) -> RuntimeResult<Arc<dyn RuntimeClient>> {
        Ok(self.client.clone())
    }
}

#[tokio::test]
async fn scoped_runtime_observations_feed_the_named_surface_reconciler() {
    let temporary = TempDir::new().unwrap();
    let store = RuntimeBindingStore::new(temporary.path());
    let mut providers = RuntimeClientRegistry::new();
    install_task_binding(&store, &mut providers).await;
    let service_runtime = install_tool_service_binding(&store, &mut providers).await;
    install_mcp_service_binding(&store, &mut providers).await;
    let candidate_plan = task_plan(8);
    let candidate_capabilities = named_capabilities(&candidate_plan, "task-runtime");
    let candidate = RuntimePreparedTaskBinding::from_plan(
        &candidate_plan,
        &evidence(&candidate_plan, &candidate_capabilities),
    )
    .unwrap();
    store
        .put(&RuntimeBindingReceipt::Task(candidate))
        .await
        .unwrap();
    let manifest = release_task_manifest();
    let observer = RuntimeSurfaceObserver::new(&store, &providers);

    let snapshot = observer
        .observe_manifest(&workspace_scope(), DIGEST_A, 7, &manifest)
        .await
        .unwrap();
    assert_eq!(snapshot.surfaces().len(), 3);
    assert_eq!(
        observed_state(&snapshot, PackageSurfaceKind::Tool, "convert"),
        RuntimeSurfaceObservedState::Prepared
    );
    assert_eq!(
        observed_state(&snapshot, PackageSurfaceKind::Tool, "index"),
        RuntimeSurfaceObservedState::Healthy
    );
    assert_eq!(
        observed_state(&snapshot, PackageSurfaceKind::Mcp, "library"),
        RuntimeSurfaceObservedState::Healthy
    );
    assert!(snapshot
        .surfaces()
        .iter()
        .all(|surface| surface.surface().id != "local-library"));
    assert!(snapshot
        .surfaces()
        .iter()
        .all(|surface| surface.generation() == Some(7)));

    let host_observations = SurfaceObservations::from([
        (
            surface(PackageSurfaceKind::Mcp, "local-library"),
            SurfaceObservedState::Prepared,
        ),
        (
            surface(PackageSurfaceKind::Ui, "review"),
            SurfaceObservedState::Prepared,
        ),
        (
            surface(PackageSurfaceKind::Ui, "status"),
            SurfaceObservedState::Prepared,
        ),
        (
            surface(PackageSurfaceKind::Skill, "review"),
            SurfaceObservedState::Prepared,
        ),
        (
            surface(PackageSurfaceKind::Skill, "quick-look"),
            SurfaceObservedState::Prepared,
        ),
    ]);
    let reconciled = reconcile_with_runtime(
        &manifest,
        PluginDesiredState::Enabled,
        true,
        &host_observations,
        Some(&snapshot),
    )
    .unwrap();
    assert_eq!(reconciled.observed, PluginObservedState::Ready);
    assert!(reconciled.capability_ready);
    assert!(reconciled.publishes(PackageSurfaceKind::Skill, "review"));

    let mut collision = host_observations.clone();
    collision.insert(
        surface(PackageSurfaceKind::Tool, "index"),
        SurfaceObservedState::Prepared,
    );
    let collision_error = reconcile_with_runtime(
        &manifest,
        PluginDesiredState::Enabled,
        true,
        &collision,
        Some(&snapshot),
    )
    .unwrap_err();
    assert_eq!(collision_error.code, "use.plugin.reconcile_invalid");

    let mut changed_contract = manifest.clone();
    let index = changed_contract
        .tools
        .iter_mut()
        .find(|tool| tool.id == "index")
        .unwrap();
    let ToolWorkload::Service(service) = &mut index.workload else {
        panic!("index should be a Tool Service");
    };
    service.base_path = "/v2".to_string();
    let contract_error = observer
        .observe_manifest(&workspace_scope(), DIGEST_A, 7, &changed_contract)
        .await
        .unwrap_err();
    assert_eq!(
        contract_error.code,
        "use.plugin.runtime.binding_contract_mismatch"
    );

    service_runtime.restart_service(1_050, 1_100);
    let stale = observer
        .observe_manifest(&workspace_scope(), DIGEST_A, 7, &manifest)
        .await
        .unwrap();
    assert_eq!(
        observed_state(&stale, PackageSurfaceKind::Tool, "index"),
        RuntimeSurfaceObservedState::Stale
    );
    let broken = reconcile_with_runtime(
        &manifest,
        PluginDesiredState::Enabled,
        true,
        &host_observations,
        Some(&stale),
    )
    .unwrap();
    assert_eq!(broken.observed, PluginObservedState::Broken);
    assert!(!broken.capability_ready);

    let mismatched = observer
        .observe_manifest(&workspace_scope(), OTHER_PACKAGE_DIGEST, 7, &manifest)
        .await
        .unwrap_err();
    assert_eq!(
        mismatched.code,
        "use.plugin.runtime.binding_package_mismatch"
    );
}

#[tokio::test]
async fn unbound_surfaces_remain_pending_without_a_default_provider() {
    let temporary = TempDir::new().unwrap();
    let store = RuntimeBindingStore::new(temporary.path());
    let providers = RuntimeClientRegistry::new();
    let manifest = ExtensionManifest::parse_acl(include_str!(
        "../../extension//fixtures/manifests/plugin-v3.acl"
    ))
    .unwrap();

    let snapshot = RuntimeSurfaceObserver::new(&store, &providers)
        .observe_manifest(&workspace_scope(), DIGEST_A, 7, &manifest)
        .await
        .unwrap();
    assert_eq!(snapshot.surfaces().len(), 2);
    assert!(snapshot
        .surfaces()
        .iter()
        .all(|surface| surface.state() == RuntimeSurfaceObservedState::Unbound));
    assert!(snapshot
        .surfaces()
        .iter()
        .all(|surface| surface.surface().id != "convert"));
    let reconciled = reconcile_with_runtime(
        &manifest,
        PluginDesiredState::Enabled,
        true,
        &SurfaceObservations::new(),
        Some(&snapshot),
    )
    .unwrap();
    assert_eq!(reconciled.observed, PluginObservedState::Reconciling);
    assert!(!reconciled.capability_ready);
}

fn workspace_scope() -> PlanScope {
    PlanScope {
        kind: PlanScopeKind::Workspace,
        id: "workspace-01".to_owned(),
    }
}

async fn install_task_binding(store: &RuntimeBindingStore, providers: &mut RuntimeClientRegistry) {
    let plan = task_plan(7);
    let capabilities = named_capabilities(&plan, "task-runtime");
    let provider = evidence(&plan, &capabilities);
    let runtime = Arc::new(FakeRuntime::new(capabilities, true));
    let binding = PluginRuntimeClient::new(runtime.clone())
        .prepare_task(&plan, &provider)
        .await
        .unwrap();
    store
        .put(&RuntimeBindingReceipt::Task(binding))
        .await
        .unwrap();
    register(providers, "task-runtime", runtime);
}

fn task_plan(generation: u64) -> RuntimeSurfacePlan {
    let descriptor = task_descriptor();
    let mut surface = task_surface();
    surface.command = "acme-research-convert".to_string();
    let base = context(PackageSurfaceKind::Tool, "convert");
    let context = RuntimeSurfaceContext::new(
        base.package_id(),
        base.package_digest(),
        base.scope().clone(),
        base.grant_digest(),
        base.surface().clone(),
        generation,
    )
    .unwrap();
    plan_tool_task_release(
        context,
        &surface,
        &descriptor,
        artifact(&descriptor.artifact.digest, &descriptor.artifact.media_type),
        RuntimeTaskInvocation::new("invoke-01", Vec::new()).unwrap(),
        policy(),
        NetworkMode::None,
    )
    .unwrap()
}

async fn install_tool_service_binding(
    store: &RuntimeBindingStore,
    providers: &mut RuntimeClientRegistry,
) -> Arc<FakeRuntime> {
    let descriptor = service_descriptor();
    let plan = plan_tool_service_release(
        context(PackageSurfaceKind::Tool, "index"),
        &service_surface(),
        &descriptor,
        artifact(&descriptor.artifact.digest, &descriptor.artifact.media_type),
        policy(),
    )
    .unwrap();
    let capabilities = named_capabilities(&plan, "service-runtime");
    let provider = evidence(&plan, &capabilities);
    let runtime = Arc::new(FakeRuntime::new(capabilities, true));
    let receipt = PluginRuntimeClient::new(runtime.clone())
        .apply_service(&plan, &provider, "apply-index", Some(9_999_999))
        .await
        .unwrap()
        .into_tool_service_receipt(RuntimeEndpointRef::parse("gateway:workspace-01/index").unwrap())
        .unwrap();
    store
        .put(&RuntimeBindingReceipt::Service(receipt))
        .await
        .unwrap();
    register(providers, "service-runtime", runtime.clone());
    runtime
}

async fn install_mcp_service_binding(
    store: &RuntimeBindingStore,
    providers: &mut RuntimeClientRegistry,
) {
    let descriptor = mcp_descriptor();
    let plan = plan_mcp_service_release(
        context(PackageSurfaceKind::Mcp, "library"),
        &mcp_surface(),
        &descriptor,
        artifact(&descriptor.artifact.digest, &descriptor.artifact.media_type),
        policy(),
    )
    .unwrap();
    let capabilities = named_capabilities(&plan, "mcp-runtime");
    let provider = evidence(&plan, &capabilities);
    let runtime = Arc::new(FakeRuntime::new(capabilities, true));
    let receipt = PluginRuntimeClient::new(runtime.clone())
        .apply_service(&plan, &provider, "apply-library", Some(9_999_999))
        .await
        .unwrap()
        .into_mcp_service_receipt(
            RuntimeEndpointRef::parse("gateway:workspace-01/library").unwrap(),
            RuntimeMcpInitializeEvidence::new("2025-06-18", 1_001).unwrap(),
        )
        .unwrap();
    store
        .put(&RuntimeBindingReceipt::Service(receipt))
        .await
        .unwrap();
    register(providers, "mcp-runtime", runtime);
}

fn named_capabilities(
    plan: &RuntimeSurfacePlan,
    provider_id: &str,
) -> aster_runtime::contract::RuntimeCapabilities {
    let mut capabilities = capabilities(plan);
    capabilities.provider_id = ProviderId::parse(provider_id).unwrap();
    capabilities
}

fn register(
    providers: &mut RuntimeClientRegistry,
    provider_id: &str,
    client: Arc<dyn RuntimeClient>,
) {
    providers
        .register(Arc::new(StaticRuntimeFactory {
            provider_id: ProviderId::parse(provider_id).unwrap(),
            client,
        }))
        .unwrap();
}

fn observed_state(
    snapshot: &RuntimeSurfaceObservationSnapshot,
    kind: PackageSurfaceKind,
    id: &str,
) -> RuntimeSurfaceObservedState {
    snapshot
        .surfaces()
        .iter()
        .find(|surface| surface.surface().kind == kind && surface.surface().id == id)
        .map(RuntimeSurfaceObservation::state)
        .unwrap()
}

fn release_task_manifest() -> ExtensionManifest {
    let manifest = include_str!("../../extension//fixtures/manifests/plugin-v3.acl").replace(
        "executable  = \"tools/convert/bin/convert\"",
        "release     = \"releases/convert-tool-v1.json\"",
    );
    ExtensionManifest::parse_acl(&manifest).unwrap()
}

fn surface(kind: PackageSurfaceKind, id: &str) -> PluginSurfaceRef {
    PluginSurfaceRef {
        kind,
        id: id.to_string(),
    }
}
