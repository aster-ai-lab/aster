//! Shared-kernel Sandbox backend support.
//!
//! The public isolation selector stays backend-neutral. This module owns the
//! Linux host evidence and OCI artifacts required by the Aster OCI backend;
//! VM-specific code must not depend on these types.

#[cfg(target_os = "linux")]
pub(crate) mod aster_oci_client;
#[cfg(target_os = "linux")]
pub(crate) mod aster_oci_controller;
#[cfg(target_os = "linux")]
pub(crate) mod aster_oci_handler;
#[cfg(target_os = "linux")]
pub(crate) mod aster_oci_owner;
pub mod capability;
pub mod controller;
mod mount_alias;
pub mod oci;
pub mod path_access;
pub mod rootfs;
#[cfg(target_os = "linux")]
pub(crate) mod runtime_record;

/// Lifecycle envelope certified by the pinned Aster OCI native service.
///
/// The native service allows its Linux executor up to ten seconds to reap a
/// process and uses a fifteen-second shutdown envelope in its own lifecycle
/// certification. Box must give container wait and owner shutdown that same
/// bounded opportunity before declaring provider loss.
#[cfg(target_os = "linux")]
pub(crate) const ASTER_OCI_LIFECYCLE_TIMEOUT_MS: u64 = 15_000;

/// Environment override for the Sandbox OCI delegated cgroup root.
///
/// Qualification hosts that cannot expose the default systemd user-instance
/// path (for example CI runners using a temporary cgroup tree) set this to an
/// absolute, pre-created, caller-owned cgroup v2 directory.
#[cfg(target_os = "linux")]
pub(crate) const SANDBOX_DELEGATED_CGROUP_ROOT_ENV: &str = "ASTER_BOX_SANDBOX_DELEGATED_CGROUP_ROOT";

/// Host cgroup root handed to the setuid Sandbox launcher / native OCI owner.
///
/// When set, [`SANDBOX_DELEGATED_CGROUP_ROOT_ENV`] must be an absolute path to
/// the tree prepared by `scripts/prepare-linux-sandbox-host.sh` (typically
/// `/sys/fs/cgroup/aster-box-sandbox/delegated`). Capability preflight and the
/// OCI owner both use this same root (#616). If unset, fall back to a path
/// under the caller's systemd user instance.
#[cfg(target_os = "linux")]
pub(crate) fn linux_sandbox_delegated_cgroup_root() -> String {
    if let Ok(path) = std::env::var(SANDBOX_DELEGATED_CGROUP_ROOT_ENV) {
        let path = path.trim();
        if !path.is_empty() {
            return path.to_string();
        }
    }
    // SAFETY: See the enclosing function's safety documentation.
    let uid = unsafe { libc::geteuid() };
    format!("/sys/fs/cgroup/user.slice/user-{uid}.slice/user@{uid}.service/aster-box-delegated")
}

/// Resolve the administrator-owned setuid launcher used for Sandbox OCI owners.
///
/// Discovery order matches other packaged Sandbox artifacts: explicit override,
/// environment, then the system libexec install location.
#[cfg(target_os = "linux")]
pub(crate) fn resolve_sandbox_oci_launcher(
    explicit: Option<&std::path::Path>,
) -> aster_box_core::error::Result<std::path::PathBuf> {
    capability::resolve_sandbox_oci_launcher(explicit)
}

/// Fail closed unless `path` is a root-owned setuid Sandbox OCI launcher.
#[cfg(target_os = "linux")]
pub(crate) fn require_operator_setuid_launcher(
    path: &std::path::Path,
) -> aster_box_core::error::Result<()> {
    capability::require_operator_setuid_launcher(path)
}

