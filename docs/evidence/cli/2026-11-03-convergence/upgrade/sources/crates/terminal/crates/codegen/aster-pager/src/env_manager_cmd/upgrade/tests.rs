use super::*;
use aster_use_tool_acquire::{StaticSource, ToolPayload, VersionListing};
use clap::Parser;
use serde::Deserialize;

const SPEC: &str = "github:astral-sh/uv";

#[test]
fn upgrading_managed_tools_does_not_inspect_unrelated_external_layouts() {
    let temporary = tempfile::tempdir().unwrap();
    let source = source();
    let mut context = context(temporary.path(), &["latest"], &source);
    let path = project_env_path(&context.project);
    let mut file = read_env_declarations(&path).unwrap().unwrap();
    file.tools
        .insert("bun".into(), vec!["path:./missing external layout".into()]);
    file.tools.insert("fd".into(), vec!["system".into()]);
    write_env_declarations(&path, &file).unwrap();
    context.declarations =
        load_env_declarations_with_sources(&context.project, &context.aster_home).unwrap();
    upgrade(
        &context,
        &UpgradeArgs {
            bump: true,
            ..Default::default()
        },
        Some(&source),
    )
    .unwrap();
    let after = read_env_declarations(&path).unwrap().unwrap();
    assert_eq!(after.tools.get("bun"), file.tools.get("bun"));
    assert_eq!(after.tools.get("fd"), file.tools.get("fd"));
    assert_eq!(
        context
            .store
            .installed_receipt("uv", None)
            .unwrap()
            .unwrap()
            .version,
        "2.0.0"
    );
}

#[derive(Parser)]
struct Arguments {
    #[command(flatten)]
    upgrade: UpgradeArgs,
}

fn source() -> StaticSource {
    let mut source = StaticSource::new();
    for (version, digest) in [
        (
            "1.0.0",
            "f343fbc16ea16f0bc1a60feeb05f5a59f42333082d3c33606f0798bb706116c3",
        ),
        (
            "1.5.0",
            "a5f89ff0957da4fec1a1c40013adae5af1661c7156c272bfead8167ac34a6b5d",
        ),
        (
            "2.0.0",
            "e29402bae469af99f3697ae70763e38998b4aac6e7f9d9982ba3c350af605782",
        ),
    ] {
        source.insert(
            SPEC,
            ToolPayload::new(
                version,
                digest,
                format!("#!/bin/sh\nprintf '%s\\n' '{version}'\n").into_bytes(),
            ),
        );
    }
    source.insert_versions(
        SPEC,
        VersionListing::new(
            ["1.0.0", "1.5.0", "2.0.0"]
                .map(|v| (v.into(), false))
                .into(),
            BTreeMap::new(),
        )
        .unwrap(),
    );
    source
}

fn context(root: &Path, requests: &[&str], source: &StaticSource) -> EnvContext {
    let home = root.join("home 中文");
    let project = root.join("project 中文");
    let store = ToolStore::open(home.join("tools")).unwrap();
    store
        .acquire_with(
            AcquireRequest {
                name: "uv".into(),
                spec: SPEC.into(),
                version: Some("1.0.0".into()),
                executable_name: "uv".into(),
            },
            source,
        )
        .unwrap();
    write_env_declarations(
        project_env_path(&project),
        &EnvDeclarations {
            tools: BTreeMap::from([("uv".into(), requests.iter().map(|r| (*r).into()).collect())]),
            vars: BTreeMap::from([("KEEP".into(), "original".into())]),
        },
    )
    .unwrap();
    let mut context = load_env_context(home, project, store).unwrap();
    context.user_home = Some(root.to_path_buf());
    context
}

#[test]
fn upgrades_match_the_twelve_frozen_native_mise_cases() {
    #[derive(Deserialize)]
    struct Reference {
        status: String,
        cases: Vec<Case>,
    }
    #[derive(Deserialize)]
    struct Case {
        name: String,
        request: String,
        flags: Vec<String>,
        pin: String,
        retained: Vec<String>,
        config_unchanged: bool,
        exit: i32,
    }
    let frozen: Reference = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../specs/031-cli-mise-replacement/contracts/mise-upgrade-reference.json"
    )))
    .unwrap();
    assert_eq!(frozen.status, "passed");
    assert_eq!(frozen.cases.len(), 12);
    for case in frozen.cases {
        let temporary = tempfile::tempdir().unwrap();
        let source = source();
        let context = context(temporary.path(), &[&case.request], &source);
        let path = project_env_path(&context.project);
        let before = std::fs::read(&path).unwrap();
        let args = Arguments::try_parse_from(
            std::iter::once("upgrade".to_string())
                .chain(case.flags.iter().map(|v| v.replace("fixture", "uv"))),
        )
        .unwrap()
        .upgrade;
        let upgrades = upgrade(&context, &args, Some(&source)).unwrap();
        let observed_exit = i32::from(args.dry_run_code && upgrades.iter().any(|u| u.changed));
        assert_eq!(observed_exit, case.exit, "{}", case.name);
        let file = read_env_declarations(&path).unwrap().unwrap();
        assert_eq!(file.tools.get("uv").unwrap(), &[case.pin], "{}", case.name);
        assert_eq!(file.vars.get("KEEP").unwrap(), "original");
        assert_eq!(
            std::fs::read(&path).unwrap() == before,
            case.config_unchanged,
            "{}",
            case.name
        );
        let installed = context.store.installed_versions("uv").unwrap();
        assert_eq!(
            installed
                .iter()
                .map(|r| r.version.clone())
                .collect::<Vec<_>>(),
            case.retained,
            "{}",
            case.name
        );
        #[cfg(unix)]
        for receipt in installed {
            let paths = context.store.installation_paths(&receipt).unwrap();
            let output = ProcessCommand::new(paths.executable()).output().unwrap();
            assert!(output.status.success(), "{}", case.name);
            assert_eq!(
                output.stdout,
                format!("{}\n", receipt.version).as_bytes(),
                "{}",
                case.name
            );
        }
    }
}

