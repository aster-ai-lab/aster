use super::*;
use crate::{CompanionFile, StaticSource};

fn config(version: &str) -> EnvConfig {
    EnvConfig {
        tools: BTreeMap::from([("uv".into(), version.into())]),
        vars: BTreeMap::from([(
            "SYNTHETIC_TOKEN".into(),
            "secret-must-not-be-in-lock".into(),
        )]),
    }
}

fn payload(version: &str) -> ToolPayload {
    let bytes = b"#!/bin/sh\nprintf 'fixture\\n'\n".to_vec();
    ToolPayload {
        version: version.into(),
        sha256: hex_encode(&Sha256::digest(&bytes)),
        bytes,
        companions: vec![CompanionFile {
            relative_path: "lib/fixture.dat".into(),
            bytes: b"required companion".to_vec(),
            executable: false,
        }],
    }
}

fn install(store: &ToolStore, payload: ToolPayload) {
    let version = payload.version.clone();
    let mut source = StaticSource::new();
    source.insert("github:astral-sh/uv", payload);
    store
        .acquire_with(
            AcquireRequest {
                name: "uv".into(),
                spec: "github:astral-sh/uv".into(),
                version: Some(version),
                executable_name: "uv".into(),
            },
            &source,
        )
        .unwrap();
}

#[test]
fn ordered_lock_binds_all_versions_order_and_companions_without_changing_current() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("工具 store")).unwrap();
    for version in ["1.0.0", "2.0.0"] {
        install(&store, payload(version));
    }
    let declarations = crate::parse_env_declarations(
        r#"env {
      tools { tool "uv" { version = ["2.0.0", "1.0.0"] } }
      vars { var "TOKEN" { value = "synthetic-ordered-lock-secret" } }
    }"#,
    )
    .unwrap();
    let lock = EnvLock::capture_declarations(&declarations, &store).unwrap();
    assert!(lock.render().contains(MULTI_SCHEMA));
    assert!(!lock.render().contains("synthetic-ordered-lock-secret"));
    let reread = EnvLock::parse(&lock.render()).unwrap();
    assert_eq!(reread, lock);
    install(&store, payload("3.0.0"));
    assert_eq!(
        reread
            .resolve_declarations(&declarations)
            .unwrap()
            .into_iter()
            .map(|tool| tool.version.unwrap())
            .collect::<Vec<_>>(),
        ["2.0.0", "1.0.0"]
    );
    assert_eq!(
        reread
            .install_with(&store, None)
            .unwrap()
            .into_iter()
            .map(|receipt| receipt.version)
            .collect::<Vec<_>>(),
        ["2.0.0", "1.0.0"]
    );
    assert!(matches!(
        reread.resolve(&config("2.0.0")),
        Err(ToolAcquireError::LockDeclarationMismatch)
    ));
    let mut reversed = declarations.clone();
    reversed.tools.get_mut("uv").unwrap().reverse();
    assert!(matches!(
        reread.resolve_declarations(&reversed),
        Err(ToolAcquireError::LockDeclarationMismatch)
    ));
    fs::write(
        store.root_path().join("installs/uv/1.0.0/lib/fixture.dat"),
        "changed",
    )
    .unwrap();
    assert!(matches!(
        reread.install_with(&store, None),
        Err(ToolAcquireError::LockContentMismatch { .. })
    ));
    assert_eq!(
        fs::read_to_string(store.root_path().join("current/uv")).unwrap(),
        "3.0.0\n"
    );
}

