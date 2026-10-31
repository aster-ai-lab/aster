use std::collections::BTreeMap;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use super::*;

fn binding(version: u32, isa: GuestIsa, host: HostKind) -> SnapshotBinding {
    SnapshotBinding { version, isa, host }
}

fn sample_root() -> RootIdentity {
    identify_root(&[
        RootImage {
            role: RootRole::Scratch,
            digest: "sha256:scratch".to_string(),
            format: RootFormat::Ext4,
        },
        RootImage {
            role: RootRole::Custom,
            digest: "sha256:custom".to_string(),
            format: RootFormat::Erofs,
        },
        RootImage {
            role: RootRole::Distro,
            digest: "sha256:distro".to_string(),
            format: RootFormat::Erofs,
        },
        RootImage {
            role: RootRole::Runtime,
            digest: "sha256:runtime".to_string(),
            format: RootFormat::Erofs,
        },
    ])
    .expect("sample root")
}

#[test]
fn profile_bytes_match_on_kvm_hvf_and_whpx() {
    let reference = profile_bytes(&profile_for(HostKind::Kvm, GuestIsa::X86_64));
    assert!(!reference.is_empty());
    for host in HostKind::all() {
        for isa in GuestIsa::all() {
            let profile = profile_for(host, isa);
            assert_eq!(profile, HardwareVmProfile::current());
            assert_eq!(profile_bytes(&profile), reference);
            assert_eq!(profile.version, PROFILE_VERSION);
            assert_eq!(profile.vcpu_choices, VCPU_CHOICES.to_vec());
            assert_eq!(profile.memory_min_mib, MEMORY_MIN_MIB);
            assert_eq!(profile.memory_max_mib, MEMORY_MAX_MIB);
            assert_eq!(profile.memory_align_mib, MEMORY_ALIGN_MIB);
            assert_eq!(profile.lifecycle, LIFECYCLE.to_vec());
            assert_eq!(
                profile.snapshot_tiers,
                SnapshotTier::all()
                    .into_iter()
                    .map(SnapshotTier::as_str)
                    .collect::<Vec<_>>()
            );
        }
    }
    for tag in ["kvm", "hvf", "whpx", "x86_64", "aarch64", "linux-direct"] {
        let needle = tag.as_bytes();
        assert!(
            !reference
                .windows(needle.len())
                .any(|window| window == needle),
            "{tag} must stay out of the shared profile bytes"
        );
    }
}

#[test]
fn boot_layout_is_two_isa_copies() {
    let x86 = boot_layout(GuestIsa::X86_64);
    let arm = boot_layout(GuestIsa::Aarch64);
    assert_eq!(x86.isa, GuestIsa::X86_64);
    assert_eq!(arm.isa, GuestIsa::Aarch64);
    assert_eq!(x86.protocol, "linux-direct-x86_64");
    assert_eq!(arm.protocol, "linux-direct-aarch64");
    assert_ne!(x86.protocol, arm.protocol);
    for host in HostKind::all() {
        let _ = host;
        assert_eq!(boot_layout(GuestIsa::X86_64), x86);
        assert_eq!(boot_layout(GuestIsa::Aarch64), arm);
    }
}

#[test]
fn device_outside_the_allowlist_requires_a_version_bump() {
    let profile = HardwareVmProfile::current();
    assert_eq!(profile.devices, DEVICE_ALLOWLIST.to_vec());
    for device in profile.devices {
        assert_eq!(admit_device(PROFILE_VERSION, device), Ok(()));
        assert_eq!(
            admit_device(PROFILE_VERSION + 1, device),
            Err(ContractError::DeviceNotAllowed(device.to_string()))
        );
    }
    for device in ["virtio-pci-blk", "pci-rng", "pmio-0x605"] {
        assert_eq!(
            admit_device(PROFILE_VERSION, device),
            Err(ContractError::DeviceNotAllowed(device.to_string()))
        );
    }
}

