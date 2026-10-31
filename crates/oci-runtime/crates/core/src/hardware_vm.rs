//! Versioned hardware-vm contract shared by Aster Box and the OCI runtime.
//!
//! KVM, HVF, and WHPX keep one profile: device roles, lifecycle names,
//! snapshot tier names, vCPU and memory bounds, and control-channel limits.
//! Guest architecture selects only the boot layout. This module does not
//! start a hypervisor.

use thiserror::Error;

use crate::{DriverKind, IsolationClass};

/// Profile accepted by restore and by every hardware-vm host.
pub const PROFILE_VERSION: u32 = 1;
/// Maximum control record accepted on every host.
pub const CONTROL_RECORD_MAX_BYTES: u32 = 65_536;
/// Credit window applied on every host.
pub const CONTROL_CREDIT_WINDOW: u32 = 32;
/// Smallest guest memory, in MiB.
pub const MEMORY_MIN_MIB: u32 = 128;
/// Largest guest memory, in MiB.
pub const MEMORY_MAX_MIB: u32 = 8_192;
/// Guest memory alignment, in MiB.
pub const MEMORY_ALIGN_MIB: u32 = 128;
/// vCPU counts the profile accepts on every host.
pub const VCPU_CHOICES: [u32; 4] = [1, 2, 4, 8];

/// Guest devices a profile may expose. A device outside this list needs a new
/// profile version.
pub const DEVICE_ALLOWLIST: [&str; 5] = [
    "virtio-mmio-blk",
    "virtio-mmio-console",
    "virtio-mmio-fs",
    "virtio-mmio-net",
    "virtio-mmio-vsock",
];

/// Lifecycle operation names stored in the profile.
pub const LIFECYCLE: [&str; 5] = ["provision", "start", "exec", "stop", "deprovision"];

/// Snapshot tier names stored in the profile.
pub const SNAPSHOT_TIERS: [&str; 3] = ["platform", "workload-start", "instance-checkpoint"];

const KVM_HVF_VIRTIOFS_SHARE: &str = "same-uid-private-per-generation-virtiofs";
const WHPX_VIRTIOFS_SHARE: &str = "protected-per-generation-virtiofs";

/// Guest ISA that a libkrun hardware VM can restore.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuestArch {
    /// x86_64 Linux guest.
    X86_64,
    /// aarch64 Linux guest.
    Aarch64,
}

impl GuestArch {
    /// Both guest architectures covered by the contract.
    #[must_use]
    pub const fn all() -> [Self; 2] {
        [Self::X86_64, Self::Aarch64]
    }

    /// Canonical architecture token stored by checkpoint compatibility.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::X86_64 => "x86_64",
            Self::Aarch64 => "aarch64",
        }
    }
}

/// Boot protocol for one guest ISA. The hypervisor kind does not change it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootLayout {
    /// Guest architecture that selects this protocol.
    pub arch: GuestArch,
    /// Protocol name stored beside the profile, not inside profile bytes.
    pub protocol: &'static str,
}

/// Select the ISA boot protocol.
#[must_use]
pub const fn boot_layout(arch: GuestArch) -> BootLayout {
    match arch {
        GuestArch::X86_64 => BootLayout {
            arch,
            protocol: "linux-direct-x86_64",
        },
        GuestArch::Aarch64 => BootLayout {
            arch,
            protocol: "linux-direct-aarch64",
        },
    }
}

/// Identical machine profile for every libkrun host and both guest ISAs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardwareVmProfile {
    /// Profile version stored with a snapshot.
    pub version: u32,
    /// Guest device allowlist, in role order.
    pub devices: Vec<&'static str>,
    /// Lifecycle names, in admission order.
    pub lifecycle: Vec<&'static str>,
    /// Snapshot tier names.
    pub snapshot_tiers: Vec<&'static str>,
    /// Allowed vCPU counts.
    pub vcpu_choices: Vec<u32>,
    /// Minimum memory in MiB.
    pub memory_min_mib: u32,
    /// Maximum memory in MiB.
    pub memory_max_mib: u32,
    /// Memory alignment in MiB.
    pub memory_align_mib: u32,
    /// Control record limit.
    pub control_record_max_bytes: u32,
    /// Control credit window.
    pub control_credit_window: u32,
}

