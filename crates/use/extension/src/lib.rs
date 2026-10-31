use std::collections::BTreeSet;
use std::path::{Component, Path};

use aster_acl::{Block, Value};
use aster_use_core::{
    PackageSurfaceKind, PluginPackageDependency, PluginSurfaceRef, RiskClass, UseError, UseResult,
};
use serde::{Deserialize, Serialize};

#[cfg(test)]
extern crate self as aster_use_extension;

#[cfg(all(test, any(unix, windows)))]
mod test_filesystem;

mod agent_parameters;
mod atomic_file;
mod composer_mode;
mod digest;
mod home;
mod package;
mod package_inventory;
#[cfg(test)]
mod package_inventory_tests;
#[cfg(test)]
mod package_tests;
mod paths;
mod plugin_manifest;
#[cfg(test)]
mod plugin_manifest_tests;
mod registry;
mod registry_io;
mod registry_sources;
mod release_bundle;
mod remote;
mod route_lock;
mod source;
mod state_maintenance;
mod studio_contract;
mod surface_files;
#[cfg(test)]
#[path = "workbench_command_tests.rs"]
mod workbench_command_tests;
mod workbench_commands;
mod workbench_manifest;
#[cfg(test)]
mod workbench_manifest_tests;
mod workspace_grant;
mod workspace_grant_io;
mod workspace_grant_lifecycle;
#[cfg(test)]
mod workspace_grant_lifecycle_fault_matrix;
mod workspace_grant_operation;
mod workspace_grant_operation_io;
mod workspace_grant_snapshot;
#[cfg(test)]
mod workspace_grant_tests;

