#![cfg(test)]

use aster_oci_sdk::{
    oci_spec::runtime::Spec, ContainerId, CreateAttachments, Generation, ProcessIo,
    StorageAccessMode, StorageAttachmentId, StorageCleanup, StorageOwnership,
};
use serde_json::json;

use super::*;

fn fixture() -> (OciBundle, AgentVmAttachmentManifest) {
    let mut configuration = serde_json::to_value(Spec::default()).unwrap();
    configuration["mounts"] = json!([
        {"destination":"/data", "type":"none", "source":"/srv/工作区 \"alpha\"\\beta", "options":["rbind","ro","nodev"]},
        {"destination":"/cache", "type":"none", "source":"/srv/cache", "options":["bind","rw"]}
    ]);
    let bundle = OciBundle::from_json(
        std::env::temp_dir().join("aster-directory-manifest-fixture"),
        serde_json::to_string(&configuration).unwrap(),
    )
    .unwrap();
    let mut attachments = CreateAttachments::from_bundle(&bundle, ProcessIo::default()).unwrap();
    for (index, access) in [StorageAccessMode::ReadOnly, StorageAccessMode::ReadWrite]
        .into_iter()
        .enumerate()
    {
        attachments = attachments
            .attach_storage_mount(
                &bundle,
                index,
                StorageAttachmentId::new(format!("directory-{index}")).unwrap(),
                access,
                StorageOwnership::Caller,
                StorageCleanup::DetachOnly,
            )
            .unwrap();
    }
    let digest = attachments.digest().unwrap();
    let mut directories = Vec::new();
    for attachment in attachments.storage() {
        let index = storage::parse_mount_index(attachment.mount().json_pointer()).unwrap();
        directories.push(
            AgentVmDirectoryAttachment::new(
                attachment.clone(),
                configuration["mounts"][index]["source"].as_str().unwrap(),
                AgentVmDirectorySourceIdentity::new(17, 4203 + index as u64).unwrap(),
                &digest,
            )
            .unwrap(),
        );
    }
    directories.sort();
    let manifest = AgentVmAttachmentManifest::new_with_directories(
        ContainerTarget::exact(
            ContainerId::new("directory-fixture").unwrap(),
            Generation(3),
        ),
        GuestPath::new("/run/aster-oci-runtime/bundle").unwrap(),
        bundle.config_digest(),
        digest,
        Vec::new(),
        Vec::new(),
        directories,
    )
    .unwrap();
    (bundle, manifest)
}

#[test]
fn directory_transport_binds_public_access_source_and_guest_mapping() {
    let (bundle, manifest) = fixture();
    let decoded = AgentVmAttachmentManifest::from_bytes(&manifest.to_bytes().unwrap()).unwrap();
    decoded.validate_bundle(&bundle).unwrap();
    assert_eq!(
        decoded.schema_version,
        AGENT_VM_DIRECTORY_MANIFEST_SCHEMA_VERSION
    );
    let directory = &decoded.directories()[0];
    assert_eq!(
        directory.attachment().access_mode(),
        StorageAccessMode::ReadOnly
    );
    assert_eq!(directory.host_source(), "/srv/工作区 \"alpha\"\\beta");
    assert!(directory
        .guest_mount_path()
        .starts_with(&format!("{AGENT_RUNTIME_SHARE_GUEST_ROOT}/")));
    assert_ne!(directory.tag(), decoded.directories()[1].tag());
}