impl HardwareVmProfile {
    /// Profile version 1. Host kind and guest ISA are not fields of this value.
    #[must_use]
    pub fn shared() -> Self {
        Self {
            version: PROFILE_VERSION,
            devices: DEVICE_ALLOWLIST.to_vec(),
            lifecycle: LIFECYCLE.to_vec(),
            snapshot_tiers: SNAPSHOT_TIERS.to_vec(),
            vcpu_choices: VCPU_CHOICES.to_vec(),
            memory_min_mib: MEMORY_MIN_MIB,
            memory_max_mib: MEMORY_MAX_MIB,
            memory_align_mib: MEMORY_ALIGN_MIB,
            control_record_max_bytes: CONTROL_RECORD_MAX_BYTES,
            control_credit_window: CONTROL_CREDIT_WINDOW,
        }
    }
}

/// Encode the profile. The bytes contain no host kind and no guest ISA.
#[must_use]
pub fn profile_bytes(profile: &HardwareVmProfile) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend(profile.version.to_le_bytes());
    for device in &profile.devices {
        bytes.extend(device.as_bytes());
        bytes.push(0);
    }
    for phase in &profile.lifecycle {
        bytes.extend(phase.as_bytes());
        bytes.push(0);
    }
    for tier in &profile.snapshot_tiers {
        bytes.extend(tier.as_bytes());
        bytes.push(0);
    }
    for vcpu in &profile.vcpu_choices {
        bytes.extend(vcpu.to_le_bytes());
    }
    bytes.extend(profile.memory_min_mib.to_le_bytes());
    bytes.extend(profile.memory_max_mib.to_le_bytes());
    bytes.extend(profile.memory_align_mib.to_le_bytes());
    bytes.extend(profile.control_record_max_bytes.to_le_bytes());
    bytes.extend(profile.control_credit_window.to_le_bytes());
    bytes
}

/// Return the shared profile for a libkrun driver. Native Linux is refused.
/// Both guest architectures receive the same profile; [`boot_layout`] carries
/// the ISA.
pub fn profile_for(
    driver: DriverKind,
    arch: GuestArch,
) -> Result<HardwareVmProfile, HardwareVmError> {
    if !is_libkrun(driver) {
        return Err(HardwareVmError::NotHardwareVmHost { driver });
    }
    match arch {
        GuestArch::X86_64 | GuestArch::Aarch64 => Ok(HardwareVmProfile::shared()),
    }
}

/// Refusal produced by the shared hardware-vm contract.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum HardwareVmError {
    /// The device is absent from [`DEVICE_ALLOWLIST`] at this profile version.
    #[error("device {0} is outside the hardware-vm allowlist")]
    DeviceNotAllowed(String),
    /// The vCPU count is not one of [`VCPU_CHOICES`].
    #[error("vCPU count {0} is outside the hardware-vm profile")]
    VcpuOutOfProfile(u32),
    /// Memory is outside the profile range or alignment.
    #[error("memory {0} MiB is outside the hardware-vm profile")]
    MemoryOutOfProfile(u32),
    /// Restore saw a different version, architecture, or driver.
    #[error("restore refused: profile version, guest architecture, or driver differs")]
    RestoreMismatch,
    /// Shared-host-kernel isolation cannot open a hardware-vm operation.
    #[error("shared-host-kernel isolation rejects hardware-vm operations")]
    SharedHostKernelRejectsVmOnly,
    /// The driver does not own a hardware VM.
    #[error("driver {driver:?} is not a hardware-vm host")]
    NotHardwareVmHost {
        /// Driver that was refused.
        driver: DriverKind,
    },
    /// The architecture token is not `x86_64` or `aarch64`.
    #[error("guest architecture {0} is outside the hardware-vm set")]
    GuestArchNotAllowed(String),
}

/// Admit one guest device at a profile version.
pub fn admit_device(version: u32, device: &str) -> Result<(), HardwareVmError> {
    if version == PROFILE_VERSION && DEVICE_ALLOWLIST.contains(&device) {
        Ok(())
    } else {
        Err(HardwareVmError::DeviceNotAllowed(device.to_string()))
    }
}

