//! Cross-platform hardware-vm contract for Linux/KVM, Apple Silicon/HVF,
//! and Windows x86_64/WHPX.
//!
//! Profile bytes, the device allowlist, lifecycle names, snapshot tier names,
//! resource bounds, and control limits come from `aster-oci-core` and are
//! identical on every host. Boot layout is selected only by guest ISA.
//! Decisions here do not start a hypervisor.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use aster_oci_core::hardware_vm::{self as core_vm, GuestArch, HardwareVmError};
use aster_oci_core::{DriverKind, IsolationClass};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub use core_vm::{
    CONTROL_CREDIT_WINDOW, CONTROL_RECORD_MAX_BYTES, DEVICE_ALLOWLIST, LIFECYCLE, MEMORY_ALIGN_MIB,
    MEMORY_MAX_MIB, MEMORY_MIN_MIB, PROFILE_VERSION, VCPU_CHOICES,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HostKind {
    Kvm,
    Hvf,
    Whpx,
}

impl HostKind {
    pub fn all() -> [Self; 3] {
        [Self::Kvm, Self::Hvf, Self::Whpx]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Kvm => "kvm",
            Self::Hvf => "hvf",
            Self::Whpx => "whpx",
        }
    }

    fn to_driver(self) -> DriverKind {
        match self {
            Self::Kvm => DriverKind::LibkrunKvm,
            Self::Hvf => DriverKind::LibkrunHvf,
            Self::Whpx => DriverKind::LibkrunWhpx,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuestIsa {
    X86_64,
    Aarch64,
}

impl GuestIsa {
    pub fn all() -> [Self; 2] {
        [Self::X86_64, Self::Aarch64]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::X86_64 => "x86_64",
            Self::Aarch64 => "aarch64",
        }
    }

    fn to_arch(self) -> GuestArch {
        match self {
            Self::X86_64 => GuestArch::X86_64,
            Self::Aarch64 => GuestArch::Aarch64,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootLayout {
    pub isa: GuestIsa,
    pub protocol: &'static str,
}

/// ISA boot protocol. Host kind does not change it.
pub fn boot_layout(isa: GuestIsa) -> BootLayout {
    let layout = core_vm::boot_layout(isa.to_arch());
    BootLayout {
        isa,
        protocol: layout.protocol,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardwareVmProfile {
    pub version: u32,
    pub devices: Vec<&'static str>,
    pub lifecycle: Vec<&'static str>,
    pub snapshot_tiers: Vec<&'static str>,
    pub vcpu_choices: Vec<u32>,
    pub memory_min_mib: u32,
    pub memory_max_mib: u32,
    pub memory_align_mib: u32,
    pub control_record_max_bytes: u32,
    pub control_credit_window: u32,
}

impl HardwareVmProfile {
    pub fn current() -> Self {
        Self::from_core(core_vm::HardwareVmProfile::shared())
    }

    fn from_core(profile: core_vm::HardwareVmProfile) -> Self {
        Self {
            version: profile.version,
            devices: profile.devices,
            lifecycle: profile.lifecycle,
            snapshot_tiers: profile.snapshot_tiers,
            vcpu_choices: profile.vcpu_choices,
            memory_min_mib: profile.memory_min_mib,
            memory_max_mib: profile.memory_max_mib,
            memory_align_mib: profile.memory_align_mib,
            control_record_max_bytes: profile.control_record_max_bytes,
            control_credit_window: profile.control_credit_window,
        }
    }

    fn to_core(&self) -> core_vm::HardwareVmProfile {
        core_vm::HardwareVmProfile {
            version: self.version,
            devices: self.devices.clone(),
            lifecycle: self.lifecycle.clone(),
            snapshot_tiers: self.snapshot_tiers.clone(),
            vcpu_choices: self.vcpu_choices.clone(),
            memory_min_mib: self.memory_min_mib,
            memory_max_mib: self.memory_max_mib,
            memory_align_mib: self.memory_align_mib,
            control_record_max_bytes: self.control_record_max_bytes,
            control_credit_window: self.control_credit_window,
        }
    }
}

/// Profile bytes ignore host, ISA, and `ASTER_BOX_OCI_MIGRATION`.
pub fn stored_profile_bytes(_migration: Option<&str>) -> Vec<u8> {
    profile_bytes(&HardwareVmProfile::current())
}

pub fn profile_bytes(profile: &HardwareVmProfile) -> Vec<u8> {
    core_vm::profile_bytes(&profile.to_core())
}

pub fn profile_for(host: HostKind, isa: GuestIsa) -> HardwareVmProfile {
    let profile = core_vm::profile_for(host.to_driver(), isa.to_arch())
        .unwrap_or_else(|_| core_vm::HardwareVmProfile::shared());
    HardwareVmProfile::from_core(profile)
}

fn map_contract(error: HardwareVmError) -> ContractError {
    match error {
        HardwareVmError::DeviceNotAllowed(name) => ContractError::DeviceNotAllowed(name),
        HardwareVmError::VcpuOutOfProfile(count) => ContractError::VcpuOutOfProfile(count),
        HardwareVmError::MemoryOutOfProfile(memory) => ContractError::MemoryOutOfProfile(memory),
        HardwareVmError::RestoreMismatch | HardwareVmError::NotHardwareVmHost { .. } => {
            ContractError::RestoreMismatch
        }
        HardwareVmError::SharedHostKernelRejectsVmOnly => ContractError::SandboxRejectsVmOnly,
        HardwareVmError::GuestArchNotAllowed(arch) => ContractError::DeviceNotAllowed(arch),
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContractError {
    #[error("explicit microvm isolation spelling is rejected")]
    MicrovmSpellingRejected,
    #[error("unknown isolation selector")]
    UnknownIsolation,
    #[error("device {0} is outside the hardware-vm allowlist")]
    DeviceNotAllowed(String),
    #[error("vCPU count {0} is outside the hardware-vm profile")]
    VcpuOutOfProfile(u32),
    #[error("memory {0} MiB is outside the hardware-vm profile")]
    MemoryOutOfProfile(u32),
    #[error("restore refused: profile version, guest ISA, or host kind differs")]
    RestoreMismatch,
    #[error("root image set is incomplete or has a duplicate role")]
    RootIdentity,
    #[error("read-only root role {0} requires erofs")]
    RootFormat(&'static str),
    #[error("scratch root role requires ext4")]
    ScratchFormat,
    #[error("virtio-fs reattach path, mode, or deny list differs")]
    VirtioFsMismatch,
    #[error("foreign UID share is fail-closed on this host")]
    ForeignUid,
    #[error("warm workload memory cannot be shared across tenants")]
    CrossTenantWarmMemory,
    #[error("snapshot restore across hypervisor kinds is refused")]
    CrossHypervisorRestore,
    #[error("snapshot destination already exists")]
    SnapshotDestinationExists,
    #[error("shared-kernel sandbox rejects hardware-vm operations")]
    SandboxRejectsVmOnly,
    #[error("network policy digest does not match the snapshot")]
    NetworkDigestMismatch,
    #[error("block role does not match the snapshot")]
    BlockRoleMismatch,
    #[error("kernel contract digest does not match the shipped sources")]
    KernelDigest,
    #[error("kernel contract inputs are missing")]
    KernelMissing,
}

pub fn admit_device(version: u32, device: &str) -> Result<(), ContractError> {
    core_vm::admit_device(version, device).map_err(map_contract)
}

pub fn validate_profile_resources(vcpus: u32, memory_mib: u32) -> Result<(), ContractError> {
    core_vm::validate_profile_resources(vcpus, memory_mib).map_err(map_contract)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotBinding {
    pub version: u32,
    pub isa: GuestIsa,
    pub host: HostKind,
}

pub fn restore_permitted(
    saved: SnapshotBinding,
    requested_version: u32,
    requested_isa: GuestIsa,
    requested_host: HostKind,
) -> Result<(), ContractError> {
    core_vm::restore_permitted(
        saved.version,
        saved.isa.to_arch(),
        saved.host.to_driver(),
        requested_version,
        requested_isa.to_arch(),
        requested_host.to_driver(),
    )
    .map_err(map_contract)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsolationSelection {
    HardwareVm,
    Sandbox,
}

/// `migration` is accepted and ignored. The stored hardware-vm profile does
/// not change when `ASTER_BOX_OCI_MIGRATION` is set or unset.
pub fn select_isolation(
    selector: Option<&str>,
    migration: Option<&str>,
) -> Result<IsolationSelection, ContractError> {
    let _ = migration;
    match selector {
        None => Ok(IsolationSelection::HardwareVm),
        Some("sandbox") => Ok(IsolationSelection::Sandbox),
        Some("microvm") => Err(ContractError::MicrovmSpellingRejected),
        Some(_) => Err(ContractError::UnknownIsolation),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RootRole {
    Distro,
    Runtime,
    Custom,
    Scratch,
}

impl RootRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Distro => "distro",
            Self::Runtime => "runtime",
            Self::Custom => "custom",
            Self::Scratch => "scratch",
        }
    }

    fn read_only(self) -> bool {
        !matches!(self, Self::Scratch)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootFormat {
    Erofs,
    Ext4,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootImage {
    pub role: RootRole,
    pub digest: String,
    pub format: RootFormat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootIdentity {
    pub layers: Vec<(RootRole, String)>,
}

impl RootIdentity {
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        for (role, digest) in &self.layers {
            bytes.extend(role.as_str().as_bytes());
            bytes.push(b':');
            bytes.extend(digest.as_bytes());
            bytes.push(0);
        }
        bytes
    }
}

/// Identity is role name plus digest. Input order is not part of the identity.
pub fn identify_root(images: &[RootImage]) -> Result<RootIdentity, ContractError> {
    let mut layers = BTreeMap::new();
    for image in images {
        if layers.insert(image.role, image.clone()).is_some() {
            return Err(ContractError::RootIdentity);
        }
        if image.role.read_only() && image.format != RootFormat::Erofs {
            return Err(ContractError::RootFormat(image.role.as_str()));
        }
        if image.role == RootRole::Scratch && image.format != RootFormat::Ext4 {
            return Err(ContractError::ScratchFormat);
        }
        if image.digest.is_empty() {
            return Err(ContractError::RootIdentity);
        }
    }
    if !layers.contains_key(&RootRole::Scratch) {
        return Err(ContractError::RootIdentity);
    }
    Ok(RootIdentity {
        layers: layers
            .into_iter()
            .map(|(role, image)| (role, image.digest))
            .collect(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareAttachment {
    pub path: String,
    pub mode: String,
    pub deny: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardwareVmSnapshot {
    pub binding: SnapshotBinding,
    pub tier: SnapshotTier,
    pub root: RootIdentity,
    pub network_digest: String,
}

/// virtio-fs shares are host directories and are not copied into snapshot bytes.
pub fn build_snapshot(
    binding: SnapshotBinding,
    tier: SnapshotTier,
    root: RootIdentity,
    network_digest: impl Into<String>,
    _shares: &[ShareAttachment],
) -> HardwareVmSnapshot {
    HardwareVmSnapshot {
        binding,
        tier,
        root,
        network_digest: network_digest.into(),
    }
}

pub fn snapshot_bytes(snapshot: &HardwareVmSnapshot) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend(snapshot.binding.version.to_le_bytes());
    bytes.extend(snapshot.binding.isa.as_str().as_bytes());
    bytes.push(0);
    bytes.extend(snapshot.binding.host.as_str().as_bytes());
    bytes.push(0);
    bytes.extend(snapshot.tier.as_str().as_bytes());
    bytes.push(0);
    bytes.extend(snapshot.root.canonical_bytes());
    bytes.extend(snapshot.network_digest.as_bytes());
    bytes
}

pub fn share_in_snapshot(snapshot: &HardwareVmSnapshot, share: &ShareAttachment) -> bool {
    let bytes = snapshot_bytes(snapshot);
    let path = share.path.as_bytes();
    !path.is_empty() && bytes.windows(path.len()).any(|window| window == path)
}

pub fn reattach_share(
    saved: &ShareAttachment,
    offered: &ShareAttachment,
) -> Result<(), ContractError> {
    let mut saved_deny = saved.deny.clone();
    let mut offered_deny = offered.deny.clone();
    saved_deny.sort();
    offered_deny.sort();
    if saved.path == offered.path && saved.mode == offered.mode && saved_deny == offered_deny {
        Ok(())
    } else {
        Err(ContractError::VirtioFsMismatch)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectoryShareMode {
    HostDirectory,
    GuestNativeExt4,
    PortableMetadata,
}

pub fn admit_foreign_uid(
    host: HostKind,
    same_uid: bool,
    mode: DirectoryShareMode,
) -> Result<(), ContractError> {
    if same_uid {
        return Ok(());
    }
    let allowed = match host {
        HostKind::Kvm => false,
        HostKind::Hvf => mode == DirectoryShareMode::GuestNativeExt4,
        HostKind::Whpx => mode == DirectoryShareMode::PortableMetadata,
    };
    if allowed {
        Ok(())
    } else {
        Err(ContractError::ForeignUid)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotTier {
    Platform,
    WorkloadStart,
    InstanceCheckpoint,
}

impl SnapshotTier {
    pub fn all() -> [Self; 3] {
        [
            Self::Platform,
            Self::WorkloadStart,
            Self::InstanceCheckpoint,
        ]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Platform => "platform",
            Self::WorkloadStart => "workload-start",
            Self::InstanceCheckpoint => "instance-checkpoint",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScratchPolicy {
    Fresh,
    Paired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreAction {
    Clone,
    Resume,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TierRule {
    pub tier: SnapshotTier,
    pub action: RestoreAction,
    pub scratch: ScratchPolicy,
    pub cross_tenant: bool,
    pub tenant_bound_by_digest: bool,
    pub single_use: bool,
}

pub fn tier_rule(tier: SnapshotTier) -> TierRule {
    match tier {
        SnapshotTier::Platform => TierRule {
            tier,
            action: RestoreAction::Clone,
            scratch: ScratchPolicy::Fresh,
            cross_tenant: true,
            tenant_bound_by_digest: false,
            single_use: false,
        },
        SnapshotTier::WorkloadStart => TierRule {
            tier,
            action: RestoreAction::Clone,
            scratch: ScratchPolicy::Paired,
            cross_tenant: false,
            tenant_bound_by_digest: true,
            single_use: false,
        },
        SnapshotTier::InstanceCheckpoint => TierRule {
            tier,
            action: RestoreAction::Resume,
            scratch: ScratchPolicy::Paired,
            cross_tenant: false,
            tenant_bound_by_digest: true,
            single_use: true,
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureDisposition {
    KeepSourceAlive,
}

pub fn capture_disposition() -> CaptureDisposition {
    CaptureDisposition::KeepSourceAlive
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreDisposition {
    Ready,
    TornDown,
}

pub fn disposition_after_restore(interrupted: bool) -> RestoreDisposition {
    if interrupted {
        RestoreDisposition::TornDown
    } else {
        RestoreDisposition::Ready
    }
}

pub fn share_warm_memory(source_tenant: &str, target_tenant: &str) -> Result<(), ContractError> {
    if source_tenant == target_tenant {
        Ok(())
    } else {
        Err(ContractError::CrossTenantWarmMemory)
    }
}

pub fn restore_on_host(saved: HostKind, requested: HostKind) -> Result<(), ContractError> {
    if saved == requested {
        Ok(())
    } else {
        Err(ContractError::CrossHypervisorRestore)
    }
}

/// Write `body` into a temp directory, then rename it once onto `name`.
/// An existing destination is left unchanged.
pub fn publish_snapshot(root: &Path, name: &str, body: &[u8]) -> Result<PathBuf, ContractError> {
    let destination = root.join(name);
    if destination.exists() {
        return Err(ContractError::SnapshotDestinationExists);
    }
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let staging = root.join(format!(".staging-{name}-{unique}"));
    fs::create_dir_all(&staging).map_err(|_| ContractError::SnapshotDestinationExists)?;
    fs::write(staging.join("payload"), body)
        .map_err(|_| ContractError::SnapshotDestinationExists)?;
    match fs::rename(&staging, &destination) {
        Ok(()) => Ok(destination),
        Err(_) => {
            let _ = fs::remove_dir_all(&staging);
            Err(ContractError::SnapshotDestinationExists)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionStage {
    Structure,
    Capability,
    Policy,
    Host,
}

pub fn admission_order() -> [AdmissionStage; 4] {
    [
        AdmissionStage::Structure,
        AdmissionStage::Capability,
        AdmissionStage::Policy,
        AdmissionStage::Host,
    ]
}

#[derive(Debug, Clone, Copy)]
pub struct AdmissionChecks {
    pub structure_ok: bool,
    pub capability_ok: bool,
    pub policy_ok: bool,
    pub host_ok: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Admission {
    pub rejected_at: Option<AdmissionStage>,
    pub state_written: bool,
}

pub fn admit(checks: AdmissionChecks) -> Admission {
    let rejected_at = if !checks.structure_ok {
        Some(AdmissionStage::Structure)
    } else if !checks.capability_ok {
        Some(AdmissionStage::Capability)
    } else if !checks.policy_ok {
        Some(AdmissionStage::Policy)
    } else if !checks.host_ok {
        Some(AdmissionStage::Host)
    } else {
        None
    };
    Admission {
        rejected_at,
        state_written: rejected_at.is_none(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecOutcome {
    Exited,
    Signaled,
    TimedOut,
    Cancelled,
    Failed,
    Indeterminate,
}

impl ExecOutcome {
    pub fn all() -> [Self; 6] {
        [
            Self::Exited,
            Self::Signaled,
            Self::TimedOut,
            Self::Cancelled,
            Self::Failed,
            Self::Indeterminate,
        ]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Exited => "exited",
            Self::Signaled => "signaled",
            Self::TimedOut => "timed-out",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
            Self::Indeterminate => "indeterminate",
        }
    }
}

pub fn exec_outcomes(_host: HostKind) -> [ExecOutcome; 6] {
    ExecOutcome::all()
}

pub fn reattach_network(saved_digest: &str, offered_digest: &str) -> Result<(), ContractError> {
    if saved_digest == offered_digest {
        Ok(())
    } else {
        Err(ContractError::NetworkDigestMismatch)
    }
}

pub fn reattach_block(saved: RootRole, offered: RootRole) -> Result<(), ContractError> {
    if saved == offered {
        Ok(())
    } else {
        Err(ContractError::BlockRoleMismatch)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointKind {
    UnixSocket,
    NamedPipe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlChannel {
    pub same_user: bool,
    pub endpoint: EndpointKind,
    pub record_max_bytes: u32,
    pub credit_window: u32,
}

pub fn control_channel(host: HostKind) -> ControlChannel {
    ControlChannel {
        same_user: true,
        endpoint: match host {
            HostKind::Kvm | HostKind::Hvf => EndpointKind::UnixSocket,
            HostKind::Whpx => EndpointKind::NamedPipe,
        },
        record_max_bytes: CONTROL_RECORD_MAX_BYTES,
        credit_window: CONTROL_CREDIT_WINDOW,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlLaunch {
    pub argv: Vec<String>,
    pub env: Vec<(String, String)>,
    pub log: String,
    pub snapshot: Vec<u8>,
}

/// The credential is not copied into argv, the environment, logs, or snapshot bytes.
pub fn control_launch(credential: &str) -> ControlLaunch {
    let _ = credential;
    ControlLaunch {
        argv: vec!["aster-box".to_string(), "exec".to_string()],
        env: vec![("ASTER_BOX_CONTROL".to_string(), "present".to_string())],
        log: "control-channel-open".to_string(),
        snapshot: stored_profile_bytes(None),
    }
}

pub fn launch_contains_credential(launch: &ControlLaunch, credential: &str) -> bool {
    if credential.is_empty() {
        return false;
    }
    launch.argv.iter().any(|arg| arg.contains(credential))
        || launch
            .env
            .iter()
            .any(|(key, value)| key.contains(credential) || value.contains(credential))
        || launch.log.contains(credential)
        || launch
            .snapshot
            .windows(credential.len())
            .any(|window| window == credential.as_bytes())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeardownReport {
    pub schema_version: u32,
    pub opaque_id: String,
    pub category: String,
    pub counts: BTreeMap<String, u64>,
}

pub fn teardown_report(
    opaque_id: impl Into<String>,
    category: impl Into<String>,
    counts: BTreeMap<String, u64>,
) -> TeardownReport {
    TeardownReport {
        schema_version: 1,
        opaque_id: opaque_id.into(),
        category: category.into(),
        counts,
    }
}

pub fn teardown_report_json(report: &TeardownReport) -> Value {
    json!({
        "schema_version": report.schema_version,
        "opaque_id": report.opaque_id,
        "category": report.category,
        "counts": report.counts,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupervisorPolicy {
    pub workload_is_child: bool,
    pub cgroup: bool,
    pub namespace: bool,
    pub drop_capabilities: bool,
}

pub fn supervisor_policy(_host: HostKind) -> SupervisorPolicy {
    SupervisorPolicy {
        workload_is_child: true,
        cgroup: true,
        namespace: true,
        drop_capabilities: true,
    }
}

pub fn kernel_source_digest(config: &[u8], patches: &[&[u8]]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(config);
    for patch in patches {
        hasher.update(patch);
    }
    hex::encode(hasher.finalize())
}

pub fn verify_kernel_source(isa: GuestIsa) -> Result<String, ContractError> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("guest-kernel")
        .join(isa.as_str());
    let config = fs::read(dir.join("config")).map_err(|_| ContractError::KernelMissing)?;
    let mut patches = Vec::new();
    let patch_dir = dir.join("patches");
    let mut names = fs::read_dir(&patch_dir)
        .map_err(|_| ContractError::KernelMissing)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("patch"))
        .collect::<Vec<_>>();
    names.sort();
    for path in &names {
        patches.push(fs::read(path).map_err(|_| ContractError::KernelMissing)?);
    }
    let patch_refs = patches.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let digest = kernel_source_digest(&config, &patch_refs);
    let recorded = fs::read_to_string(dir.join("SOURCE-DIGEST"))
        .map_err(|_| ContractError::KernelMissing)?
        .trim()
        .to_string();
    if digest == recorded {
        Ok(digest)
    } else {
        Err(ContractError::KernelDigest)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LatencySample {
    pub host: HostKind,
    pub launch_to_runnable_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LatencyGate {
    pub max_ms: u64,
}

/// Available only when KVM, HVF, and WHPX each have a sample inside the gate.
/// A missing series stays unavailable. This does not record a live measurement.
pub fn latency_feature_available(samples: &[LatencySample], gate: LatencyGate) -> bool {
    HostKind::all().iter().all(|host| {
        let series = samples.iter().filter(|sample| sample.host == *host);
        let mut seen = false;
        for sample in series {
            seen = true;
            if sample.launch_to_runnable_ms > gate.max_ms {
                return false;
            }
        }
        seen
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmOnlyFeature {
    Profile,
    Snapshot,
    ControlChannel,
    Supervisor,
    KernelContract,
    Latency,
}

impl VmOnlyFeature {
    pub fn all() -> [Self; 6] {
        [
            Self::Profile,
            Self::Snapshot,
            Self::ControlChannel,
            Self::Supervisor,
            Self::KernelContract,
            Self::Latency,
        ]
    }
}

pub fn open_vm_only(
    selection: IsolationSelection,
    _feature: VmOnlyFeature,
) -> Result<(), ContractError> {
    let isolation = match selection {
        IsolationSelection::HardwareVm => IsolationClass::DedicatedVm,
        IsolationSelection::Sandbox => IsolationClass::SharedHostKernel,
    };
    core_vm::vm_only_permitted(isolation).map_err(map_contract)
}

#[cfg(test)]
#[path = "hardware_vm_tests.rs"]
mod tests;
