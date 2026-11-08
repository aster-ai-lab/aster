use std::path::{Path, PathBuf};
use std::sync::Arc;

use aster_runtime::contract::{RuntimeObservation, RuntimeServiceEndpoint};
use aster_runtime::RuntimeClientRegistry;
use aster_use_core::{
    PackageSurfaceKind, PlanQualifiedSurfaceRef, PluginSurfaceRef, UseError, UseResult,
};
use aster_use_extension::{
    ExtensionLifecyclePackage, ExtensionManifest, ExtensionRegistry, PluginFlowSurface,
    PluginMcpLaunch, PluginMcpSurface, ToolSurface, ToolTaskSource, ToolWorkload,
};
use async_trait::async_trait;

use crate::okf_knowledge::{
    OkfKnowledgeBindingStore, OkfKnowledgeClient, SqliteOkfKnowledgeAdapter,
};
use crate::plugin_lifecycle::{
    ExtensionCapabilityLifecycleHost, ExtensionPackageLifecycleHost, OkfKnowledgeLifecycleHost,
    PluginFlowLifecycleHost, PluginLifecycleCoordinator, PluginLifecycleEvidence,
    PluginLifecycleHosts, PluginLifecycleIntent, PluginMcpServiceReadiness,
    PluginRuntimeServiceReadinessHost, PluginUiLifecycleHostFactory,
    RuntimePluginSurfaceLifecycleHost, StaticPluginSurfaceLifecycleHost,
    StaticPluginSurfaceLifecycleHostFactory,
};
use crate::plugin_runtime::{
    RuntimeBindingStore, RuntimeEndpointRef, RuntimeProviderSelection, RuntimeSurfacePlan,
};

use super::PackageLifecycleFactory;

/// Narrow lifecycle composition used by the standalone package engine.
///
/// Embedding hosts may wrap this factory for executable Tool Tasks, stdio MCP,
/// Skill, UI, and OKF packages. A declared Flow surface is refused. It deliberately
/// rejects Runtime Service and HTTP MCP surfaces until the host supplies their
/// real lifecycle adapters. OKF uses the scope-isolated SQLite/FTS5 Knowledge
/// backend.
#[derive(Debug, Clone, Default)]
pub struct StandalonePackageLifecycleFactory {
    flow_compiler_binary: Option<PathBuf>,
}

/// Host-composed lifecycle for release-backed Runtime Tasks and Services.
///
/// The selection contains exact process-local Runtime clients plus the
/// provider evidence already bound into the reviewed package plan. Gateway
/// readiness owns private endpoint publication, MCP initialization, route
/// drain, and route removal. Neither dependency can be selected by package
/// content.
#[derive(Clone)]
pub struct ManagedPackageLifecycleFactory {
    selection: RuntimeProviderSelection,
    runtime_registry: Arc<RuntimeClientRegistry>,
    readiness: Arc<dyn PluginRuntimeServiceReadinessHost>,
    ui_factory: Arc<dyn PluginUiLifecycleHostFactory>,
    flow_compiler_binary: Option<PathBuf>,
}

impl std::fmt::Debug for ManagedPackageLifecycleFactory {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ManagedPackageLifecycleFactory")
            .field("selection", &self.selection)
            .field("flow_compiler_binary", &self.flow_compiler_binary)
            .finish_non_exhaustive()
    }
}

/// Cross-host environment variable selecting the reviewed native TypeScript
/// compiler used by standalone Aster Flow package lifecycle operations.
pub const ASTER_FLOW_NATIVE_TS_COMPILER_ENV: &str = "ASTER_FLOW_NATIVE_TS_COMPILER";

#[derive(Clone)]
struct RuntimeLifecycleComposition {
    selection: RuntimeProviderSelection,
    registry: Arc<RuntimeClientRegistry>,
    readiness: Arc<dyn PluginRuntimeServiceReadinessHost>,
}

impl StandalonePackageLifecycleFactory {
    /// Construct the deterministic provider-free standalone lifecycle.
    pub fn new() -> Self {
        Self::default()
    }

    /// Refuse a Flow compiler. The Flow runtime is no longer built into aster-use.
    pub fn with_flow_compiler(_compiler_binary: impl Into<PathBuf>) -> UseResult<Self> {
        Err(flow_runtime_removed())
    }