/// Admit vCPU and memory counts before any host-specific gate.
pub fn validate_profile_resources(vcpus: u32, memory_mib: u32) -> Result<(), HardwareVmError> {
    if !VCPU_CHOICES.contains(&vcpus) {
        return Err(HardwareVmError::VcpuOutOfProfile(vcpus));
    }
    if memory_mib < MEMORY_MIN_MIB
        || memory_mib > MEMORY_MAX_MIB
        || memory_mib % MEMORY_ALIGN_MIB != 0
    {
        return Err(HardwareVmError::MemoryOutOfProfile(memory_mib));
    }
    Ok(())
}

/// Restore only when the saved profile version, guest ISA, and libkrun driver
/// are the ones requested.
pub fn restore_permitted(
    saved_version: u32,
    saved_arch: GuestArch,
    saved_driver: DriverKind,
    requested_version: u32,
    requested_arch: GuestArch,
    requested_driver: DriverKind,
) -> Result<(), HardwareVmError> {
    if is_libkrun(saved_driver)
        && saved_driver == requested_driver
        && saved_version == requested_version
        && saved_arch == requested_arch
    {
        Ok(())
    } else {
        Err(HardwareVmError::RestoreMismatch)
    }
}

/// Whether this driver is one of the three libkrun hardware-vm hosts.
#[must_use]
pub const fn is_libkrun(driver: DriverKind) -> bool {
    matches!(
        driver,
        DriverKind::LibkrunKvm | DriverKind::LibkrunHvf | DriverKind::LibkrunWhpx
    )
}

/// Driver and isolation pairs that checkpoint compatibility already accepts.
///
/// Native Linux pairs with shared-host-kernel isolation. Each libkrun driver
/// pairs with a dedicated VM or a shared guest kernel. Every other pair is
/// refused.
#[must_use]
pub const fn checkpoint_pair_permitted(driver: DriverKind, isolation: IsolationClass) -> bool {
    matches!(
        (driver, isolation),
        (DriverKind::NativeLinux, IsolationClass::SharedHostKernel)
            | (
                DriverKind::LibkrunKvm | DriverKind::LibkrunHvf | DriverKind::LibkrunWhpx,
                IsolationClass::DedicatedVm | IsolationClass::SharedGuestKernel,
            )
    )
}

/// Admit the guest architecture token used by a libkrun checkpoint.
pub fn admit_guest_arch(architecture: &str) -> Result<GuestArch, HardwareVmError> {
    match architecture {
        "x86_64" => Ok(GuestArch::X86_64),
        "aarch64" => Ok(GuestArch::Aarch64),
        other => Err(HardwareVmError::GuestArchNotAllowed(other.to_string())),
    }
}

/// Hardware-vm operations are available to a dedicated VM and to containers
/// that share one guest kernel. Shared-host-kernel isolation is refused.
pub fn vm_only_permitted(isolation: IsolationClass) -> Result<(), HardwareVmError> {
    match isolation {
        IsolationClass::DedicatedVm | IsolationClass::SharedGuestKernel => Ok(()),
        IsolationClass::SharedHostKernel => Err(HardwareVmError::SharedHostKernelRejectsVmOnly),
    }
}

/// Virtio-fs share mode recorded by a libkrun driver.
///
/// KVM and HVF advertise a same-UID private share. WHPX advertises a
/// protected per-generation share. Native Linux has no hardware-vm share.
/// Box keeps the foreign-UID lane: KVM refuses it, HVF accepts a guest-native
/// ext4 root, and WHPX accepts portable metadata.
#[must_use]
pub const fn virtiofs_share_mode(driver: DriverKind) -> Option<&'static str> {
    match driver {
        DriverKind::LibkrunKvm | DriverKind::LibkrunHvf => Some(KVM_HVF_VIRTIOFS_SHARE),
        DriverKind::LibkrunWhpx => Some(WHPX_VIRTIOFS_SHARE),
        DriverKind::NativeLinux => None,
    }
}

#[cfg(test)]
#[path = "hardware_vm_tests.rs"]
mod tests;