#[test]
fn restricted_directories_cannot_downgrade_or_drop_the_host_enforcement_binding() {
    let (bundle, mut manifest) = fixture();
    let restrictions =
        aster_oci_sdk::StoragePathRestrictions::new(vec![aster_oci_sdk::StoragePathRestriction::new(
            StorageAttachmentId::new("directory-1").unwrap(),
            vec![".env".into()],
            vec![".aster".into()],
            vec![".aster/loops".into()],
        )
        .unwrap()])
        .unwrap();
    let mut config: Value = serde_json::from_str(bundle.config_json()).unwrap();
    config["annotations"] = json!({aster_oci_sdk::STORAGE_PATH_RESTRICTIONS_EXTENSION: restrictions.to_annotation_value().unwrap()});
    let restricted_bundle = OciBundle::from_json(bundle.directory(), config.to_string()).unwrap();
    manifest.config_digest = restricted_bundle.config_digest().into();
    assert!(manifest.validate_bundle(&restricted_bundle).is_err());
    let manifest = manifest
        .with_storage_path_restrictions(&restricted_bundle, &restrictions)
        .unwrap();
    let bytes = manifest.to_bytes().unwrap();
    let decoded = AgentVmAttachmentManifest::from_bytes(&bytes).unwrap();
    decoded.validate_bundle(&restricted_bundle).unwrap();
    assert_eq!(
        decoded.storage_path_restrictions_digest(),
        Some(restrictions.digest().unwrap().as_str())
    );
    assert!(decoded.validate_bundle(&bundle).is_err());
    for version in [
        AGENT_VM_DIRECTORY_MANIFEST_SCHEMA_VERSION,
        AGENT_VM_ATTACHMENT_MANIFEST_SCHEMA_VERSION,
    ] {
        let mut value = serde_json::to_value(&manifest).unwrap();
        value["schemaVersion"] = json!(version);
        assert!(
            AgentVmAttachmentManifest::from_bytes(&serde_json::to_vec(&value).unwrap()).is_err()
        );
    }
    let mut value = serde_json::to_value(&manifest).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .remove("storagePathRestrictionsDigest");
    assert!(AgentVmAttachmentManifest::from_bytes(&serde_json::to_vec(&value).unwrap()).is_err());
}

#[test]
fn rejects_rebinding_directory_source_access_tag_identity_or_schema() {
    let (bundle, manifest) = fixture();
    for field in ["hostSource", "accessMode", "tag", "inode", "schemaVersion"] {
        let mut value = serde_json::to_value(&manifest).unwrap();
        match field {
            "hostSource" => value["directories"][0]["hostSource"] = json!("/srv/ungranted"),
            "accessMode" => {
                value["directories"][0]["attachment"]["accessMode"] = json!("read-write")
            }
            "tag" => value["directories"][0]["tag"] = json!(manifest.directories()[1].tag()),
            "inode" => value["directories"][0]["sourceIdentity"]["inode"] = json!(0),
            "schemaVersion" => {
                value["schemaVersion"] = json!(AGENT_VM_ATTACHMENT_MANIFEST_SCHEMA_VERSION)
            }
            _ => unreachable!(),
        }
        let result = AgentVmAttachmentManifest::from_bytes(&serde_json::to_vec(&value).unwrap())
            .and_then(|decoded| decoded.validate_bundle(&bundle));
        assert!(result.is_err(), "accepted directory {field} rebinding");
    }
}

#[test]
fn rejects_duplicate_directory_inodes_and_mounts() {
    let (_, manifest) = fixture();
    for field in ["sourceIdentity", "mount"] {
        let mut value = serde_json::to_value(&manifest).unwrap();
        if field == "mount" {
            value["directories"][1]["attachment"]["mount"] =
                value["directories"][0]["attachment"]["mount"].clone();
        } else {
            value["directories"][1][field] = value["directories"][0][field].clone();
        }
        let error = AgentVmAttachmentManifest::from_bytes(&serde_json::to_vec(&value).unwrap());
        assert!(error.is_err(), "accepted duplicate directory {field}");
    }
}

#[test]
fn rejects_directory_grants_without_an_oci_bind_mount() {
    let (bundle, manifest) = fixture();
    let mut value: Value = serde_json::from_str(bundle.config_json()).unwrap();
    value["mounts"][0]["type"] = json!("bind");
    value["mounts"][0]["options"] = json!(["ro"]);
    let changed =
        OciBundle::from_json(bundle.directory(), serde_json::to_string(&value).unwrap()).unwrap();
    let public = CreateAttachments::from_bundle(&changed, ProcessIo::default())
        .unwrap()
        .attach_storage_mount(
            &changed,
            0,
            StorageAttachmentId::new("non-bind").unwrap(),
            StorageAccessMode::ReadOnly,
            StorageOwnership::Caller,
            StorageCleanup::DetachOnly,
        )
        .unwrap();
    let directory = AgentVmDirectoryAttachment::new(
        public.storage()[0].clone(),
        manifest.directories()[0].host_source(),
        AgentVmDirectorySourceIdentity::new(17, 4203).unwrap(),
        &public.digest().unwrap(),
    )
    .unwrap();
    assert!(directory.validate_configuration(&value).is_err());
}
