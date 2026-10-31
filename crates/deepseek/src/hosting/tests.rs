use super::*;

#[tokio::test]
async fn public_power_composition_refuses_two_competing_authentication_policies_before_startup() {
    let mut config = aster_power::config::PowerConfig::default();
    config.api_keys = vec!["fixture-static-token".into()];
    let provider = std::sync::Arc::new(ApiKeyAuth::new(&["fixture-host-token".into()]));
    let result = aster_power::server::PowerServerBuilder::new(config)
        .with_auth_provider(provider)
        .start()
        .await;
    assert!(
        matches!(result,Err(PowerError::Config(message)) if message.contains("mutually exclusive"))
    );
}

#[test]
fn native_hosting_acl_requires_pinned_identity_and_a_bounded_usable_profile() {
    let model = crate::model::DeepSeekModel::V41Flash1017Xq;
    let root = std::env::temp_dir().to_string_lossy().replace('\\', "/");
    let acl = format!("schema = \"aster.deepseek.xq.serving.v1\"\nmodel = \"{}\"\nrevision = \"{}\"\npayload_root = \"{root}\"\nofficial_blob_root = \"{root}\"\nauth_root = \"{root}\"\ntokenizer_file = \"tokenizer\"\ntokenizer_bytes = 6367257\nlayout = \"4x2\"\nthinking_mode = \"chat\"\ncontext_tokens = 256\nmax_generated_tokens = 32\ncapacity_per_replica = 4\nload_seconds = 1200\nreasoning_effort = 75\n",model.as_str(),model.revision());
    assert_eq!(
        NativeServingConfig::from_acl(&acl).unwrap().layout,
        XqReplicaLayout::FourByTwo
    );
    for invalid in [
        acl.replace("4x2", "8x1"),
        acl.replace("context_tokens = 256", "context_tokens = 32"),
        acl.replace("context_tokens = 256", "context_tokens = 32768"),
        acl.clone() + "unknown = true\n",
    ] {
        assert!(NativeServingConfig::from_acl(&invalid).is_err());
    }
}

#[test]
fn credential_expiry_is_absolute_and_invalid_or_extended_leases_fail_closed() {
    let lease = |start: u64, end: u64| {
        format!("schema = \"aster.deepseek.credential-lease.v1\"\napi_key_sha256 = \"{}\"\nstarts_unix = {start}\nexpires_unix = {end}\n","a".repeat(64))
    };
    let input = lease(100, 7300);
    let parsed = CredentialLease::from_acl(&input).unwrap();
    assert!(!parsed.active(99));
    assert!(parsed.active(100));
    assert!(parsed.active(7299));
    assert!(!parsed.active(7300));
    assert!(!CredentialLease::from_acl(&input).unwrap().active(7301));
    assert!(CredentialLease::from_acl(&lease(100, 7301)).is_err());
    assert!(CredentialLease::from_acl(&lease(100, 100)).is_err());
    assert!(CredentialLease::from_acl(&(input + "unknown = true\n")).is_err());
}

#[cfg(unix)]
#[test]
fn actual_private_lease_refuses_wrong_keys_revocation_bad_acl_and_insecure_permissions() {
    use sha2::{Digest, Sha256};
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let input=format!("schema = \"aster.deepseek.credential-lease.v1\"\napi_key_sha256 = \"{:x}\"\nstarts_unix = {}\nexpires_unix = {}\n",Sha256::digest(b"hermetic-only-token"),now-1,now+60);
    let path = root.path().join("credential.acl");
    std::fs::write(&path, input).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let auth = FileLeaseAuth::new(root.path().to_owned()).unwrap();
    assert!(auth.authenticate("hermetic-only-token"));
    assert!(!auth.authenticate("wrong"));
    std::fs::write(root.path().join("revoked"), b"revoked").unwrap();
    assert!(!auth.authenticate("hermetic-only-token"));
    std::fs::remove_file(root.path().join("revoked")).unwrap();
    std::fs::write(&path, b"malformed").unwrap();
    assert!(!auth.authenticate("hermetic-only-token"));
    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(!auth.authenticate("hermetic-only-token"));
}
