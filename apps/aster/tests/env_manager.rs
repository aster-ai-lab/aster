//! Isolated environment-manager tests against the actual Aster CLI binary.

use std::ffi::OsStr;
use std::path::Path;
use std::process::Output;
use std::time::Duration;

use aster_use_tool_acquire::{EnvConfig, project_env_path, write_env_acl};
#[cfg(unix)]
use aster_use_tool_acquire::{LocalApply, ToolStore};

async fn run(project: &Path, home: &Path, args: &[impl AsRef<OsStr>]) -> Output {
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_aster"));
    command
        .env_clear()
        .env("ASTER_HOME", home)
        .current_dir(project)
        .arg("--no-auto-update")
        .args(args)
        .kill_on_drop(true);
    for name in [
        "PATH",
        "SystemRoot",
        "COMSPEC",
        "PATHEXT",
        "TEMP",
        "TMP",
        "TMPDIR",
    ] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    tokio::time::timeout(Duration::from_secs(20), command.output())
        .await
        .expect("fixture command timed out")
        .expect("spawn Aster CLI")
}

#[cfg(unix)]
#[tokio::test]
async fn exec_preserves_non_utf8_process_arguments() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let temporary = tempfile::tempdir().unwrap();
    let raw = vec![b'a', 0xff, b'b'];
    let args = vec![
        OsString::from("exec"),
        OsString::from("--no-install"),
        OsString::from("--"),
        OsString::from("/bin/sh"),
        OsString::from("-c"),
        OsString::from("printf '%s' \"$1\""),
        OsString::from("sh"),
        OsString::from_vec(raw.clone()),
    ];

    let output = run(
        temporary.path(),
        &temporary.path().join("aster-home"),
        &args,
    )
    .await;

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, raw);
}

#[cfg(unix)]
#[tokio::test]
async fn exec_preserves_exit_codes_and_signal_termination() {
    use std::os::unix::process::ExitStatusExt;

    let temporary = tempfile::tempdir().unwrap();
    let home = temporary.path().join("aster-home");
    let failed = run(
        temporary.path(),
        &home,
        &["exec", "--no-install", "--", "/bin/sh", "-c", "exit 37"],
    )
    .await;
    assert_eq!(failed.status.code(), Some(37));

    let signaled = run(
        temporary.path(),
        &home,
        &[
            "exec",
            "--no-install",
            "--",
            "/bin/sh",
            "-c",
            "kill -TERM $$",
        ],
    )
    .await;
    assert_eq!(signaled.status.signal(), Some(libc::SIGTERM));
}

#[tokio::test]
async fn nested_cli_environment_inherits_user_and_ancestor_values() {
    let temporary = tempfile::tempdir().unwrap();
    let project = temporary.path().join("项目 with spaces");
    let nested = project.join("nested");
    let home = temporary.path().join("aster-home");
    std::fs::create_dir_all(&nested).unwrap();
    assert!(
        run(
            &project,
            &home,
            &["env", "set", "USER_VALUE=global", "--global"]
        )
        .await
        .status
        .success()
    );
    assert!(
        run(&project, &home, &["env", "set", "PROJECT_VALUE=root"])
            .await
            .status
            .success()
    );

    let output = run(&nested, &home, &["env", "show", "--json"]).await;

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["vars"]["USER_VALUE"], "global");
    assert_eq!(value["vars"]["PROJECT_VALUE"], "root");
}

#[tokio::test]
async fn failed_install_does_not_panic_or_replace_the_project_pin() {
    let temporary = tempfile::tempdir().unwrap();
    let project = temporary.path().join("project");
    let home = temporary.path().join("aster-home");
    let path = project_env_path(&project);
    write_env_acl(&path, &EnvConfig::default()).unwrap();
    let before = std::fs::read(&path).unwrap();

    let output = run(&project, &home, &["tool", "use", "hermes"]).await;

    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("tool runtime is missing: uv"));
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[tokio::test]
async fn invalid_variable_name_does_not_replace_the_project_file() {
    let temporary = tempfile::tempdir().unwrap();
    let project = temporary.path().join("project");
    let home = temporary.path().join("aster-home");
    let path = project_env_path(&project);
    write_env_acl(&path, &EnvConfig::default()).unwrap();
    let before = std::fs::read(&path).unwrap();

    let output = run(&project, &home, &["env", "set", "1INVALID=value"]).await;

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[tokio::test]
async fn unknown_shell_emits_no_activation_script() {
    let temporary = tempfile::tempdir().unwrap();
    let output = run(
        temporary.path(),
        &temporary.path().join("aster-home"),
        &["activate", "unknown-shell"],
    )
    .await;

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn cli_exec_and_which_select_project_versions_without_network_or_pointer_changes() {
    let temporary = tempfile::tempdir().unwrap();
    let home = temporary.path().join("aster-home");
    let store = ToolStore::open(home.join("tools")).unwrap();
    let first = temporary.path().join("项目 A");
    let second = temporary.path().join("project B");
    for (project, version) in [(&first, "1.0.0"), (&second, "2.0.0")] {
        let payload = temporary.path().join(format!("payload-{version}"));
        std::fs::write(&payload, format!("#!/bin/sh\nprintf '%s\\n' '{version}'\n")).unwrap();
        store
            .apply_local(LocalApply {
                name: "uv".into(),
                spec: "github:astral-sh/uv".into(),
                version: version.into(),
                executable_name: "uv".into(),
                payload,
                sha256: None,
                require_checksum: false,
                companions: Vec::new(),
            })
            .await
            .unwrap();
        let config = EnvConfig {
            tools: std::collections::BTreeMap::from([("uv".into(), version.into())]),
            vars: std::collections::BTreeMap::new(),
        };
        write_env_acl(project_env_path(project), &config).unwrap();
    }
    for (project, version) in [(&first, "1.0.0"), (&second, "2.0.0"), (&first, "1.0.0")] {
        for args in [
            vec!["exec", "--no-install", "--", "uv"],
            vec!["exec", "--", "uv"],
        ] {
            let output = run(project, &home, &args).await;
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), version);
        }
        let output = run(project, &home, &["tool", "which", "uv", "--json"]).await;
        assert!(output.status.success());
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let expected = store
            .root_path()
            .join("installs")
            .join("uv")
            .join(version)
            .join("uv");
        assert_eq!(value["path"].as_str().unwrap(), expected.to_str().unwrap());
    }
    assert_eq!(
        std::fs::read_to_string(store.root_path().join("current/uv")).unwrap(),
        "2.0.0\n"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn missing_pin_refuses_before_starting_the_requested_process() {
    let temporary = tempfile::tempdir().unwrap();
    let home = temporary.path().join("aster-home");
    let marker = temporary.path().join("must-not-run");
    let config = EnvConfig {
        tools: std::collections::BTreeMap::from([("uv".into(), "1.0.0".into())]),
        vars: std::collections::BTreeMap::new(),
    };
    write_env_acl(project_env_path(temporary.path()), &config).unwrap();

    let output = run(
        temporary.path(),
        &home,
        &[
            "exec",
            "--no-install",
            "--",
            "/bin/sh",
            "-c",
            "touch \"$1\"",
            "sh",
            marker.to_str().unwrap(),
        ],
    )
    .await;

    assert_eq!(output.status.code(), Some(1));
    assert!(!marker.exists());
}