pub use agent_parameters::{
    parse_agent_parameter_schema, validate_agent_parameter_values, AgentParameterField,
    AgentParameterKind, AgentParameterSchema,
};
#[doc(hidden)]
pub use atomic_file::{
    persist_named_temporary_noclobber_blocking, persist_temporary_noclobber_blocking,
    persist_temporary_replace_blocking, rename_path_with_windows_retry_blocking,
};
pub use composer_mode::{
    PluginComposerMode, PluginComposerModeKeptNotice, PluginComposerModeScope,
    COMPOSER_MODE_CONTRACT_VERSION, MAX_COMPOSER_MODES,
};
pub use paths::ExtensionPaths;
pub use plugin_manifest::{
    AgentHarness, AgentResources, AgentRuntimePlace, PackagePluginSurface, PluginAgentSurface,
    PluginFlowEngine, PluginFlowRuntime, PluginFlowSurface, PluginMcpLaunch, PluginMcpSurface,
    PluginOkfSurface, PluginSkillSurface, PluginUiSurface, SkillHostMode, SurfaceActivation,
    ToolServiceSurface, ToolSurface, ToolTaskSource, ToolTaskSurface, ToolWorkload,
};
pub use registry::{
    validate_catalog_manifest_binding, ExtensionLifecycleGraphPublication,
    ExtensionLifecycleIdentity, ExtensionLifecyclePackage, ExtensionLifecycleResult,
    ExtensionLifecycleRollbackResult, ExtensionReceipt, ExtensionRegistry,
    ExtensionRegistryCutoverRecord, ExtensionRegistrySnapshot, ExtensionRouteBinding,
    ExtensionRouteLease, ExtensionSnapshotCursor, ExtensionSnapshotLease, ExtensionSnapshotPackage,
    ExtensionTrust, InstalledExtension, UninstallResult, EXTENSION_RECEIPT_SCHEMA_VERSION,
    EXTENSION_REGISTRY_CUTOVER_SCHEMA, EXTENSION_SNAPSHOT_CURSOR_SCHEMA,
    MAX_PENDING_REGISTRY_CUTOVERS,
};
pub use registry_sources::{
    GitHubRegistryRepository, RegistrySource, RegistrySourceInput, RegistrySourceMutation,
    RegistrySourceSnapshot, RegistrySourceStore, ResolvedRegistrySources,
    DEFAULT_GITHUB_REGISTRY_PATH, DEFAULT_GITHUB_REGISTRY_REF, MAX_CONFIGURED_REGISTRY_SOURCES,
    REGISTRY_SOURCE_CONFIG_SCHEMA_VERSION,
};
pub use release_bundle::{
    inspect_release_bundle, ReleaseBundlePackage, RELEASE_BUNDLE_SCHEMA_VERSION,
};
pub use remote::{
    download_locked_cached_remote_packages, download_locked_remote_packages,
    download_selected_locked_cached_remote_packages, download_selected_locked_remote_packages,
    fetch_cached_package_media, fetch_package_media, inspect_bootstrap_root,
    inspect_cached_package_presentation, inspect_cached_plugin, inspect_package_presentation,
    inspect_remote_plugin, inspect_verified_target_cache, list_remote_packages,
    plugin_catalog_host_input_schema, plugin_catalog_inspection_input_schema,
    plugin_catalog_search_input_schema, prepare_cached_remote_package, prepare_remote_package,
    prune_verified_target_cache, refresh_remote_registry, resolve_cached_remote_package_lock,
    resolve_cached_remote_package_lock_with_observer, resolve_remote_package_lock,
    resolve_remote_package_lock_with_observer, search_cached_plugins, search_remote_plugins,
    DownloadedRemotePackage, PackageFormFactor, PackageMediaKind, PackagePresentationIndexV1,
    PackagePresentationMediaV1, PackagePresentationRecordV1, PackagePresentationV1,
    PackageRegistryResolutionObserver, PinnedBootstrapRoot, PluginCatalogAvailability,
    PluginCatalogHost, PluginCatalogInspection, PluginCatalogPage, PluginCatalogSearch,
    PluginCatalogSnapshot, PluginCatalogSnapshotSource, PreparedRemotePackage,
    RegistryNetworkPolicy, ResolvedRemotePackage, TrustedRegistry, VerifiedPackageMedia,
    VerifiedPackagePresentation, VerifiedRegistryCatalog, VerifiedRegistryMetadata,
    VerifiedTargetCachePolicy, VerifiedTargetCachePruneResult, VerifiedTargetCacheUsage,
    VerifiedTargetObservation, VerifiedTargetObservationStatus,
    DEFAULT_VERIFIED_TARGET_CACHE_MAX_BYTES, DEFAULT_VERIFIED_TARGET_CACHE_MAX_ENTRIES,
    DEFAULT_VERIFIED_TARGET_CACHE_MIN_FREE_BYTES, MAX_BOOTSTRAP_ROOT_BYTES,
    MAX_PACKAGE_MEDIA_BYTES, MAX_PACKAGE_PRESENTATION_MEDIA, MAX_PLUGIN_CATALOG_PAGE_BYTES,
    MAX_PLUGIN_CATALOG_PAGE_SIZE, PACKAGE_PRESENTATION_INDEX_SCHEMA, PACKAGE_PRESENTATION_SCHEMA,
    VERIFIED_TARGET_CACHE_SCHEMA_VERSION,
};
pub use state_maintenance::{
    StateMaintenanceGuard, StateMaintenanceLock, ACTIVE_STATE_RESTORE_MARKER,
};
pub use studio_contract::{
    studio_extension_point_contracts, StudioComposition, StudioCompositionMode, StudioDeclaration,
    StudioExtensionPoint, StudioExtensionPointContract, StudioFallback, StudioStatePolicy,
};
pub use surface_files::{
    inspect_agent_surface_file, inspect_flow_surface_file, inspect_mcp_surface_files,
    inspect_plugin_surface_file, inspect_skill_surface_file, inspect_tool_surface_files,
    inspect_ui_surface_files, list_ui_package_assets, load_okf_bundle_files, ui_asset_media,
    PluginSurfaceFileEvidence, UiPackageAsset,
};
pub use workbench_commands::{
    PluginKeybinding, PluginPackageCommand, PluginUiCommand, PluginUiMenu, PluginUiMenuLocation,
    PluginWorkbenchMenu, PluginWorkbenchMenuLocation,
};
pub use workbench_manifest::{
    PluginEditorPriority, PluginUiCondition, PluginUiContextKey, PluginUiContributionIdentity,
    PluginUiContributionKind, PluginUiPoint, PluginUiPresentation, PluginUiScope, PluginUiTheme,
    PluginViewContainer, PluginViewLocation, STUDIO_CONTRIBUTION_PROTOCOL_VERSION,
};
pub use workspace_grant::{
    StoredWorkspaceGrant, WorkspaceGrantReceipt, WorkspaceGrantRevocation, WorkspaceGrantStore,
    WORKSPACE_GRANT_RECEIPT_SCHEMA, WORKSPACE_GRANT_REVOCATION_SCHEMA,
};
pub use workspace_grant_operation::{
    WorkspaceGrantCandidateCeiling, WorkspaceGrantCutoverEvidence, WorkspaceGrantLifecyclePhase,
    WorkspaceGrantOperationIntent, WorkspaceGrantOperationJournal, WorkspaceGrantPreparedCandidate,
    WorkspaceGrantRetirement, WorkspaceGrantRollbackEvidence, WORKSPACE_GRANT_CUTOVER_SCHEMA,
    WORKSPACE_GRANT_OPERATION_SCHEMA, WORKSPACE_GRANT_ROLLBACK_SCHEMA,
};

const RESERVED_ROUTES: &[&str] = &[
    "browser",
    "box",
    "capability",
    "ocr",
    "capabilities",
    "component",
    "registry",
    "extension",
    "doctor",
    "mcp",
    "help",
    "version",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionManifest {
    pub schema_version: u32,
    pub package_id: String,
    pub version: String,
    pub route: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires_use: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<PluginPackageDependency>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<ExtensionRepository>,
    pub actions: Vec<RiskClass>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolSurface>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mcp_servers: Vec<PluginMcpSurface>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub okf: Vec<PluginOkfSurface>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flows: Vec<PluginFlowSurface>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<PluginSkillSurface>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agents: Vec<PluginAgentSurface>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plugins: Vec<PackagePluginSurface>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ui: Vec<PluginUiSurface>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub view_containers: Vec<PluginViewContainer>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<PluginPackageCommand>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub menus: Vec<PluginWorkbenchMenu>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keybindings: Vec<PluginKeybinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub harness: Vec<workbench_commands::PluginHarnessComponent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub home: Vec<home::PluginHomeCard>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub composer_modes: Vec<PluginComposerMode>,
}

/// One named package contribution and its manifest-local
/// dependencies.
///
/// The package remains the lifecycle unit. Hosts use this inventory to stage
/// Tool, MCP, OKF, Flow, Skill, and UI contributions in dependency order and to
/// remove them in reverse order; a surface is never installed independently
/// from its owning package generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManifestPluginSurface {
    pub surface: PluginSurfaceRef,
    pub activation: SurfaceActivation,
    pub optional: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<PluginSurfaceRef>,
}

