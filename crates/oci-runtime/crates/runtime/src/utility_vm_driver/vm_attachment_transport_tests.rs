#![cfg(test)]

use std::os::unix::fs::PermissionsExt;

use aster_oci_agent_protocol::{AgentVmMacAddress, AgentVmNetworkAttachment, GuestPath};
use aster_oci_sdk::{
    oci_spec::runtime::Spec, ContainerId, ContainerTarget, CreateAttachments, Generation,
    NetworkAttachmentIdentity, NetworkCleanup, NetworkCleanupId, NetworkInterfaceId,
    NetworkNamespaceId, OciBundle, ProcessIo,
};
use serde_json::json;

use super::{
    ensure_absent, manifest_conflict, persist_manifest, publish_manifest, read_manifest,
    remove_private_file_bound, AgentVmAttachmentManifest, ErrorCode,
    AGENT_VM_ATTACHMENT_MANIFEST_FILE_NAME, PENDING_MANIFEST_FILE_NAME,
};

fn manifest(generation: u64) -> AgentVmAttachmentManifest {
    let mut value = serde_json::to_value(Spec::default()).expect("default OCI spec");
    value["linux"] = json!({
        "namespaces": [{"type": "network"}, {"type": "uts"}],
        "netDevices": {"tap0": {"name": "eth0"}}
    });
    let bundle = OciBundle::from_json(
        std::env::temp_dir().join("aster-kvm-network-manifest"),
        serde_json::to_string(&value).expect("fixture JSON"),
    )
    .expect("valid bundle");
    let attachments = CreateAttachments::from_bundle(&bundle, ProcessIo::default())
        .expect("base attachments")
        .attach_linux_network_interface(
            &bundle,
            0,
            "tap0",
            NetworkAttachmentIdentity::new(
                NetworkNamespaceId::new("namespace-1").expect("namespace ID"),
                NetworkInterfaceId::new("interface-1").expect("interface ID"),
                NetworkCleanupId::new("cleanup-1").expect("cleanup ID"),
            ),
            NetworkCleanup::ReleaseRuntimeNamespace,
        )
        .expect("network attachment");
    let attachment_digest = attachments.digest().expect("attachment digest");
    let attachment = &attachments.network_attachments()[0];
    let network = AgentVmNetworkAttachment::new(
        attachment.identity().clone(),
        "tap0",
        attachment.namespace().clone(),
        attachment.interface().clone(),
        attachment.cleanup(),
        AgentVmMacAddress::derive(&attachment_digest, attachment.identity(), "tap0")
            .expect("transport MAC"),
    )
    .expect("VM network attachment");
    AgentVmAttachmentManifest::new(
        ContainerTarget::exact(
            ContainerId::new("networked").expect("container ID"),
            Generation(generation),
        ),
        GuestPath::new("/run/aster-oci-runtime/bundle").expect("guest bundle"),
        bundle.config_digest(),
        attachment_digest,
        vec![network],
        Vec::new(),
    )
    .expect("VM attachment manifest")
}

#[tokio::test]
async fn persists_exact_replay_and_rejects_drift_or_unrequested_evidence() {
    let temporary = tempfile::tempdir().expect("temporary runtime share");
    let first = manifest(1);
    persist_manifest(temporary.path(), &first)
        .await
        .expect("persist manifest");
    persist_manifest(temporary.path(), &first)
        .await
        .expect("replay exact manifest");

    let final_path = temporary
        .path()
        .join(AGENT_VM_ATTACHMENT_MANIFEST_FILE_NAME);
    assert_eq!(
        read_manifest(&final_path).await.expect("read manifest"),
        first
    );
    let pending_path = temporary.path().join(PENDING_MANIFEST_FILE_NAME);
    let stale = ensure_absent(&final_path, &pending_path)
        .await
        .expect_err("unrequested evidence must fail closed");
    assert_eq!(stale.code, ErrorCode::Conflict);

    let drift = persist_manifest(temporary.path(), &manifest(2))
        .await
        .expect_err("different generation must conflict");
    assert_eq!(drift.code, ErrorCode::Conflict);
}

#[tokio::test]
async fn recovers_only_a_private_pending_manifest() {
    let temporary = tempfile::tempdir().expect("temporary runtime share");
    let pending_path = temporary.path().join(PENDING_MANIFEST_FILE_NAME);
    std::fs::write(&pending_path, b"interrupted").expect("write pending manifest");
    std::fs::set_permissions(&pending_path, std::fs::Permissions::from_mode(0o600))
        .expect("protect pending manifest");

    persist_manifest(temporary.path(), &manifest(1))
        .await
        .expect("recover private pending manifest");
    assert!(!pending_path.exists());

    let other = tempfile::tempdir().expect("other runtime share");
    let public_pending = other.path().join(PENDING_MANIFEST_FILE_NAME);
    std::fs::write(&public_pending, b"untrusted").expect("write public pending manifest");
    std::fs::set_permissions(&public_pending, std::fs::Permissions::from_mode(0o644))
        .expect("make pending manifest public");
    let error = persist_manifest(other.path(), &manifest(1))
        .await
        .expect_err("non-private pending evidence must not be removed");
    assert_eq!(error.code, ErrorCode::FailedPrecondition);
    assert!(public_pending.exists());
}

