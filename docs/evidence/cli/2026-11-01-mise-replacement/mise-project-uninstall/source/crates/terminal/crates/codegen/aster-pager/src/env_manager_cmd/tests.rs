use super::*;

fn configured_context(root: &Path, store: &ToolStore, config: &EnvConfig) -> EnvContext {
    let aster_home = root.join("isolated-user-config");
    write_env_acl(project_env_path(root), config).unwrap();
    let declarations = load_env_declarations_with_sources(root, &aster_home).unwrap();
    EnvContext {
        aster_home,
        project: root.to_path_buf(),
        store: store.clone(),
        declarations,
        user_home: Some(root.to_path_buf()),
    }
}

#[test]
fn argument_debug_and_containing_commands_redact_values() {
    use crate::app::Command;
    let secret = "synthetic-argument-secret";
    let commands = [
        Command::Tool(ToolArgs {
            command: ToolCommand::ShimExec(ShimExecArgs {
                home: PathBuf::from("fixture home"),
                tool: "node".into(),
                command: vec![secret.into()],
            }),
        }),
        Command::Env(EnvArgs {
            command: EnvCommand::Set {
                assignment: format!("TOKEN={secret}"),
                global: false,
                json: false,
            },
        }),
        Command::Exec(ExecArgs {
            tools: vec![secret.into()],
            no_install: true,
            offline: false,
            locked: false,
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

    let error = acquire_all(&store, &resolved, false).await.unwrap_err();

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

    assert!(acquire_all(&store, &[], false).await.unwrap().is_empty());
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
    let context = configured_context(temporary.path(), &store, &config);
    let script = activation_script("bash", &context).unwrap();
    assert!(script.contains("export PATH="));
    assert!(script.contains("export NODE_ENV='development'"));
    assert!(!script.contains("$PATH"));
}

#[test]
fn unsupported_activation_shell_is_refused() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("tools")).unwrap();

    let context = configured_context(temporary.path(), &store, &EnvConfig::default());
    assert!(activation_script("unknown-shell", &context).is_err());
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
    let context = configured_context(temporary.path(), &store, &config);
    let script = activation_script("sh", &context).unwrap();

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
        let context = configured_context(temporary.path(), &store, &config);
        let output = ProcessCommand::new("uv")
            .envs(runtime_env(&context).unwrap())
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), version);
        let activation = activation_script("sh", &context).unwrap();
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

    let records = acquire_all(&store, &resolve_tools(&config).unwrap(), false)
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

    let context = configured_context(temporary.path(), &store, &config);
    assert!(runtime_env(&context).is_err());
}

#[tokio::test]
async fn path_and_system_installs_need_no_managed_receipt_or_http_client() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("tools")).unwrap();
    std::fs::create_dir_all(temporary.path().join("SDK/bin")).unwrap();
    for (request, origin) in [
        ("path:./SDK", ExternalOrigin::Path),
        ("system", ExternalOrigin::System),
    ] {
        let config = EnvConfig {
            tools: BTreeMap::from([("node".into(), request.into())]),
            vars: BTreeMap::new(),
        };
        let context = configured_context(temporary.path(), &store, &config);
        for offline in [false, true] {
            let records = install_env_tools(&context, offline, true, BTreeMap::new())
                .await
                .unwrap();
            assert_eq!(records.len(), 1);
            assert_eq!(records.first().unwrap().origin, Some(origin));
            assert_eq!(records.first().unwrap().version, request);
        }
    }
    assert!(!store.root_path().join("receipts").exists());
    assert!(!store.root_path().join("current").exists());
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

#[tokio::test]
async fn batch_install_arguments_preserve_declarations_origins_and_request_order() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("tools")).unwrap();
    std::fs::create_dir_all(temporary.path().join("SDK node/bin")).unwrap();
    let context = configured_context(
        temporary.path(),
        &store,
        &EnvConfig {
            tools: BTreeMap::from([
                ("uv".into(), "1.0.0".into()),
                ("node".into(), "path:./SDK node".into()),
                ("fd".into(), "system".into()),
            ]),
            vars: BTreeMap::new(),
        },
    );
    let selections = resolve_tool_arguments(
        &context,
        ["uv", "node", "fd", "uv", "uv@2.0.0"]
            .map(String::from)
            .to_vec(),
        ToolArgumentMode::Install,
    )
    .await
    .unwrap();
    assert_eq!(
        selections
            .iter()
            .map(|s| (s.tool().name.as_str(), s.tool().version.as_deref()))
            .collect::<Vec<_>>(),
        [
            ("uv", Some("1.0.0")),
            ("node", Some("path:./SDK node")),
            ("fd", Some("system")),
            ("uv", Some("2.0.0"))
        ]
    );
    let EnvToolOrigin::Path(paths) = selections[1].origin() else {
        panic!("expected the declaration's external path");
    };
    assert_eq!(
        paths.root(),
        temporary.path().join("SDK node").canonicalize().unwrap()
    );
    assert!(matches!(selections[2].origin(), EnvToolOrigin::System));
    assert!(store.list_sync().unwrap().is_empty());
}