/// Source repository identity carried by a versioned external capability
/// package. Trust still comes from the installation source and package digest;
/// this metadata lets hosts and users trace the capability back to its
/// canonical project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionRepository {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
}

impl ExtensionManifest {
    pub fn parse_acl(input: &str) -> UseResult<Self> {
        let document = aster_acl::parse_acl(input).map_err(|error| {
            UseError::new(
                "use.extension.manifest_invalid",
                format!("Failed to parse extension ACL: {error}"),
            )
        })?;
        if document.blocks.len() != 1 || document.blocks[0].name != "extension" {
            return Err(UseError::new(
                "use.extension.manifest_invalid",
                "The manifest must contain only one extension block.",
            ));
        }
        let extension_blocks = document
            .blocks
            .iter()
            .filter(|block| block.name == "extension")
            .collect::<Vec<_>>();
        let [block] = extension_blocks.as_slice() else {
            return Err(UseError::new(
                "use.extension.manifest_invalid",
                "The manifest must contain exactly one extension block.",
            ));
        };
        parse_extension_block(block)
    }

    pub fn validate_package_root(&self, package_root: &Path) -> UseResult<()> {
        let mut paths = Vec::new();
        for tool in &self.tools {
            paths.extend(tool.package_paths());
        }
        for mcp in &self.mcp_servers {
            paths.extend(mcp.package_paths());
        }
        paths.extend(
            self.okf
                .iter()
                .map(|surface| Path::new(surface.bundle.root.as_str())),
        );
        for flow in &self.flows {
            paths.extend(flow.package_paths());
        }
        paths.extend(self.skills.iter().map(|surface| surface.path.as_path()));
        for agent in &self.agents {
            paths.extend(agent.package_paths());
        }
        paths.extend(self.plugins.iter().map(|surface| surface.entry.as_path()));
        for ui in &self.ui {
            paths.extend(ui.package_paths());
        }

        for path in paths {
            validate_relative_path(path)?;
            let resolved = package_root.join(path);
            if !resolved.starts_with(package_root) {
                return Err(UseError::new(
                    "use.extension.path_escape",
                    format!("Surface path '{}' escapes the package.", path.display()),
                ));
            }
        }
        Ok(())
    }

