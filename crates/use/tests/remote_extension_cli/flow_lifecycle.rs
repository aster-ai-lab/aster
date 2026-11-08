use std::path::Path;

use super::*;

#[test]
fn flow_package_install_is_refused_and_publishes_nothing() {
    let temp = tempfile::tempdir().unwrap();
    let target = host_target();
    let package = package_flow_target_version(temp.path(), "1.0.0", &target);
    let repository = TestRepository::with_targets(vec![package], 71, FUTURE);
    let server = TestServer::start(repository.routes.clone());
    let home = temp.path().join("home");
    let compiler = temp.path().join("unused-flow-compiler");

    let output = flow_registry_command(&server, &repository, &home, &compiler, "install", "1.0.0");
    assert!(!output.status.success(), "{output:?}");
    assert_eq!(json(&output)["error"]["code"], "use.flow_runtime_removed");
    assert!(flow_bindings(&home).is_empty());
    assert!(flow_capability(&home).is_none());
}

fn flow_registry_command(
    server: &TestServer,
    repository: &TestRepository,
    home: &Path,
    compiler: &Path,
    action: &str,
    version: &str,
) -> Output {
    configure_registry(server, repository, home, &[]);
    Command::new(binary())
        .args([
            action,
            "acme/flow-suite",
            "--scope-kind",
            "user",
            "--scope-id",
            "user/current",
            "--registry-name",
            "fixture",
            "--version",
            version,
            "--json",
        ])
        .env("ASTER_USE_HOME", home)
        .env("ASTER_FLOW_NATIVE_TS_COMPILER", compiler)
        .output()
        .unwrap()
}

fn flow_bindings(home: &Path) -> Vec<serde_json::Value> {
    fn collect(directory: &Path, output: &mut Vec<serde_json::Value>) {
        let Ok(entries) = std::fs::read_dir(directory) else {
            return;
        };
        for entry in entries {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect(&path, output);
            } else if path
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                output.push(serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap());
            }
        }
    }

    let mut bindings = Vec::new();
    collect(&home.join("state/bindings/flow"), &mut bindings);
    bindings.sort_by_key(|binding| binding["generation"].as_u64().unwrap_or_default());
    bindings
}

fn flow_capability(home: &Path) -> Option<serde_json::Value> {
    capability_registry(home)["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|capability| capability["route"] == "flow-suite")
        .cloned()
}

fn capability_registry(home: &Path) -> serde_json::Value {
    let snapshot = Command::new(binary())
        .args(["capability", "snapshot", "--json"])
        .env("ASTER_USE_HOME", home)
        .output()
        .unwrap();
    assert!(snapshot.status.success(), "{snapshot:?}");
    json(&snapshot)["data"]["registry"].clone()
}