#[test]
fn vcpu_and_memory_bounds_are_not_per_hypervisor() {
    for vcpus in VCPU_CHOICES {
        assert_eq!(validate_profile_resources(vcpus, MEMORY_MIN_MIB), Ok(()));
        assert_eq!(validate_profile_resources(vcpus, MEMORY_MAX_MIB), Ok(()));
    }
    assert_eq!(
        validate_profile_resources(3, 256),
        Err(ContractError::VcpuOutOfProfile(3))
    );
    assert_eq!(
        validate_profile_resources(16, 256),
        Err(ContractError::VcpuOutOfProfile(16))
    );
    assert_eq!(
        validate_profile_resources(1, 64),
        Err(ContractError::MemoryOutOfProfile(64))
    );
    assert_eq!(
        validate_profile_resources(1, 129),
        Err(ContractError::MemoryOutOfProfile(129))
    );
    assert_eq!(
        validate_profile_resources(1, MEMORY_MAX_MIB + MEMORY_ALIGN_MIB),
        Err(ContractError::MemoryOutOfProfile(
            MEMORY_MAX_MIB + MEMORY_ALIGN_MIB
        ))
    );
    let choices: Vec<_> = HostKind::all()
        .into_iter()
        .map(|host| profile_for(host, GuestIsa::Aarch64).vcpu_choices)
        .collect();
    assert!(choices.windows(2).all(|pair| pair[0] == pair[1]));
}

#[test]
fn restore_refuses_version_isa_or_host_mismatch() {
    let saved = binding(PROFILE_VERSION, GuestIsa::X86_64, HostKind::Kvm);
    assert_eq!(
        restore_permitted(saved, PROFILE_VERSION, GuestIsa::X86_64, HostKind::Kvm),
        Ok(())
    );
    assert_eq!(
        restore_permitted(saved, PROFILE_VERSION + 1, GuestIsa::X86_64, HostKind::Kvm),
        Err(ContractError::RestoreMismatch)
    );
    assert_eq!(
        restore_permitted(saved, PROFILE_VERSION, GuestIsa::Aarch64, HostKind::Kvm),
        Err(ContractError::RestoreMismatch)
    );
    for host in [HostKind::Hvf, HostKind::Whpx] {
        assert_eq!(
            restore_permitted(saved, PROFILE_VERSION, GuestIsa::X86_64, host),
            Err(ContractError::RestoreMismatch)
        );
        assert_eq!(
            restore_on_host(saved.host, host),
            Err(ContractError::CrossHypervisorRestore)
        );
    }
    assert_eq!(restore_on_host(HostKind::Hvf, HostKind::Hvf), Ok(()));
}

#[test]
fn stored_profile_ignores_oci_migration_and_isolation_selector() {
    let baseline = stored_profile_bytes(None);
    for migration in [None, Some(""), Some("off"), Some("microvm"), Some("all")] {
        assert_eq!(stored_profile_bytes(migration), baseline);
        assert_eq!(
            select_isolation(None, migration),
            Ok(IsolationSelection::HardwareVm)
        );
        assert_eq!(
            select_isolation(Some("microvm"), migration),
            Err(ContractError::MicrovmSpellingRejected)
        );
        assert_eq!(
            select_isolation(Some("sandbox"), migration),
            Ok(IsolationSelection::Sandbox)
        );
    }
    assert_eq!(
        select_isolation(Some("container"), None),
        Err(ContractError::UnknownIsolation)
    );
    assert_eq!(baseline, profile_bytes(&HardwareVmProfile::current()));
}

#[test]
fn root_identity_is_role_plus_digest_not_input_order() {
    let forward = sample_root();
    let reversed = identify_root(&[
        RootImage {
            role: RootRole::Runtime,
            digest: "sha256:runtime".to_string(),
            format: RootFormat::Erofs,
        },
        RootImage {
            role: RootRole::Distro,
            digest: "sha256:distro".to_string(),
            format: RootFormat::Erofs,
        },
        RootImage {
            role: RootRole::Scratch,
            digest: "sha256:scratch".to_string(),
            format: RootFormat::Ext4,
        },
        RootImage {
            role: RootRole::Custom,
            digest: "sha256:custom".to_string(),
            format: RootFormat::Erofs,
        },
    ])
    .expect("reversed root");
    assert_eq!(forward.canonical_bytes(), reversed.canonical_bytes());
    let text = String::from_utf8(forward.canonical_bytes()).expect("utf8 identity");
    let roles: Vec<_> = text
        .split('\0')
        .filter(|part| !part.is_empty())
        .map(|part| part.split(':').next().expect("role"))
        .collect();
    assert_eq!(roles, ["distro", "runtime", "custom", "scratch"]);
    assert!(text.contains("sha256:distro"));
    assert!(text.contains("sha256:scratch"));
}