    pub fn surface_kinds(&self) -> Vec<&'static str> {
        let mut surfaces = Vec::with_capacity(6);
        if !self.tools.is_empty() {
            surfaces.push("tool");
        }
        if !self.mcp_servers.is_empty() {
            surfaces.push("mcp");
        }
        if !self.okf.is_empty() {
            surfaces.push("okf");
        }
        if !self.flows.is_empty() {
            surfaces.push("flow");
        }
        if !self.skills.is_empty() {
            surfaces.push("skill");
        }
        if !self.agents.is_empty() {
            surfaces.push("agent");
        }
        if !self.plugins.is_empty() {
            surfaces.push("plugin");
        }
        if !self.ui.is_empty() {
            surfaces.push("ui");
        }
        surfaces
    }

    /// Return the complete package surface graph in canonical identity
    /// order.
    ///
    /// Dependency validation happens while parsing the manifest. This method
    /// deliberately exposes one shared graph to reconciliation and lifecycle
    /// orchestration so their activation and removal rules cannot drift.
    pub fn plugin_surfaces(&self) -> UseResult<Vec<ManifestPluginSurface>> {
        let mut surfaces = Vec::with_capacity(
            self.tools.len()
                + self.mcp_servers.len()
                + self.okf.len()
                + self.flows.len()
                + self.skills.len()
                + self.agents.len()
                + self.plugins.len()
                + self.ui.len(),
        );
        surfaces.extend(self.tools.iter().map(|surface| ManifestPluginSurface {
            surface: plugin_surface_ref(PackageSurfaceKind::Tool, &surface.id),
            activation: surface.activation,
            optional: surface.optional,
            dependencies: Vec::new(),
        }));
        surfaces.extend(
            self.mcp_servers
                .iter()
                .map(|surface| ManifestPluginSurface {
                    surface: plugin_surface_ref(PackageSurfaceKind::Mcp, &surface.id),
                    activation: surface.activation,
                    optional: surface.optional,
                    dependencies: Vec::new(),
                }),
        );
        surfaces.extend(self.okf.iter().map(|surface| ManifestPluginSurface {
            surface: plugin_surface_ref(PackageSurfaceKind::Okf, &surface.id),
            activation: SurfaceActivation::Eager,
            optional: surface.optional,
            dependencies: Vec::new(),
        }));
        surfaces.extend(self.flows.iter().map(|surface| {
            let mut dependencies = surface
                .requires_tools
                .iter()
                .map(|id| plugin_surface_ref(PackageSurfaceKind::Tool, id))
                .chain(
                    surface
                        .requires_mcp
                        .iter()
                        .map(|id| plugin_surface_ref(PackageSurfaceKind::Mcp, id)),
                )
                .chain(
                    surface
                        .requires_okf
                        .iter()
                        .map(|id| plugin_surface_ref(PackageSurfaceKind::Okf, id)),
                )
                .collect::<Vec<_>>();
            dependencies.sort();
            ManifestPluginSurface {
                surface: plugin_surface_ref(PackageSurfaceKind::Flow, &surface.id),
                activation: SurfaceActivation::Lazy,
                optional: surface.optional,
                dependencies,
            }
        }));
        surfaces.extend(self.skills.iter().map(|surface| {
            let mut dependencies = surface
                .requires_tools
                .iter()
                .map(|id| plugin_surface_ref(PackageSurfaceKind::Tool, id))
                .chain(
                    surface
                        .requires_mcp
                        .iter()
                        .map(|id| plugin_surface_ref(PackageSurfaceKind::Mcp, id)),
                )
                .chain(
                    surface
                        .requires_okf
                        .iter()
                        .map(|id| plugin_surface_ref(PackageSurfaceKind::Okf, id)),
                )
                .chain(
                    surface
                        .requires_flows
                        .iter()
                        .map(|id| plugin_surface_ref(PackageSurfaceKind::Flow, id)),
                )
                .collect::<Vec<_>>();
            dependencies.sort();
            ManifestPluginSurface {
                surface: plugin_surface_ref(PackageSurfaceKind::Skill, &surface.id),
                activation: SurfaceActivation::Lazy,
                optional: surface.optional,
                dependencies,
            }
        }));
        surfaces.extend(self.agents.iter().map(|surface| ManifestPluginSurface {
            surface: plugin_surface_ref(PackageSurfaceKind::Agent, &surface.id),
            activation: SurfaceActivation::Eager,
            optional: surface.optional,
            dependencies: owned_dependencies(
                &surface.requires_tools,
                &surface.requires_mcp,
                &surface.requires_okf,
                &surface.requires_skills,
                &surface.requires_flows,
                &surface.requires_ui,
            ),
        }));
        surfaces.extend(self.plugins.iter().map(|surface| ManifestPluginSurface {
            surface: plugin_surface_ref(PackageSurfaceKind::Plugin, &surface.id),
            activation: SurfaceActivation::Lazy,
            optional: surface.optional,
            dependencies: owned_dependencies(
                &surface.requires_tools,
                &surface.requires_mcp,
                &surface.requires_okf,
                &surface.requires_skills,
                &surface.requires_flows,
                &surface.requires_ui,
            ),
        }));
        surfaces.extend(self.ui.iter().map(|surface| {
            let mut dependencies = surface
                .skill
                .iter()
                .map(|id| plugin_surface_ref(PackageSurfaceKind::Skill, id))
                .chain(
                    surface
                        .bind_tools
                        .iter()
                        .map(|id| plugin_surface_ref(PackageSurfaceKind::Tool, id)),
                )
                .chain(
                    surface
                        .bind_mcp
                        .iter()
                        .map(|id| plugin_surface_ref(PackageSurfaceKind::Mcp, id)),
                )
                .chain(
                    surface
                        .bind_flows
                        .iter()
                        .map(|id| plugin_surface_ref(PackageSurfaceKind::Flow, id)),
                )
                .collect::<Vec<_>>();
            dependencies.sort();
            ManifestPluginSurface {
                surface: plugin_surface_ref(PackageSurfaceKind::Ui, &surface.id),
                activation: SurfaceActivation::Lazy,
                optional: surface.optional,
                dependencies,
            }
        }));
        surfaces.sort_by(|left, right| left.surface.cmp(&right.surface));

        let known = surfaces
            .iter()
            .map(|surface| surface.surface.clone())
            .collect::<BTreeSet<_>>();
        if known.len() != surfaces.len()
            || surfaces.iter().any(|surface| {
                surface
                    .dependencies
                    .iter()
                    .any(|dependency| !known.contains(dependency))
            })
        {
            return Err(manifest_error(
                "The named plugin surface graph is internally inconsistent.",
            ));
        }
        Ok(surfaces)
    }

    /// Studio pages this package contributes to.
    ///
    /// Each surface kind has its own page. UI blocks add the extension points
    /// they declare, so one package can appear on several pages at once.
    pub fn studio_pages(&self) -> Vec<&'static str> {
        let mut pages = BTreeSet::new();
        if !self.tools.is_empty() {
            pages.insert("studio.tools");
        }
        if !self.mcp_servers.is_empty() {
            pages.insert("studio.connectors");
        }
        if !self.okf.is_empty() {
            pages.insert("studio.knowledge");
        }
        if !self.flows.is_empty() {
            pages.insert("studio.flows");
        }
        if !self.skills.is_empty() {
            pages.insert("studio.skills");
        }
        if !self.agents.is_empty() {
            pages.insert("studio.agents");
        }
        if !self.plugins.is_empty() {
            pages.insert("studio.plugins");
        }
        for ui in &self.ui {
            let page = match ui.point {
                Some(point) => point.as_str(),
                None if ui.container.is_some() => "studio.sidebar",
                None => "studio.editor",
            };
            pages.insert(page);
        }
        pages.into_iter().collect()
    }

    pub fn has_mcp(&self) -> bool {
        !self.mcp_servers.is_empty()
    }

    pub fn ui_count(&self) -> usize {
        self.ui.len()
    }

    pub fn supports_use_version(&self, version: &str) -> UseResult<bool> {
        let version = semver::Version::parse(version).map_err(|error| {
            manifest_error(format!("Invalid Aster Use host version '{version}': {error}"))
        })?;
        let Some(requirement) = &self.requires_use else {
            return Ok(true);
        };
        let requirement = semver::VersionReq::parse(requirement).map_err(|error| {
            manifest_error(format!(
                "Invalid Aster Use compatibility requirement '{requirement}': {error}"
            ))
        })?;
        Ok(requirement.matches(&version))
    }

    /// Require compatibility with the Use contract compiled into this library.
    pub fn require_current_use(&self) -> UseResult<()> {
        let host = env!("CARGO_PKG_VERSION");
        if !self.supports_use_version(host)? {
            return Err(UseError::new(
                "use.extension.host_incompatible",
                format!(
                    "Package '{}' is incompatible with Aster Use {host}.",
                    self.package_id
                ),
            ));
        }
        Ok(())
    }
}

