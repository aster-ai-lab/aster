use super::*;
use crate::external_observation::{
    claim_write, refuse_foreign_workspace_owner, release_session, session_owns_write,
};

fn owned_file(root: &Path, owner: &str, name: &str) {
    std::fs::write(root.join(name), "pending change\n").unwrap();
    claim_write(owner, root, name).unwrap();
}

#[test]
fn holds_parent_files_until_verification_finishes() {
    let root = tempfile::tempdir().unwrap();
    owned_file(root.path(), "loan-parent", "pending.txt");
    {
        let _loan = WriteClaimLoan::new("loan-parent", "loan-child", root.path()).unwrap();
        assert!(session_owns_write("loan-child", root.path(), "pending.txt"));
        assert!(refuse_foreign_workspace_owner(Some("loan-child"), root.path()).is_ok());
        assert!(claim_write("loan-parent", root.path(), "pending.txt").is_err());
        assert!(refuse_foreign_workspace_owner(Some("loan-parent"), root.path()).is_err());
        assert!(refuse_foreign_workspace_owner(Some("loan-unrelated"), root.path()).is_err());
    }
    assert!(session_owns_write(
        "loan-parent",
        root.path(),
        "pending.txt"
    ));
    assert!(!session_owns_write(
        "loan-child",
        root.path(),
        "pending.txt"
    ));
    release_session("loan-parent");
}

#[tokio::test]
async fn cancellation_returns_parent_claims() {
    let root = tempfile::tempdir().unwrap();
    owned_file(root.path(), "cancel-parent", "pending.txt");
    let (ready, started) = tokio::sync::oneshot::channel();
    let workspace = root.path().to_path_buf();
    let run = tokio::spawn(async move {
        let _loan = WriteClaimLoan::new("cancel-parent", "cancel-child", &workspace).unwrap();
        ready.send(()).unwrap();
        std::future::pending::<()>().await;
    });
    started.await.unwrap();
    run.abort();
    assert!(run.await.unwrap_err().is_cancelled());
    assert!(session_owns_write(
        "cancel-parent",
        root.path(),
        "pending.txt"
    ));
    assert!(!session_owns_write(
        "cancel-child",
        root.path(),
        "pending.txt"
    ));
    release_session("cancel-parent");
}

#[test]
fn refuses_unrelated_claims_without_partial_transfer() {
    let root = tempfile::tempdir().unwrap();
    owned_file(root.path(), "mixed-parent", "pending.txt");
    owned_file(root.path(), "mixed-other", "other.txt");
    assert!(WriteClaimLoan::new("mixed-parent", "mixed-child", root.path()).is_err());
    assert!(session_owns_write(
        "mixed-parent",
        root.path(),
        "pending.txt"
    ));
    assert!(session_owns_write("mixed-other", root.path(), "other.txt"));
    release_session("mixed-parent");
    release_session("mixed-other");
}