#[tokio::test]
async fn batch_arguments_reject_later_invalid_requests_before_installation() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("tools")).unwrap();
    let context = configured_context(temporary.path(), &store, &EnvConfig::default());
    for arguments in [["uv@1.0.0", "unknown-tool"], ["uv@1.0.0", "fd@"]] {
        assert!(
            resolve_tool_arguments(
                &context,
                arguments.map(String::from).to_vec(),
                ToolArgumentMode::Use
            )
            .await
            .is_err()
        );
    }
    let selected = resolve_tool_arguments(
        &context,
        ["uv@1.0.0", "uv@2.0.0"].map(String::from).to_vec(),
        ToolArgumentMode::Use,
    )
    .await
    .unwrap();
    assert_eq!(
        selected
            .iter()
            .map(|tool| tool.tool().version.as_deref().unwrap())
            .collect::<Vec<_>>(),
        ["1.0.0", "2.0.0"]
    );
    assert!(store.list_sync().unwrap().is_empty());
}

#[tokio::test]
async fn bare_use_reuses_installed_versions_while_explicit_latest_remains_a_remote_request() {
    use aster_use_tool_acquire::{LocalApply, StaticSource, VersionListing};

    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("tools")).unwrap();
    for version in ["1.0.0", "2.0.0"] {
        let payload = temporary.path().join(format!("payload-{version}"));
        std::fs::write(&payload, "fixture executable").unwrap();
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
    let mut source = StaticSource::new();
    source.insert_versions(
        "github:astral-sh/uv",
        VersionListing::new(
            vec![
                ("1.0.0".into(), false),
                ("2.0.0".into(), false),
                ("3.0.0".into(), false),
            ],
            BTreeMap::from([("latest".into(), "3.0.0".into())]),
        )
        .unwrap(),
    );
    store
        .remote_versions_with(
            &parse_tool_spec("github:astral-sh/uv").unwrap(),
            Some(&source),
        )
        .unwrap();
    let context = configured_context(
        temporary.path(),
        &store,
        &EnvConfig {
            tools: BTreeMap::from([("uv".into(), "1.0.0".into())]),
            vars: BTreeMap::new(),
        },
    );
    let bare = resolve_tool_arguments(&context, vec!["uv".into()], ToolArgumentMode::Use)
        .await
        .unwrap();
    assert_eq!(bare[0].tool().version.as_deref(), Some("2.0.0"));
    let latest = resolve_tool_arguments(&context, vec!["uv@latest".into()], ToolArgumentMode::Use)
        .await
        .unwrap();
    assert!(latest[0].tool().version.is_none());
    assert_eq!(
        std::fs::read_to_string(store.root_path().join("current/uv")).unwrap(),
        "2.0.0\n"
    );
}

#[test]
fn clap_accepts_tool_env_exec_activate_use() {
    use crate::app::{Command, PagerArgs};
    use clap::Parser;
    assert!(matches!(
        PagerArgs::try_parse_from(["aster", "tool", "ls-remote", "uv", "--offline"])
            .unwrap()
            .command,
        Some(Command::Tool(_))
    ));

    assert!(matches!(
        PagerArgs::try_parse_from(["aster", "tool", "lock", "--offline"])
            .unwrap()
            .command,
        Some(Command::Tool(_))
    ));
    assert!(PagerArgs::try_parse_from(["aster", "tool", "install", "uv", "--locked"]).is_err());
    assert!(
        PagerArgs::try_parse_from(["aster", "tool", "uninstall", "--project", "--yes"]).is_ok()
    );
    for flag in ["--all", "--unpin", "uv"] {
        assert!(
            PagerArgs::try_parse_from(["aster", "tool", "uninstall", "--project", flag]).is_err()
        );
    }
    for subcommand in ["install", "use"] {
        let Some(Command::Tool(args)) = PagerArgs::try_parse_from([
            "aster",
            "tool",
            subcommand,
            "uv@1.0.0",
            "fd@10.0.0",
            "--json",
        ])
        .unwrap()
        .command
        else {
            panic!("expected tool arguments");
        };
        let tools = match args.command {
            ToolCommand::Install { tool, .. } | ToolCommand::Use { tool, .. } => tool,
            _ => panic!("expected install or use arguments"),
        };
        assert_eq!(tools, ["uv@1.0.0", "fd@10.0.0"]);
    }
    assert!(PagerArgs::try_parse_from(["aster", "tool", "use"]).is_err());
    assert!(matches!(
        PagerArgs::try_parse_from(["aster", "exec", "--locked", "--offline", "--", "uv"])
            .unwrap()
            .command,
        Some(Command::Exec(_))
    ));
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

#[test]
fn exec_overrides_are_explicit_and_leave_command_arguments_intact() {
    use crate::app::{Command, PagerArgs};
    use clap::Parser;
    let parsed = PagerArgs::try_parse_from([
        "aster",
        "exec",
        "--tool",
        "node@system",
        "-t",
        "uv@1.2",
        "--",
        "node",
        "--tool",
        "literal",
    ])
    .unwrap();
    let Some(Command::Exec(args)) = parsed.command else {
        panic!("expected exec arguments")
    };
    assert_eq!(args.tools, ["node@system", "uv@1.2"]);
    assert_eq!(
        args.command,
        ["node", "--tool", "literal"].map(OsString::from)
    );
    assert!(
        PagerArgs::try_parse_from([
            "aster",
            "exec",
            "--locked",
            "--tool",
            "node@system",
            "--",
            "node",
        ])
        .is_err()
    );
}

#[tokio::test]
async fn direct_locked_exec_refuses_overrides_before_opening_a_context() {
    let error = run_exec(ExecArgs {
        tools: vec!["node@system".into()],
        no_install: true,
        offline: true,
        locked: true,
        command: vec!["unused-command".into()],
    })
    .await
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "temporary tool overrides cannot be used with a project lock"
    );
}
