#![cfg(test)]

use std::os::unix::fs::PermissionsExt;

use aster_oci_agent_protocol::{AgentVmDirectorySourceIdentity, GuestPath};
use aster_oci_sdk::{
    oci_spec::runtime::Spec, ContainerId, ContainerTarget, CreateAttachments, Generation,
    ProcessIo, StorageAccessMode, StorageAttachmentId, StorageCleanup, StorageOwnership,
};
use serde_json::json;

use super::*;

struct Fixture {
    _temporary: tempfile::TempDir,
    root: PathBuf,
    share: MacosRuntimeShare,
    digest: String,
}

impl Fixture {
    fn new() -> Self {
        Self::with_restrictions(false)
    }

    fn with_restrictions(restricted: bool) -> Self {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().canonicalize().unwrap();
        let share = root.join("runtime");
        fs::create_dir(&share).unwrap();
        fs::set_permissions(&share, fs::Permissions::from_mode(0o700)).unwrap();
        fs::create_dir_all(share.join("bundle/rootfs")).unwrap();
        let source = root.join("工作区 \"alpha\"\\beta");
        fs::create_dir(&source).unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o755)).unwrap();
        let metadata = fs::metadata(&source).unwrap();
        let mut config = serde_json::to_value(Spec::default()).unwrap();
        config["root"] = json!({"path":"rootfs", "readonly":true});
        config["mounts"] = json!([{"destination":"/workspace", "type":"none",
            "source":source, "options":["rbind","ro"]}]);
        let policy = aster_oci_sdk::StoragePathRestrictions::new(vec![
            aster_oci_sdk::StoragePathRestriction::new(
                StorageAttachmentId::new("workspace").unwrap(),
                vec![".env".into()],
                vec![],
                vec![],
            )
            .unwrap(),
        ])
        .unwrap();
        if restricted {
            config["annotations"] = json!({aster_oci_sdk::STORAGE_PATH_RESTRICTIONS_EXTENSION: policy.to_annotation_value().unwrap()});
        }
        let encoded = config.to_string();
        fs::write(share.join("bundle/config.json"), &encoded).unwrap();
        fs::set_permissions(
            share.join("bundle/config.json"),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        let bundle = OciBundle::from_json(share.join("bundle"), encoded).unwrap();
        let mut attachments = CreateAttachments::from_bundle(&bundle, ProcessIo::default())
            .unwrap()
            .attach_storage_mount(
                &bundle,
                0,
                StorageAttachmentId::new("workspace").unwrap(),
                StorageAccessMode::ReadOnly,
                StorageOwnership::Caller,
                StorageCleanup::DetachOnly,
            )
            .unwrap();
        if restricted {
            attachments = attachments.with_storage_path_restrictions(&bundle).unwrap();
        }
        let digest = attachments.digest().unwrap();
        let directory = AgentVmDirectoryAttachment::new(
            attachments.storage()[0].clone(),
            source.to_str().unwrap(),
            AgentVmDirectorySourceIdentity::new(metadata.dev(), metadata.ino()).unwrap(),
            &digest,
        )
        .unwrap();
        let mut manifest = AgentVmAttachmentManifest::new_with_directories(
            ContainerTarget::exact(ContainerId::new("workspace").unwrap(), Generation(1)),
            GuestPath::new("/run/aster-oci-runtime/bundle").unwrap(),
            bundle.config_digest(),
            digest,
            Vec::new(),
            Vec::new(),
            vec![directory],
        )
        .unwrap();
        if restricted {
            manifest = manifest
                .with_storage_path_restrictions(&bundle, &policy)
                .unwrap();
        }
        let digest = manifest.digest().unwrap();
        fs::write(
            share.join(AGENT_VM_ATTACHMENT_MANIFEST_FILE_NAME),
            manifest.to_bytes().unwrap(),
        )
        .unwrap();
        fs::set_permissions(
            share.join(AGENT_VM_ATTACHMENT_MANIFEST_FILE_NAME),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        Self {
            _temporary: temporary,
            root,
            share: MacosRuntimeShare::open(&share).unwrap(),
            digest,
        }
    }
}

#[test]
fn pins_caller_directory_without_changing_its_permissions() {
    let fixture = Fixture::new();
    let attachments = MacosVmAttachments::open(&fixture.share, Some(&fixture.digest))
        .unwrap()
        .unwrap();
    let directory = &attachments.directories()[0];
    let pinned = File::open(directory.pinned_path())
        .unwrap()
        .metadata()
        .unwrap();
    assert_eq!(pinned.ino(), directory.grant().source_identity().inode());
    assert_eq!(pinned.permissions().mode() & 0o777, 0o755);
    attachments.reverify().unwrap();
}

#[test]
fn rejects_missing_or_wrong_digest_and_changed_manifest() {
    let fixture = Fixture::new();
    assert!(MacosVmAttachments::open(&fixture.share, None).is_err());
    assert!(
        MacosVmAttachments::open(&fixture.share, Some(&format!("sha256:{}", "0".repeat(64))))
            .is_err()
    );
    let pinned = MacosVmAttachments::open(&fixture.share, Some(&fixture.digest))
        .unwrap()
        .unwrap();
    fs::write(
        fixture
            .share
            .path()
            .join(AGENT_VM_ATTACHMENT_MANIFEST_FILE_NAME),
        b"{}",
    )
    .unwrap();
    assert!(pinned.reverify().is_err());
}

#[test]
fn rejects_directory_replacement_before_open_and_before_vm_entry() {
    for before_open in [true, false] {
        let fixture = Fixture::new();
        let pinned = if before_open {
            None
        } else {
            MacosVmAttachments::open(&fixture.share, Some(&fixture.digest)).unwrap()
        };
        let source = fixture.root.join("工作区 \"alpha\"\\beta");
        fs::rename(&source, fixture.root.join("retained")).unwrap();
        fs::create_dir(&source).unwrap();
        if let Some(pinned) = pinned {
            assert!(pinned.reverify().is_err());
        } else {
            assert!(MacosVmAttachments::open(&fixture.share, Some(&fixture.digest)).is_err());
        }
        assert!(fixture.root.join("retained").is_dir());
    }
}

#[test]
fn rejects_shared_or_hardlinked_control_manifest() {
    for hardlink in [false, true] {
        let fixture = Fixture::new();
        let path = fixture
            .share
            .path()
            .join(AGENT_VM_ATTACHMENT_MANIFEST_FILE_NAME);
        if hardlink {
            fs::hard_link(&path, fixture.root.join("alias.json")).unwrap();
        } else {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        }
        assert!(MacosVmAttachments::open(&fixture.share, Some(&fixture.digest)).is_err());
    }
}

#[test]
fn pins_restrictions_from_the_same_configuration_as_the_directory_grant() {
    let fixture = Fixture::with_restrictions(true);
    let attachments = MacosVmAttachments::open(&fixture.share, Some(&fixture.digest))
        .unwrap()
        .unwrap();
    assert!(attachments.restrictions_digest().is_some());
    assert_eq!(attachments.path_restrictions().len(), 1);
    assert_eq!(
        attachments.path_restrictions()[0].0,
        fixture.root.join("工作区 \"alpha\"\\beta")
    );
    assert_eq!(attachments.path_restrictions()[0].1.deny_read(), &[".env"]);
    attachments.reverify().unwrap();
}
