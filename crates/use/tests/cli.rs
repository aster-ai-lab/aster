use std::process::Command;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_aster-use")
}

#[test]
fn capabilities_are_available_as_versioned_json() {
    let output = Command::new(binary())
        .args(["capabilities", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["schemaVersion"], 1);
    let domains = value["data"]["domains"].as_array().unwrap();
    assert!(domains.is_empty(), "{domains:?}");
    for id in ["browser", "ocr", "box"] {
        assert!(
            domains.iter().all(|domain| domain["id"] != id),
            "removed built-in domain {id} is still published: {domains:?}"
        );
    }
    assert!(value["data"].get("customJsonRpc").is_none());
    assert!(value.get("jsonrpc").is_none());
}

#[test]
fn unified_capability_snapshot_projects_builtin_skills() {
    let temp = tempfile::tempdir().unwrap();
    let output = Command::new(binary())
        .args(["capability", "snapshot", "--json"])
        .env("ASTER_USE_HOME", temp.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let capabilities = value["data"]["registry"]["capabilities"]
        .as_array()
        .unwrap();
    assert!(capabilities.iter().all(|capability| !matches!(
        capability["id"].as_str(),
        Some("use/browser" | "use/ocr" | "use/box")
    )));
    assert!(capabilities
        .iter()
        .all(|capability| capability["route"] != "office"));
    assert!(value.get("jsonrpc").is_none());
}

#[test]
fn capability_watch_uses_revision_to_report_an_unchanged_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let snapshot = Command::new(binary())
        .args(["capability", "snapshot", "--json"])
        .env("ASTER_USE_HOME", temp.path())
        .output()
        .unwrap();
    assert!(snapshot.status.success(), "{snapshot:?}");
    let snapshot: serde_json::Value = serde_json::from_slice(&snapshot.stdout).unwrap();
    let registry = &snapshot["data"]["registry"];
    let generation = registry["generation"].as_u64().unwrap().to_string();
    let revision = registry["revision"].as_str().unwrap();

    let watched = Command::new(binary())
        .args([
            "capability",
            "watch",
            "--after-generation",
            &generation,
            "--after-revision",
            revision,
            "--timeout-ms",
            "1",
            "--json",
        ])
        .env("ASTER_USE_HOME", temp.path())
        .output()
        .unwrap();
    assert!(watched.status.success(), "{watched:?}");
    let watched: serde_json::Value = serde_json::from_slice(&watched.stdout).unwrap();
    assert_eq!(watched["data"]["changed"], false);
}

#[test]
fn box_route_is_removed() {
    let output = Command::new(binary())
        .args(["box", "compose", "up", "--detach", "--json"])
        .env("ASTER_USE_BOX_EXECUTABLE", "/tmp/aster-box-must-not-launch")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["error"]["code"], "use.builtin_removed");
    assert!(output.stderr.is_empty());
}

#[test]
fn box_route_fails_closed_without_an_explicit_component_path() {
    let output = Command::new(binary())
        .args(["box", "ps", "--json"])
        .env_remove("ASTER_USE_BOX_EXECUTABLE")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["error"]["code"], "use.builtin_removed");
}