#[test]
fn lock_round_trip_covers_companions_and_ancestor_discovery_without_secrets() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("工具 store")).unwrap();
    install(&store, payload("1.0.0"));
    let config = config("latest");
    let lock = EnvLock::capture(&config, &store).unwrap();
    let path = project_env_lock_path(temporary.path());
    lock.write(&path).unwrap();
    let reread = EnvLock::read(&path).unwrap().unwrap();
    assert_eq!(lock, reread);
    assert_eq!(lock.render(), reread.render());
    assert!(!lock.render().contains("secret-must-not-be-in-lock"));
    assert!(!format!("{lock:?}").contains("secret-must-not-be-in-lock"));
    assert!(lock.render().contains("lib/fixture.dat"));
    let nested = temporary.path().join("nested 空格/deeper");
    fs::create_dir_all(&nested).unwrap();
    assert_eq!(
        EnvLock::discover(&nested).unwrap(),
        Some((path, lock.clone()))
    );

    install(&store, payload("2.0.0"));
    assert_eq!(
        lock.resolve(&config).unwrap()[0].version.as_deref(),
        Some("1.0.0")
    );
    assert_eq!(lock.install_with(&store, None).unwrap()[0].version, "1.0.0");
    assert_eq!(
        fs::read_to_string(store.root_path().join("current/uv")).unwrap(),
        "2.0.0\n"
    );
}

#[test]
fn lock_refuses_declaration_and_platform_drift() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("tools")).unwrap();
    install(&store, payload("1.0.0"));
    let lock = EnvLock::capture(&config("latest"), &store).unwrap();
    for config in [config("1.0.0"), EnvConfig::default()] {
        assert!(matches!(
            lock.resolve(&config),
            Err(ToolAcquireError::LockDeclarationMismatch)
        ));
    }
    let foreign = EnvLock::parse(&lock.render().replace(&platform(), "foreign-platform")).unwrap();
    assert!(matches!(
        foreign.resolve(&config("latest")),
        Err(ToolAcquireError::LockPlatformMismatch { .. })
    ));
    assert!(matches!(
        foreign.install_with(&store, None),
        Err(ToolAcquireError::LockPlatformMismatch { .. })
    ));
    let exact = EnvLock::capture(&config("1.0.0"), &store).unwrap().render();
    assert!(EnvLock::parse(&exact.replace("version = \"1.0.0\"", "version = \"2.0.0\"")).is_err());
}

#[test]
fn locked_download_verifies_before_publication_and_preserves_previous_pointer_on_failure() {
    let temporary = tempfile::tempdir().unwrap();
    let origin = ToolStore::open(temporary.path().join("origin")).unwrap();
    install(&origin, payload("1.0.0"));
    let lock = EnvLock::capture(&config("latest"), &origin).unwrap();
    let destination = ToolStore::open(temporary.path().join("destination")).unwrap();
    install(&destination, payload("0.9.0"));
    assert!(matches!(
        lock.install_with(&destination, None),
        Err(ToolAcquireError::ToolNotInstalled { .. })
    ));
    let mut source = StaticSource::new();
    let mut altered = payload("1.0.0");
    altered.companions[0].bytes = b"different publisher payload".to_vec();
    source.insert("github:astral-sh/uv", altered);
    assert!(matches!(
        lock.install_with(&destination, Some(&source)),
        Err(ToolAcquireError::LockContentMismatch { .. })
    ));
    assert!(!destination.root_path().join("installs/uv/1.0.0").exists());
    assert_eq!(
        fs::read_to_string(destination.root_path().join("current/uv")).unwrap(),
        "0.9.0\n"
    );
    source.insert("github:astral-sh/uv", payload("1.0.0"));
    assert_eq!(
        lock.install_with(&destination, Some(&source)).unwrap()[0].version,
        "1.0.0"
    );
}

