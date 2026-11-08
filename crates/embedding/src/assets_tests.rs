use std::path::Path;
use std::sync::Mutex;

use super::*;

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[cfg(feature = "bundled-assets")]
#[test]
fn every_bundle_asset_refuses_digest_drift() {
    for (name, bytes, digest) in [
        ("model", MODEL_BYTES, MODEL_SHA256),
        ("tokenizer", TOKENIZER_BYTES, TOKENIZER_SHA256),
        ("config", CONFIG_BYTES, CONFIG_SHA256),
    ] {
        assert!(verify_asset(name, bytes, digest).is_ok());
        let error = verify_asset(name, b"synthetic drift", digest).unwrap_err();
        assert!(error.starts_with(&format!("{name} asset sha256 drift:")));
    }
}

#[test]
fn load_from_repo_assets_dir_verifies_digests() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let loaded = load_from_dir(&dir).expect("repo assets must verify");
    assert_eq!(loaded.source, AssetSource::Directory);
    assert_eq!(hex(&Sha256::digest(&loaded.model)), MODEL_SHA256);
}

#[test]
fn load_from_dir_refuses_digest_drift() {
    let tmp = tempfile::tempdir().unwrap();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    for name in [MODEL_FILE, TOKENIZER_FILE, CONFIG_FILE] {
        std::fs::copy(src.join(name), tmp.path().join(name)).unwrap();
    }
    std::fs::write(tmp.path().join(MODEL_FILE), b"not-the-model").unwrap();
    let err = match load_from_dir(tmp.path()) {
        Ok(_) => panic!("corrupt model must refuse"),
        Err(error) => error,
    };
    assert!(
        err.contains("sha256 drift"),
        "expected digest drift, got {err}"
    );
}

#[test]
fn env_dir_override_is_preferred_when_populated() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|poison| poison.into_inner());
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let tmp = tempfile::tempdir().unwrap();
    for name in [MODEL_FILE, TOKENIZER_FILE, CONFIG_FILE] {
        std::fs::copy(src.join(name), tmp.path().join(name)).unwrap();
    }
    let previous = std::env::var_os(EMBEDDING_DIR_ENV);
    // SAFETY: single-threaded under ENV_LOCK; restored before unlock.
    unsafe {
        std::env::set_var(EMBEDDING_DIR_ENV, tmp.path());
    }
    let loaded = load_verified().expect("env override must load");
    assert_eq!(loaded.source, AssetSource::Directory);
    unsafe {
        match previous {
            Some(value) => std::env::set_var(EMBEDDING_DIR_ENV, value),
            None => std::env::remove_var(EMBEDDING_DIR_ENV),
        }
    }
}
