use super::*;
use tempfile::tempdir;

#[test]
fn nearest_project_discovery_stops_at_empty_or_invalid_files_and_ignores_outer_layers() {
    let temporary = tempdir().unwrap();
    let parent = temporary.path().join("parent");
    let project = parent.join("项目 with spaces");
    let nested = project.join("src/nested");
    fs::create_dir_all(&nested).unwrap();
    fs::create_dir_all(project_env_path(&parent).parent().unwrap()).unwrap();
    fs::write(project_env_path(&parent), "invalid outer declaration").unwrap();
    write_env_declarations(project_env_path(&project), &EnvDeclarations::default()).unwrap();
    assert_eq!(
        nearest_project_env_declarations(&nested).unwrap(),
        Some((project.clone(), EnvDeclarations::default()))
    );
    fs::write(project_env_path(&project), "invalid nearest declaration").unwrap();
    assert!(matches!(
        nearest_project_env_declarations(&nested),
        Err(ToolAcquireError::InvalidEnvAcl(_))
    ));
    fs::remove_file(project_env_path(&project)).unwrap();
    write_env_declarations(project_env_path(&parent), &EnvDeclarations::default()).unwrap();
    assert_eq!(
        nearest_project_env_declarations(&nested).unwrap(),
        Some((parent.clone(), EnvDeclarations::default()))
    );
    fs::remove_file(project_env_path(&parent)).unwrap();
    assert!(nearest_project_env_declarations(&nested).unwrap().is_none());
}

#[test]
fn ordered_declarations_round_trip_without_losing_versions_or_scalar_compatibility() {
    let declarations = parse_env_declarations(
        r#"env {
      tools { tool "uv" { version = ["2.0.0", "1.0.0"] } tool "fd" { version = "10.0.0" } }
      vars { var "TOKEN" { value = "synthetic-multi-version-secret" } }
    }"#,
    )
    .unwrap();
    assert_eq!(declarations.tools.get("uv").unwrap(), &["2.0.0", "1.0.0"]);
    assert_eq!(
        parse_env_declarations(&render_env_declarations(&declarations)).unwrap(),
        declarations
    );
    assert_eq!(
        resolve_declarations(&declarations)
            .unwrap()
            .into_iter()
            .map(|tool| (tool.name, tool.version.unwrap()))
            .collect::<Vec<_>>(),
        [
            ("fd".into(), "10.0.0".into()),
            ("uv".into(), "2.0.0".into()),
            ("uv".into(), "1.0.0".into())
        ]
    );
    assert!(parse_env_acl(&render_env_declarations(&declarations)).is_err());
    let scalar = parse_env_acl(r#"env { tools { tool "uv" { version = "1.0.0" } } }"#).unwrap();
    assert_eq!(
        EnvDeclarations::from(&scalar),
        parse_env_declarations(&render_env_acl(&scalar)).unwrap()
    );
    for debug in [
        format!("{declarations:?}"),
        format!("{:?}", Some(&declarations)),
    ] {
        assert!(!debug.contains("synthetic-multi-version-secret"));
        assert!(debug.contains("redacted"));
    }
}

#[test]
fn malformed_version_lists_and_invalid_variable_writes_preserve_the_file() {
    for version in ["[]", "[\"1.0.0\", true]", "[[\"1.0.0\"]]", "true", "[\"\"]"] {
        assert!(
            parse_env_declarations(&format!(
                "env {{ tools {{ tool \"uv\" {{ version = {version} }} }} }}"
            ))
            .is_err(),
            "{version}"
        );
    }
    let temporary = tempdir().unwrap();
    let path = temporary.path().join("配置 env.acl");
    fs::write(&path, "env {}\n").unwrap();
    for config in [
        EnvDeclarations {
            tools: BTreeMap::from([("uv".into(), Vec::new())]),
            vars: BTreeMap::new(),
        },
        EnvDeclarations {
            tools: BTreeMap::new(),
            vars: BTreeMap::from([("1INVALID".into(), "value".into())]),
        },
    ] {
        assert!(write_env_declarations(&path, &config).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "env {}\n");
    }
}

#[cfg(unix)]
#[test]
fn atomic_environment_write_preserves_existing_file_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let temporary = tempdir().unwrap();
    let path = temporary.path().join("env.acl");
    write_env_acl(&path, &EnvConfig::default()).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();

    write_env_acl(
        &path,
        &EnvConfig {
            tools: BTreeMap::new(),
            vars: BTreeMap::from([("MARKER".into(), "updated".into())]),
        },
    )
    .unwrap();

    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[cfg(unix)]