/// Apply a complete resource contract to the exact recorded Sandbox generation.
///
/// The Aster OCI SDK is the only live-update path for a host Sandbox. MicroVM
/// cgroup updates remain guest-local and never pass through this function.
#[cfg(target_os = "linux")]
pub fn update_recorded_resources(
    box_dir: &std::path::Path,
    box_id: &str,
    config: &aster_box_core::config::BoxConfig,
) -> aster_box_core::Result<()> {
    if !config.isolation.is_sandbox() {
        return Err(aster_box_core::BoxError::ConfigError(
            "Aster OCI resource updates require Sandbox isolation".to_string(),
        ));
    }
    let runtime = crate::vm::reap::load_recorded_sandbox_runtime(
        &aster_box_core::dirs_home(),
        box_dir,
        box_id,
    )?
    .ok_or_else(|| {
        aster_box_core::BoxError::StateError(format!(
            "Recorded Aster OCI runtime is missing for Sandbox {box_id}"
        ))
    })?;
    let socket = runtime.runtime_socket.as_deref().ok_or_else(|| {
        aster_box_core::BoxError::StateError(format!(
            "Recorded Aster OCI runtime socket is missing for Sandbox {box_id}"
        ))
    })?;
    let generation = runtime.generation.ok_or_else(|| {
        aster_box_core::BoxError::StateError(format!(
            "Recorded Aster OCI generation is missing for Sandbox {box_id}"
        ))
    })?;
    let resources = oci::compile_resources(&SandboxResources::from_box_config(config)?)?;
    AsterOciHandler::update_at(socket, box_id, generation, resources)
}

#[cfg(not(target_os = "linux"))]
pub fn update_recorded_resources(
    _box_dir: &std::path::Path,
    _box_id: &str,
    _config: &aster_box_core::config::BoxConfig,
) -> aster_box_core::Result<()> {
    Err(aster_box_core::BoxError::StateError(
        "Aster OCI Sandbox resource updates require Linux".to_string(),
    ))
}

#[cfg(target_os = "linux")]
pub use aster_oci_controller::AsterOciController;
#[cfg(target_os = "linux")]
pub use aster_oci_handler::AsterOciHandler;
pub use capability::{
    map_container_gid, map_container_uid, plan_id_mappings, probe_sandbox_capabilities,
    probe_sandbox_capabilities_for, unmap_host_gid, unmap_host_uid, CertifiedAsterOci, IdMapping,
    SandboxCapabilitySnapshot, SandboxIdMappingPlan, UserNamespaceEvidence,
};
pub use controller::{write_bundle, SandboxLaunchSpec};
pub(crate) use mount_alias::cleanup_sandbox_mount_aliases;
#[cfg(target_os = "linux")]
pub(crate) use mount_alias::sandbox_mount_alias_root;
#[cfg(feature = "vm")]
pub(crate) use mount_alias::stage_external_mount_aliases;
#[cfg(not(target_os = "linux"))]
pub struct AsterOciController;
#[cfg(not(target_os = "linux"))]
impl AsterOciController {
    pub fn new(_runtime: CertifiedAsterOci) -> Self {
        Self
    }

    pub fn require_absent(
        &self,
        _runtime_root: &std::path::Path,
        _container_id: &str,
    ) -> aster_box_core::Result<()> {
        Err(aster_box_core::BoxError::BoxBootError {
            message: "Aster OCI Sandbox execution requires Linux".to_string(),
            hint: None,
        })
    }

    pub async fn start(&self, _launch: SandboxLaunchSpec) -> aster_box_core::Result<AsterOciHandler> {
        Err(aster_box_core::BoxError::BoxBootError {
            message: "Aster OCI Sandbox execution requires Linux".to_string(),
            hint: None,
        })
    }
}

#[cfg(not(target_os = "linux"))]
pub struct AsterOciHandler;
#[cfg(not(target_os = "linux"))]
impl aster_box_core::vmm::VmHandler for AsterOciHandler {
    fn stop(&mut self, _signal: i32, _timeout_ms: u64) -> aster_box_core::Result<()> {
        Err(aster_box_core::BoxError::StateError(
            "Aster OCI Sandbox execution requires Linux".to_string(),
        ))
    }

    fn metrics(&self) -> aster_box_core::vmm::VmMetrics {
        aster_box_core::vmm::VmMetrics::default()
    }

    fn is_running(&self) -> bool {
        false
    }

    fn pid(&self) -> u32 {
        0
    }
}
pub use oci::{
    compile_oci_spec, compile_portable_microvm_oci_spec, compile_runtime_owned_oci_spec,
    SandboxBundleSpec, SandboxMount, SandboxResources, SandboxRuntimeProcess, SandboxTmpfs,
    DEFAULT_SANDBOX_PIDS_LIMIT, PORTABLE_MICROVM_BUNDLE_SCHEMA,
};
pub use path_access::prepare_sandbox_path_access;
pub use rootfs::{
    inspect_rootfs_identity_requirements, mapped_root_ids, prepare_managed_mount_source,
    prepare_managed_secret_mount_source, prepare_rootfs_ownership, validate_external_mount_access,
    RootfsIdentityRequirements,
};
