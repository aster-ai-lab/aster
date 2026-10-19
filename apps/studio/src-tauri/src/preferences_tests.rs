#![cfg(test)]

use super::*;
use serde_json::json;
use std::sync::Barrier;

const MEMBER: &str = "11111111-1111-4111-8111-111111111111";
const OTHER: &str = "22222222-2222-4222-8222-222222222222";

fn commit(revision: u64, values: Value) -> Commit {
    Commit {
        scope: MEMBER.to_owned(),
        expected_revision: revision,
        updates: serde_json::from_value(values).expect("test updates"),
    }
}

#[test]
fn acl_survives_restart_and_preserves_arbitrary_map_keys_and_numeric_precision() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferenceStore::new(dir.path().to_owned());
    let input = commit(
        0,
        json!({"app.harness.assemble": "lean", "feature.binary.install_settings": {
            "a/b.c \" quoted": [true, null, {}, [], 9007199254740993u64, -3.125, "line\nnext"]
        }}),
    );
    let saved = store.commit(&input).unwrap();
    let text = fs::read_to_string(dir.path().join(format!("{MEMBER}.acl"))).unwrap();
    assert!(a3s_acl::parse(&text).is_ok());
    assert_eq!(saved.revision, 1);
    assert_eq!(
        PreferenceStore::new(dir.path().to_owned())
            .read(MEMBER)
            .unwrap(),
        saved
    );
    assert_eq!(saved.values, input.updates);
}

#[test]
fn plugin_activity_choices_survive_restart_without_crossing_member_scope() {
    let dir = tempfile::tempdir().unwrap();
    let input = commit(
        0,
        json!({"plugins.activity_bar": {
            "aix.a3s-digital-twin": true, "aix.a3s-finance": false
        }}),
    );
    PreferenceStore::new(dir.path().to_owned())
        .commit(&input)
        .unwrap();
    let restarted = PreferenceStore::new(dir.path().to_owned());
    assert_eq!(restarted.read(MEMBER).unwrap().values, input.updates);
    assert!(restarted.read(OTHER).unwrap().values.is_empty());
}

#[test]
fn simultaneous_native_stores_use_cas_and_retry_only_their_own_keys() {
    let dir = tempfile::tempdir().unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let threads: Vec<_> = [
        ("app.harness.assemble", json!("host")),
        ("app.language", json!("en-US")),
    ]
    .into_iter()
    .map(|(key, value)| {
        let root = dir.path().to_owned();
        let barrier = Arc::clone(&barrier);
        std::thread::spawn(move || {
            let store = PreferenceStore::new(root);
            let mut input = commit(0, json!({ key: value }));
            barrier.wait();
            match store.commit(&input) {
                Ok(snapshot) => snapshot,
                Err(PreferenceError::Conflict { snapshot }) => {
                    input.expected_revision = snapshot.revision;
                    store.commit(&input).unwrap()
                }
                Err(error) => panic!("unexpected failure: {error}"),
            }
        })
    })
    .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    let snapshot = PreferenceStore::new(dir.path().to_owned())
        .read(MEMBER)
        .unwrap();
    assert_eq!(snapshot.revision, 2);
    assert_eq!(
        snapshot.values,
        commit(
            0,
            json!({"app.harness.assemble":"host","app.language":"en-US"})
        )
        .updates
    );
}

#[test]
fn stale_commit_does_not_replace_a_newer_choice() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferenceStore::new(dir.path().to_owned());
    let saved = store
        .commit(&commit(0, json!({"app.harness.assemble":"host"})))
        .unwrap();
    assert!(matches!(
        store.commit(&commit(0, json!({"app.harness.assemble":"lean"}))),
        Err(PreferenceError::Conflict { .. })
    ));
    assert_eq!(store.read(MEMBER).unwrap(), saved);
}