    /// Build the standalone lifecycle. A declared Flow surface is refused.
    pub fn from_env() -> UseResult<Self> {
        Ok(Self::default())
    }

    pub fn flow_compiler_binary(&self) -> Option<&Path> {
        self.flow_compiler_binary.as_deref()
    }
}

impl ManagedPackageLifecycleFactory {
    pub fn new(
        selection: RuntimeProviderSelection,
        runtime_registry: Arc<RuntimeClientRegistry>,
        readiness: Arc<dyn PluginRuntimeServiceReadinessHost>,
    ) -> Self {
        Self {
            selection,
            runtime_registry,
            readiness,
            ui_factory: Arc::new(StaticPluginSurfaceLifecycleHostFactory),
            flow_compiler_binary: None,
        }
    }

    /// Replace the default static UI host with one trusted embedding-host
    /// composition. The injected host remains responsible for validating the
    /// immutable package assets before adding product-owned behavior.
    pub fn with_ui_lifecycle_factory(
        mut self,
        ui_factory: Arc<dyn PluginUiLifecycleHostFactory>,
    ) -> Self {
        self.ui_factory = ui_factory;
        self
    }

    pub fn with_flow_compiler(self, _compiler_binary: impl Into<PathBuf>) -> UseResult<Self> {
        Err(flow_runtime_removed())
    }

    pub fn selection(&self) -> &RuntimeProviderSelection {
        &self.selection
    }

    fn runtime_composition(&self) -> RuntimeLifecycleComposition {
        RuntimeLifecycleComposition {
            selection: self.selection.clone(),
            registry: self.runtime_registry.clone(),
            readiness: self.readiness.clone(),
        }
    }
}

impl PackageLifecycleFactory for StandalonePackageLifecycleFactory {
    fn name(&self) -> &'static str {
        "standalone"
    }

    fn validate_manifest(&self, manifest: &ExtensionManifest) -> UseResult<()> {
        validate_available_hosts(manifest, self.flow_compiler_binary())
    }

    fn install_coordinator(
        &self,
        registry: ExtensionRegistry,
        candidate: ExtensionLifecyclePackage,
        package_root: std::path::PathBuf,
    ) -> UseResult<PluginLifecycleCoordinator> {
        Ok(install_coordinator(
            registry,
            candidate,
            package_root,
            self.flow_compiler_binary(),
        ))
    }

    fn published_install_coordinator(
        &self,
        registry: ExtensionRegistry,
        package_root: std::path::PathBuf,
    ) -> UseResult<PluginLifecycleCoordinator> {
        Ok(published_install_coordinator(
            registry,
            package_root,
            self.flow_compiler_binary(),
        ))
    }

    fn uninstall_coordinator(
        &self,
        registry: ExtensionRegistry,
        package_root: std::path::PathBuf,
    ) -> UseResult<PluginLifecycleCoordinator> {
        Ok(uninstall_coordinator(
            registry,
            package_root,
            self.flow_compiler_binary(),
        ))
    }
}

impl PackageLifecycleFactory for ManagedPackageLifecycleFactory {
    fn name(&self) -> &'static str {
        "managed-runtime-gateway"
    }

    fn validate_manifest(&self, manifest: &ExtensionManifest) -> UseResult<()> {
        validate_managed_hosts(
            manifest,
            &self.selection,
            self.flow_compiler_binary.as_deref(),
        )
    }

    fn validate_manifest_for_planning(&self, manifest: &ExtensionManifest) -> UseResult<()> {
        validate_managed_host_availability(manifest, self.flow_compiler_binary.as_deref())
    }

    fn validate_manifest_for_retirement(&self, manifest: &ExtensionManifest) -> UseResult<()> {
        validate_managed_host_availability(manifest, self.flow_compiler_binary.as_deref())
    }

    fn install_coordinator(
        &self,
        registry: ExtensionRegistry,
        candidate: ExtensionLifecyclePackage,
        package_root: std::path::PathBuf,
    ) -> UseResult<PluginLifecycleCoordinator> {
        Ok(managed_install_coordinator(
            registry,
            candidate,
            package_root,
            self.runtime_composition(),
            self.ui_factory.clone(),
            self.flow_compiler_binary.as_deref(),
        ))
    }

    fn published_install_coordinator(
        &self,
        registry: ExtensionRegistry,
        package_root: std::path::PathBuf,
    ) -> UseResult<PluginLifecycleCoordinator> {
        Ok(managed_published_install_coordinator(
            registry,
            package_root,
            self.runtime_composition(),
            self.ui_factory.clone(),
            self.flow_compiler_binary.as_deref(),
        ))
    }

    fn uninstall_coordinator(
        &self,
        registry: ExtensionRegistry,
        package_root: std::path::PathBuf,
    ) -> UseResult<PluginLifecycleCoordinator> {
        Ok(managed_uninstall_coordinator(
            registry,
            package_root,
            self.runtime_composition(),
            self.ui_factory.clone(),
            self.flow_compiler_binary.as_deref(),
        ))
    }
}

