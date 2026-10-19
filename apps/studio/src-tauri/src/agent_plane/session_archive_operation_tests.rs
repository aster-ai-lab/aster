#![cfg(test)]

use serde_json::{Value, json};
use std::sync::Arc;

use super::tests::{TOKEN, request, start_in_memory};
use super::{catalog::SessionCatalog, session_query::SessionQuery, store::PlaneStore};

fn epoch(catalog: &SessionCatalog) -> String {
    catalog.list(&SessionQuery::default(), None, 20).unwrap()["archiveEpoch"]
        .as_str()
        .unwrap()
        .into()
}

fn operation(epoch: &str, ids: &[&str]) -> Value {
    json!({ "operationId": uuid::Uuid::now_v7().to_string(), "epoch": epoch, "sessionIds": ids })
}

fn campaign(persist: bool) {
    let directory = tempfile::tempdir().unwrap();
    let root = persist.then(|| directory.path().join("state"));
    let store = Arc::new(PlaneStore::open(root.clone()));
    let catalog = SessionCatalog::new(store.clone());
    let make = |name| {
        catalog
            .create(&json!({ "name": name, "workspace": { "type": "system" } }))
            .unwrap()
    };
    let first = make("First");
    let second = make("Second");
    let file = std::path::Path::new(&first.workspace.path).join("report.md");
    std::fs::write(&file, "Preserve the exact output\n").unwrap();
    let native_epoch = epoch(&catalog);
    let command = operation(
        &native_epoch,
        &[&first.id, &second.id, "missing", &first.id],
    );
    let receipt = catalog.archive(&command).unwrap();
    assert_eq!(receipt["deletedIds"], json!([first.id, second.id]));
    assert_eq!(
        receipt["results"],
        json!([
            {"sessionId":first.id,"status":"archived"},
            {"sessionId":second.id,"status":"archived"},
            {"sessionId":"missing","status":"not-found"}
        ])
    );
    assert_eq!(
        catalog
            .archive_outcome(command["operationId"].as_str().unwrap(), &native_epoch)
            .unwrap()["receipt"],
        receipt
    );
    let mut stale = first.clone();
    stale.name = "A metadata write admitted before archive".into();
    store.save_session(stale).unwrap();
    assert!(store.session(&first.id).unwrap().deleted_at.is_some());
    catalog.restore(&first.id).unwrap();
    assert_eq!(catalog.archive(&command).unwrap(), receipt);
    assert!(catalog.get(&first.id).unwrap().deleted_at.is_none());
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "Preserve the exact output\n"
    );
    let mut changed = command.clone();
    changed["sessionIds"] = json!([first.id]);
    assert!(catalog.archive(&changed).is_err());
    let reopened = SessionCatalog::new(Arc::new(PlaneStore::open(root.clone())));
    if persist {
        assert!(reopened.get(&first.id).is_ok());
        assert_eq!(
            reopened
                .list(
                    &SessionQuery {
                        in_trash: true,
                        ..SessionQuery::default()
                    },
                    None,
                    20
                )
                .unwrap()["items"][0]["id"],
            second.id
        );
    }
    assert!(reopened.archive(&command).is_err());
    let lookup = reopened
        .archive_outcome(command["operationId"].as_str().unwrap(), &native_epoch)
        .unwrap();
    assert_ne!(lookup["epoch"], native_epoch);
    assert!(lookup["receipt"].is_null());
    catalog
        .destroy(&json!({"sessionIds":[first.id, second.id]}))
        .unwrap();
}

#[test]
fn replay_after_restore_cannot_rearchive_and_new_instances_refuse_old_operations() {
    campaign(false);
    campaign(true);
}

#[test]
fn mixed_archive_outcomes_preserve_failed_rows_and_replay_does_not_retry_them() {
    let directory = tempfile::tempdir().unwrap();
    let store = Arc::new(PlaneStore::open(Some(directory.path().into())));
    let catalog = SessionCatalog::new(store.clone());
    let first = catalog
        .create(&json!({"name":"Archived","workspace":{"type":"system"}}))
        .unwrap();
    let second = catalog
        .create(&json!({"name":"Active","workspace":{"type":"system"}}))
        .unwrap();
    catalog.archive(&json!({"sessionIds":[first.id]})).unwrap();
    let state = directory.path().join("sessions.json");
    let backup = directory.path().join("sessions-backup");
    std::fs::rename(&state, &backup).unwrap();
    std::fs::create_dir(&state).unwrap();
    let command = operation(&epoch(&catalog), &[&first.id, &second.id, "missing"]);
    let receipt = catalog.archive(&command).unwrap();
    assert_eq!(receipt["deletedIds"], json!([]));
    assert_eq!(
        receipt["results"],
        json!([
            {"sessionId":first.id,"status":"already-archived"},
            {"sessionId":second.id,"status":"failed"},
            {"sessionId":"missing","status":"not-found"}
        ])
    );
    assert!(catalog.get(&second.id).is_ok());
    assert!(catalog.archive(&json!({"sessionIds":[second.id]})).is_err());
    std::fs::remove_dir(&state).unwrap();
    std::fs::rename(backup, state).unwrap();
    assert_eq!(catalog.archive(&command).unwrap(), receipt);
    assert!(catalog.get(&second.id).is_ok());
    let retry = operation(&epoch(&catalog), &[&second.id]);
    assert_eq!(
        catalog.archive(&retry).unwrap()["results"][0]["status"],
        "archived"
    );
}

#[test]
fn operation_receipts_and_validation_use_the_real_authenticated_http_surface() {
    let base = start_in_memory();
    let call = |method: &str, path: &str, body: Option<&Value>| {
        let body = body.map(Value::to_string);
        request(&base, method, path, Some(TOKEN), body.as_deref())
    };
    let (_, body) = call("GET", "/api/agent-sessions", None);
    let epoch = serde_json::from_str::<Value>(&body).unwrap()["archiveEpoch"]
        .as_str()
        .unwrap()
        .to_owned();
    let (_, body) = call(
        "POST",
        "/api/agent-sessions",
        Some(&json!({"name":"HTTP batch", "workspace":{"type":"system"}})),
    );
    let row: Value = serde_json::from_str(&body).unwrap();
    let id = row["id"].as_str().unwrap();
    let command = operation(&epoch, &[id]);
    let lookup = format!(
        "/api/agent-sessions/archive-operations/{}?epoch={epoch}",
        command["operationId"].as_str().unwrap()
    );
    assert_eq!(request(&base, "GET", &lookup, None, None).0, 403);
    assert!(
        serde_json::from_str::<Value>(&call("GET", &lookup, None).1).unwrap()["receipt"].is_null()
    );
    let (status, body) = call("POST", "/api/agent-sessions/archive", Some(&command));
    assert_eq!(status, 200, "{body}");
    let receipt: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(receipt["results"][0]["status"], "archived");
    assert_eq!(
        serde_json::from_str::<Value>(&call("GET", &lookup, None).1).unwrap()["receipt"],
        receipt
    );
    for invalid in [
        json!({"sessionIds":[]}),
        json!({"sessionIds":[id],"permanent":true}),
        json!({"sessionIds":[id],"operationId":"bad","epoch":epoch}),
        json!({"sessionIds":[id],"operationId":command["operationId"]}),
        json!({"sessionIds":[id, null]}),
    ] {
        assert_eq!(
            call("POST", "/api/agent-sessions/archive", Some(&invalid)).0,
            400
        );
    }
}