#[test]
fn root_identity_rejects_bad_format_digest_and_missing_scratch() {
    let scratch = RootImage {
        role: RootRole::Scratch,
        digest: "sha256:scratch".to_string(),
        format: RootFormat::Ext4,
    };
    let distro = RootImage {
        role: RootRole::Distro,
        digest: "sha256:distro".to_string(),
        format: RootFormat::Erofs,
    };
    assert_eq!(
        identify_root(&[distro.clone()]),
        Err(ContractError::RootIdentity)
    );
    assert_eq!(
        identify_root(&[
            distro.clone(),
            RootImage {
                role: RootRole::Distro,
                digest: "sha256:other".to_string(),
                format: RootFormat::Erofs,
            },
            scratch.clone(),
        ]),
        Err(ContractError::RootIdentity)
    );
    assert_eq!(
        identify_root(&[
            RootImage {
                role: RootRole::Distro,
                digest: String::new(),
                format: RootFormat::Erofs,
            },
            scratch.clone(),
        ]),
        Err(ContractError::RootIdentity)
    );
    assert_eq!(
        identify_root(&[
            RootImage {
                role: RootRole::Custom,
                digest: "sha256:custom".to_string(),
                format: RootFormat::Ext4,
            },
            scratch.clone(),
        ]),
        Err(ContractError::RootFormat("custom"))
    );
    assert_eq!(
        identify_root(&[RootImage {
            role: RootRole::Scratch,
            digest: "sha256:scratch".to_string(),
            format: RootFormat::Erofs,
        }]),
        Err(ContractError::ScratchFormat)
    );
}

#[test]
fn virtio_fs_shares_stay_out_of_snapshot_bytes_and_reattach_on_match() {
    let share = ShareAttachment {
        path: "/aster-box-virtiofs-share/not-in-snapshot-7f3c".to_string(),
        mode: "read-write".to_string(),
        deny: vec!["b".to_string(), "a".to_string()],
    };
    let with_share = build_snapshot(
        binding(PROFILE_VERSION, GuestIsa::Aarch64, HostKind::Hvf),
        SnapshotTier::WorkloadStart,
        sample_root(),
        "sha256:net",
        &[share.clone()],
    );
    let without_share = build_snapshot(
        binding(PROFILE_VERSION, GuestIsa::Aarch64, HostKind::Hvf),
        SnapshotTier::WorkloadStart,
        sample_root(),
        "sha256:net",
        &[],
    );
    assert_eq!(snapshot_bytes(&with_share), snapshot_bytes(&without_share));
    assert!(!share_in_snapshot(&with_share, &share));
    let reordered = ShareAttachment {
        deny: vec!["a".to_string(), "b".to_string()],
        ..share.clone()
    };
    assert_eq!(reattach_share(&share, &reordered), Ok(()));
    let other_path = ShareAttachment {
        path: "/elsewhere".to_string(),
        ..share.clone()
    };
    assert_eq!(
        reattach_share(&share, &other_path),
        Err(ContractError::VirtioFsMismatch)
    );
    let other_mode = ShareAttachment {
        mode: "read-only".to_string(),
        ..share.clone()
    };
    assert_eq!(
        reattach_share(&share, &other_mode),
        Err(ContractError::VirtioFsMismatch)
    );
    let other_deny = ShareAttachment {
        deny: vec!["a".to_string()],
        ..share
    };
    assert_eq!(
        reattach_share(&reordered, &other_deny),
        Err(ContractError::VirtioFsMismatch)
    );
}

