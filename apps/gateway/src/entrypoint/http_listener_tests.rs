#![cfg(test)]

use super::private_unix::validate_parent;
use std::os::unix::fs::{MetadataExt, PermissionsExt};

#[test]
fn private_parent_is_checked_against_process_identity_before_binding() {
    let directory = tempfile::Builder::new()
        .prefix("aster-owner-")
        .tempdir_in("/tmp")
        .unwrap();
    let parent = directory.path().canonicalize().unwrap();
    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).unwrap();
    let owner = std::fs::symlink_metadata(&parent).unwrap().uid();
    validate_parent(&parent, owner).unwrap();
    let error = validate_parent(&parent, owner.wrapping_add(1)).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(error.to_string().contains("listener owner"));
    assert_eq!(std::fs::read_dir(&parent).unwrap().count(), 0);
}