#[test]
fn atomic_environment_write_updates_the_referent_and_preserves_the_symlink() {
    let temporary = tempdir().unwrap();
    let relative_target = Path::new("shared env").join("配置.acl");
    let target = temporary.path().join(&relative_target);
    let link = temporary.path().join("env.acl");
    write_env_acl(&target, &EnvConfig::default()).unwrap();
    std::os::unix::fs::symlink(&relative_target, &link).unwrap();
    let expected = EnvConfig {
        tools: BTreeMap::new(),
        vars: BTreeMap::from([("MARKER".into(), "updated".into())]),
    };

    write_env_acl(&link, &expected).unwrap();

    assert_eq!(fs::read_link(&link).unwrap(), relative_target);
    assert_eq!(read_env_acl(&target).unwrap().unwrap(), expected);
    assert_eq!(read_env_acl(&link).unwrap().unwrap(), expected);
}

#[cfg(unix)]
#[test]
fn atomic_environment_write_follows_a_dangling_file_symlink() {
    let temporary = tempdir().unwrap();
    let target = temporary.path().join("target.acl");
    let link = temporary.path().join("env.acl");
    std::os::unix::fs::symlink(Path::new("target.acl"), &link).unwrap();

    write_env_acl(&link, &EnvConfig::default()).unwrap();

    assert!(fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(
        read_env_acl(&target).unwrap().unwrap(),
        EnvConfig::default()
    );
}

#[test]
fn environment_debug_and_containing_debug_redact_values() {
    let config = EnvConfig {
        tools: BTreeMap::new(),
        vars: BTreeMap::from([("TOKEN".into(), "synthetic-secret-marker".into())]),
    };
    for debug in [format!("{config:?}"), format!("{:?}", Some(&config))] {
        assert!(!debug.contains("synthetic-secret-marker"));
        assert!(debug.contains("redacted"));
    }
}

#[test]
fn invalid_variable_name_does_not_replace_an_existing_declaration() {
    let temporary = tempdir().unwrap();
    let path = temporary.path().join("env.acl");
    fs::write(&path, "env {}\n").unwrap();
    let config = EnvConfig {
        tools: BTreeMap::new(),
        vars: BTreeMap::from([("1INVALID".into(), "value".into())]),
    };

    assert!(matches!(
        write_env_acl(&path, &config),
        Err(ToolAcquireError::InvalidEnvAcl(_))
    ));
    assert_eq!(fs::read_to_string(&path).unwrap(), "env {}\n");
}

#[test]
fn ancestor_layers_apply_before_the_nearest_project() {
    let temporary = tempdir().unwrap();
    let home = temporary.path().join("aster-home");
    let project = temporary.path().join("项目 with spaces");
    let nested = project.join("nested");
    fs::create_dir_all(&nested).unwrap();
    for (path, vars) in [
        (
            user_env_path(&home),
            [("SHARED", "user"), ("USER_ONLY", "keep")],
        ),
        (
            project_env_path(&project),
            [("SHARED", "root"), ("ROOT_ONLY", "keep")],
        ),
        (
            project_env_path(&nested),
            [("SHARED", "nested"), ("NESTED_ONLY", "keep")],
        ),
    ] {
        write_env_acl(
            &path,
            &EnvConfig {
                tools: BTreeMap::new(),
                vars: vars
                    .into_iter()
                    .map(|(key, value)| (key.into(), value.into()))
                    .collect(),
            },
        )
        .unwrap();
    }

    let loaded = load_env_config(&nested, &home).unwrap();

    assert_eq!(
        loaded.vars.get("SHARED").map(String::as_str),
        Some("nested")
    );
    for name in ["USER_ONLY", "ROOT_ONLY", "NESTED_ONLY"] {
        assert_eq!(loaded.vars.get(name).map(String::as_str), Some("keep"));
    }
}

#[test]
fn unknown_top_level_blocks_are_refused_alongside_env() {
    let error = parse_env_acl("env {}\nunknown {}\n").unwrap_err();
    assert!(matches!(error, ToolAcquireError::InvalidEnvAcl(_)));
}

#[test]
fn oversized_acl_is_refused_in_memory_and_on_disk() {
    let temporary = tempdir().unwrap();
    let project = temporary.path().join("project");
    let home = temporary.path().join("aster-home");
    let path = project_env_path(&project);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let oversized = format!("// {}\nenv {{}}\n", "x".repeat(1024 * 1024));
    fs::write(&path, &oversized).unwrap();

    assert!(matches!(
        parse_env_acl(&oversized),
        Err(ToolAcquireError::InvalidEnvAcl(_))
    ));
    assert!(matches!(
        load_env_config(&project, &home),
        Err(ToolAcquireError::InvalidEnvAcl(_))
    ));
}

#[test]
fn a_directory_at_the_config_path_is_an_io_error() {
    let temporary = tempdir().unwrap();
    let project = temporary.path().join("project");
    fs::create_dir_all(project_env_path(&project)).unwrap();

    let error = load_env_config(&project, temporary.path().join("aster-home")).unwrap_err();
    assert!(matches!(error, ToolAcquireError::Io { .. }));
}

#[test]
fn parses_labeled_tools_and_vars() {
    let config = parse_env_acl(
        r#"
env {
  tools {
tool "uv" {
  version = "latest"
}
tool "kbox" {
  version = "0.1.10"
}
  }
  vars {
var "NODE_ENV" {
  value = "development"
}
  }
}
"#,
    )
    .unwrap();
    assert_eq!(config.tools.get("uv").map(String::as_str), Some("latest"));
    assert_eq!(
        config.tools.get("kbox").map(String::as_str),
        Some("0.1.10")
    );
    assert_eq!(
        config.vars.get("NODE_ENV").map(String::as_str),
        Some("development")
    );
}

#[test]
fn rejects_unknown_child_blocks() {
    let error = parse_env_acl(
        r#"
env {
  tasks {}
}
"#,
    )
    .unwrap_err();
    assert!(matches!(error, ToolAcquireError::InvalidEnvAcl(_)));
}

#[test]
fn resolve_requires_catalog_names() {
    let config = parse_env_acl(
        r#"
env {
  tools {
tool "not-a-real-tool" {
  version = "1"
}
  }
}
"#,
    )
    .unwrap();
    let error = resolve_tools(&config).unwrap_err();
    assert!(matches!(error, ToolAcquireError::UnknownEnvTool(_)));
}

#[test]
fn round_trip_write_and_load_merges_layers() {
    let temporary = tempdir().unwrap();
    let aster_home = temporary.path().join("aster-home");
    let project = temporary.path().join("project");
    fs::create_dir_all(&aster_home).unwrap();
    fs::create_dir_all(project.join(".aster")).unwrap();

    write_env_acl(
        user_env_path(&aster_home),
        &EnvConfig {
            tools: BTreeMap::from([("uv".into(), "0.1.0".into())]),
            vars: BTreeMap::from([("A".into(), "user".into()), ("B".into(), "keep".into())]),
        },
    )
    .unwrap();
    write_env_acl(
        project_env_path(&project),
        &EnvConfig {
            tools: BTreeMap::from([("gh".into(), "latest".into())]),
            vars: BTreeMap::from([("A".into(), "project".into())]),
        },
    )
    .unwrap();

    let merged = load_env_config(&project, &aster_home).unwrap();
    assert_eq!(merged.tools.get("uv").map(String::as_str), Some("0.1.0"));
    assert_eq!(merged.tools.get("gh").map(String::as_str), Some("latest"));
    assert_eq!(merged.vars.get("A").map(String::as_str), Some("project"));
    assert_eq!(merged.vars.get("B").map(String::as_str), Some("keep"));
}

#[test]
fn resolve_maps_catalog_and_latest() {
    let config = EnvConfig {
        tools: BTreeMap::from([
            ("uv".into(), "latest".into()),
            ("kbox".into(), "0.1.10".into()),
        ]),
        vars: BTreeMap::new(),
    };
    let resolved = resolve_tools(&config).unwrap();
    assert_eq!(resolved.len(), 2);
    let uv = resolved.iter().find(|entry| entry.name == "uv").unwrap();
    assert!(uv.version.is_none());
    assert_eq!(uv.spec, "github:astral-sh/uv");
    let sandbox = resolved
        .iter()
        .find(|entry| entry.name == "kbox")
        .unwrap();
    assert_eq!(sandbox.version.as_deref(), Some("0.1.10"));
}