#[test]
fn foreign_uid_share_fails_closed_per_host() {
    for host in HostKind::all() {
        for mode in [
            DirectoryShareMode::HostDirectory,
            DirectoryShareMode::GuestNativeExt4,
            DirectoryShareMode::PortableMetadata,
        ] {
            assert_eq!(admit_foreign_uid(host, true, mode), Ok(()));
        }
    }
    for mode in [
        DirectoryShareMode::HostDirectory,
        DirectoryShareMode::GuestNativeExt4,
        DirectoryShareMode::PortableMetadata,
    ] {
        assert_eq!(
            admit_foreign_uid(HostKind::Kvm, false, mode),
            Err(ContractError::ForeignUid)
        );
    }
    assert_eq!(
        admit_foreign_uid(HostKind::Hvf, false, DirectoryShareMode::GuestNativeExt4),
        Ok(())
    );
    assert_eq!(
        admit_foreign_uid(HostKind::Hvf, false, DirectoryShareMode::HostDirectory),
        Err(ContractError::ForeignUid)
    );
    assert_eq!(
        admit_foreign_uid(HostKind::Hvf, false, DirectoryShareMode::PortableMetadata),
        Err(ContractError::ForeignUid)
    );
    assert_eq!(
        admit_foreign_uid(HostKind::Whpx, false, DirectoryShareMode::PortableMetadata),
        Ok(())
    );
    assert_eq!(
        admit_foreign_uid(HostKind::Whpx, false, DirectoryShareMode::HostDirectory),
        Err(ContractError::ForeignUid)
    );
    assert_eq!(
        admit_foreign_uid(HostKind::Whpx, false, DirectoryShareMode::GuestNativeExt4),
        Err(ContractError::ForeignUid)
    );
}

#[test]
fn snapshot_tiers_mean_the_same_thing_on_every_host() {
    let platform = tier_rule(SnapshotTier::Platform);
    assert_eq!(platform.action, RestoreAction::Clone);
    assert_eq!(platform.scratch, ScratchPolicy::Fresh);
    assert!(platform.cross_tenant);
    assert!(!platform.tenant_bound_by_digest);
    assert!(!platform.single_use);

    let workload = tier_rule(SnapshotTier::WorkloadStart);
    assert_eq!(workload.action, RestoreAction::Clone);
    assert_eq!(workload.scratch, ScratchPolicy::Paired);
    assert!(!workload.cross_tenant);
    assert!(workload.tenant_bound_by_digest);
    assert!(!workload.single_use);

    let checkpoint = tier_rule(SnapshotTier::InstanceCheckpoint);
    assert_eq!(checkpoint.action, RestoreAction::Resume);
    assert_eq!(checkpoint.scratch, ScratchPolicy::Paired);
    assert!(!checkpoint.cross_tenant);
    assert!(checkpoint.tenant_bound_by_digest);
    assert!(checkpoint.single_use);

    let names: Vec<_> = SnapshotTier::all()
        .into_iter()
        .map(SnapshotTier::as_str)
        .collect();
    assert_eq!(names, ["platform", "workload-start", "instance-checkpoint"]);
    for host in HostKind::all() {
        let profile = profile_for(host, GuestIsa::X86_64);
        assert_eq!(profile.snapshot_tiers, names);
    }
}