fn plugin_surface_ref(kind: PackageSurfaceKind, id: &str) -> PluginSurfaceRef {
    PluginSurfaceRef {
        kind,
        id: id.to_string(),
    }
}

fn owned_dependencies(
    tools: &[String],
    mcp: &[String],
    okf: &[String],
    skills: &[String],
    flows: &[String],
    ui: &[String],
) -> Vec<PluginSurfaceRef> {
    let mut dependencies = tools
        .iter()
        .map(|id| plugin_surface_ref(PackageSurfaceKind::Tool, id))
        .chain(
            mcp.iter()
                .map(|id| plugin_surface_ref(PackageSurfaceKind::Mcp, id)),
        )
        .chain(
            okf.iter()
                .map(|id| plugin_surface_ref(PackageSurfaceKind::Okf, id)),
        )
        .chain(
            skills
                .iter()
                .map(|id| plugin_surface_ref(PackageSurfaceKind::Skill, id)),
        )
        .chain(
            flows
                .iter()
                .map(|id| plugin_surface_ref(PackageSurfaceKind::Flow, id)),
        )
        .chain(
            ui.iter()
                .map(|id| plugin_surface_ref(PackageSurfaceKind::Ui, id)),
        )
        .collect::<Vec<_>>();
    dependencies.sort();
    dependencies
}

