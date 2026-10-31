#![cfg(test)]

use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};

use aster_oci_agent_protocol::{AgentVmAttachmentManifest, GuestPath};
use aster_oci_sdk::{
    oci_spec::runtime::Spec, ContainerId, ContainerTarget, CreateAttachments, Generation,
    OciBundle, ProcessIo, StorageAccessMode, StorageAttachmentId, StorageCleanup, StorageOwnership,
};
use serde_json::json;

use super::*;

struct Fixture {
    _temporary: tempfile::TempDir,
    root: PathBuf,
    share: PathBuf,
    bundle: OciBundle,
    attachments: CreateAttachments,
    target: ContainerTarget,
    guest_bundle: GuestPath,
}

impl Fixture {
    fn new() -> Self {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().canonicalize().unwrap();
        let share = root.join("share");
        std::fs::create_dir_all(share.join("bundle/rootfs")).unwrap();
        let mut config = serde_json::to_value(Spec::default()).unwrap();
        let mut mounts = Vec::new();
        for (index, access) in ["ro", "rw"].into_iter().enumerate() {
            let source = root.join(format!("工作区-{index}"));
            std::fs::create_dir(&source).unwrap();
            std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o755)).unwrap();
            mounts.push(
                json!({"destination":format!("/workspace-{index}"), "type":"none",
                "source":source, "options":["rbind",access]}),
            );
        }
        config["root"] = json!({"path":"rootfs", "readonly":true});
        config["mounts"] = json!(mounts);
        let bundle = OciBundle::from_json(share.join("bundle"), config.to_string()).unwrap();
        let attachments = Self::grants(&bundle);
        Self {
            _temporary: temporary,
            root,
            share,
            bundle,
            attachments,
            target: ContainerTarget::exact(ContainerId::new("directories").unwrap(), Generation(1)),
            guest_bundle: GuestPath::new("/run/aster-oci-runtime/bundle").unwrap(),
        }
    }

    fn grants(bundle: &OciBundle) -> CreateAttachments {
        let mut attachments = CreateAttachments::from_bundle(bundle, ProcessIo::default()).unwrap();
        for (index, access) in [StorageAccessMode::ReadOnly, StorageAccessMode::ReadWrite]
            .into_iter()
            .enumerate()
        {
            attachments = attachments
                .attach_storage_mount(
                    bundle,
                    index,
                    StorageAttachmentId::new(format!("caller-{index}")).unwrap(),
                    access,
                    StorageOwnership::Caller,
                    StorageCleanup::DetachOnly,
                )
                .unwrap();
        }
        attachments
    }

    fn request(&self) -> UtilityVmLaunchRequest<'_> {
        UtilityVmLaunchRequest {
            target: &self.target,
            runtime_share: &self.share,
            bundle: &self.bundle,
            guest_bundle: &self.guest_bundle,
            attachment_contract: &self.attachments,
        }
    }
}

#[tokio::test]
async fn transport_retains_exact_identity_access_and_caller_permissions() {
    let fixture = Fixture::new();
    let digest = prepare(&fixture.request()).await.unwrap().unwrap();
    let bytes = std::fs::read(fixture.share.join(AGENT_VM_ATTACHMENT_MANIFEST_FILE_NAME)).unwrap();
    let manifest = AgentVmAttachmentManifest::from_bytes(&bytes).unwrap();
    assert_eq!(manifest.digest().unwrap(), digest);
    assert_eq!(manifest.directories().len(), 2);
    for directory in manifest.directories() {
        let metadata = std::fs::metadata(directory.host_source()).unwrap();
        assert_eq!(directory.source_identity().device(), metadata.dev());
        assert_eq!(directory.source_identity().inode(), metadata.ino());
        assert_eq!(metadata.permissions().mode() & 0o777, 0o755);
        assert!(fixture
            .attachments
            .storage()
            .contains(directory.attachment()));
    }
}