#[test]
fn publish_renames_once_and_does_not_replace() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("aster-box-hwvm-publish-{unique}"));
    fs::create_dir_all(&root).expect("temp root");
    let destination = publish_snapshot(&root, "snap-a", b"body-v1").expect("publish");
    assert_eq!(
        fs::read(destination.join("payload")).expect("payload"),
        b"body-v1"
    );
    let staging_left = fs::read_dir(&root).expect("list").any(|entry| {
        entry
            .expect("entry")
            .file_name()
            .to_string_lossy()
            .starts_with(".staging-")
    });
    assert!(!staging_left);
    assert_eq!(
        publish_snapshot(&root, "snap-a", b"body-v2"),
        Err(ContractError::SnapshotDestinationExists)
    );
    assert_eq!(
        fs::read(destination.join("payload")).expect("payload"),
        b"body-v1"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn interrupted_restore_tears_down_and_capture_keeps_the_source() {
    assert_eq!(capture_disposition(), CaptureDisposition::KeepSourceAlive);
    assert_eq!(
        disposition_after_restore(true),
        RestoreDisposition::TornDown
    );
    assert_eq!(disposition_after_restore(false), RestoreDisposition::Ready);
    assert_eq!(share_warm_memory("tenant-a", "tenant-a"), Ok(()));
    assert_eq!(
        share_warm_memory("tenant-a", "tenant-b"),
        Err(ContractError::CrossTenantWarmMemory)
    );
}

#[test]
fn admission_rejects_in_order_and_writes_no_state() {
    assert_eq!(
        admission_order(),
        [
            AdmissionStage::Structure,
            AdmissionStage::Capability,
            AdmissionStage::Policy,
            AdmissionStage::Host,
        ]
    );
    let cases = [
        (false, false, false, false, Some(AdmissionStage::Structure)),
        (true, false, true, true, Some(AdmissionStage::Capability)),
        (true, true, false, true, Some(AdmissionStage::Policy)),
        (true, true, true, false, Some(AdmissionStage::Host)),
        (true, true, true, true, None),
    ];
    for (structure_ok, capability_ok, policy_ok, host_ok, rejected_at) in cases {
        let admission = admit(AdmissionChecks {
            structure_ok,
            capability_ok,
            policy_ok,
            host_ok,
        });
        assert_eq!(admission.rejected_at, rejected_at);
        assert_eq!(admission.state_written, rejected_at.is_none());
    }
}

#[test]
fn exec_outcomes_and_channel_limits_match_across_hosts() {
    let expected = [
        "exited",
        "signaled",
        "timed-out",
        "cancelled",
        "failed",
        "indeterminate",
    ];
    let mut limits = Vec::new();
    for host in HostKind::all() {
        let got: Vec<_> = exec_outcomes(host)
            .iter()
            .map(|outcome| outcome.as_str())
            .collect();
        assert_eq!(got, expected);
        let channel = control_channel(host);
        assert!(channel.same_user);
        limits.push((channel.record_max_bytes, channel.credit_window));
        match host {
            HostKind::Kvm | HostKind::Hvf => {
                assert_eq!(channel.endpoint, EndpointKind::UnixSocket);
            }
            HostKind::Whpx => assert_eq!(channel.endpoint, EndpointKind::NamedPipe),
        }
    }
    assert!(limits.windows(2).all(|pair| pair[0] == pair[1]));
    assert_eq!(limits[0].0, CONTROL_RECORD_MAX_BYTES);
    assert_eq!(limits[0].1, CONTROL_CREDIT_WINDOW);
    assert_eq!(reattach_network("sha256:net", "sha256:net"), Ok(()));
    assert_eq!(
        reattach_network("sha256:net", "sha256:other"),
        Err(ContractError::NetworkDigestMismatch)
    );
    assert_eq!(reattach_block(RootRole::Distro, RootRole::Distro), Ok(()));
    assert_eq!(
        reattach_block(RootRole::Distro, RootRole::Runtime),
        Err(ContractError::BlockRoleMismatch)
    );
}

#[test]
fn control_credential_is_absent_from_the_launch_record() {
    let credential = "launch-credential-6c1e9a44";
    let launch = control_launch(credential);
    assert!(!launch_contains_credential(&launch, credential));
    assert!(launch.argv.iter().all(|arg| !arg.contains(credential)));
    assert!(launch
        .env
        .iter()
        .all(|(key, value)| !key.contains(credential) && !value.contains(credential)));
    assert!(!launch.log.contains(credential));
    assert!(!launch
        .snapshot
        .windows(credential.len())
        .any(|window| window == credential.as_bytes()));
}

#[test]
fn teardown_report_is_schema_id_category_and_counts() {
    let mut counts = BTreeMap::new();
    counts.insert("exited".to_string(), 2);
    let report = teardown_report("opaque-1", "stopped", counts);
    let json = teardown_report_json(&report);
    let object = json.as_object().expect("object");
    let mut keys: Vec<_> = object.keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, ["category", "counts", "opaque_id", "schema_version"]);
    assert_eq!(
        object.get("schema_version").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        object.get("opaque_id").and_then(Value::as_str),
        Some("opaque-1")
    );
    assert_eq!(
        object.get("category").and_then(Value::as_str),
        Some("stopped")
    );
    assert_eq!(
        object
            .get("counts")
            .and_then(Value::as_object)
            .and_then(|counts| counts.get("exited"))
            .and_then(Value::as_u64),
        Some(2)
    );
}