fn parse_extension_block(block: &Block) -> UseResult<ExtensionManifest> {
    require_known_attributes(
        block,
        &[
            "schema_version",
            "version",
            "route",
            "requires_use",
            "actions",
        ],
    )?;
    let package_id = block
        .labels
        .first()
        .cloned()
        .ok_or_else(|| manifest_error("The extension block requires a package ID label."))?;
    if block.labels.len() != 1 || !valid_package_id(&package_id) {
        return Err(manifest_error(
            "Package IDs must be '<publisher>/<name>' lowercase identifiers.",
        ));
    }
    let schema_number = number_attribute(block, "schema_version")?;
    if !schema_number.is_finite()
        || schema_number.fract() != 0.0
        || !(0.0..=u32::MAX as f64).contains(&schema_number)
    {
        return Err(manifest_error(
            "Extension schema_version must be a non-negative integer.",
        ));
    }
    let schema_version = schema_number as u32;
    if schema_version != 3 {
        return Err(manifest_error(
            "Only package manifest schema version 3 is supported; rebuild packages created with a pre-release schema.",
        ));
    }
    let version = string_attribute(block, "version")?;
    semver::Version::parse(&version)
        .map_err(|error| manifest_error(format!("Invalid extension version: {error}")))?;
    let route = string_attribute(block, "route")?;
    if !valid_segment(&route) || RESERVED_ROUTES.contains(&route.as_str()) {
        return Err(manifest_error(format!(
            "Extension route '{route}' is invalid or reserved."
        )));
    }
    let action_names = list_attribute(block, "actions")?;
    if action_names.iter().collect::<BTreeSet<_>>().len() != action_names.len() {
        return Err(manifest_error("Action classes must be unique."));
    }
    let actions = action_names
        .into_iter()
        .map(|action| parse_risk_class(&action))
        .collect::<UseResult<Vec<_>>>()?;
    let mut seen = BTreeSet::new();
    let mut repository = None;
    let mut dependencies = Vec::new();
    let mut tools = Vec::new();
    let mut mcp_servers = Vec::new();
    let mut okf = Vec::new();
    let mut flows = Vec::new();
    let mut skills = Vec::new();
    let mut agents = Vec::new();
    let mut plugins = Vec::new();
    let mut ui = Vec::new();
    let mut view_containers = Vec::new();
    let mut commands = Vec::new();
    let mut menus = Vec::new();
    let mut keybindings = Vec::new();
    let mut harness = Vec::new();
    let mut home = Vec::new();
    let mut composer_modes = Vec::new();
    for surface in &block.blocks {
        let name = surface.name.as_str();
        let singleton = name == "repository";
        if singleton && !seen.insert(name) {
            return Err(manifest_error(format!(
                "Duplicate '{}' surface.",
                surface.name
            )));
        }
        match name {
            "tool" => tools.push(plugin_manifest::parse_tool(surface)?),
            "mcp" => mcp_servers.push(plugin_manifest::parse_mcp(surface)?),
            "okf" => okf.push(plugin_manifest::parse_okf(surface)?),
            "flow" => flows.push(plugin_manifest::parse_flow(surface)?),
            "skill" => skills.push(plugin_manifest::parse_skill(surface)?),
            "agent" => agents.push(plugin_manifest::parse_agent(surface)?),
            "plugin" => plugins.push(plugin_manifest::parse_plugin(surface)?),
            "ui" => ui.push(plugin_manifest::parse_ui(surface)?),
            "view_container" => view_containers.push(workbench_manifest::parse_container(surface)?),
            "command" => commands.push(workbench_commands::parse_package_command(surface)?),
            "menu" => menus.push(workbench_commands::parse_workbench_menu(surface)?),
            "keybinding" => keybindings.push(workbench_commands::parse_keybinding(surface)?),
            "harness" => harness.push(workbench_commands::parse_harness(surface)?),
            "home" => home.push(home::parse_home(surface)?),
            "composer_mode" => composer_modes.push(composer_mode::parse(surface)?),
            "dependency" => dependencies.push(parse_package_dependency(surface)?),
            "repository" => repository = Some(parse_repository(surface)?),
            name => {
                return Err(manifest_error(format!(
                    "Unknown extension surface '{name}'."
                )));
            }
        }
    }
    if tools.is_empty()
        && mcp_servers.is_empty()
        && okf.is_empty()
        && flows.is_empty()
        && skills.is_empty()
        && agents.is_empty()
        && plugins.is_empty()
        && ui.is_empty()
        && commands.is_empty()
        && harness.is_empty()
        && home.is_empty()
        && composer_modes.is_empty()
    {
        return Err(manifest_error(
            "A schema version 3 package must declare Tool, MCP, OKF, Flow, Skill, Agent, Plugin, UI, a command, a harness component, a home card, and/or a composer mode.",
        ));
    }
    composer_mode::validate(&composer_modes, &agents, &ui)?;
    if harness.len() > 32
        || harness
            .iter()
            .map(|item| item.id.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != harness.len()
    {
        return Err(manifest_error(
            "AIX harness components must be unique and at most 32.",
        ));
    }
    if home.len() > 32
        || home
            .iter()
            .map(|item| item.id.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != home.len()
    {
        return Err(manifest_error(
            "AIX home cards must be unique and at most 32.",
        ));
    }
    if agents.len() > 1 {
        return Err(manifest_error(
            "An agent package declares exactly one agent surface.",
        ));
    }
    plugin_manifest::validate_dependencies(
        &tools,
        &mcp_servers,
        &okf,
        &flows,
        &skills,
        &agents,
        &ui,
        &plugins,
    )?;
    workbench_manifest::validate(&package_id, &view_containers, &ui)?;
    workbench_commands::validate_palette(&commands, &menus, &keybindings)?;
    dependencies.sort_by(|left, right| left.package_id.cmp(&right.package_id));
    if dependencies
        .windows(2)
        .any(|pair| pair[0].package_id == pair[1].package_id)
    {
        return Err(manifest_error(
            "Package dependencies must be sorted and unique by package ID.",
        ));
    }
    if dependencies
        .iter()
        .any(|dependency| dependency.package_id == package_id)
    {
        return Err(manifest_error("A package cannot depend on itself."));
    }
    PluginPackageDependency::validate_set(&package_id, &dependencies).map_err(|error| {
        manifest_error(format!("Invalid package dependency set: {}", error.message))
    })?;
    let requires_use = optional_string_attribute(block, "requires_use")?;
    let requirement = requires_use
        .as_deref()
        .ok_or_else(|| manifest_error("Package manifests require 'requires_use'."))?;
    let requirement = semver::VersionReq::parse(requirement).map_err(|error| {
        manifest_error(format!(
            "Invalid Aster Use compatibility requirement '{requirement}': {error}"
        ))
    })?;
    if repository.is_none() {
        return Err(manifest_error(
            "Package manifests require a repository block.",
        ));
    }
    let current_host = semver::Version::new(0, 3, 0);
    let obsolete_host = semver::Version::new(0, 2, 1);
    if !requirement.matches(&current_host) || requirement.matches(&obsolete_host) {
        return Err(manifest_error(
            "Schema version 3 must require Aster Use 0.3 and exclude pre-0.3 hosts.",
        ));
    }
    Ok(ExtensionManifest {
        schema_version,
        package_id,
        version,
        route,
        requires_use,
        dependencies,
        repository,
        actions,
        tools,
        mcp_servers,
        okf,
        flows,
        skills,
        agents,
        plugins,
        ui,
        view_containers,
        commands,
        menus,
        keybindings,
        harness,
        home,
        composer_modes,
    })
}

fn parse_package_dependency(block: &Block) -> UseResult<PluginPackageDependency> {
    if block.labels.len() != 1 || !block.blocks.is_empty() {
        return Err(manifest_error(
            "A dependency block requires exactly one package ID label and no nested blocks.",
        ));
    }
    require_known_attributes(block, &["version"])?;
    PluginPackageDependency::new(block.labels[0].clone(), string_attribute(block, "version")?)
        .map_err(|error| {
            manifest_error(format!(
                "Package dependency '{}' must use a canonical semantic-version requirement: {}",
                block.labels[0], error.message
            ))
        })
}

fn parse_repository(block: &Block) -> UseResult<ExtensionRepository> {
    require_surface_shape(block)?;
    require_known_attributes(block, &["url", "revision"])?;
    let url = string_attribute(block, "url")?;
    let parsed = url::Url::parse(&url)
        .map_err(|error| manifest_error(format!("Invalid repository URL '{url}': {error}")))?;
    if parsed.scheme() != "https"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || parsed.host_str().is_none()
    {
        return Err(manifest_error(
            "Repository URLs must be credential-free HTTPS URLs without a query or fragment.",
        ));
    }
    let revision = optional_string_attribute(block, "revision")?;
    if revision.as_deref().is_some_and(|revision| {
        !matches!(revision.len(), 40 | 64)
            || !revision
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    }) {
        return Err(manifest_error(
            "Repository revisions must be lowercase 40- or 64-character commit digests.",
        ));
    }
    Ok(ExtensionRepository { url, revision })
}

fn bounded_text(value: String, label: &str, max_chars: usize) -> UseResult<String> {
    let value = value.trim().to_string();
    if value.is_empty() || value.chars().count() > max_chars {
        return Err(manifest_error(format!(
            "{label} must contain between 1 and {max_chars} characters."
        )));
    }
    Ok(value)
}

fn validate_relative_path(path: &Path) -> UseResult<()> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::CurDir
            )
        })
    {
        return Err(UseError::new(
            "use.extension.path_escape",
            format!("Surface path '{}' is not package-relative.", path.display()),
        ));
    }
    Ok(())
}