#[tokio::test]
async fn restricted_directory_requires_a_declared_extension_and_enforcement_digest() {
    let mut fixture = Fixture::new();
    let restrictions =
        aster_oci_sdk::StoragePathRestrictions::new(vec![aster_oci_sdk::StoragePathRestriction::new(
            StorageAttachmentId::new("caller-1").unwrap(),
            vec![".env".into()],
            vec![".aster".into()],
            vec![".aster/loops".into()],
        )
        .unwrap()])
        .unwrap();
    let mut config: serde_json::Value = serde_json::from_str(fixture.bundle.config_json()).unwrap();
    config["annotations"] = json!({aster_oci_sdk::STORAGE_PATH_RESTRICTIONS_EXTENSION: restrictions.to_annotation_value().unwrap()});
    fixture.bundle = OciBundle::from_json(fixture.bundle.directory(), config.to_string()).unwrap();
    fixture.attachments = Fixture::grants(&fixture.bundle);
    assert_eq!(
        prepare(&fixture.request()).await.unwrap_err().code,
        ErrorCode::InvalidArgument
    );
    assert!(!fixture
        .share
        .join(AGENT_VM_ATTACHMENT_MANIFEST_FILE_NAME)
        .exists());
    fixture.attachments = fixture
        .attachments
        .with_storage_path_restrictions(&fixture.bundle)
        .unwrap();
    let digest = prepare(&fixture.request()).await.unwrap().unwrap();
    let manifest = AgentVmAttachmentManifest::from_bytes(
        &std::fs::read(fixture.share.join(AGENT_VM_ATTACHMENT_MANIFEST_FILE_NAME)).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest.digest().unwrap(), digest);
    assert_eq!(
        manifest.storage_path_restrictions_digest(),
        Some(restrictions.digest().unwrap().as_str())
    );
    manifest.validate_bundle(&fixture.bundle).unwrap();
}

#[tokio::test]
async fn replay_cannot_silently_export_a_replaced_directory() {
    let fixture = Fixture::new();
    prepare(&fixture.request()).await.unwrap();
    let manifest_path = fixture.share.join(AGENT_VM_ATTACHMENT_MANIFEST_FILE_NAME);
    let retained = std::fs::read(&manifest_path).unwrap();
    let source = fixture.root.join("工作区-0");
    std::fs::rename(&source, fixture.root.join("original")).unwrap();
    std::fs::create_dir(&source).unwrap();
    let error = prepare(&fixture.request()).await.unwrap_err();
    assert_eq!(error.code, ErrorCode::Conflict);
    assert_eq!(std::fs::read(manifest_path).unwrap(), retained);
    assert!(fixture.root.join("original").is_dir());
}

#[tokio::test]
async fn aliases_and_overlapping_grants_fail_before_manifest_publication() {
    for alias in [true, false] {
        let mut fixture = Fixture::new();
        let source = fixture.root.join("工作区-0");
        if alias {
            std::fs::rename(&source, fixture.root.join("original")).unwrap();
            symlink(fixture.root.join("original"), &source).unwrap();
        } else {
            let mut config: serde_json::Value =
                serde_json::from_str(fixture.bundle.config_json()).unwrap();
            let nested = source.join("child");
            std::fs::create_dir(&nested).unwrap();
            config["mounts"][1]["source"] = json!(nested);
            fixture.bundle =
                OciBundle::from_json(fixture.bundle.directory(), config.to_string()).unwrap();
            fixture.attachments = Fixture::grants(&fixture.bundle);
        }
        let error = prepare(&fixture.request()).await.unwrap_err();
        assert_eq!(error.code, ErrorCode::FailedPrecondition);
        assert!(!fixture
            .share
            .join(AGENT_VM_ATTACHMENT_MANIFEST_FILE_NAME)
            .exists());
    }
}

#[tokio::test]
async fn a_create_without_directory_grants_refuses_stale_manifest_evidence() {
    let mut fixture = Fixture::new();
    prepare(&fixture.request()).await.unwrap();
    fixture.attachments =
        CreateAttachments::from_bundle(&fixture.bundle, ProcessIo::default()).unwrap();
    assert_eq!(
        prepare(&fixture.request()).await.unwrap_err().code,
        ErrorCode::Conflict
    );
}
