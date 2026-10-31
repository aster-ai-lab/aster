use super::*;
use aster_effect::{ToolFileIdentityV1, ToolFileKind};

fn directory(mode: u32) -> ToolFileIdentityV1 {
    ToolFileIdentityV1 {
        kind: ToolFileKind::Directory,
        content_digest: None,
        mode: Some(mode),
    }
}

#[test]
fn unchanged_parent_directory_is_not_a_workspace_mutation() {
    let identity = directory(0o40755);
    let mut ledger = MutationLedger::default();
    ledger.observe_receipt_path("write", "concepts", Some(&identity), &identity);
    assert!(ledger.is_empty());
}

#[test]
fn directory_creation_and_mode_change_still_require_verification() {
    let identity = directory(0o40755);
    let missing = ToolFileIdentityV1 {
        kind: ToolFileKind::Missing,
        content_digest: None,
        mode: None,
    };
    let mut created = MutationLedger::default();
    created.observe_receipt_path("write", "concepts", Some(&missing), &identity);
    created.observe_receipt_path("write", "concepts", Some(&identity), &identity);
    assert_eq!(created.paths().collect::<Vec<_>>(), ["concepts"]);
    let mut changed = MutationLedger::default();
    changed.observe_receipt_path("write", "concepts", Some(&identity), &directory(0o40700));
    assert_eq!(changed.paths().collect::<Vec<_>>(), ["concepts"]);
}