#[test]
fn missing_or_other_member_never_reads_the_first_members_values() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferenceStore::new(dir.path().to_owned());
    store
        .commit(&commit(0, json!({"app.harness.assemble":"host"})))
        .unwrap();
    assert!(store.read(OTHER).unwrap().values.is_empty());
    for scope in [
        "",
        "default",
        "../escape",
        "11111111-1111-4111-8111-11111111111A",
    ] {
        assert!(matches!(store.read(scope), Err(PreferenceError::Denied)));
    }
    let session = crate::member_session::MemberSessionStore::open_at(None);
    assert!(matches!(
        session.with_preference_scope(|scope| store.read(scope)),
        Err(PreferenceError::Denied)
    ));
    session
        .save(br#"{"token":"a3s_ms_0123456789abcdef0123456789abcdef"}"#)
        .unwrap();
    session.remember_member_id(MEMBER).unwrap();
    assert_eq!(
        session
            .with_preference_scope(|scope| store.read(scope))
            .unwrap()
            .values["app.harness.assemble"],
        "host"
    );
    session.clear().unwrap();
    session
        .save(br#"{"token":"a3s_ms_abcdef0123456789abcdef0123456789"}"#)
        .unwrap();
    session.remember_member_id(OTHER).unwrap();
    assert!(
        session
            .with_preference_scope(|scope| store.read(scope))
            .unwrap()
            .values
            .is_empty()
    );
}

#[test]
fn corruption_and_duplicate_keys_refuse_reads_and_writes_without_overwriting_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(format!("{MEMBER}.acl"));
    let store = PreferenceStore::new(dir.path().to_owned());
    let corruptions = [
        "{broken".to_owned(),
        format!(
            "preferences \"{MEMBER}\" {{ revision = \"1\" preference \"app.language\" {{ value = \"en-US\" }} preference \"app.language\" {{ value = \"zh-CN\" }} }}"
        ),
        format!("preferences \"{OTHER}\" {{ revision = \"1\" }}"),
    ];
    for text in corruptions {
        fs::write(&path, &text).unwrap();
        assert!(matches!(store.read(MEMBER), Err(PreferenceError::Invalid)));
        assert!(
            store
                .commit(&commit(0, json!({"app.harness.assemble":"host"})))
                .is_err()
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), text);
    }
}

#[test]
fn invalid_batch_and_oversized_data_never_partially_commit() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferenceStore::new(dir.path().to_owned());
    for values in [
        json!({"app.language":"en-US","made.up.key":true}),
        json!({"app.harness.assemble":"unknown"}),
        json!({"BootConfig.temp.secret":"x"}),
        json!({"app.language":"x".repeat(LIMIT as usize + 1)}),
    ] {
        assert!(matches!(
            store.commit(&commit(0, values)),
            Err(PreferenceError::Invalid | PreferenceError::InvalidHarnessRecipe)
        ));
        assert_eq!(store.read(MEMBER).unwrap().revision, 0);
    }
}

#[test]
fn invalid_destination_is_preserved_without_publishing_a_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferenceStore::new(dir.path().to_owned());
    let target = dir.path().join(format!("{MEMBER}.acl"));
    store
        .locked(MEMBER, |path, snapshot| {
            fs::create_dir(path).unwrap();
            assert_eq!(snapshot.revision, 0);
            Ok(())
        })
        .unwrap();
    assert!(
        store
            .commit(&commit(0, json!({"app.harness.assemble":"host"})))
            .is_err()
    );
    assert!(target.is_dir());
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
}

#[cfg(unix)]
#[test]
fn actual_write_failure_keeps_the_previously_committed_acl_and_revision() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let store = PreferenceStore::new(dir.path().to_owned());
    let saved = store
        .commit(&commit(0, json!({"app.harness.assemble":"lean"})))
        .unwrap();
    let path = dir.path().join(format!("{MEMBER}.acl"));
    let bytes = fs::read(&path).unwrap();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o500)).unwrap();
    let result = store.commit(&commit(1, json!({"app.harness.assemble":"host"})));
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
    assert!(matches!(result, Err(PreferenceError::Unavailable)));
    assert_eq!(fs::read(path).unwrap(), bytes);
    assert_eq!(store.read(MEMBER).unwrap(), saved);
}

#[test]
fn guests_and_foreign_origins_cannot_use_the_preference_command() {
    let main = "tauri://localhost/windows/main/index.html";
    assert!(check_window("main", "main", main, "host-key", "host-key").is_ok());
    assert!(check_window("other", "main", main, "host-key", "host-key").is_err());
    for (label, url) in [
        ("aix-guest", main),
        ("main", "https://evil.test/windows/main/index.html"),
        ("main", "tauri://localhost/plugin/index.html"),
    ] {
        assert!(matches!(
            check_window("main", label, url, "host-key", "host-key"),
            Err(PreferenceError::Denied)
        ));
    }
    for (presented, expected) in [("", "host-key"), ("guest-key", "host-key"), ("", "")] {
        assert!(matches!(
            check_window("main", "main", main, presented, expected),
            Err(PreferenceError::Denied)
        ));
    }
}