#[test]
fn failed_upgrade_keeps_the_previous_executable_pointer_and_declaration() {
    let temporary = tempfile::tempdir().unwrap();
    let mut source = source();
    let context = context(temporary.path(), &["1"], &source);
    let path = project_env_path(&context.project);
    let before = std::fs::read(&path).unwrap();
    source.insert(
        SPEC,
        ToolPayload::new("2.0.0", "0".repeat(64), b"bad update".to_vec()),
    );
    let error = upgrade(
        &context,
        &UpgradeArgs {
            bump: true,
            ..Default::default()
        },
        Some(&source),
    )
    .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<ToolAcquireError>(),
        Some(ToolAcquireError::ChecksumMismatch)
    ));
    assert_eq!(std::fs::read(path).unwrap(), before);
    assert_eq!(
        context.store.list_sync().unwrap().first().unwrap().version,
        "1.0.0"
    );
    assert!(
        context
            .store
            .installed_receipt("uv", Some("2.0.0"))
            .unwrap()
            .is_none()
    );
    #[cfg(unix)]
    {
        let output = ProcessCommand::new(context.store.which("uv").unwrap().unwrap())
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, b"1.0.0\n");
    }
}

#[test]
fn bump_updates_the_owning_parent_file_and_prune_keeps_another_projects_pin() {
    let temporary = tempfile::tempdir().unwrap();
    let source = source();
    let mut context = context(temporary.path(), &["1"], &source);
    let owner = project_env_path(&context.project);
    let child = context.project.join("child 中文");
    let other = temporary.path().join("other project");
    write_env_declarations(
        project_env_path(&other),
        &EnvDeclarations {
            tools: BTreeMap::from([("uv".into(), vec!["1.0.0".into()])]),
            vars: BTreeMap::new(),
        },
    )
    .unwrap();
    load_env_declarations_with_sources(&other, &context.aster_home)
        .unwrap()
        .track_sources(&context.store)
        .unwrap();
    context.project = child;
    context.declarations =
        load_env_declarations_with_sources(&context.project, &context.aster_home).unwrap();
    upgrade(
        &context,
        &UpgradeArgs {
            bump: true,
            prune: true,
            ..Default::default()
        },
        Some(&source),
    )
    .unwrap();
    assert_eq!(
        read_env_declarations(owner)
            .unwrap()
            .unwrap()
            .tools
            .get("uv")
            .unwrap(),
        &["2"]
    );
    assert!(!project_env_path(&context.project).exists());
    assert!(
        context
            .store
            .installed_receipt("uv", Some("1.0.0"))
            .unwrap()
            .is_some()
    );
    assert!(
        context
            .store
            .installed_receipt("uv", Some("2.0.0"))
            .unwrap()
            .is_some()
    );
}

#[test]
fn offline_missing_target_and_invalid_bump_refuse_before_publication() {
    let temporary = tempfile::tempdir().unwrap();
    let source = source();
    let context = context(temporary.path(), &["1"], &source);
    context
        .store
        .remote_versions_with(&parse_tool_spec(SPEC).unwrap(), Some(&source))
        .unwrap();
    let path = project_env_path(&context.project);
    let before = std::fs::read(&path).unwrap();
    assert!(
        upgrade(
            &context,
            &UpgradeArgs {
                bump: true,
                offline: true,
                ..Default::default()
            },
            None
        )
        .is_err()
    );
    assert_eq!(std::fs::read(path).unwrap(), before);
    assert_eq!(
        context.store.list_sync().unwrap().first().unwrap().version,
        "1.0.0"
    );
    assert!(bumped_request(&VersionRequest::parse("prefix:1").unwrap(), "2.0.0").is_err());
}
