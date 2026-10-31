#![cfg(test)]

use super::*;

fn fixture(directory: &Path) {
    for name in [
        "aster-oci",
        "aster-oci-krun-shim",
        "system-image.json",
        "bundle/config.json",
        "bundle/rootfs/bin/bash",
    ] {
        let path = directory.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"not executable: metadata must not launch this").unwrap();
    }
}

#[test]
fn actual_oci_metadata_reports_per_call_lifetime_without_guessing_optional_tools() {
    let directory = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    fixture(directory.path());
    let sandbox = OciBashSandbox::from_artifacts(
        workspace.path(),
        OciSandboxAssets::from_directory(directory.path()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        sandbox.execution_target(),
        super::super::ShellExecutionTarget::LinuxGuestBash
    );
    let note = sandbox.shell_note().unwrap();
    assert!(note.contains("fresh Linux guest"));
    assert!(note.contains("/tmp and guest rootfs changes are discarded"));
    assert!(note.contains("mounted workspace persist across calls"));
    assert!(note.contains("availability is unreported"));
    assert!(!note.contains("python3 is installed"));
    std::fs::remove_file(directory.path().join("bundle/rootfs/bin/bash")).unwrap();
    let error = sandbox
        .validate_execution_available()
        .unwrap_err()
        .to_string();
    assert!(error.contains("sandbox companion is missing"));
    assert!(error.contains("bin/bash"));
}

#[test]
fn every_missing_required_companion_fails_before_command_dispatch() {
    let directory = tempfile::tempdir().unwrap();
    for name in [
        "aster-oci",
        "aster-oci-krun-shim",
        "system-image.json",
        "bundle/config.json",
        "bundle/rootfs/bin/bash",
    ] {
        fixture(directory.path());
        std::fs::remove_file(directory.path().join(name)).unwrap();
        let error = OciSandboxAssets::from_directory(directory.path())
            .unwrap_err()
            .to_string();
        assert!(error.contains("sandbox companion is missing"), "{error}");
        assert!(error.contains(name), "{error}");
    }
}
