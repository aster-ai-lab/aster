use std::path::PathBuf;

use crate::ExtensionLifecyclePackage;

// Catch a candidate that can be admitted by a host predating the toolchain owner.
#[tokio::test]
#[ignore = "requires the exact prepared toolchain AIX candidate list"]
async fn acquisition_candidates_require_the_implementing_host_before_admission() {
    let input = std::env::var_os("ASTER_TEST_HOST_FLOOR_CANDIDATES").unwrap();
    let paths: Vec<PathBuf> = serde_json::from_slice(&std::fs::read(input).unwrap()).unwrap();
    assert!(!paths.is_empty());
    for path in paths {
        let inspected = crate::inspect_aix_package(&path).await.unwrap();
        let id = &inspected.package_id;
        let previous =
            ExtensionLifecyclePackage::prepare_local_for_host_version(id, &path, true, "0.3.3")
                .await
                .unwrap_err();
        assert_eq!(
            previous.code,
            "use.extension.host_incompatible",
            "{}",
            path.display()
        );
        let current = ExtensionLifecyclePackage::prepare_local(id, &path, true)
            .await
            .unwrap();
        assert_eq!(
            current.manifest().requires_use.as_deref(),
            Some(">=0.3.4, <0.4.0")
        );
        assert!(current
            .manifest()
            .plugins
            .iter()
            .any(|surface| surface.id == "toolchain"));
    }
}