fn require_surface_shape(block: &Block) -> UseResult<()> {
    if !block.labels.is_empty() || !block.blocks.is_empty() {
        return Err(manifest_error(format!(
            "The '{}' surface cannot have labels or nested blocks.",
            block.name
        )));
    }
    Ok(())
}

fn require_known_attributes(block: &Block, allowed: &[&str]) -> UseResult<()> {
    if let Some(unknown) = block
        .attributes
        .keys()
        .find(|key| !allowed.contains(&key.as_str()))
    {
        return Err(manifest_error(format!(
            "Unknown '{}' attribute '{}'.",
            block.name, unknown
        )));
    }
    Ok(())
}

fn string_attribute(block: &Block, name: &str) -> UseResult<String> {
    block
        .attributes
        .get(name)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            manifest_error(format!(
                "'{}' requires string attribute '{name}'.",
                block.name
            ))
        })
}

fn number_attribute(block: &Block, name: &str) -> UseResult<f64> {
    block
        .attributes
        .get(name)
        .and_then(Value::as_number)
        .ok_or_else(|| {
            manifest_error(format!(
                "'{}' requires numeric attribute '{name}'.",
                block.name
            ))
        })
}

fn optional_bool_attribute(block: &Block, name: &str) -> UseResult<Option<bool>> {
    match block.attributes.get(name) {
        None => Ok(None),
        Some(value) => value.as_bool().map(Some).ok_or_else(|| {
            manifest_error(format!(
                "'{}' requires boolean attribute '{name}'.",
                block.name
            ))
        }),
    }
}

fn optional_string_attribute(block: &Block, name: &str) -> UseResult<Option<String>> {
    match block.attributes.get(name) {
        None => Ok(None),
        Some(value) => value.as_str().map(str::to_string).map(Some).ok_or_else(|| {
            manifest_error(format!(
                "'{}' requires string attribute '{name}'.",
                block.name
            ))
        }),
    }
}

fn optional_i32_attribute(block: &Block, name: &str) -> UseResult<Option<i32>> {
    let Some(value) = block.attributes.get(name) else {
        return Ok(None);
    };
    let Some(value) = value.as_number() else {
        return Err(manifest_error(format!(
            "'{}' requires numeric attribute '{name}'.",
            block.name
        )));
    };
    if !value.is_finite()
        || value.fract() != 0.0
        || !(i32::MIN as f64..=i32::MAX as f64).contains(&value)
    {
        return Err(manifest_error(format!(
            "'{}' attribute '{name}' must be a 32-bit integer.",
            block.name
        )));
    }
    Ok(Some(value as i32))
}

