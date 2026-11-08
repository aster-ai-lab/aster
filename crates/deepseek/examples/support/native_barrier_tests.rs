use super::*;
use std::os::unix::fs::PermissionsExt;

#[test]
fn ready_and_release_keep_the_exact_model_and_private_directory_contract() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::write(
        root.join("go.json"),
        serde_json::to_vec(&serde_json::json!({"operation":"generate","model_sha256":MODEL_SHA}))
            .unwrap(),
    )
    .unwrap();
    let ready = serde_json::json!({"model_sha256":MODEL_SHA,"devices":["fixture-a","fixture-b"]});
    wait_until(&root, &ready, Instant::now() + Duration::from_secs(1)).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            &std::fs::read(root.join("ready.json")).unwrap()
        )
        .unwrap(),
        ready
    );
    assert!(!root.join("ready.tmp").exists());
    assert!(wait_until(&root, &ready, Instant::now() + Duration::from_secs(1)).is_err());
}

#[test]
fn unknown_model_operation_and_expired_deadline_never_release_a_replica() {
    for (model, operation) in [("wrong", "generate"), (MODEL_SHA, "unknown")] {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::write(
            root.join("go.json"),
            serde_json::to_vec(&serde_json::json!({"operation":operation,"model_sha256":model}))
                .unwrap(),
        )
        .unwrap();
        assert!(wait_until(
            &root,
            &serde_json::json!({}),
            Instant::now() + Duration::from_secs(1)
        )
        .is_err());
    }
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(wait_until(&root, &serde_json::json!({}), Instant::now()).is_err());
}