#[test]
fn preference_ipc_reads_and_commits_after_a_child_webview_is_created() {
    use tauri::{LogicalPosition, LogicalSize, Manager, WebviewUrl};
    let dir = tempfile::tempdir().unwrap();
    let member = Arc::new(crate::member_session::MemberSessionStore::open_at(None));
    member
        .save(br#"{"token":"a3s_ms_0123456789abcdef0123456789abcdef"}"#)
        .unwrap();
    member.remember_member_id(MEMBER).unwrap();
    let app = tauri::test::mock_builder()
        .manage(PreferenceHost::new(
            dir.path().to_owned(),
            member,
            "host-key".into(),
        ))
        .invoke_handler(tauri::generate_handler![
            super::preference_read,
            super::preference_commit
        ])
        .build(crate::studio_context())
        .unwrap();
    let page: url::Url = crate::main_renderer_url(false).parse().unwrap();
    tauri::WebviewWindowBuilder::new(&app, "main", WebviewUrl::External(page.clone()))
        .build()
        .unwrap();
    let child = app
        .get_window("main")
        .unwrap()
        .add_child(
            tauri::webview::WebviewBuilder::new("studio-embed", WebviewUrl::External(page)),
            LogicalPosition::new(20.0, 40.0),
            LogicalSize::new(400.0, 600.0),
        )
        .unwrap();
    assert!(app.get_webview_window("main").is_none());
    let main = app.get_webview("main").unwrap();
    let invoke = |view: &tauri::Webview<tauri::test::MockRuntime>, command: &str, body| {
        tauri::test::get_ipc_response(
            &Arc::new(view.clone()),
            tauri::webview::InvokeRequest {
                cmd: command.into(),
                callback: tauri::ipc::CallbackFn(1),
                error: tauri::ipc::CallbackFn(2),
                url: view.url().unwrap(),
                body: tauri::ipc::InvokeBody::Json(body),
                headers: Default::default(),
                invoke_key: tauri::test::INVOKE_KEY.into(),
            },
        )
    };
    let args = json!({"hostToken":"host-key"});
    assert!(invoke(&child, "preference_read", args.clone()).is_err());
    let initial = invoke(&main, "preference_read", args.clone())
        .unwrap()
        .deserialize::<Value>()
        .unwrap();
    assert_eq!(initial["revision"], 0);
    invoke(
        &main,
        "preference_commit",
        json!({"hostToken":"host-key","input":{
            "scope":MEMBER,"expectedRevision":0,"updates":{"app.language":"en-US"}
        }}),
    )
    .unwrap();
    let saved = invoke(&main, "preference_read", args)
        .unwrap()
        .deserialize::<Value>()
        .unwrap();
    assert_eq!(saved["revision"], 1);
    assert_eq!(saved["values"]["app.language"], "en-US");
}

#[test]
fn strict_commit_rejects_extra_fields_and_unchanged_writes_keep_the_revision() {
    assert!(
        serde_json::from_value::<Commit>(
            json!({"scope":MEMBER,"expectedRevision":0,"updates":{},"grant":true})
        )
        .is_err()
    );
    let dir = tempfile::tempdir().unwrap();
    let store = PreferenceStore::new(dir.path().to_owned());
    let saved = store
        .commit(&commit(0, json!({"app.harness.assemble":"lean"})))
        .unwrap();
    assert_eq!(
        store
            .commit(&commit(1, json!({"app.harness.assemble":"lean"})))
            .unwrap(),
        saved
    );
}

#[test]
#[ignore = "stdio transport for the native Preference bridge campaign"]
fn native_preference_stdio() {
    use std::io::BufRead;
    let root =
        std::env::var_os("A3S_CLOUD_TEST_PREFERENCE_ROOT").expect("test-owned preference root");
    let store = PreferenceStore::new(PathBuf::from(root));
    let session = crate::member_session::MemberSessionStore::open_at(None);
    for line in std::io::stdin().lock().lines() {
        let request: Value = serde_json::from_str(&line.unwrap()).unwrap();
        let result = match request["action"].as_str().unwrap() {
            "member" => {
                session.clear().unwrap();
                session
                    .save(br#"{"token":"a3s_ms_preference_campaign_test"}"#)
                    .unwrap();
                session
                    .remember_member_id(request["member"].as_str().unwrap())
                    .unwrap();
                Ok(json!(null))
            }
            "read" => session
                .with_preference_scope(|scope| store.read(scope))
                .map(|snapshot| serde_json::to_value(snapshot).unwrap()),
            "commit" => {
                let input: Commit = serde_json::from_value(request["input"].clone()).unwrap();
                session
                    .with_preference_scope(|scope| {
                        if scope != input.scope {
                            return Err(PreferenceError::Denied);
                        }
                        store.commit(&input)
                    })
                    .map(|snapshot| serde_json::to_value(snapshot).unwrap())
            }
            _ => panic!("unknown test action"),
        };
        let response = match result {
            Ok(data) => json!({"id": request["id"], "ok": true, "data": data}),
            Err(error) => json!({"id": request["id"], "ok": false, "error": error}),
        };
        println!("A3S_PREFERENCE_PROOF {response}");
        std::io::stdout().flush().unwrap();
    }
}
