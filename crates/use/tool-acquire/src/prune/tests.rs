use super::*;
use crate::{
    load_env_declarations_with_sources, project_env_path, write_env_declarations, EnvDeclarations,
    LocalApply,
};
use std::collections::BTreeMap;

fn declare(path: &Path, requests: &[&str]) {
    write_env_declarations(
        path,
        &EnvDeclarations {
            tools: BTreeMap::from([(
                "uv".into(),
                requests.iter().map(|x| (*x).to_owned()).collect(),
            )]),
            vars: BTreeMap::new(),
        },
    )
    .unwrap();
}

async fn install(store: &ToolStore, root: &Path, version: &str) {
    let payload = root.join("payload");
    fs::write(&payload, b"native fixture bytes").unwrap();
    store
        .apply_local(LocalApply {
            name: "uv".into(),
            spec: "github:astral-sh/uv".into(),
            version: version.into(),
            executable_name: "uv".into(),
            payload,
            sha256: None,
            require_checksum: false,
            companions: vec![],
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn tracked_declarations_protect_selected_versions_and_preview_is_read_only() {
    let temporary = tempfile::tempdir().unwrap();
    let home = temporary.path().join("home");
    let a = temporary.path().join("项目 with spaces/a");
    let b = temporary.path().join("项目 with spaces/b");
    let store = ToolStore::open(home.join("tools")).unwrap();
    for version in ["1.0.0", "1.5.0", "2.0.0"] {
        install(&store, temporary.path(), version).await;
    }
    declare(&project_env_path(&a), &["1"]);
    declare(&project_env_path(&b), &["2.0.0"]);
    for project in [&a, &b] {
        let loaded = load_env_declarations_with_sources(project, &home).unwrap();
        if project == &a {
            assert!(!store.root_path().join(TRACKING).exists());
        }
        loaded.track_sources(&store).unwrap();
    }
    let before = fs::read(store.root_path().join(TRACKING)).unwrap();
    let plan = store.plan_prune(&home, &[]).unwrap();
    assert_eq!(
        plan.tools()
            .iter()
            .map(|x| x.version.as_str())
            .collect::<Vec<_>>(),
        ["1.0.0"]
    );
    assert_eq!(fs::read(store.root_path().join(TRACKING)).unwrap(), before);
    assert_eq!(store.all_uninstall_candidates().unwrap().len(), 3);
    declare(&project_env_path(&a), &["2.0.0"]);
    fs::remove_file(project_env_path(&b)).unwrap();
    let plan = store.plan_prune(&home, &[]).unwrap();
    assert_eq!(plan.tools().len(), 2);
    assert_eq!(plan.stale_configs(), &[project_env_path(&b)]);
    assert_eq!(store.prune_env_sources().unwrap(), 1);
    assert_eq!(
        read_sources(&store).unwrap(),
        BTreeSet::from([project_env_path(&a)])
    );
}

#[tokio::test]
async fn global_requests_and_ordered_versions_do_not_use_mutable_current_pointers() {
    let temporary = tempfile::tempdir().unwrap();
    let home = temporary.path().join("home");
    let store = ToolStore::open(home.join("tools")).unwrap();
    for version in ["1.0.0", "1.5.0", "2.0.0"] {
        install(&store, temporary.path(), version).await;
    }
    assert_eq!(store.plan_prune(&home, &[]).unwrap().tools().len(), 3);
    declare(&user_env_path(&home), &["1.0.0", "1.5.0"]);
    assert_eq!(
        store.plan_prune(&home, &[]).unwrap().tools()[0].version,
        "2.0.0"
    );
    assert!(store
        .plan_prune(&home, &["fd".into()])
        .unwrap()
        .tools()
        .is_empty());
    assert!(store.plan_prune(&home, &["../uv".into()]).is_err());
}

#[tokio::test]
async fn missing_payloads_remain_candidates_but_invalid_declarations_stop_planning() {
    let temporary = tempfile::tempdir().unwrap();
    let home = temporary.path().join("home");
    let store = ToolStore::open(home.join("tools")).unwrap();
    install(&store, temporary.path(), "1.0.0").await;
    fs::remove_file(store.root_path().join("installs/uv/1.0.0/uv")).unwrap();
    assert_eq!(store.plan_prune(&home, &[]).unwrap().tools().len(), 1);
    fs::write(user_env_path(&home), "invalid { ").unwrap();
    assert!(store.plan_prune(&home, &[]).is_err());
    assert!(store.stale_env_sources().unwrap().is_empty());
    assert_eq!(store.all_uninstall_candidates().unwrap().len(), 1);
}

#[test]
fn tracking_preserves_native_path_units_and_refuses_invalid_metadata() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("tools")).unwrap();
    let path = temporary.path().join("项目 with spaces/.aster/env.acl");
    let mut sources = BTreeSet::from([path]);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::ffi::OsStringExt;
        sources.insert(
            temporary
                .path()
                .join(OsString::from_vec(b"native-\xff".to_vec()))
                .join(".aster/env.acl"),
        );
    }
    #[cfg(not(target_os = "linux"))]
    let _ = &mut sources;
    store.track_env_sources(&sources).unwrap();
    assert_eq!(read_sources(&store).unwrap(), sources);
    fs::write(
        store.root_path().join(TRACKING),
        br#"{"schema":"foreign","paths":[]}"#,
    )
    .unwrap();
    assert!(matches!(
        read_sources(&store),
        Err(ToolAcquireError::InvalidEnvTracking(_))
    ));
    assert!(store.plan_prune(temporary.path(), &[]).is_err());
}

#[test]
fn native_process_inspection_includes_this_test_executable() {
    let current = std::env::current_exe().unwrap().canonicalize().unwrap();
    assert!(process::running_paths().unwrap().contains(&current));
}

#[cfg(unix)]
#[tokio::test]
async fn a_running_interpreted_tool_keeps_its_installation() {
    let temporary = tempfile::tempdir().unwrap();
    let home = temporary.path().join("项目 with spaces");
    let store = ToolStore::open(home.join("tools")).unwrap();
    let payload = temporary.path().join("script");
    fs::write(&payload, b"#!/bin/sh\nread -r line\n").unwrap();
    store
        .apply_local(LocalApply {
            name: "uv".into(),
            spec: "github:astral-sh/uv".into(),
            version: "1.0.0".into(),
            executable_name: "uv".into(),
            payload,
            sha256: None,
            require_checksum: false,
            companions: vec![],
        })
        .await
        .unwrap();
    let script = store.root_path().join("installs/uv/1.0.0/uv");
    let mut child = tokio::process::Command::new("/bin/sh")
        .arg(script)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert!(child.try_wait().unwrap().is_none());
    assert!(store.plan_prune(&home, &[]).unwrap().tools().is_empty());
    child.kill().await.unwrap();
    child.wait().await.unwrap();
}
