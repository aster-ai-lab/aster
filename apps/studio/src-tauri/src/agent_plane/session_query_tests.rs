#![cfg(test)]

use std::{collections::HashSet, sync::Arc, time::Instant};

use serde_json::{Value, json};

use super::catalog::SessionCatalog;
use super::domain::now_iso;
use super::session_query::{ExecutionStatus, SessionExecution, SessionQuery};
use super::store::PlaneStore;
use super::tests::{TOKEN, request, start_in_memory};

fn execution(anchor: &str, status: ExecutionStatus) -> SessionExecution {
    SessionExecution {
        anchor_message_id: anchor.into(),
        execution_id: "fixture-execution".into(),
        status,
        observed_at: now_iso(),
    }
}

fn query_campaign(persist: bool) {
    let directory = tempfile::tempdir().unwrap();
    let root = persist.then(|| directory.path().join("state"));
    let store = Arc::new(PlaneStore::open(root.clone()));
    let catalog = SessionCatalog::new(store.clone());
    let template = catalog
        .create(&json!({
            "name": "template", "agentId": "query-agent", "workspace": { "type": "system" }
        }))
        .unwrap();
    catalog
        .destroy(&json!({ "sessionIds": [template.id] }))
        .unwrap();
    let prefix = uuid::Uuid::now_v7().to_string();
    let ids: Vec<_> = (0..1000)
        .map(|index| format!("{prefix}-{index:04}"))
        .collect();
    for (index, id) in ids.iter().enumerate() {
        let mut row = template.clone();
        row.id = id.clone();
        row.name = if index == 999 {
            "中文计划%_".into()
        } else {
            format!("Task {index}")
        };
        row.last_activity_at = "2026-10-01T00:00:00.000Z".into();
        row.workspace_id = if index % 2 == 0 {
            "project-a"
        } else {
            "project-b"
        }
        .into();
        row.workspace.kind = "user".into();
        row.workspace.id = row.workspace_id.clone();
        row.workspace.path = directory.path().to_string_lossy().into_owned();
        store.save_session(row).unwrap();
    }
    store
        .save_session_execution(
            &ids[999],
            execution("finished", ExecutionStatus::Pending),
            true,
        )
        .unwrap();
    store
        .save_session_execution(
            &ids[999],
            execution("finished", ExecutionStatus::Done),
            false,
        )
        .unwrap();
    store
        .save_session_execution(&ids[998], execution("live", ExecutionStatus::Pending), true)
        .unwrap();

    let query = SessionQuery {
        search: Some(" 中文计划%_ ".into()),
        status: Some(ExecutionStatus::Done),
        workspace_id: Some("project-b".into()),
        activity_from: Some("2026-10-01T08:00:00+08:00".into()),
        activity_before: Some("2026-10-02T00:00:00Z".into()),
        ..SessionQuery::default()
    };
    let mut durations = Vec::new();
    for _ in 0..20 {
        let started = Instant::now();
        let page = catalog.list(&query, None, 20).unwrap();
        durations.push(started.elapsed().as_micros());
        assert_eq!(page["total"], 1);
        assert_eq!(page["items"][0]["id"], ids[999]);
        assert_eq!(page["items"][0]["execution"]["status"], "done");
    }
    durations.sort();
    eprintln!(
        "query campaign disk={persist}, tasks=1000, warm name/filter p95={}us",
        durations[18]
    );
    let mut excluded = query.clone();
    excluded.activity_before = Some("2026-10-01T00:00:00Z".into());
    excluded.activity_from = None;
    assert_eq!(catalog.list(&excluded, None, 20).unwrap()["total"], 0);

    let first = catalog.list(&SessionQuery::default(), None, 128).unwrap();
    assert_eq!(first["total"], 1000);
    let cursor = first["nextCursor"].as_str().unwrap();
    assert!(catalog.list(&query, Some(cursor), 20).is_err());
    assert!(catalog.list(&query, Some("garbage"), 20).is_err());
    assert!(catalog.list(&query, Some("128"), 20).is_err());
    assert_eq!(
        catalog
            .list(&SessionQuery::default(), Some("128"), 128)
            .unwrap()["items"][0]["id"],
        ids[128]
    );
    let mut inserted = store.session(&ids[0]).unwrap();
    inserted.id = format!("{prefix}-inserted");
    inserted.last_activity_at = "2026-10-02T00:00:00.000Z".into();
    store.save_session(inserted).unwrap();
    let mut seen: Vec<String> = first["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap().into())
        .collect();
    let mut cursor = Some(cursor.to_owned());
    while let Some(boundary) = cursor {
        let page = catalog
            .list(&SessionQuery::default(), Some(&boundary), 128)
            .unwrap();
        seen.extend(
            page["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| row["id"].as_str().unwrap().to_owned()),
        );
        cursor = page["nextCursor"].as_str().map(str::to_owned);
    }
    assert_eq!(seen, ids);
    assert_eq!(seen.iter().collect::<HashSet<_>>().len(), 1000);
    let historical = catalog
        .list(
            &SessionQuery {
                ids: vec![ids[0].clone()],
                status: Some(ExecutionStatus::Unknown),
                ..SessionQuery::default()
            },
            None,
            20,
        )
        .unwrap();
    assert_eq!(historical["items"][0]["id"], ids[0]);
    assert!(historical["items"][0].get("execution").is_none());
    if let Some(root) = root {
        let reopened = SessionCatalog::new(Arc::new(PlaneStore::open(Some(root))));
        assert_eq!(
            reopened.list(&query, None, 20).unwrap()["items"][0]["execution"]["status"],
            "done"
        );
        let live = reopened
            .list(
                &SessionQuery {
                    ids: vec![ids[998].clone()],
                    ..SessionQuery::default()
                },
                None,
                20,
            )
            .unwrap();
        assert_eq!(live["items"][0]["execution"]["status"], "unknown");
    }
}

#[test]
fn complete_query_and_cursor_campaign_on_memory_and_disk() {
    query_campaign(false);
    query_campaign(true);
}

#[test]
fn retired_execution_and_failed_disk_writes_cannot_report_a_new_success() {
    let directory = tempfile::tempdir().unwrap();
    let store = Arc::new(PlaneStore::open(Some(directory.path().to_owned())));
    let catalog = SessionCatalog::new(store.clone());
    let row = catalog
        .create(&json!({ "name": "Original", "workspace": {"type":"system"} }))
        .unwrap();
    store
        .save_session_execution(&row.id, execution("old", ExecutionStatus::Pending), true)
        .unwrap();
    store
        .save_session_execution(
            &row.id,
            execution("current", ExecutionStatus::Pending),
            true,
        )
        .unwrap();
    store
        .save_session_execution(&row.id, execution("old", ExecutionStatus::Done), false)
        .unwrap();
    assert_eq!(
        store.session_executions()[&row.id].status,
        ExecutionStatus::Pending
    );
    let file = directory.path().join("session-executions.json");
    let backup = directory.path().join("execution-backup");
    std::fs::rename(&file, &backup).unwrap();
    std::fs::create_dir(&file).unwrap();
    assert!(
        store
            .save_session_execution(&row.id, execution("current", ExecutionStatus::Done), false)
            .is_err()
    );
    assert!(!store.session_executions().contains_key(&row.id));
    assert_eq!(
        catalog
            .list(
                &SessionQuery {
                    status: Some(ExecutionStatus::Unknown),
                    ..SessionQuery::default()
                },
                None,
                20
            )
            .unwrap()["items"][0]["id"],
        row.id
    );
    std::fs::remove_dir(&file).unwrap();
    std::fs::rename(&backup, &file).unwrap();
    store
        .save_session_execution(&row.id, execution("current", ExecutionStatus::Done), false)
        .unwrap();
    store
        .save_session_execution(
            &row.id,
            execution("current", ExecutionStatus::Streaming),
            false,
        )
        .unwrap();
    assert_eq!(
        store.session_executions()[&row.id].status,
        ExecutionStatus::Done
    );

    let file = directory.path().join("sessions.json");
    let backup = directory.path().join("session-backup");
    std::fs::rename(&file, &backup).unwrap();
    std::fs::create_dir(&file).unwrap();
    let mut changed = row.clone();
    changed.name = "Uncommitted".into();
    assert!(store.save_session(changed).is_err());
    assert_eq!(store.session(&row.id).unwrap().name, "Original");
    std::fs::remove_dir(&file).unwrap();
    std::fs::rename(backup, file).unwrap();
    store.remove_session(&row.id).unwrap();
    assert!(!store.session_executions().contains_key(&row.id));
    assert!(
        store
            .save_session_execution(
                &row.id,
                execution("missing", ExecutionStatus::Pending),
                true
            )
            .is_err()
    );
    assert!(
        PlaneStore::open(Some(directory.path().to_owned()))
            .session_executions()
            .is_empty()
    );
}

#[test]
fn real_http_query_rejects_invalid_state_dates_cursor_and_limit() {
    let base = start_in_memory();
    for query in [
        "status=success",
        "activityFrom=bad",
        "activityFrom=2026-10-02T00%3A00%3A00Z&activityBefore=2026-10-01T00%3A00%3A00Z",
        "cursor=garbage",
        "ids=%2C",
        "workspaceId=",
        "limit=0",
        "limit=201",
    ] {
        let (status, body) = request(
            &base,
            "GET",
            &format!("/api/agent-sessions?{query}"),
            Some(TOKEN),
            None,
        );
        assert_eq!(status, 400, "{query}: {body}");
    }
    let (_, body) = request(
        &base,
        "GET",
        "/api/agent-sessions?status=unknown",
        Some(TOKEN),
        None,
    );
    let page: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(page["total"], 0);
    assert_eq!(
        request(&base, "GET", "/api/agent-sessions", None, None).0,
        403
    );
    let (status, body) = request(
        &base,
        "POST",
        "/api/agent-sessions",
        Some(TOKEN),
        Some(r#"{"name":"中文计划%_","workspace":{"type":"system"}}"#),
    );
    assert_eq!(status, 201, "{body}");
    let created: Value = serde_json::from_str(&body).unwrap();
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("search", "中文计划%_")
        .append_pair("status", "unknown")
        .append_pair("activityFrom", created["lastActivityAt"].as_str().unwrap())
        .finish();
    let (status, body) = request(
        &base,
        "GET",
        &format!("/api/agent-sessions?{query}"),
        Some(TOKEN),
        None,
    );
    assert_eq!(status, 200, "{body}");
    let page: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(page["total"], 1);
    assert_eq!(page["items"][0]["id"], created["id"]);
    for (key, value) in [("search", "中".repeat(256)), ("cursor", "x".repeat(4097))] {
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair(key, &value)
            .finish();
        assert_eq!(
            request(
                &base,
                "GET",
                &format!("/api/agent-sessions?{query}"),
                Some(TOKEN),
                None
            )
            .0,
            400
        );
    }
}
