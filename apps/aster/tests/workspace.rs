use std::path::Path;
use std::process::Command;

#[test]
fn product_help_and_version_use_aster() {
    let version = format!("aster {}", env!("CARGO_PKG_VERSION"));
    for (arg, expected) in [
        ("--help", "Aster terminal coding agent"),
        ("--version", version.as_str()),
    ] {
        let output = Command::new(
            std::env::var_os("ASTER_TUI_BIN")
                .unwrap_or_else(|| env!("CARGO_BIN_EXE_aster").into()),
        )
        .arg(arg)
        .output()
        .expect("run aster");
        assert!(output.status.success(), "{output:?}");
        let stdout = String::from_utf8(output.stdout).expect("UTF-8 output");
        assert!(stdout.contains(expected), "{stdout}");
        if arg == "--help" {
            assert!(stdout.contains("Usage: aster "), "{stdout}");
        }
    }
}

#[test]
fn workspace_contains_local_rust_harness_and_both_product_binaries() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--locked",
            "--offline",
            "--no-deps",
            "--format-version",
            "1",
        ])
        .current_dir(&root)
        .output()
        .expect("read Cargo metadata");
    assert!(output.status.success(), "{output:?}");
    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Cargo metadata");
    let packages = metadata["packages"].as_array().expect("workspace packages");
    let defaults = metadata["workspace_default_members"]
        .as_array()
        .expect("default members");
    for name in ["aster", "aster-acp"] {
        let package = packages
            .iter()
            .find(|p| p["name"] == name)
            .expect("product member");
        assert!(
            defaults.contains(&package["id"]),
            "{name} must build by default"
        );
    }
    for package in packages {
        let name = package["name"].as_str().expect("package name");
        assert!(!["aster-node", "aster-py", "aster-go-bridge"].contains(&name));
        for dependency in package["dependencies"].as_array().expect("dependencies") {
            if dependency["name"]
                .as_str()
                .is_some_and(|name| name.starts_with("aster-") || name.starts_with("aster-"))
            {
                let path = dependency["path"]
                    .as_str()
                    .expect("internal dependency must be local");
                assert!(
                    Path::new(path).starts_with(metadata["workspace_root"].as_str().expect("root"))
                );
            }
        }
    }
}
