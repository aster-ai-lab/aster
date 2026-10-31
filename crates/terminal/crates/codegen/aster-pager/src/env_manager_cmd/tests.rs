use super::*;

#[test]
fn argument_debug_and_containing_commands_redact_values() {
    use crate::app::Command;
    let secret = "synthetic-argument-secret";
    let commands = [
        Command::Env(EnvArgs {
            command: EnvCommand::Set {
                assignment: format!("TOKEN={secret}"),
                global: false,
                json: false,
            },
        }),
        Command::Exec(ExecArgs {
            no_install: true,
            command: vec![secret.into()],
        }),
        Command::Use(UseForwardArgs {
            args: vec![secret.into()],
        }),
    ];

    for command in commands {
        let debug = format!("{command:?}");
        assert!(!debug.contains(secret));
        assert!(debug.contains("redacted"));
    }
}

#[tokio::test]
async fn install_failure_returns_typed_error_without_panicking_in_async_context() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("tools")).unwrap();
    let resolved = resolve_tools(&EnvConfig {
        tools: BTreeMap::from([("hermes".into(), "latest".into())]),
        vars: BTreeMap::new(),
    })
    .unwrap();

    let error = acquire_all(&store, &resolved).await.unwrap_err();

    assert!(matches!(
        error.downcast_ref::<aster_use_tool_acquire::ToolAcquireError>(),
        Some(aster_use_tool_acquire::ToolAcquireError::RuntimeMissing(runtime)) if runtime == "uv"
    ));
    assert!(store.list_sync().unwrap().is_empty());
}

#[tokio::test]
async fn empty_tool_declaration_does_not_create_an_http_source() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("tools")).unwrap();

    assert!(acquire_all(&store, &[]).await.unwrap().is_empty());
    assert!(store.list_sync().unwrap().is_empty());
}

#[test]
fn activation_bash_exports_path_and_vars() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("tools")).unwrap();
    let config = EnvConfig {
        tools: BTreeMap::new(),
        vars: BTreeMap::from([("NODE_ENV".into(), "development".into())]),
    };
    let script = activation_script("bash", &store, &config).unwrap();
    assert!(script.contains("export PATH="));
    assert!(script.contains("export NODE_ENV='development'"));
    assert!(!script.contains("$PATH"));
}

#[test]
fn unsupported_activation_shell_is_refused() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("tools")).unwrap();

    assert!(activation_script("unknown-shell", &store, &EnvConfig::default()).is_err());
}

#[cfg(unix)]
#[test]
fn activation_preserves_shell_metacharacters_as_literal_variable_values() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("tools")).unwrap();
    let value = "a'b\\c\n$HOME $(printf must-stay-literal)";
    let config = EnvConfig {
        tools: BTreeMap::new(),
        vars: BTreeMap::from([("ASTER_LITERAL".into(), value.into())]),
    };
    let script = activation_script("sh", &store, &config).unwrap();

    let output = ProcessCommand::new("/bin/sh")
        .arg("-c")
        .arg(format!("{script}\nprintf '%s' \"$ASTER_LITERAL\""))
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), value);
}

#[test]
fn fish_values_escape_both_backslashes_and_single_quotes() {
    assert_eq!(fish_quote("a\\'b"), "'a\\\\\\'b'");
}

#[cfg(unix)]
#[tokio::test]
async fn project_execution_uses_its_pin_after_another_version_is_installed() {
    use aster_use_tool_acquire::LocalApply;

    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("store with 空格")).unwrap();
    for version in ["1.0.0", "2.0.0"] {
        let payload = temporary.path().join(format!("uv-{version}"));
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
    }
    for version in ["1.0.0", "2.0.0", "1.0.0"] {
        let config = EnvConfig {
            tools: BTreeMap::from([("uv".into(), version.into())]),
            vars: BTreeMap::from([("PATH".into(), "/must-not-bypass-the-project-pin".into())]),
        };
        let output = ProcessCommand::new("uv")
            .envs(runtime_env(&store, &config).unwrap())
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), version);
        let activation = activation_script("sh", &store, &config).unwrap();
        let output = ProcessCommand::new("/bin/sh")
            .arg("-c")
            .arg(format!("{activation}\nuv"))
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), version);
    }
    assert_eq!(
        std::fs::read_to_string(store.root_path().join("current/uv")).unwrap(),
        "2.0.0\n"
    );
}

#[tokio::test]
async fn exact_installed_pin_is_reused_without_a_download() {
    use aster_use_tool_acquire::LocalApply;

    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("tools")).unwrap();
    let payload = temporary.path().join("payload");
    std::fs::write(&payload, "fixture executable").unwrap();
    store
        .apply_local(LocalApply {
            name: "hermes".into(),
            spec: "pipx:hermes-agent".into(),
            version: "1.0.0".into(),
            executable_name: "hermes".into(),
            payload,
            sha256: None,
            require_checksum: false,
            companions: Vec::new(),
        })
        .await
        .unwrap();
    let config = EnvConfig {
        tools: BTreeMap::from([("hermes".into(), "1.0.0".into())]),
        vars: BTreeMap::new(),
    };

    let records = acquire_all(&store, &resolve_tools(&config).unwrap())
        .await
        .unwrap();

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].version, "1.0.0");
}

#[test]
fn missing_declared_pin_does_not_fall_back_to_an_unrelated_path() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("tools")).unwrap();
    let config = EnvConfig {
        tools: BTreeMap::from([("uv".into(), "1.0.0".into())]),
        vars: BTreeMap::new(),
    };

    assert!(runtime_env(&store, &config).is_err());
}

#[test]
fn split_tool_pin_defaults_latest() {
    let (name, version) = split_tool_pin("uv").unwrap();
    assert_eq!(name, "uv");
    assert_eq!(version, "latest");
    let (name, version) = split_tool_pin("uv@0.6.0").unwrap();
    assert_eq!(name, "uv");
    assert_eq!(version, "0.6.0");
}

#[test]
fn clap_accepts_tool_env_exec_activate_use() {
    use crate::app::{Command, PagerArgs};
    use clap::Parser;

    assert!(matches!(
        PagerArgs::try_parse_from(["aster", "tool", "ls", "--json"])
            .unwrap()
            .command,
        Some(Command::Tool(_))
    ));
    assert!(matches!(
        PagerArgs::try_parse_from(["aster", "env", "show"])
            .unwrap()
            .command,
        Some(Command::Env(_))
    ));
    assert!(matches!(
        PagerArgs::try_parse_from(["aster", "exec", "--", "uv", "--version"])
            .unwrap()
            .command,
        Some(Command::Exec(_))
    ));
    assert!(matches!(
        PagerArgs::try_parse_from(["aster", "activate", "zsh"])
            .unwrap()
            .command,
        Some(Command::Activate(_))
    ));
    assert!(matches!(
        PagerArgs::try_parse_from(["aster", "use", "plugin", "list"])
            .unwrap()
            .command,
        Some(Command::Use(_))
    ));
}
