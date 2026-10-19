#![cfg(test)]

use std::sync::Arc;

use serde_json::json;

use super::catalog::SessionCatalog;
use super::session_query::SessionQuery;
use super::store::PlaneStore;

#[test]
fn a_knowledge_compile_session_stays_off_the_task_list() {
    let directory = tempfile::tempdir().unwrap();
    let store = Arc::new(PlaneStore::open(Some(directory.path().join("state"))));
    let catalog = SessionCatalog::new(store);
    let compile = catalog
        .create(&json!({
            "agentId": "code",
            "name": "知识编译",
            "description": "knowledge-compile",
            "workspace": { "type": "system" }
        }))
        .unwrap();
    let task = catalog
        .create(&json!({
            "agentId": "code",
            "name": "写一页",
            "workspace": { "type": "system" }
        }))
        .unwrap();

    let page = catalog.list(&SessionQuery::default(), None, 20).unwrap();
    let ids: Vec<_> = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|row| row["id"].as_str())
        .collect();
    assert_eq!(ids, vec![task.id.as_str()]);
    assert_eq!(
        catalog.get(&compile.id).unwrap().description.as_deref(),
        Some("knowledge-compile")
    );
    assert!(catalog.latest(None)["session"]["id"].as_str().unwrap() == task.id);
}

fn user_workspace(path: &str) -> serde_json::Value {
    json!({
        "type": "user",
        "workspace": {
            "id": "w1",
            "name": "proj",
            "path": path,
            "type": "user",
            "orderKey": "a0",
            "createdAt": "2026-01-01T00:00:00.000Z",
            "updatedAt": "2026-01-01T00:00:00.000Z",
        },
    })
}

#[test]
fn a_personal_vault_session_stays_off_the_task_list() {
    let directory = tempfile::tempdir().unwrap();
    let store = Arc::new(PlaneStore::open(Some(directory.path().join("state"))));
    let catalog = SessionCatalog::new(store);
    let vault = catalog
        .create(&json!({
            "agentId": "code",
            "name": "写笔记",
            "workspace": user_workspace("/tmp/wiki/local")
        }))
        .unwrap();
    let trailing = catalog
        .create(&json!({
            "agentId": "code",
            "name": "再写",
            "workspace": user_workspace("/tmp/wiki/local/")
        }))
        .unwrap();
    let named = catalog
        .create(&json!({
            "agentId": "code",
            "name": "知识编译",
            "workspace": user_workspace("/tmp/proj")
        }))
        .unwrap();
    let nested = catalog
        .create(&json!({
            "agentId": "code",
            "name": "库内文件",
            "workspace": user_workspace("/tmp/wiki/local/note.md")
        }))
        .unwrap();
    let locale = catalog
        .create(&json!({
            "agentId": "code",
            "name": "另一种目录",
            "workspace": user_workspace("/tmp/wiki/locale")
        }))
        .unwrap();

    let page = catalog.list(&SessionQuery::default(), None, 20).unwrap();
    let mut ids: Vec<_> = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|row| row["id"].as_str())
        .collect();
    ids.sort();
    let mut expected = vec![named.id.as_str(), nested.id.as_str(), locale.id.as_str()];
    expected.sort();
    assert_eq!(ids, expected);
    assert!(catalog.get(&vault.id).is_ok());
    assert_ne!(
        catalog.latest(None)["session"]["id"].as_str(),
        Some(vault.id.as_str())
    );
    assert_ne!(
        catalog.latest(None)["session"]["id"].as_str(),
        Some(trailing.id.as_str())
    );

    let mut addressed = SessionQuery::default();
    addressed.ids = vec![vault.id.clone()];
    let direct = catalog.list(&addressed, None, 20).unwrap();
    assert_eq!(direct["items"][0]["id"].as_str(), Some(vault.id.as_str()));
}