#[test]
fn locked_install_refuses_changed_missing_and_extra_files() {
    for change in ["executable", "companion", "missing", "extra"] {
        let temporary = tempfile::tempdir().unwrap();
        let store = ToolStore::open(temporary.path().join("tools")).unwrap();
        install(&store, payload("1.0.0"));
        let lock = EnvLock::capture(&config("1.0.0"), &store).unwrap();
        let directory = store.root_path().join("installs/uv/1.0.0");
        match change {
            "executable" => fs::write(directory.join("uv"), b"different").unwrap(),
            "companion" => fs::write(directory.join("lib/fixture.dat"), b"different").unwrap(),
            "missing" => fs::remove_file(directory.join("lib/fixture.dat")).unwrap(),
            "extra" => fs::write(directory.join("extra"), b"unexpected").unwrap(),
            _ => unreachable!(),
        }
        assert!(
            matches!(
                EnvLock::capture(&config("1.0.0"), &store),
                Err(ToolAcquireError::LockContentMismatch { .. })
            ),
            "capture: {change}"
        );
        assert!(
            matches!(
                lock.install_with(&store, None),
                Err(ToolAcquireError::LockContentMismatch { .. })
            ),
            "{change}"
        );
    }
}

#[test]
fn legacy_manifest_is_not_inferred_from_an_unverified_directory() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("tools")).unwrap();
    install(&store, payload("1.0.0"));
    let record = store.root_path().join("receipts/uv/1.0.0.files.json");
    fs::remove_file(&record).unwrap();
    assert!(matches!(
        EnvLock::capture(&config("1.0.0"), &store),
        Err(ToolAcquireError::InstallationManifestMissing { .. })
    ));
    install(&store, payload("1.0.0"));
    assert!(EnvLock::capture(&config("1.0.0"), &store).is_ok());
    fs::remove_file(store.root_path().join("installs/uv/1.0.0/lib/fixture.dat")).unwrap();
    assert!(matches!(
        EnvLock::capture(&config("1.0.0"), &store),
        Err(ToolAcquireError::LockContentMismatch { .. })
    ));
}

#[test]
fn malformed_paths_unknown_fields_duplicates_and_oversized_locks_refuse() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("tools")).unwrap();
    install(&store, payload("1.0.0"));
    let text = EnvLock::capture(&config("latest"), &store)
        .unwrap()
        .render();
    for malformed in [
        text.replace("lib/fixture.dat", "../outside"),
        text.replace("lib/fixture.dat", "/absolute"),
        text.replace("lib/fixture.dat", "C:/absolute"),
        text.replace("lib/fixture.dat", "uv"),
        text.replace("request =", "unknown ="),
        text.replace("github:astral-sh/uv", "github:other/source"),
        format!("{text}\nunknown {{}}"),
        " ".repeat(MAX_LOCK_BYTES as usize + 1),
    ] {
        assert!(EnvLock::parse(&malformed).is_err());
    }
    let path = project_env_lock_path(temporary.path());
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, vec![b' '; MAX_LOCK_BYTES as usize + 1]).unwrap();
    assert!(matches!(
        EnvLock::read(&path),
        Err(ToolAcquireError::InvalidEnvLock(_))
    ));
}

#[test]
fn legacy_v1_locks_remain_readable_after_resolved_selectors_are_added() {
    for (version, request) in [
        ("1.0.0", "1.0.0"),
        ("tip", "latest"),
        ("latest", "latest"),
        ("system", "latest"),
    ] {
        let temporary = tempfile::tempdir().unwrap();
        let store = ToolStore::open(temporary.path()).unwrap();
        install(&store, payload(version));
        let current = EnvLock::capture(&config(request), &store).unwrap();
        let legacy = current
            .render()
            .replace(SCHEMA, LEGACY_SCHEMA)
            .lines()
            .filter(|line| !line.trim_start().starts_with("selector ="))
            .collect::<Vec<_>>()
            .join("\n");
        let parsed = EnvLock::parse(&legacy).unwrap();
        assert_eq!(
            parsed.install_with(&store, None).unwrap()[0].version,
            version
        );
        assert!(EnvLock::parse(&parsed.render()).is_ok());
    }
}