fn list_attribute(block: &Block, name: &str) -> UseResult<Vec<String>> {
    let Some(Value::List(values)) = block.attributes.get(name) else {
        return Err(manifest_error(format!(
            "'{}' requires list attribute '{name}'.",
            block.name
        )));
    };
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| manifest_error(format!("'{name}' accepts only strings.")))
        })
        .collect()
}

fn optional_list_attribute(block: &Block, name: &str) -> UseResult<Vec<String>> {
    if block.attributes.contains_key(name) {
        list_attribute(block, name)
    } else {
        Ok(Vec::new())
    }
}

fn parse_risk_class(value: &str) -> UseResult<RiskClass> {
    match value {
        "read" => Ok(RiskClass::Read),
        "navigate" => Ok(RiskClass::Navigate),
        "mutate" => Ok(RiskClass::Mutate),
        "submit" => Ok(RiskClass::Submit),
        "download" => Ok(RiskClass::Download),
        "execute" => Ok(RiskClass::Execute),
        _ => Err(manifest_error(format!("Unknown action class '{value}'."))),
    }
}

fn valid_package_id(value: &str) -> bool {
    let segments = value.split('/').collect::<Vec<_>>();
    segments.len() == 2 && segments.into_iter().all(valid_segment)
}

fn valid_segment(value: &str) -> bool {
    let mut characters = value.chars();
    matches!(characters.next(), Some(first) if first.is_ascii_lowercase())
        && characters.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

fn manifest_error(message: impl Into<String>) -> UseError {
    UseError::new("use.extension.manifest_invalid", message)
}

#[cfg(test)]
mod review_package_manifest {
    use super::ExtensionManifest;

    #[test]
    fn aster_review_declares_skill_and_picker_together() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../apps/use-registry/packages/review/aster-use-extension.acl");
        // Use can be built outside the OS monorepo, where this package source is absent.
        if !path.is_file() {
            return;
        }
        let manifest =
            ExtensionManifest::parse_acl(&std::fs::read_to_string(&path).expect("review manifest"))
                .expect("review manifest");
        assert_eq!(manifest.package_id, "aster/review");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.skills.len(), 1);
        assert_eq!(manifest.skills[0].id, "review");
        assert!(manifest.skills[0].requires_tools.is_empty());
        assert_eq!(manifest.ui.len(), 1);
        assert_eq!(manifest.ui[0].id, "picker");
        assert_eq!(manifest.ui[0].skill.as_deref(), Some("review"));
        assert!(manifest.ui[0].bind_tools.is_empty());
        assert!(manifest.tools.is_empty());
    }

    #[test]
    fn aster_robot_workbenches_declare_the_workbench_and_skill() {
        for name in ["cobot-arm", "g1-arm"] {
            let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
                "../../../../apps/use-registry/packages/{name}/aster-use-extension.acl"
            ));
            // Use can be built outside the OS monorepo, where this package source is absent.
            if !path.is_file() {
                continue;
            }
            let manifest = ExtensionManifest::parse_acl(
                &std::fs::read_to_string(&path).expect("workbench manifest"),
            )
            .expect("workbench manifest");
            assert_eq!(manifest.package_id, format!("aster/{name}"));
            assert_eq!(manifest.version, "1.0.0");
            assert_eq!(manifest.skills.len(), 1);
            assert_eq!(manifest.skills[0].id, name);
            assert!(manifest.skills[0].requires_tools.is_empty());
            assert!(manifest.skills[0].requires_mcp.is_empty());
            assert_eq!(manifest.ui.len(), 1);
            assert_eq!(manifest.ui[0].id, "workbench");
            assert!(manifest.ui[0].point.is_none());
            assert_eq!(manifest.ui[0].container.as_deref(), Some("lab"));
            assert_eq!(manifest.view_containers.len(), 1);
            assert_eq!(manifest.view_containers[0].id, "lab");
            assert_eq!(manifest.view_containers[0].icon, "bot");
            assert_eq!(
                manifest.view_containers[0].location,
                super::PluginViewLocation::ActivityBar
            );
            assert_eq!(
                manifest.ui[0]
                    .presentation
                    .as_ref()
                    .map(|presentation| presentation.as_str()),
                Some("webview")
            );
            assert_eq!(manifest.ui[0].skill.as_deref(), Some(name));
            assert_eq!(
                manifest.ui[0].entry,
                std::path::PathBuf::from("ui/workbench/index.html")
            );
            assert!(manifest.tools.is_empty());
            assert_eq!(manifest.agents.len(), 1);
            assert_eq!(manifest.agents[0].id, name);
            assert_eq!(manifest.agents[0].runtime, super::AgentRuntimePlace::Edge);
            assert_eq!(
                manifest.agents[0].harness,
                Some(super::AgentHarness::Package)
            );
            assert_eq!(
                manifest.agents[0].entry.as_deref(),
                Some(std::path::Path::new("agent/run.mjs"))
            );
            assert_eq!(manifest.agents[0].requires_skills, [name]);
            assert_eq!(
                manifest.agents[0]
                    .resources
                    .as_ref()
                    .map(|resources| (resources.cpu_millis, resources.memory_bytes)),
                Some((500, 268_435_456))
            );
        }
    }
}
