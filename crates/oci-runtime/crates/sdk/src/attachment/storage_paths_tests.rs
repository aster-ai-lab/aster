#![cfg(test)]

use super::*;
use crate::{oci_spec::runtime::Spec, ProcessIo, StorageCleanup, StorageOwnership};
use serde_json::json;

fn rule() -> StoragePathRestriction {
    StoragePathRestriction::new(
        StorageAttachmentId::new("workspace").unwrap(),
        vec![".env".into(), "嵌套/凭据 \"alpha\"\\beta".into()],
        vec![".aster".into(), ".env".into()],
        vec![".aster/loops".into()],
    )
    .unwrap()
}

fn fixture(access: StorageAccessMode) -> (OciBundle, CreateAttachments) {
    let policy = StoragePathRestrictions::new(vec![rule()]).unwrap();
    let mut config = serde_json::to_value(Spec::default()).unwrap();
    config["mounts"] = json!([{"source":"/srv/workspace","destination":"/workspace","type":"none","options":["rbind",if access==StorageAccessMode::ReadOnly {"ro"} else {"rw"}]}]);
    config["annotations"] =
        json!({STORAGE_PATH_RESTRICTIONS_EXTENSION: policy.to_annotation_value().unwrap()});
    let bundle = OciBundle::from_json("/srv/bundle", config.to_string()).unwrap();
    let attachments = CreateAttachments::from_bundle(&bundle, ProcessIo::default())
        .unwrap()
        .attach_storage_mount(
            &bundle,
            0,
            StorageAttachmentId::new("workspace").unwrap(),
            access,
            StorageOwnership::Caller,
            StorageCleanup::DetachOnly,
        )
        .unwrap();
    (bundle, attachments)
}

#[test]
fn rejects_escape_paths_and_writable_exceptions_without_a_denied_ancestor() {
    for path in [
        "",
        "/etc",
        "../secret",
        "a/../secret",
        "a//secret",
        "a/./secret",
        "a/",
        "a\0b",
    ] {
        assert!(
            StoragePathRestriction::new(
                StorageAttachmentId::new("workspace").unwrap(),
                vec![path.into()],
                vec![],
                vec![]
            )
            .is_err(),
            "{path:?}"
        );
    }
    for exception in [".", ".aster", ".aster-other/loops"] {
        assert!(StoragePathRestriction::new(
            StorageAttachmentId::new("workspace").unwrap(),
            vec![],
            vec![".aster".into()],
            vec![exception.into()]
        )
        .is_err());
    }
    assert!(StoragePathRestriction::new(
        StorageAttachmentId::new("workspace").unwrap(),
        vec![".".into()],
        vec![".".into()],
        vec!["out".into()]
    )
    .is_ok());
}

#[test]
fn binds_required_extension_to_exact_storage_and_preserves_unicode_paths() {
    let (bundle, attachments) = fixture(StorageAccessMode::ReadWrite);
    let attachments = attachments.with_storage_path_restrictions(&bundle).unwrap();
    let policy = attachments
        .storage_path_restrictions(&bundle)
        .unwrap()
        .unwrap();
    assert_eq!(policy.entries()[0].deny_read(), rule().deny_read());
    assert!(crate::AttachmentCapabilities::base_v2()
        .require(&attachments)
        .is_err());
    for (version, required) in [(1, false), (2, true)] {
        let (_, unbound) = fixture(StorageAccessMode::ReadWrite);
        assert!(unbound
            .add_extension_from_annotation(
                &bundle,
                STORAGE_PATH_RESTRICTIONS_EXTENSION,
                version,
                required
            )
            .is_err());
    }
    let empty = CreateAttachments::from_bundle(&bundle, ProcessIo::default()).unwrap();
    assert!(empty.with_storage_path_restrictions(&bundle).is_err());
    let (bundle, readonly) = fixture(StorageAccessMode::ReadOnly);
    assert!(readonly.with_storage_path_restrictions(&bundle).is_err());
}

#[test]
fn rejects_noncanonical_duplicates_unknown_fields_and_oversized_annotations() {
    let policy = StoragePathRestrictions::new(vec![rule()]).unwrap();
    assert_eq!(
        StoragePathRestrictions::from_annotation_value(&policy.to_annotation_value().unwrap())
            .unwrap(),
        policy
    );
    assert!(StoragePathRestrictions::new(vec![rule(), rule()]).is_err());
    for change in ["unknown", "duplicate-path", "version"] {
        let mut value = serde_json::to_value(&policy).unwrap();
        match change {
            "unknown" => value["entries"][0]["absoluteAllow"] = json!("/"),
            "version" => value["schemaVersion"] = json!("future"),
            _ => value["entries"][0]["denyRead"] = json!([".env", ".env"]),
        }
        assert!(StoragePathRestrictions::from_annotation_value(&value.to_string()).is_err());
    }
    assert!(StoragePathRestrictions::from_annotation_value(&" ".repeat(MAX_BYTES + 1)).is_err());
}