#[test]
fn supervisor_stays_the_workload_parent_on_every_host() {
    let mut policies = Vec::new();
    for host in HostKind::all() {
        let policy = supervisor_policy(host);
        assert!(policy.workload_is_child);
        assert!(policy.cgroup);
        assert!(policy.namespace);
        assert!(policy.drop_capabilities);
        policies.push(policy);
    }
    assert!(policies.windows(2).all(|pair| pair[0] == pair[1]));
}

#[test]
fn kernel_contract_checks_each_isa_source_digest() {
    let x86 = verify_kernel_source(GuestIsa::X86_64).expect("x86_64 kernel contract");
    let arm = verify_kernel_source(GuestIsa::Aarch64).expect("aarch64 kernel contract");
    assert_eq!(x86.len(), 64);
    assert_eq!(arm.len(), 64);
    assert_ne!(x86, arm);
    let config = fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("guest-kernel")
            .join("x86_64")
            .join("config"),
    )
    .expect("config");
    let patch = fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("guest-kernel")
            .join("x86_64")
            .join("patches")
            .join("0001-role-block.patch"),
    )
    .expect("patch");
    let partial = kernel_source_digest(&config, &[patch.as_slice()]);
    assert_ne!(partial, x86);
}

#[test]
fn latency_feature_stays_unavailable_until_every_series_is_inside_the_gate() {
    let gate = LatencyGate { max_ms: 100 };
    let samples = [
        LatencySample {
            host: HostKind::Kvm,
            launch_to_runnable_ms: 40,
        },
        LatencySample {
            host: HostKind::Hvf,
            launch_to_runnable_ms: 100,
        },
        LatencySample {
            host: HostKind::Whpx,
            launch_to_runnable_ms: 70,
        },
    ];
    assert!(latency_feature_available(&samples, gate));
    assert!(!latency_feature_available(&samples[..2], gate));
    assert!(!latency_feature_available(&[], gate));
    let mut over = samples;
    over[2].launch_to_runnable_ms = 101;
    assert!(!latency_feature_available(&over, gate));
    let kvm_only = [
        LatencySample {
            host: HostKind::Kvm,
            launch_to_runnable_ms: 1,
        },
        LatencySample {
            host: HostKind::Kvm,
            launch_to_runnable_ms: 2,
        },
    ];
    assert!(!latency_feature_available(&kvm_only, gate));
}

#[test]
fn sandbox_rejects_every_vm_only_feature() {
    let sandbox = select_isolation(Some("sandbox"), Some("all")).expect("sandbox");
    let hardware = select_isolation(None, None).expect("hardware-vm");
    assert_eq!(VmOnlyFeature::all().len(), 6);
    for feature in VmOnlyFeature::all() {
        assert_eq!(
            open_vm_only(sandbox, feature),
            Err(ContractError::SandboxRejectsVmOnly)
        );
        assert_eq!(open_vm_only(hardware, feature), Ok(()));
    }
}

#[test]
fn profile_bytes_match_the_oci_core_contract() {
    use aster_oci_core::hardware_vm::{self as core_vm, GuestArch};
    use aster_oci_core::{DriverKind, IsolationClass};

    let shared = core_vm::profile_bytes(&core_vm::HardwareVmProfile::shared());
    for host in HostKind::all() {
        for isa in GuestIsa::all() {
            assert_eq!(profile_bytes(&profile_for(host, isa)), shared);
            assert_eq!(
                core_vm::virtiofs_share_mode(host.to_driver()),
                Some(match host {
                    HostKind::Kvm | HostKind::Hvf => "same-uid-private-per-generation-virtiofs",
                    HostKind::Whpx => "protected-per-generation-virtiofs",
                })
            );
        }
    }
    assert_eq!(
        HardwareVmProfile::current().snapshot_tiers,
        SnapshotTier::all()
            .into_iter()
            .map(SnapshotTier::as_str)
            .collect::<Vec<_>>()
    );
    assert!(core_vm::profile_for(DriverKind::NativeLinux, GuestArch::X86_64).is_err());
    assert!(core_vm::vm_only_permitted(IsolationClass::SharedHostKernel).is_err());
    assert!(core_vm::vm_only_permitted(IsolationClass::DedicatedVm).is_ok());
    assert!(core_vm::vm_only_permitted(IsolationClass::SharedGuestKernel).is_ok());
}