#[test]
fn delegated_component_status_matches_the_root_cli_contract() {
    let output = Command::new(binary())
        .args(["component", "status", "browser", "--json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["error"]["code"], "use.builtin_removed");
}

#[test]
fn machine_errors_are_single_json_documents() {
    let output = Command::new(binary())
        .args(["unknown", "--json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["ok"], false);
    assert_eq!(value["error"]["code"], "use.route_unknown");
}

#[cfg(feature = "extensions")]
#[test]
fn knowledge_search_fails_closed_without_an_active_projection() {
    let temporary = tempfile::tempdir().unwrap();
    let output = Command::new(binary())
        .args(["knowledge", "search", "package activation", "--json"])
        .env("ASTER_USE_HOME", temporary.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["error"]["code"], "use.okf.knowledge_unavailable");
}

#[cfg(feature = "extensions")]
#[test]
fn coordinated_state_backup_cli_creates_and_verifies_offline() {
    let temporary = tempfile::tempdir().unwrap();
    let home = temporary.path().join("home");
    let package_file = home.join("data/extensions/acme/tool/package/bin/tool");
    std::fs::create_dir_all(package_file.parent().unwrap()).unwrap();
    std::fs::write(&package_file, b"portable CLI backup fixture").unwrap();
    let backup_path = temporary.path().join("state.aster-use-state-backup");

    let backup = Command::new(binary())
        .args(["state", "backup", backup_path.to_str().unwrap(), "--json"])
        .env("ASTER_USE_HOME", &home)
        .output()
        .unwrap();
    assert!(backup.status.success(), "{backup:?}");
    assert!(backup.stderr.is_empty());
    let backup_json: serde_json::Value = serde_json::from_slice(&backup.stdout).unwrap();
    assert_eq!(backup_json["data"]["schema"], "aster.use.state-backup.v1");
    assert_eq!(backup_json["data"]["fileCount"], 1);
    assert_eq!(backup_json["data"]["entries"][0]["root"], "data");
    assert_eq!(
        backup_json["data"]["entries"][0]["path"],
        "extensions/acme/tool/package/bin/tool"
    );
    assert!(backup_path.is_file());

    let offline_home = temporary.path().join("unrelated-empty-home");
    let verified = Command::new(binary())
        .args([
            "state",
            "verify-backup",
            backup_path.to_str().unwrap(),
            "--json",
        ])
        .env("ASTER_USE_HOME", offline_home)
        .output()
        .unwrap();
    assert!(verified.status.success(), "{verified:?}");
    assert!(verified.stderr.is_empty());
    let verified_json: serde_json::Value = serde_json::from_slice(&verified.stdout).unwrap();
    assert_eq!(verified_json["data"], backup_json["data"]);
}

#[cfg(feature = "extensions")]
#[test]
fn coordinated_state_backup_retention_cli_requires_exact_review() {
    let temporary = tempfile::tempdir().unwrap();
    let home = temporary.path().join("home");
    let state_file = home.join("state/registry-trust-roots/sha256/root.json");
    std::fs::create_dir_all(state_file.parent().unwrap()).unwrap();
    let backup_directory = temporary.path().join("backups");
    std::fs::create_dir(&backup_directory).unwrap();

    for (name, bytes) in [
        ("001.aster-use-state-backup", b"one".as_slice()),
        ("002.aster-use-state-backup", b"two".as_slice()),
        ("003.aster-use-state-backup", b"three".as_slice()),
    ] {
        std::fs::write(&state_file, bytes).unwrap();
        let output = Command::new(binary())
            .args([
                "state",
                "backup",
                backup_directory.join(name).to_str().unwrap(),
                "--json",
            ])
            .env("ASTER_USE_HOME", &home)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
    }

    let plan = Command::new(binary())
        .args([
            "state",
            "backup-retention",
            backup_directory.to_str().unwrap(),
            "--max-backups",
            "2",
            "--max-bytes",
            "1073741824",
            "--json",
        ])
        .env("ASTER_USE_HOME", &home)
        .output()
        .unwrap();
    assert!(plan.status.success(), "{plan:?}");
    assert!(plan.stderr.is_empty());
    let plan: serde_json::Value = serde_json::from_slice(&plan.stdout).unwrap();
    let retention = &plan["data"]["state"]["backupRetention"];
    assert_eq!(
        retention["plan"]["remove"][0]["fileName"],
        "001.aster-use-state-backup"
    );
    assert_eq!(retention["plan"]["retainedBackupCount"], 2);
    let plan_digest = retention["planDigest"].as_str().unwrap();

    let applied = Command::new(binary())
        .args([
            "state",
            "backup-retention",
            backup_directory.to_str().unwrap(),
            "--max-backups",
            "2",
            "--max-bytes",
            "1073741824",
            "--plan-digest",
            plan_digest,
            "--yes",
            "--json",
        ])
        .env("ASTER_USE_HOME", &home)
        .output()
        .unwrap();
    assert!(applied.status.success(), "{applied:?}");
    assert!(applied.stderr.is_empty());
    let applied: serde_json::Value = serde_json::from_slice(&applied.stdout).unwrap();
    let result = &applied["data"]["state"]["backupRetention"]["result"];
    assert_eq!(result["schema"], "aster.use.state-backup-retention-result.v1");
    assert_eq!(result["removed"].as_array().unwrap().len(), 1);
    assert_eq!(result["retainedBackupCount"], 2);
    assert!(!backup_directory.join("001.aster-use-state-backup").exists());
    assert!(backup_directory.join("002.aster-use-state-backup").is_file());
    assert!(backup_directory.join("003.aster-use-state-backup").is_file());
}

#[cfg(feature = "extensions")]
#[test]
fn coordinated_state_restore_cli_requires_review_rollback_and_confirmation() {
    let temporary = tempfile::tempdir().unwrap();
    let home = temporary.path().join("home");
    let state_file = home.join("state/knowledge/value.bin");
    std::fs::create_dir_all(state_file.parent().unwrap()).unwrap();
    std::fs::write(&state_file, b"candidate").unwrap();
    let backup_path = temporary.path().join("candidate.aster-use-state-backup");
    let backup = Command::new(binary())
        .args(["state", "backup", backup_path.to_str().unwrap(), "--json"])
        .env("ASTER_USE_HOME", &home)
        .output()
        .unwrap();
    assert!(backup.status.success(), "{backup:?}");
    std::fs::write(&state_file, b"live drift").unwrap();

    let plan = Command::new(binary())
        .args([
            "state",
            "plan-restore",
            backup_path.to_str().unwrap(),
            "--json",
        ])
        .env("ASTER_USE_HOME", &home)
        .output()
        .unwrap();
    assert!(plan.status.success(), "{plan:?}");
    let plan: serde_json::Value = serde_json::from_slice(&plan.stdout).unwrap();
    let restore = &plan["data"]["state"]["restore"];
    assert_eq!(restore["plan"]["schema"], "aster.use.state-restore-plan.v1");
    assert_eq!(restore["plan"]["summary"]["replaceFiles"], 1);
    let plan_digest = restore["planDigest"].as_str().unwrap();
    let rollback_path = temporary.path().join("rollback.aster-use-state-backup");

    let unconfirmed = Command::new(binary())
        .args([
            "state",
            "restore",
            backup_path.to_str().unwrap(),
            "--rollback-backup",
            rollback_path.to_str().unwrap(),
            "--plan-digest",
            plan_digest,
            "--json",
        ])
        .env("ASTER_USE_HOME", &home)
        .output()
        .unwrap();
    assert!(!unconfirmed.status.success());
    let unconfirmed: serde_json::Value = serde_json::from_slice(&unconfirmed.stdout).unwrap();
    assert_eq!(unconfirmed["error"]["code"], "use.cli.invalid_usage");
    assert!(!rollback_path.exists());

    let applied = Command::new(binary())
        .args([
            "state",
            "restore",
            backup_path.to_str().unwrap(),
            "--rollback-backup",
            rollback_path.to_str().unwrap(),
            "--plan-digest",
            plan_digest,
            "--yes",
            "--json",
        ])
        .env("ASTER_USE_HOME", &home)
        .output()
        .unwrap();
    assert!(applied.status.success(), "{applied:?}");
    let applied: serde_json::Value = serde_json::from_slice(&applied.stdout).unwrap();
    let result = &applied["data"]["state"]["restore"]["result"];
    assert_eq!(result["schema"], "aster.use.state-restore-result.v1");
    assert_eq!(result["changed"], true);
    assert_eq!(result["planDigest"], plan_digest);
    assert_eq!(std::fs::read(&state_file).unwrap(), b"candidate");
    assert!(rollback_path.is_file());

    let status = Command::new(binary())
        .args(["state", "restore-status", "--json"])
        .env("ASTER_USE_HOME", &home)
        .output()
        .unwrap();
    assert!(status.status.success(), "{status:?}");
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    let diagnostic = &status["data"]["state"]["restore"]["diagnostic"];
    assert_eq!(diagnostic["schema"], "aster.use.state-restore-diagnostic.v1");
    assert!(diagnostic["active"].is_null());
    assert_eq!(diagnostic["operations"][0]["status"], "completed");
    assert_eq!(diagnostic["operations"][0]["planDigest"], plan_digest);
}

#[cfg(feature = "extensions")]
#[test]
fn knowledge_usage_reports_an_empty_exact_default_scope() {
    let temporary = tempfile::tempdir().unwrap();
    let output = Command::new(binary())
        .args(["knowledge", "usage", "--json"])
        .env("ASTER_USE_HOME", temporary.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let storage = &value["data"]["knowledge"]["storage"];
    assert_eq!(storage["scope"]["kind"], "user");
    assert_eq!(storage["scope"]["id"], "user/current");
    assert_eq!(storage["retainedProjections"], 0);
    assert_eq!(storage["removedTombstones"], 0);
    assert_eq!(storage["retainedExpandedBytes"], 0);
    assert_eq!(storage["maxScopeProjections"], 256);
    assert_eq!(storage["maxScopeExpandedBytes"], 512 * 1024 * 1024);
    assert_eq!(storage["maxSurfaceGenerations"], 32);
    assert_eq!(storage["maxScopeTombstones"], 256);
    assert_eq!(storage["reclaimableDatabaseBytes"], 0);
}

#[cfg(feature = "extensions")]
#[test]
fn knowledge_usage_reports_an_empty_exact_workspace_scope() {
    let temporary = tempfile::tempdir().unwrap();
    let output = Command::new(binary())
        .args([
            "knowledge",
            "usage",
            "--scope-kind",
            "workspace",
            "--scope-id",
            "workspace/acme-project",
            "--json",
        ])
        .env("ASTER_USE_HOME", temporary.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let storage = &value["data"]["knowledge"]["storage"];
    assert_eq!(storage["scope"]["kind"], "workspace");
    assert_eq!(storage["scope"]["id"], "workspace/acme-project");
    assert_eq!(storage["retainedProjections"], 0);
    assert_eq!(storage["removedTombstones"], 0);
    assert_eq!(storage["retainedExpandedBytes"], 0);
}

#[cfg(feature = "extensions")]
#[test]
fn knowledge_usage_rejects_an_unknown_scope_kind() {
    let temporary = tempfile::tempdir().unwrap();
    let output = Command::new(binary())
        .args([
            "knowledge",
            "usage",
            "--scope-kind",
            "organization",
            "--json",
        ])
        .env("ASTER_USE_HOME", temporary.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["error"]["code"], "use.cli.invalid_usage");
}

#[cfg(feature = "extensions")]
#[test]
fn knowledge_usage_requires_an_exact_workspace_scope_id() {
    let temporary = tempfile::tempdir().unwrap();
    let output = Command::new(binary())
        .args(["knowledge", "usage", "--scope-kind", "workspace", "--json"])
        .env("ASTER_USE_HOME", temporary.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["error"]["code"], "use.cli.invalid_usage");
}

#[test]
fn mcp_stop_is_safe_when_no_service_is_running() {
    let temp = tempfile::tempdir().unwrap();
    let output = Command::new(binary())
        .args(["mcp", "stop", "--json"])
        .env("ASTER_USE_RUNTIME_DIR", temp.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["error"]["code"], "use.builtin_removed");
}
