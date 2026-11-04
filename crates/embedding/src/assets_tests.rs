use super::*;

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
