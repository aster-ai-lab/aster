#![cfg(test)]

use std::time::{Duration, Instant};

/// Owned HTTP fixture for renderer/native integration; never starts a GUI or a model.
#[test]
#[ignore = "requires A3S_STUDIO_REVIEW_OBSERVATION_DIR owned by the native renderer test"]
fn serve_review_observation() {
    let root = std::path::PathBuf::from(
        std::env::var_os("A3S_STUDIO_REVIEW_OBSERVATION_DIR").expect("owned fixture directory"),
    );
    assert!(root.is_absolute() && root.join(".owned").is_file());
    assert!(!root.join("runtime.json").exists());
    let mut authority =
        super::StudioAuthority::start_with_store("http://127.0.0.1:9", root.join("capabilities"))
            .expect("isolated in-memory authority");
    std::fs::write(
        root.join("runtime.json"),
        serde_json::to_vec(&serde_json::json!({
            "apiBaseUrl": authority.api_base_url(),
            "token": authority.token(),
        }))
        .unwrap(),
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(120);
    while !root.join("stop").exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    authority.shutdown();
    std::fs::remove_file(root.join("runtime.json")).unwrap();
    assert!(
        root.join("stop").exists(),
        "fixture owner did not close the authority"
    );
}