#[test]
fn v2_file_locks_remain_readable_after_archive_locks_are_added() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path()).unwrap();
    install(&store, payload("1.0.0"));
    let current = EnvLock::capture(&config("latest"), &store).unwrap();
    let text = current.render().replace(SCHEMA, SELECTOR_SCHEMA);
    let parsed = EnvLock::parse(&text).unwrap();
    assert_eq!(parsed, current);
    assert_eq!(
        parsed.install_with(&store, None).unwrap()[0].version,
        "1.0.0"
    );
}

#[test]
fn legacy_v1_literal_operator_names_keep_their_exact_identity_through_locked_install() {
    for version in [
        "system",
        "ref-main",
        "tag-v1.2.3",
        "branch-stable",
        "rev-abc123",
        "ref-",
        "tag-",
        "branch-",
        "rev-",
    ] {
        let temporary = tempfile::tempdir().unwrap();
        let origin = ToolStore::open(temporary.path().join("origin")).unwrap();
        install(&origin, payload(version));
        let legacy = EnvLock::capture(&config("latest"), &origin)
            .unwrap()
            .render()
            .replace(SCHEMA, LEGACY_SCHEMA)
            .replace("request = \"latest\"", &format!("request = \"{version}\""))
            .lines()
            .filter(|line| !line.trim_start().starts_with("selector ="))
            .collect::<Vec<_>>()
            .join("\n");
        let parsed = EnvLock::parse(&legacy).unwrap();
        let declaration = crate::parse_env_acl(&crate::render_env_acl(&config(version))).unwrap();
        assert_eq!(
            parsed.resolve(&declaration).unwrap()[0].version.as_deref(),
            Some(version)
        );
        let rendered = parsed.render();
        assert!(rendered.contains(LEGACY_SCHEMA));
        assert!(!rendered.contains("selector ="));
        let parsed = EnvLock::parse(&rendered).unwrap();
        let destination = ToolStore::open(temporary.path().join("工具 destination")).unwrap();
        let mut source = StaticSource::new();
        source.insert("github:astral-sh/uv", payload(version));
        assert_eq!(
            parsed.install_with(&destination, Some(&source)).unwrap()[0].version,
            version
        );
        assert_eq!(
            parsed.install_with(&destination, None).unwrap()[0].version,
            version
        );
        assert_eq!(
            fs::read(
                destination
                    .root_path()
                    .join("installs/uv")
                    .join(version)
                    .join("lib/fixture.dat")
            )
            .unwrap(),
            b"required companion"
        );
        let changed = legacy.replace(
            &format!("version = \"{version}\""),
            "version = \"different-literal\"",
        );
        assert!(matches!(
            EnvLock::parse(&changed),
            Err(ToolAcquireError::InvalidEnvLock(_))
        ));
    }
}

#[test]
fn a_managed_lock_cannot_replace_a_context_dependent_request_or_a_different_prefix() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path()).unwrap();
    install(&store, payload("1.0.0"));
    let text = EnvLock::capture(&config("1.0.0"), &store).unwrap().render();
    for request in ["system", "path:../tools", "branch:main", "prefix:2"] {
        let changed = text.replace("request = \"1.0.0\"", &format!("request = \"{request}\""));
        assert!(matches!(
            EnvLock::parse(&changed),
            Err(ToolAcquireError::InvalidEnvLock(_))
        ));
    }
}

#[cfg(unix)]
#[test]
fn lock_refuses_linked_installation_files() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("tools")).unwrap();
    install(&store, payload("1.0.0"));
    let lock = EnvLock::capture(&config("latest"), &store).unwrap();
    let companion = store.root_path().join("installs/uv/1.0.0/lib/fixture.dat");
    fs::remove_file(&companion).unwrap();
    let elsewhere = temporary.path().join("elsewhere");
    fs::write(&elsewhere, "required companion").unwrap();
    std::os::unix::fs::symlink(elsewhere, companion).unwrap();
    assert!(matches!(
        lock.install_with(&store, None),
        Err(ToolAcquireError::LinkRefused { .. })
    ));
}
