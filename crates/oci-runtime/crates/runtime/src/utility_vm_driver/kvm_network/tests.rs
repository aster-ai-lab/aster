#![cfg(test)]

use aster_oci_agent_protocol::{
    AgentVmAttachmentManifest, GuestPath, AGENT_VM_ATTACHMENT_MANIFEST_FILE_NAME,
};
use aster_oci_sdk::{
    oci_spec::runtime::Spec, ContainerId, ContainerTarget, CreateAttachments, ErrorCode,
    Generation, OciBundle, ProcessIo, StorageAccessMode, StorageAttachmentId, StorageCleanup,
    StorageOwnership,
};
use serde_json::json;

use super::{prepare, UtilityVmLaunchRequest};

#[tokio::test]
async fn prepares_caller_owned_raw_storage_without_absorbing_its_image() {
    let temporary = tempfile::tempdir().expect("temporary KVM storage fixture");
    let runtime_share = temporary.path().join("share");
    let bundle_directory = runtime_share.join("bundle");
    std::fs::create_dir_all(bundle_directory.join("rootfs")).expect("bundle rootfs");
    std::fs::create_dir(runtime_share.join("run")).expect("runtime state");
    let image = temporary.path().join("caller-owned.raw");
    let image_file = std::fs::File::create(&image).expect("raw image");
    image_file.set_len(4096).expect("aligned raw image");
    drop(image_file);

    let mut value = serde_json::to_value(Spec::default()).expect("default OCI spec");
    value["root"] = json!({"path": "rootfs", "readonly": false});
    value["mounts"] = json!([{
        "destination": "/data",
        "type": "ext4",
        "source": image.to_str().expect("UTF-8 image path"),
        "options": ["ro", "nodev"]
    }]);
    let bundle = OciBundle::from_json(
        bundle_directory,
        serde_json::to_string(&value).expect("storage config"),
    )
    .expect("storage bundle");
    let attachments = CreateAttachments::from_bundle(&bundle, ProcessIo::default())
        .expect("base attachments")
        .attach_storage_mount(
            &bundle,
            0,
            StorageAttachmentId::new("raw-volume-7").expect("storage identity"),
            StorageAccessMode::ReadOnly,
            StorageOwnership::Caller,
            StorageCleanup::DetachOnly,
        )
        .expect("storage attachments");
    let target = ContainerTarget::exact(
        ContainerId::new("stored").expect("container ID"),
        Generation(1),
    );
    let guest_bundle = GuestPath::new("/run/aster-oci-runtime/bundle").unwrap();
    let digest = prepare(&UtilityVmLaunchRequest {
        target: &target,
        runtime_share: &runtime_share,
        bundle: &bundle,
        guest_bundle: &guest_bundle,
        attachment_contract: &attachments,
    })
    .await
    .expect("prepare storage manifest")
    .expect("manifest digest");

    let manifest = AgentVmAttachmentManifest::from_bytes(
        &tokio::fs::read(runtime_share.join(AGENT_VM_ATTACHMENT_MANIFEST_FILE_NAME))
            .await
            .expect("read storage manifest"),
    )
    .expect("persisted storage manifest");
    assert_eq!(manifest.digest().unwrap(), digest);
    assert_eq!(manifest.storage().len(), 1);
    assert_eq!(manifest.storage()[0].host_source(), image.to_str().unwrap());
    assert_eq!(manifest.storage()[0].source_identity().size(), 4096);
    assert!(image.exists(), "caller-owned raw image must be preserved");

    std::fs::hard_link(&image, temporary.path().join("image-alias.raw")).expect("hard-link alias");
    let error = prepare(&UtilityVmLaunchRequest {
        target: &target,
        runtime_share: &runtime_share,
        bundle: &bundle,
        guest_bundle: &guest_bundle,
        attachment_contract: &attachments,
    })
    .await
    .expect_err("hard-linked raw image must fail closed");
    assert_eq!(error.code, ErrorCode::Unsupported);
}

#[tokio::test]
async fn rejects_storage_images_inside_the_runtime_owned_share() {
    let temporary = tempfile::tempdir().expect("temporary KVM storage fixture");
    let runtime_share = temporary.path().join("share");
    let bundle_directory = runtime_share.join("bundle");
    std::fs::create_dir_all(bundle_directory.join("rootfs")).expect("bundle rootfs");
    std::fs::create_dir(runtime_share.join("run")).expect("runtime state");
    let image = runtime_share.join("owned.raw");
    let image_file = std::fs::File::create(&image).expect("raw image");
    image_file.set_len(4096).expect("aligned image");
    drop(image_file);
    let mut value = serde_json::to_value(Spec::default()).expect("default OCI spec");
    value["root"] = json!({"path": "rootfs", "readonly": false});
    value["mounts"] = json!([{
        "destination": "/data",
        "type": "ext4",
        "source": image.to_str().unwrap(),
        "options": ["rw"]
    }]);
    let bundle = OciBundle::from_json(bundle_directory, value.to_string()).unwrap();
    let attachments = CreateAttachments::from_bundle(&bundle, ProcessIo::default())
        .unwrap()
        .attach_storage_mount(
            &bundle,
            0,
            StorageAttachmentId::new("owned-volume").unwrap(),
            StorageAccessMode::ReadWrite,
            StorageOwnership::Caller,
            StorageCleanup::DetachOnly,
        )
        .unwrap();
    let target = ContainerTarget::exact(ContainerId::new("stored").unwrap(), Generation(1));
    let guest_bundle = GuestPath::new("/run/aster-oci-runtime/bundle").unwrap();

    let error = prepare(&UtilityVmLaunchRequest {
        target: &target,
        runtime_share: &runtime_share,
        bundle: &bundle,
        guest_bundle: &guest_bundle,
        attachment_contract: &attachments,
    })
    .await
    .expect_err("runtime-owned image must fail closed");
    assert_eq!(error.code, ErrorCode::FailedPrecondition);
}