pub(super) fn validate_available_hosts(
    manifest: &ExtensionManifest,
    flow_compiler_binary: Option<&Path>,
) -> UseResult<()> {
    let _ = flow_compiler_binary;
    if !manifest.flows.is_empty() {
        return Err(flow_runtime_removed().with_detail(
            "surfaces",
            serde_json::json!(manifest
                .flows
                .iter()
                .map(|value| &value.id)
                .collect::<Vec<_>>()),
        ));
    }

    let runtime_tools = manifest
        .tools
        .iter()
        .filter(|surface| {
            !matches!(
                &surface.workload,
                ToolWorkload::Task(task)
                    if matches!(&task.source, ToolTaskSource::Executable { .. })
            )
        })
        .map(|surface| surface.id.as_str())
        .collect::<Vec<_>>();
    let runtime_mcp = manifest
        .mcp_servers
        .iter()
        .filter(|surface| matches!(surface.launch, PluginMcpLaunch::StreamableHttp { .. }))
        .map(|surface| surface.id.as_str())
        .collect::<Vec<_>>();
    if !runtime_tools.is_empty() || !runtime_mcp.is_empty() {
        return Err(provider_error(
            "use.plugin.runtime_provider_required",
            format!(
                "package '{}' requires explicit Runtime and Gateway provider evidence.",
                manifest.package_id
            ),
        )
        .with_detail("toolSurfaces", serde_json::json!(runtime_tools))
        .with_detail("mcpSurfaces", serde_json::json!(runtime_mcp))
        .with_suggestion(
            "Install through an Aster host that injects exact Runtime provider selections and service readiness evidence.",
        ));
    }
    Ok(())
}

fn validate_managed_hosts(
    manifest: &ExtensionManifest,
    selection: &RuntimeProviderSelection,
    flow_compiler_binary: Option<&Path>,
) -> UseResult<()> {
    validate_managed_host_availability(manifest, flow_compiler_binary)?;
    let mut required = manifest
        .tools
        .iter()
        .filter(|surface| {
            !matches!(
                &surface.workload,
                ToolWorkload::Task(task)
                    if matches!(&task.source, ToolTaskSource::Executable { .. })
            )
        })
        .map(|surface| PlanQualifiedSurfaceRef {
            package_id: manifest.package_id.clone(),
            surface: PluginSurfaceRef {
                kind: PackageSurfaceKind::Tool,
                id: surface.id.clone(),
            },
        })
        .chain(
            manifest
                .mcp_servers
                .iter()
                .filter(|surface| matches!(surface.launch, PluginMcpLaunch::StreamableHttp { .. }))
                .map(|surface| PlanQualifiedSurfaceRef {
                    package_id: manifest.package_id.clone(),
                    surface: PluginSurfaceRef {
                        kind: PackageSurfaceKind::Mcp,
                        id: surface.id.clone(),
                    },
                }),
        )
        .collect::<Vec<_>>();
    required.sort();
    let missing = required
        .iter()
        .filter(|required| {
            !selection
                .surfaces()
                .iter()
                .any(|selected| selected.plan().surface() == **required)
        })
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(provider_error(
            "use.plugin.runtime_provider_required",
            format!(
                "package '{}' lacks exact managed Runtime selections.",
                manifest.package_id
            ),
        )
        .with_detail(
            "surfaces",
            serde_json::to_value(missing).unwrap_or_default(),
        ));
    }
    Ok(())
}

fn validate_managed_host_availability(
    manifest: &ExtensionManifest,
    flow_compiler_binary: Option<&Path>,
) -> UseResult<()> {
    let _ = flow_compiler_binary;
    if !manifest.flows.is_empty() {
        return Err(flow_runtime_removed());
    }
    Ok(())
}

