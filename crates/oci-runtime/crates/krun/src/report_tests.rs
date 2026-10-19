#![cfg(test)]

use super::{KrunAgentVmSmokeReport, MacosBootAssetsEvidence};
use a3s_oci_core::{CapabilityStatus, HostPlatform};

#[test]
fn macos_agent_success_requires_explicit_host_confinement_evidence() {
    let mut report = KrunAgentVmSmokeReport::initial(HostPlatform::Macos, crate::fallback_config());
    report.status = CapabilityStatus::Available;
    report.runtime_bundle_loaded = true;
    report.context_created = true;
    report.vm_configured = true;
    report.rootfs_configured = true;
    report.runtime_share_configured = true;
    report.agent_binary_present = true;
    report.agent_vsock_configured = true;
    report.workload_configured = true;
    report.console_configured = true;
    report.vm_entered = true;
    report.guest_exit_code = Some(0);
    report.console_created = true;
    let digest = "a".repeat(64);
    report.macos_boot_assets = Some(MacosBootAssetsEvidence {
        manifest_sha256: digest.clone(),
        system_image_sha256: digest.clone(),
        system_image_size: 67_108_864,
        runtime_archive_sha256: digest.clone(),
        libkrun_sha256: digest.clone(),
        firmware_sha256: digest.clone(),
        kernel_bundle_sha256: digest,
        kernel_bundle_size: 22_740_992,
        kernel_guest_load_address: "0x0000000080000000".into(),
        kernel_entry_address: "0x0000000080000000".into(),
        root_disk_read_only: true,
        runtime_share_separate: true,
    });
    assert!(!report.is_success());
    report.macos_host_sandbox_enforced = Some(false);
    assert!(!report.is_success());
    report.macos_host_sandbox_enforced = Some(true);
    assert!(report.is_success());
    assert_eq!(
        serde_json::to_value(report).unwrap()["macos_host_sandbox_enforced"],
        true
    );
}
