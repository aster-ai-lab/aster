use super::*;
use crate::{project_env_path, user_env_path, write_env_acl};

fn config(request: &str) -> EnvConfig {
    EnvConfig {
        tools: BTreeMap::from([("node".into(), request.into())]),
        vars: BTreeMap::from([("TOKEN".into(), "synthetic-context-secret".into())]),
    }
}

#[test]
fn ordered_layers_keep_each_versions_logical_root_and_replace_whole_tool_lists() {
    let temporary = tempfile::tempdir().unwrap();
    let home = temporary.path().join("home");
    let aster_home = temporary.path().join("aster-home");
    let parent = temporary.path().join("项目 with spaces");
    let nested = parent.join("nested");
    for root in [&home, &parent, &nested] {
        for version in ["one", "two"] {
            std::fs::create_dir_all(root.join(version).join("bin")).unwrap();
        }
    }
    let declarations = crate::EnvDeclarations {
        tools: BTreeMap::from([(
            "node".into(),
            vec!["path:./two".into(), "path:./one".into()],
        )]),
        vars: BTreeMap::from([("TOKEN".into(), "synthetic-ordered-context-secret".into())]),
    };
    crate::write_env_declarations(user_env_path(&aster_home), &declarations).unwrap();
    let loaded = load_env_declarations_with_sources(&nested, &aster_home).unwrap();
    let selected = loaded.resolve_tools(Some(&home)).unwrap();
    let roots = selected
        .iter()
        .map(|tool| match tool.origin() {
            EnvToolOrigin::Path(paths) => paths.root().to_owned(),
            _ => panic!("expected external roots"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        roots,
        [
            home.join("two").canonicalize().unwrap(),
            home.join("one").canonicalize().unwrap()
        ]
    );
    crate::write_env_declarations(project_env_path(&parent), &declarations).unwrap();
    let loaded = load_env_declarations_with_sources(&nested, &aster_home).unwrap();
    let selected = loaded
        .resolve_tool_versions("node", Some(&home))
        .unwrap()
        .unwrap();
    for (tool, name) in selected.iter().zip(["two", "one"]) {
        let EnvToolOrigin::Path(paths) = tool.origin() else {
            panic!("expected external root")
        };
        assert_eq!(paths.root(), parent.join(name).canonicalize().unwrap());
    }
    write_env_acl(project_env_path(&nested), &config("system")).unwrap();
    let loaded = load_env_declarations_with_sources(&nested, &aster_home).unwrap();
    assert_eq!(loaded.config().tools.get("node").unwrap(), &["system"]);
    assert!(matches!(
        loaded
            .resolve_tools(Some(&home))
            .unwrap()
            .first()
            .unwrap()
            .origin(),
        EnvToolOrigin::System
    ));
    assert!(!format!("{loaded:?}").contains("synthetic-context-secret"));
    assert!(load_env_config_with_sources(&parent, &aster_home).is_err());
}

#[test]
fn layered_paths_keep_declaring_roots_overrides_and_secret_redaction() {
    let temporary = tempfile::tempdir().unwrap();
    let home = temporary.path().join("home");
    let aster_home = temporary.path().join("aster-home");
    let parent = temporary.path().join("项目 with spaces");
    let nested = parent.join("nested/work");
    let home_tool = home.join("SDK node");
    let parent_tool = parent.join("SDK node");
    let local_tool = nested.join("local node");
    for directory in [&home_tool, &parent_tool, &local_tool] {
        std::fs::create_dir_all(directory.join("bin")).unwrap();
    }
    write_env_acl(user_env_path(&aster_home), &config("path:./SDK node")).unwrap();
    let loaded = load_env_config_with_sources(&nested, &aster_home).unwrap();
    assert_eq!(
        loaded.config(),
        &crate::load_env_config(&nested, &aster_home).unwrap()
    );
    assert!(!format!("{loaded:?}").contains("synthetic-context-secret"));
    let selected = loaded.resolve_tool("node", Some(&home)).unwrap().unwrap();
    let EnvToolOrigin::Path(paths) = selected.origin() else {
        panic!("expected an external path")
    };
    assert_eq!(paths.root(), home_tool.canonicalize().unwrap());
    write_env_acl(project_env_path(&parent), &config("path:./SDK node")).unwrap();
    let loaded = load_env_config_with_sources(&nested, &aster_home).unwrap();
    let selected = loaded.resolve_tools(Some(&home)).unwrap().remove(0);
    let EnvToolOrigin::Path(paths) = selected.origin() else {
        panic!("expected an external path")
    };
    assert_eq!(paths.root(), parent_tool.canonicalize().unwrap());
    write_env_acl(project_env_path(&nested), &config("path:./local node")).unwrap();
    let loaded = load_env_config_with_sources(&nested, &aster_home).unwrap();
    let selected = loaded.resolve_tools(Some(&home)).unwrap().remove(0);
    let EnvToolOrigin::Path(paths) = selected.origin() else {
        panic!("expected an external path")
    };
    assert_eq!(paths.root(), local_tool.canonicalize().unwrap());
}

#[test]
fn external_paths_retain_native_errors_and_refuse_absent_context() {
    use std::error::Error;
    let temporary = tempfile::tempdir().unwrap();
    let absent =
        resolve_env_tool("node", "path:./missing", Some(temporary.path()), None).unwrap_err();
    let ToolAcquireError::Io { source, .. } = &absent else {
        panic!("expected a native I/O source")
    };
    assert_eq!(source.kind(), std::io::ErrorKind::NotFound);
    assert!(absent.source().is_some());
    let file = temporary.path().join("file");
    std::fs::write(&file, b"not a directory").unwrap();
    let error = resolve_env_tool("node", "path:./file", Some(temporary.path()), None).unwrap_err();
    let ToolAcquireError::Io { source, .. } = error else {
        panic!("expected native directory admission")
    };
    assert_eq!(source.kind(), std::io::ErrorKind::NotADirectory);
    for request in ["path:~/SDK", "path:./SDK"] {
        assert!(matches!(
            resolve_env_tool("node", request, None, None),
            Err(ToolAcquireError::InvalidEnvAcl(_))
        ));
    }
    assert!(matches!(
        resolve_env_tool("node", "system", None, None)
            .unwrap()
            .origin(),
        EnvToolOrigin::System
    ));
    assert!(matches!(
        resolve_env_tool("uv", "1.2", None, None).unwrap().origin(),
        EnvToolOrigin::Managed
    ));
    assert!(matches!(
        resolve_env_tool("agy", "path:.", Some(temporary.path()), None),
        Err(ToolAcquireError::UnsupportedVersionRequest(_))
    ));
}

#[cfg(unix)]
fn executable(path: &Path, word: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, format!("#!/bin/sh\nprintf '%s\\n' '{word}'\nif [ \"$#\" -gt 0 ]; then printf '%s\\n' '{word}' > \"$1\"; fi\n")).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[cfg(unix)]
#[test]
fn context_selections_execute_external_and_system_commands_without_store_mutation() {
    let temporary = tempfile::tempdir().unwrap();
    let cwd = temporary.path().join("工作 directory");
    let root = cwd.join("SDK node");
    std::fs::create_dir_all(&cwd).unwrap();
    executable(&root.join("bin/node"), "external-node");
    let home = temporary.path().join("home");
    executable(&home.join("SDK node/bin/node"), "external-node");
    let store = ToolStore::open(temporary.path().join("store")).unwrap();
    executable(&store.root_path().join("shims/node"), "managed-shim");
    executable(
        &store.root_path().join("installs/node/old/bin/node"),
        "old-managed",
    );
    let system_bin = cwd.join("system bin");
    executable(&system_bin.join("node"), "system-node");
    let inherited = std::env::join_paths([
        store.root_path().join("shims"),
        store.root_path().join("installs/node/old/bin"),
        system_bin.clone(),
    ])
    .unwrap();
    let absolute = format!("path:{}", root.display());
    for (request, expected) in [
        (absolute.as_str(), "external-node"),
        ("path:./SDK node", "external-node"),
        ("path:~/SDK node", "external-node"),
        ("system", "system-node"),
    ] {
        let selection = resolve_env_tool("node", request, Some(&cwd), Some(&home)).unwrap();
        let program = selection
            .which(&store, Some(&inherited), &cwd)
            .unwrap()
            .unwrap();
        let marker = cwd.join("effect 文件");
        let result = std::process::Command::new(program)
            .arg(&marker)
            .current_dir(&cwd)
            .env_clear()
            .output()
            .unwrap();
        assert!(result.status.success());
        assert_eq!(result.stdout, format!("{expected}\n").as_bytes());
        assert_eq!(
            std::fs::read(&marker).unwrap(),
            format!("{expected}\n").as_bytes()
        );
    }
    assert!(!store.root_path().join("receipts").exists());
    assert!(!store.root_path().join("current").exists());
    let lookalike = store.root_path().join("shims-other");
    let raw =
        std::env::join_paths([PathBuf::new(), lookalike.clone(), system_bin.clone()]).unwrap();
    assert_eq!(
        store.unmanaged_path_dirs(Some(&raw)),
        vec![PathBuf::new(), lookalike, system_bin]
    );
}

#[cfg(unix)]
#[test]
fn ordered_temporary_versions_replace_a_tool_and_preserve_other_declaration_roots() {
    let temporary = tempfile::tempdir().unwrap();
    let project = temporary.path().join("项目 with spaces");
    let cwd = project.join("nested");
    let aster_home = temporary.path().join("aster-home");
    for (path, word) in [
        (cwd.join("one/bin/node"), "first-node"),
        (cwd.join("two/bin/node"), "second-node"),
        (project.join("uv-one/bin/uv"), "first-uv"),
        (project.join("uv-two/bin/uv"), "second-uv"),
    ] {
        executable(&path, word);
    }
    let declaration = project_env_path(&project);
    crate::write_env_declarations(
        &declaration,
        &EnvDeclarations {
            tools: BTreeMap::from([
                ("node".into(), vec!["path:./overridden missing node".into()]),
                (
                    "uv".into(),
                    vec!["path:./uv-one".into(), "path:./uv-two".into()],
                ),
            ]),
            vars: BTreeMap::from([("KEPT".into(), "literal value".into())]),
        },
    )
    .unwrap();
    let before = std::fs::read(&declaration).unwrap();
    let loaded = load_env_declarations_with_sources(&cwd, &aster_home).unwrap();
    let overrides = BTreeMap::from([
        ("bun".into(), vec!["system".into()]),
        (
            "node".into(),
            vec![
                "path:./two".into(),
                "path:./one".into(),
                "path:./two".into(),
            ],
        ),
    ]);
    let selected = loaded
        .resolve_tools_with_version_overrides(&overrides, &cwd, None)
        .unwrap();
    assert_eq!(selected.len(), 5);
    assert!(matches!(selected[0].origin(), EnvToolOrigin::System));
    let store = ToolStore::open(aster_home.join("tools")).unwrap();
    for (tool, (root, word)) in selected[1..].iter().zip([
        (cwd.join("two"), "second-node"),
        (cwd.join("one"), "first-node"),
        (project.join("uv-one"), "first-uv"),
        (project.join("uv-two"), "second-uv"),
    ]) {
        let EnvToolOrigin::Path(paths) = tool.origin() else {
            panic!("expected an external tool")
        };
        assert_eq!(paths.root(), root.canonicalize().unwrap());
        let program = tool.which(&store, None, &cwd).unwrap().unwrap();
        let marker = cwd.join("effect 文件");
        let output = std::process::Command::new(program)
            .arg(&marker)
            .current_dir(&cwd)
            .env_clear()
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, format!("{word}\n").as_bytes());
        assert_eq!(std::fs::read(&marker).unwrap(), output.stdout);
    }
    let scalar = loaded
        .resolve_tools_with_overrides(
            &BTreeMap::from([("node".into(), "system".into())]),
            &cwd,
            None,
        )
        .unwrap();
    assert_eq!(scalar.len(), 3);
    assert!(matches!(scalar[0].origin(), EnvToolOrigin::System));
    assert_eq!(std::fs::read(&declaration).unwrap(), before);
    assert!(!project_env_path(&cwd).exists());
    assert!(!store.root_path().join("receipts").exists());
    assert!(!store.root_path().join("current").exists());
}

#[test]
fn temporary_version_lists_refuse_empty_or_oversized_inputs_before_path_lookup() {
    let temporary = tempfile::tempdir().unwrap();
    let project = temporary.path().join("project");
    let aster_home = temporary.path().join("aster-home");
    write_env_acl(project_env_path(&project), &config("path:./missing node")).unwrap();
    let loaded = load_env_declarations_with_sources(&project, &aster_home).unwrap();
    for requests in [
        Vec::new(),
        vec!["system".to_owned(); crate::version::MAX_VERSIONS + 1],
    ] {
        assert!(matches!(
            loaded.resolve_tools_with_version_overrides(
                &BTreeMap::from([("node".into(), requests)]),
                &project,
                None,
            ),
            Err(ToolAcquireError::InvalidEnvAcl(_))
        ));
    }
    assert!(!aster_home.join("tools").exists());
}

#[cfg(unix)]
#[test]
fn temporary_selections_replace_declarations_before_path_resolution_without_writing_them() {
    let temporary = tempfile::tempdir().unwrap();
    let project = temporary.path().join("项目 with spaces");
    let cwd = project.join("nested");
    let external = cwd.join("SDK node");
    executable(&external.join("bin/node"), "override-node");
    executable(&project.join("SDK uv/bin/uv"), "parent-uv");
    let declaration = project_env_path(&project);
    let mut declared = config("path:./missing node");
    declared.tools.insert("uv".into(), "path:./SDK uv".into());
    write_env_acl(&declaration, &declared).unwrap();
    let before = std::fs::read(&declaration).unwrap();
    let aster_home = temporary.path().join("aster-home");
    let loaded = load_env_config_with_sources(&cwd, &aster_home).unwrap();
    assert!(matches!(
        loaded.resolve_tools(None),
        Err(ToolAcquireError::Io { .. })
    ));
    let mut overrides = BTreeMap::from([
        ("node".into(), "path:./SDK node".into()),
        ("bun".into(), "system".into()),
    ]);
    let selections = loaded
        .resolve_tools_with_overrides(&overrides, &cwd, None)
        .unwrap();
    assert_eq!(selections.len(), 3);
    let store = ToolStore::open(aster_home.join("tools")).unwrap();
    for selected in &selections {
        match selected.tool().name.as_str() {
            "bun" => assert!(matches!(selected.origin(), EnvToolOrigin::System)),
            "node" | "uv" => {
                let (expected_root, word) = if selected.tool().name == "node" {
                    (external.clone(), "override-node")
                } else {
                    (project.join("SDK uv"), "parent-uv")
                };
                let EnvToolOrigin::Path(paths) = selected.origin() else {
                    panic!("expected an external selection")
                };
                assert_eq!(paths.root(), expected_root.canonicalize().unwrap());
                let program = selected.which(&store, None, &cwd).unwrap().unwrap();
                let marker = cwd.join("effect 文件");
                let output = std::process::Command::new(program)
                    .arg(&marker)
                    .current_dir(&cwd)
                    .env_clear()
                    .output()
                    .unwrap();
                assert!(output.status.success());
                assert_eq!(output.stdout, format!("{word}\n").as_bytes());
                assert_eq!(std::fs::read(&marker).unwrap(), output.stdout);
            }
            _ => panic!("unexpected tool"),
        }
    }
    overrides.insert("node".into(), "prefix:20".into());
    let selections = loaded
        .resolve_tools_with_overrides(&overrides, &cwd, None)
        .unwrap();
    let selected = selections
        .iter()
        .find(|tool| tool.tool().name == "node")
        .unwrap();
    assert!(matches!(selected.origin(), EnvToolOrigin::Managed));
    assert_eq!(selected.tool().version.as_deref(), Some("prefix:20"));
    overrides.insert("node".into(), "path:./still missing".into());
    assert!(matches!(
        loaded.resolve_tools_with_overrides(&overrides, &cwd, None),
        Err(ToolAcquireError::Io { .. })
    ));
    assert_eq!(std::fs::read(&declaration).unwrap(), before);
    assert_eq!(loaded.config(), &declared);
    assert!(!store.root_path().join("receipts").exists());
    assert!(!store.root_path().join("current").exists());
    assert!(!project_env_path(&cwd).exists());
}

#[cfg(unix)]
#[test]
fn native_github_and_npm_default_layouts_remain_distinct_and_bounded() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    executable(&root.join("flat/uv"), "flat");
    executable(&root.join("flat/bin/uv"), "bin");
    let selected = resolve_env_tool("uv", "path:./flat", Some(root), None).unwrap();
    let EnvToolOrigin::Path(paths) = selected.origin() else {
        panic!("expected native paths")
    };
    assert_eq!(
        paths.bin_dirs(),
        [root.join("flat/bin").canonicalize().unwrap()]
    );
    executable(&root.join("bundle/Demo.app/Contents/MacOS/uv"), "bundle");
    let selected = resolve_env_tool("uv", "path:./bundle", Some(root), None).unwrap();
    let EnvToolOrigin::Path(paths) = selected.origin() else {
        panic!("expected bundle paths")
    };
    assert_eq!(
        paths.bin_dirs(),
        [root
            .join("bundle/Demo.app/Contents/MacOS")
            .canonicalize()
            .unwrap()]
    );
    for directory in ["npm/node_modules/.bin", "npm/bin"] {
        std::fs::create_dir_all(root.join(directory)).unwrap();
    }
    let selected = resolve_env_tool("ntn", "path:./npm", Some(root), None).unwrap();
    let EnvToolOrigin::Path(paths) = selected.origin() else {
        panic!("expected npm paths")
    };
    assert_eq!(
        paths.bin_dirs(),
        [
            root.join("npm/node_modules/.bin").canonicalize().unwrap(),
            root.join("npm/bin").canonicalize().unwrap()
        ]
    );
    std::fs::create_dir(root.join("large")).unwrap();
    for number in 0..=crate::installation::MAX_TREE_ENTRIES {
        std::fs::write(root.join("large").join(number.to_string()), b"").unwrap();
    }
    assert!(matches!(
        resolve_env_tool("uv", "path:./large", Some(root), None),
        Err(ToolAcquireError::InvalidInstallation(_))
    ));
}

#[cfg(target_os = "linux")]
#[test]
fn relative_declarations_keep_non_utf8_native_parent_paths() {
    use std::os::unix::ffi::OsStringExt;
    let temporary = tempfile::tempdir().unwrap();
    let parent = temporary
        .path()
        .join(std::ffi::OsString::from_vec(b"native-\xff".to_vec()));
    executable(&parent.join("SDK/bin/node"), "native-bytes");
    let loaded_home = temporary.path().join("aster-home");
    write_env_acl(project_env_path(&parent), &config("path:./SDK")).unwrap();
    let context = load_env_config_with_sources(&parent, &loaded_home).unwrap();
    let selected = context.resolve_tool("node", None).unwrap().unwrap();
    let EnvToolOrigin::Path(paths) = selected.origin() else {
        panic!("expected native path")
    };
    assert_eq!(paths.root(), parent.join("SDK").canonicalize().unwrap());
}