pub(super) fn install_coordinator(
    registry: ExtensionRegistry,
    candidate: ExtensionLifecyclePackage,
    package_root: impl Into<std::path::PathBuf>,
    flow_compiler_binary: Option<&Path>,
) -> PluginLifecycleCoordinator {
    let paths = registry.paths().clone();
    let package = Arc::new(ExtensionPackageLifecycleHost::new(
        registry.clone(),
        candidate,
    ));
    coordinator(
        registry,
        package,
        package_root,
        &paths,
        RuntimeLifecycleComposition {
            selection: RuntimeProviderSelection::default(),
            registry: Arc::new(RuntimeClientRegistry::new()),
            readiness: Arc::new(UnavailableRuntimeServiceReadinessHost),
        },
        Arc::new(StaticPluginSurfaceLifecycleHostFactory),
        flow_compiler_binary,
    )
}

pub(super) fn uninstall_coordinator(
    registry: ExtensionRegistry,
    package_root: impl Into<std::path::PathBuf>,
    flow_compiler_binary: Option<&Path>,
) -> PluginLifecycleCoordinator {
    let paths = registry.paths().clone();
    let package = Arc::new(ExtensionPackageLifecycleHost::for_installed(
        registry.clone(),
    ));
    coordinator(
        registry,
        package,
        package_root,
        &paths,
        RuntimeLifecycleComposition {
            selection: RuntimeProviderSelection::default(),
            registry: Arc::new(RuntimeClientRegistry::new()),
            readiness: Arc::new(UnavailableRuntimeServiceReadinessHost),
        },
        Arc::new(StaticPluginSurfaceLifecycleHostFactory),
        flow_compiler_binary,
    )
}

/// Resume an install whose exact generation is already committed and visible.
/// The installed package host deliberately carries no candidate: a replay may
/// finish publication journals, but it cannot recommit missing package bytes.
pub(super) fn published_install_coordinator(
    registry: ExtensionRegistry,
    package_root: impl Into<std::path::PathBuf>,
    flow_compiler_binary: Option<&Path>,
) -> PluginLifecycleCoordinator {
    let paths = registry.paths().clone();
    let package = Arc::new(ExtensionPackageLifecycleHost::for_installed(
        registry.clone(),
    ));
    coordinator(
        registry,
        package,
        package_root,
        &paths,
        RuntimeLifecycleComposition {
            selection: RuntimeProviderSelection::default(),
            registry: Arc::new(RuntimeClientRegistry::new()),
            readiness: Arc::new(UnavailableRuntimeServiceReadinessHost),
        },
        Arc::new(StaticPluginSurfaceLifecycleHostFactory),
        flow_compiler_binary,
    )
}

fn managed_install_coordinator(
    registry: ExtensionRegistry,
    candidate: ExtensionLifecyclePackage,
    package_root: impl Into<std::path::PathBuf>,
    runtime: RuntimeLifecycleComposition,
    ui_factory: Arc<dyn PluginUiLifecycleHostFactory>,
    flow_compiler_binary: Option<&Path>,
) -> PluginLifecycleCoordinator {
    let paths = registry.paths().clone();
    let package = Arc::new(ExtensionPackageLifecycleHost::new(
        registry.clone(),
        candidate,
    ));
    coordinator(
        registry,
        package,
        package_root,
        &paths,
        runtime,
        ui_factory,
        flow_compiler_binary,
    )
}

fn managed_uninstall_coordinator(
    registry: ExtensionRegistry,
    package_root: impl Into<std::path::PathBuf>,
    runtime: RuntimeLifecycleComposition,
    ui_factory: Arc<dyn PluginUiLifecycleHostFactory>,
    flow_compiler_binary: Option<&Path>,
) -> PluginLifecycleCoordinator {
    let paths = registry.paths().clone();
    let package = Arc::new(ExtensionPackageLifecycleHost::for_installed(
        registry.clone(),
    ));
    coordinator(
        registry,
        package,
        package_root,
        &paths,
        runtime,
        ui_factory,
        flow_compiler_binary,
    )
}