#[tokio::test]
async fn partial_pending_manifest_is_discarded_as_legacy_interruption() {
    let temporary = tempfile::tempdir().expect("temporary runtime share");
    let pending_path = temporary.path().join(PENDING_MANIFEST_FILE_NAME);
    std::fs::write(&pending_path, br#"{"target":"partial""#)
        .expect("write partial pending manifest");
    std::fs::set_permissions(&pending_path, std::fs::Permissions::from_mode(0o600))
        .expect("protect partial pending manifest");

    persist_manifest(temporary.path(), &manifest(1))
        .await
        .expect("discard legacy partial pending manifest");
    assert!(!pending_path.exists());
    assert!(temporary
        .path()
        .join(AGENT_VM_ATTACHMENT_MANIFEST_FILE_NAME)
        .exists());
}

#[tokio::test]
async fn manifest_reader_rejects_a_final_component_symlink() {
    let temporary = tempfile::tempdir().expect("temporary runtime share");
    let target = temporary.path().join("target.json");
    let alias = temporary
        .path()
        .join(AGENT_VM_ATTACHMENT_MANIFEST_FILE_NAME);
    let encoded = manifest(1).to_bytes().expect("encode manifest");
    std::fs::write(&target, &encoded).expect("write target manifest");
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600))
        .expect("protect target manifest");
    std::os::unix::fs::symlink(&target, &alias).expect("create manifest symlink");

    let error = read_manifest(&alias)
        .await
        .expect_err("manifest symlink must be rejected");
    assert_eq!(error.code, ErrorCode::FailedPrecondition);
    assert_eq!(
        std::fs::read(&target).expect("read target manifest"),
        encoded
    );
}

#[tokio::test]
async fn manifest_cleanup_rejects_a_replaced_private_path() {
    let temporary = tempfile::tempdir().expect("temporary runtime share");
    let path = temporary.path().join(PENDING_MANIFEST_FILE_NAME);
    let retained = temporary.path().join("retained-old");
    std::fs::write(&path, manifest(1).to_bytes().expect("encode original"))
        .expect("write original manifest");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
        .expect("protect original manifest");
    let initial_metadata = std::fs::symlink_metadata(&path).expect("inspect original");
    std::fs::hard_link(&path, &retained).expect("retain original inode");
    std::fs::remove_file(&path).expect("remove original path");
    std::fs::write(&path, manifest(2).to_bytes().expect("encode replacement"))
        .expect("write replacement manifest");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
        .expect("protect replacement manifest");

    let error = remove_private_file_bound(&path, &initial_metadata)
        .await
        .expect_err("replacement must not be removed");
    assert_eq!(error.code, ErrorCode::Unavailable);
    assert!(path.exists(), "replacement manifest must remain");
    assert!(retained.exists(), "original inode must remain retained");
}

#[tokio::test]
async fn no_replace_manifest_publication_preserves_an_incumbent() {
    let temporary = tempfile::tempdir().expect("temporary runtime share");
    let final_path = temporary
        .path()
        .join(AGENT_VM_ATTACHMENT_MANIFEST_FILE_NAME);
    let pending_path = temporary.path().join(PENDING_MANIFEST_FILE_NAME);
    let incumbent = manifest(2);
    let expected = manifest(1);
    let incumbent_bytes = incumbent.to_bytes().expect("encode incumbent");
    let expected_bytes = expected.to_bytes().expect("encode expected");
    std::fs::write(&final_path, &incumbent_bytes).expect("write incumbent");
    std::fs::write(&pending_path, &expected_bytes).expect("write pending");
    for path in [&final_path, &pending_path] {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .expect("protect manifest");
    }

    let error = publish_manifest(temporary.path(), &pending_path, &final_path, &expected)
        .await
        .expect_err("occupied manifest must not be replaced");
    assert_eq!(error, manifest_conflict());
    assert_eq!(
        std::fs::read(&final_path).expect("read incumbent"),
        incumbent_bytes
    );
    assert_eq!(
        std::fs::read(&pending_path).expect("read pending"),
        expected_bytes
    );
}

#[tokio::test]
async fn concurrent_manifest_persistence_publishes_one_complete_file() {
    let temporary = tempfile::tempdir().expect("temporary runtime share");
    let expected = manifest(1);
    let mut calls = Vec::new();
    for _ in 0..16 {
        let root = temporary.path().to_path_buf();
        let expected = expected.clone();
        calls.push(tokio::spawn(async move {
            persist_manifest(&root, &expected).await
        }));
    }
    for call in calls {
        call.await
            .expect("manifest task must not panic")
            .expect("concurrent manifest persistence must succeed");
    }

    assert_eq!(
        read_manifest(
            &temporary
                .path()
                .join(AGENT_VM_ATTACHMENT_MANIFEST_FILE_NAME)
        )
        .await
        .expect("read concurrent manifest"),
        expected
    );
    let mut entries = tokio::fs::read_dir(temporary.path())
        .await
        .expect("enumerate manifest root");
    let mut names = Vec::new();
    while let Some(entry) = entries.next_entry().await.expect("read manifest entry") {
        names.push(entry.file_name());
    }
    assert_eq!(
        names,
        vec![std::ffi::OsString::from(
            AGENT_VM_ATTACHMENT_MANIFEST_FILE_NAME
        )]
    );
}
