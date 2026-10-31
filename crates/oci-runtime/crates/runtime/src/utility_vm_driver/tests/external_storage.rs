#![cfg(test)]

use aster_oci_sdk::{
    AttachmentCapabilities, StorageAccessMode, StorageAttachmentId, StorageCleanup,
    StorageOwnership, RUNTIME_BUNDLE_HANDOFF_EXTERNAL_STORAGE_VERSION,
    RUNTIME_BUNDLE_HANDOFF_MOVE_V2,
};

use super::*;

fn fixture() -> Fixture {
    Fixture::with_profile(
        candidate_capability(),
        AttachmentCapabilities::base_v2()
            .with_extension(
                RUNTIME_BUNDLE_HANDOFF_EXTENSION,
                vec![1, RUNTIME_BUNDLE_HANDOFF_EXTERNAL_STORAGE_VERSION],
            )
            .expect("storage-aware handoff capability"),
    )
}

fn storage_request(fixture: &Fixture, source: &Path) -> DriverCreateRequest {
    let mut request = fixture.handoff_request("external-storage");
    let mut config: serde_json::Value =
        serde_json::from_str(request.bundle.config_json()).expect("fixture configuration");
    config["annotations"][RUNTIME_BUNDLE_HANDOFF_EXTENSION] =
        serde_json::json!(RUNTIME_BUNDLE_HANDOFF_MOVE_V2);
    config["mounts"] = serde_json::json!([{
        "destination": "/workspace", "type": "none", "source": source,
        "options": ["rbind", "rw", "nosuid", "nodev"]
    }]);
    set_configuration(&mut request, config);
    request
}

fn set_configuration(request: &mut DriverCreateRequest, config: serde_json::Value) {
    let directory = request.bundle.directory().to_path_buf();
    let encoded = serde_json::to_string(&config).expect("encode storage bundle");
    std::fs::write(directory.join("config.json"), &encoded).expect("write storage bundle");
    request.bundle = OciBundle::from_json(directory, encoded).expect("storage bundle");
    request.attachment_contract =
        CreateAttachments::from_bundle(&request.bundle, ProcessIo::default())
            .expect("base attachments")
            .attach_storage_mount(
                &request.bundle,
                0,
                StorageAttachmentId::new("caller-workspace").expect("storage identity"),
                StorageAccessMode::ReadWrite,
                StorageOwnership::Caller,
                StorageCleanup::DetachOnly,
            )
            .expect("storage grant")
            .with_runtime_bundle_storage_handoff(&request.bundle)
            .expect("explicit storage-aware handoff");
}

fn workspace(fixture: &Fixture) -> PathBuf {
    let path = fixture
        .runtime_root
        .parent()
        .unwrap()
        .join("工作区 \"alpha\"\\beta");
    std::fs::create_dir(&path).expect("create caller workspace");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .expect("caller workspace permissions");
    std::fs::write(path.join("keep.txt"), b"caller-owned").expect("caller data");
    path
}

#[tokio::test]
async fn move_replay_and_delete_preserve_external_storage_and_its_permissions() {
    let fixture = fixture();
    let workspace = workspace(&fixture);
    let request = storage_request(&fixture, &workspace);
    let original = request.bundle.directory().to_path_buf();
    let replay = request.clone();
    let staged = fixture.stage(request).await;
    assert!(!original.exists());
    let reopened = fixture
        .driver
        .prepare_create_bundle(&replay)
        .await
        .expect("replay handoff");
    assert_eq!(reopened.directory(), staged.bundle.directory());
    fixture
        .driver
        .create(staged)
        .await
        .expect("create with storage grant");
    fixture
        .driver
        .delete(DriverDeleteRequest {
            context: context("delete-external-storage"),
            target: target(),
            mode: DeleteMode::Force,
        })
        .await
        .expect("delete exact generation");
    assert!(!fixture.generation_share().exists());
    assert_eq!(
        std::fs::read(workspace.join("keep.txt")).unwrap(),
        b"caller-owned"
    );
    assert_eq!(
        std::fs::metadata(&workspace).unwrap().permissions().mode() & 0o777,
        0o755
    );
}

#[tokio::test]
async fn storage_alias_and_runtime_control_directory_are_rejected_before_move() {
    for alias in [false, true] {
        let fixture = fixture();
        let source = if alias {
            let workspace = workspace(&fixture);
            let alias = workspace.parent().unwrap().join("linked-workspace");
            std::os::unix::fs::symlink(&workspace, &alias).unwrap();
            alias
        } else {
            fixture.recovery_directory.clone()
        };
        let request = storage_request(&fixture, &source);
        let handoff = request.bundle.directory().to_path_buf();
        let error = fixture
            .driver
            .prepare_create_bundle(&request)
            .await
            .expect_err("aliases and runtime control state must not be exported");
        assert_eq!(error.code, ErrorCode::FailedPrecondition);
        assert!(handoff.is_dir());
        assert!(!fixture.generation_share().exists());
        assert_eq!(fixture.factory.launches.load(Ordering::Relaxed), 0);
    }
}

#[tokio::test]
async fn unclassified_absolute_bind_source_is_rejected_with_a_valid_storage_grant() {
    let fixture = fixture();
    let workspace = workspace(&fixture);
    let mut request = storage_request(&fixture, &workspace);
    let mut config: serde_json::Value = serde_json::from_str(request.bundle.config_json()).unwrap();
    config["mounts"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "destination": "/ungranted", "type": "none", "source": workspace.parent().unwrap(),
            "options": ["bind", "ro"]
        }));
    set_configuration(&mut request, config);
    let error = fixture
        .driver
        .prepare_create_bundle(&request)
        .await
        .expect_err("one storage grant must not authorize other host paths");
    assert_eq!(error.code, ErrorCode::InvalidArgument);
    assert!(request.bundle.directory().is_dir());
    assert!(!fixture.generation_share().exists());
}

#[tokio::test]
async fn cleanup_preserves_moved_caller_data_without_reopening_its_source() {
    let fixture = fixture();
    let workspace = workspace(&fixture);
    let staged = fixture.stage(storage_request(&fixture, &workspace)).await;
    fixture
        .driver
        .create(staged)
        .await
        .expect("create storage fixture");
    let moved = workspace.with_file_name("moved-workspace");
    std::fs::rename(&workspace, &moved).expect("caller renames their directory");
    fixture
        .driver
        .delete(DriverDeleteRequest {
            context: context("delete-moved-storage"),
            target: target(),
            mode: DeleteMode::Force,
        })
        .await
        .expect("cleanup does not depend on caller source still existing");
    assert!(!fixture.generation_share().exists());
    assert_eq!(
        std::fs::read(moved.join("keep.txt")).unwrap(),
        b"caller-owned"
    );
}

#[tokio::test]
async fn old_handoff_capability_rejects_storage_transfer_without_touching_caller_paths() {
    let fixture = Fixture::with_profile(candidate_capability(), AttachmentCapabilities::base_v2());
    let workspace = workspace(&fixture);
    let request = storage_request(&fixture, &workspace);
    let error = fixture
        .driver
        .prepare_create_bundle(&request)
        .await
        .expect_err("v1 runtime cannot silently accept external storage handoff");
    assert_eq!(error.code, ErrorCode::Unsupported);
    assert!(request.bundle.directory().is_dir());
    assert!(!fixture.generation_share().exists());
    assert_eq!(
        std::fs::read(workspace.join("keep.txt")).unwrap(),
        b"caller-owned"
    );
    assert_eq!(fixture.factory.launches.load(Ordering::Relaxed), 0);
}