fn managed_published_install_coordinator(
    registry: ExtensionRegistry,
    package_root: impl Into<std::path::PathBuf>,
    runtime: RuntimeLifecycleComposition,
    ui_factory: Arc<dyn PluginUiLifecycleHostFactory>,
    flow_compiler_binary: Option<&Path>,
) -> PluginLifecycleCoordinator {
    managed_uninstall_coordinator(
        registry,
        package_root,
        runtime,
        ui_factory,
        flow_compiler_binary,
    )
}

fn coordinator(
    registry: ExtensionRegistry,
    package: Arc<dyn crate::plugin_lifecycle::PluginPackageLifecycleHost>,
    package_root: impl Into<std::path::PathBuf>,
    paths: &aster_use_extension::ExtensionPaths,
    runtime: RuntimeLifecycleComposition,
    ui_factory: Arc<dyn PluginUiLifecycleHostFactory>,
    flow_compiler_binary: Option<&Path>,
) -> PluginLifecycleCoordinator {
    let package_root = package_root.into();
    let capability = Arc::new(ExtensionCapabilityLifecycleHost::new(registry));
    let runtime = Arc::new(RuntimePluginSurfaceLifecycleHost::new(
        &package_root,
        runtime.selection,
        runtime.registry,
        RuntimeBindingStore::from_extension_paths(paths),
        runtime.readiness,
    ));
    let static_surfaces = Arc::new(StaticPluginSurfaceLifecycleHost::new(package_root.clone()));
    let ui = ui_factory.create(package_root.clone());
    let okf = Arc::new(OkfKnowledgeLifecycleHost::new(
        package_root.clone(),
        OkfKnowledgeClient::new(Arc::new(SqliteOkfKnowledgeAdapter::from_extension_paths(
            paths,
        ))),
        OkfKnowledgeBindingStore::from_extension_paths(paths),
    ));
    let _flow_compiler_binary = flow_compiler_binary;
    let flow: Arc<dyn PluginFlowLifecycleHost> = Arc::new(UnavailableFlowLifecycleHost);
    let hosts = PluginLifecycleHosts::new(
        package,
        capability,
        runtime.clone(),
        runtime,
        okf,
        flow,
        static_surfaces.clone(),
        ui,
    );
    PluginLifecycleCoordinator::new(
        crate::plugin_lifecycle::PluginLifecycleJournalStore::from_extension_paths(paths),
        hosts,
    )
}

struct UnavailableRuntimeServiceReadinessHost;

#[async_trait]
impl PluginRuntimeServiceReadinessHost for UnavailableRuntimeServiceReadinessHost {
    async fn bind_tool_service(
        &self,
        _intent: &PluginLifecycleIntent,
        _surface: &ToolSurface,
        _plan: &RuntimeSurfacePlan,
        _observation: &RuntimeObservation,
        _runtime_endpoint: &RuntimeServiceEndpoint,
        _idempotency_key: &str,
        _deadline_at_ms: Option<u64>,
    ) -> UseResult<RuntimeEndpointRef> {
        Err(provider_error(
            "use.plugin.runtime_provider_required",
            "No Runtime Service readiness host was injected for this package operation.",
        ))
    }

    async fn bind_mcp_service(
        &self,
        _intent: &PluginLifecycleIntent,
        _surface: &PluginMcpSurface,
        _plan: &RuntimeSurfacePlan,
        _observation: &RuntimeObservation,
        _runtime_endpoint: &RuntimeServiceEndpoint,
        _idempotency_key: &str,
        _deadline_at_ms: Option<u64>,
    ) -> UseResult<PluginMcpServiceReadiness> {
        Err(provider_error(
            "use.plugin.runtime_provider_required",
            "No MCP Gateway readiness host was injected for this package operation.",
        ))
    }

    async fn drain_service(
        &self,
        _intent: &PluginLifecycleIntent,
        _receipt: &crate::plugin_runtime::RuntimeServiceBindingReceipt,
        _idempotency_key: &str,
        _deadline_at_ms: Option<u64>,
    ) -> UseResult<()> {
        Err(provider_error(
            "use.plugin.runtime_provider_required",
            "No Gateway lifecycle host was injected to drain this package Service.",
        ))
    }

    async fn remove_service(
        &self,
        _intent: &PluginLifecycleIntent,
        _receipt: &crate::plugin_runtime::RuntimeServiceBindingReceipt,
        _idempotency_key: &str,
        _deadline_at_ms: Option<u64>,
    ) -> UseResult<()> {
        Err(provider_error(
            "use.plugin.runtime_provider_required",
            "No Gateway lifecycle host was injected to remove this package Service binding.",
        ))
    }
}

struct UnavailableFlowLifecycleHost;

#[async_trait]
impl PluginFlowLifecycleHost for UnavailableFlowLifecycleHost {
    async fn prepare_flow(
        &self,
        _intent: &PluginLifecycleIntent,
        _surface: &PluginFlowSurface,
        _idempotency_key: &str,
    ) -> UseResult<PluginLifecycleEvidence> {
        Err(flow_unavailable())
    }

    async fn stop_flow(
        &self,
        _intent: &PluginLifecycleIntent,
        _surface: &PluginFlowSurface,
        _idempotency_key: &str,
    ) -> UseResult<PluginLifecycleEvidence> {
        Err(flow_unavailable())
    }

    async fn remove_flow(
        &self,
        _intent: &PluginLifecycleIntent,
        _surface: &PluginFlowSurface,
        _idempotency_key: &str,
    ) -> UseResult<PluginLifecycleEvidence> {
        Err(flow_unavailable())
    }
}

fn flow_unavailable() -> UseError {
    flow_runtime_removed()
}

fn flow_runtime_removed() -> UseError {
    provider_error(
        "use.flow_runtime_removed",
        "The Flow runtime is no longer built into aster-use.",
    )
}

fn provider_error(code: &'static str, message: impl Into<String>) -> UseError {
    UseError::new(code, message)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;

    struct InjectedLifecycleFactory;

    struct RecordingUiFactory(Arc<AtomicBool>);

    impl PluginUiLifecycleHostFactory for RecordingUiFactory {
        fn create(
            &self,
            package_root: PathBuf,
        ) -> Arc<dyn crate::plugin_lifecycle::PluginUiLifecycleHost> {
            self.0.store(true, Ordering::SeqCst);
            Arc::new(StaticPluginSurfaceLifecycleHost::new(package_root))
        }
    }

    impl PackageLifecycleFactory for InjectedLifecycleFactory {
        fn name(&self) -> &'static str {
            "test-injected"
        }

        fn validate_manifest(&self, _manifest: &ExtensionManifest) -> UseResult<()> {
            Ok(())
        }

        fn install_coordinator(
            &self,
            _registry: ExtensionRegistry,
            _candidate: ExtensionLifecyclePackage,
            _package_root: std::path::PathBuf,
        ) -> UseResult<PluginLifecycleCoordinator> {
            Err(provider_error(
                "use.plugin.test_factory_not_applied",
                "The test factory does not compose an install coordinator.",
            ))
        }

        fn published_install_coordinator(
            &self,
            _registry: ExtensionRegistry,
            _package_root: std::path::PathBuf,
        ) -> UseResult<PluginLifecycleCoordinator> {
            Err(provider_error(
                "use.plugin.test_factory_not_applied",
                "The test factory does not compose a replay coordinator.",
            ))
        }

        fn uninstall_coordinator(
            &self,
            _registry: ExtensionRegistry,
            _package_root: std::path::PathBuf,
        ) -> UseResult<PluginLifecycleCoordinator> {
            Err(provider_error(
                "use.plugin.test_factory_not_applied",
                "The test factory does not compose an uninstall coordinator.",
            ))
        }
    }

    #[test]
    fn runtime_services_fail_before_lifecycle_composition_without_an_injected_provider() {
        let manifest = ExtensionManifest::parse_acl(include_str!(
            "../../extension//fixtures/manifests/plugin-v3.acl"
        ))
        .unwrap();
        let error = validate_available_hosts(&manifest, None).unwrap_err();
        assert_eq!(error.code, "use.plugin.runtime_provider_required");
    }

    #[test]
    fn standalone_accepts_okf_surfaces_with_the_local_knowledge_backend() {
        let manifest = ExtensionManifest::parse_acl(include_str!(
            "../../extension//fixtures/manifests/plugin-v3-okf.acl"
        ))
        .unwrap();
        validate_available_hosts(&manifest, None).unwrap();
    }

    #[test]
    fn managed_factory_requires_an_exact_selection_for_each_runtime_surface() {
        let manifest = ExtensionManifest::parse_acl(include_str!(
            "../../extension//fixtures/manifests/plugin-v3.acl"
        ))
        .unwrap();
        let factory = ManagedPackageLifecycleFactory::new(
            RuntimeProviderSelection::default(),
            Arc::new(RuntimeClientRegistry::new()),
            Arc::new(UnavailableRuntimeServiceReadinessHost),
        );

        let error = factory.validate_manifest(&manifest).unwrap_err();
        assert_eq!(factory.name(), "managed-runtime-gateway");
        assert_eq!(error.code, "use.plugin.runtime_provider_required");
    }

    #[test]
    fn managed_factory_retires_runtime_surfaces_without_a_candidate_selection() {
        let manifest = ExtensionManifest::parse_acl(include_str!(
            "../../extension//fixtures/manifests/plugin-v3.acl"
        ))
        .unwrap();
        let factory = ManagedPackageLifecycleFactory::new(
            RuntimeProviderSelection::default(),
            Arc::new(RuntimeClientRegistry::new()),
            Arc::new(UnavailableRuntimeServiceReadinessHost),
        );

        factory.validate_manifest_for_retirement(&manifest).unwrap();
    }

    #[test]
    fn managed_factory_uses_the_embedding_hosts_ui_composition() {
        let temp = tempfile::tempdir().unwrap();
        let created = Arc::new(AtomicBool::new(false));
        let factory = ManagedPackageLifecycleFactory::new(
            RuntimeProviderSelection::default(),
            Arc::new(RuntimeClientRegistry::new()),
            Arc::new(UnavailableRuntimeServiceReadinessHost),
        )
        .with_ui_lifecycle_factory(Arc::new(RecordingUiFactory(created.clone())));
        let registry = ExtensionRegistry::new(aster_use_extension::ExtensionPaths::new(
            temp.path().join("data"),
            temp.path().join("state"),
        ));

        factory
            .published_install_coordinator(registry, temp.path().join("package"))
            .unwrap();

        assert!(created.load(Ordering::SeqCst));
    }

    #[test]
    fn flow_surfaces_fail_before_lifecycle_composition_without_aster_flow() {
        let manifest = flow_manifest();
        let factory = StandalonePackageLifecycleFactory::default();
        let error = factory.validate_manifest(&manifest).unwrap_err();
        assert_eq!(error.code, "use.flow_runtime_removed");
    }

    #[test]
    fn explicit_flow_compiler_is_refused() {
        let temp = tempfile::tempdir().unwrap();
        let error = StandalonePackageLifecycleFactory::with_flow_compiler(
            temp.path().join("aster-flow-native-compiler"),
        )
        .unwrap_err();
        assert_eq!(error.code, "use.flow_runtime_removed");
    }

    #[test]
    fn relative_aster_flow_compiler_is_rejected_before_composition() {
        let error =
            StandalonePackageLifecycleFactory::with_flow_compiler("bin/aster-flow-native-compiler")
                .unwrap_err();

        assert_eq!(error.code, "use.flow_runtime_removed");
    }

    fn flow_manifest() -> ExtensionManifest {
        ExtensionManifest::parse_acl(
            r#"
extension "acme/flow" {
  schema_version = 3
  version        = "1.0.0"
  route          = "flow"
  requires_use   = ">=0.3.0, <0.4.0"
  actions        = ["read"]

  repository {
    url      = "https://github.com/acme/flow"
    revision = "0123456789abcdef0123456789abcdef01234567"
  }

  flow "review" {
    engine        = "aster-flow"
    runtime       = "native-ts"
    source        = "flows/review.ts"
    export        = "run"
    requires_tool = []
    requires_mcp  = []
    requires_okf  = []
    optional      = false
  }
}
"#,
        )
        .unwrap()
    }

    #[test]
    fn embedding_hosts_can_replace_the_standalone_lifecycle_factory() {
        let temp = tempfile::tempdir().unwrap();
        let registry = ExtensionRegistry::new(aster_use_extension::ExtensionPaths::new(
            temp.path().join("data"),
            temp.path().join("state"),
        ));
        let manager = super::super::PackageManager::with_lifecycle(
            registry,
            Arc::new(InjectedLifecycleFactory),
        )
        .unwrap();
        let manifest = ExtensionManifest::parse_acl(include_str!(
            "../../extension//fixtures/manifests/plugin-v3-okf.acl"
        ))
        .unwrap();

        assert_eq!(manager.lifecycle().name(), "test-injected");
        manager.lifecycle().validate_manifest(&manifest).unwrap();
    }
}
