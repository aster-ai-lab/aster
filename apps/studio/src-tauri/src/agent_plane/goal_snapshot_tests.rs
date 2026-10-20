#![cfg(test)]

use super::*;

fn wire(state: &str, tick: Value) -> Value {
    json!({"definition":{"binding":{"session_id":"ACP","goal_id":"G","goal_revision":2},
        "state":state,"goal":{"description":"合成目标"},"goal_sequence":3,"plan_revision":1},"tick":tick})
}

#[test]
fn typed_goal_snapshot_projects_native_state_without_prose() {
    for (state, tick) in [
        ("active", "advance"),
        ("paused", "paused"),
        ("waiting", "wait"),
        ("completed", "complete"),
    ] {
        let actual = project(wire(state, json!(tick)), "studio", "ACP", Some("run"));
        assert_eq!(actual["status"], "known");
        assert_eq!(actual["sessionId"], "studio");
        assert_eq!(actual["acpSessionId"], "ACP");
        assert_eq!(actual["executionId"], "run");
        assert_eq!(actual["goal"]["state"], state);
        assert_eq!(actual["goal"]["tick"], tick);
        assert_eq!(actual["goal"]["goalRevision"], 2);
        assert_eq!(actual["goal"]["planRevision"], 1);
        assert_eq!(actual["goal"]["goalSequence"], 3);
        assert!(actual.get("reason").is_none());
    }
    let cancelled = project(wire("cancelled", Value::Null), "studio", "ACP", None);
    assert_eq!(cancelled["status"], "known");
    assert_eq!(cancelled["goal"]["state"], "cancelled");
    assert_eq!(cancelled["goal"]["tick"], Value::Null);
}

#[test]
fn only_a_typed_empty_snapshot_clears_the_goal() {
    assert_eq!(
        project(
            json!({"definition":null,"tick":null}),
            "studio",
            "ACP",
            None
        )["status"],
        "known"
    );
    for value in [
        json!({}),
        json!({"definition":null}),
        json!({"tick":null}),
        json!({"definition":null,"tick":"advance"}),
        wire("active", Value::Null),
        wire("cancelled", json!("complete")),
        wire("unknown", json!("advance")),
        wire("active", json!("unknown")),
    ] {
        let actual = project(value, "studio", "ACP", None);
        assert_eq!(actual["status"], "unavailable");
        assert_eq!(actual["goal"], Value::Null);
        assert_eq!(actual["reason"], "invalid_snapshot");
    }
}

#[test]
fn foreign_binding_and_lossy_javascript_sequences_fail_closed() {
    for (pointer, value) in [
        ("/definition/binding/session_id", json!("foreign")),
        ("/definition/binding/goal_id", json!("")),
        ("/definition/binding/goal_revision", json!(0)),
        (
            "/definition/goal_sequence",
            json!(9_007_199_254_740_992_u64),
        ),
    ] {
        let mut input = wire("active", json!("advance"));
        *input.pointer_mut(pointer).unwrap() = value;
        assert_eq!(
            project(input, "studio", "ACP", Some("run"))["status"],
            "unavailable",
            "{pointer}"
        );
    }
}

#[tokio::test]
async fn a_cold_snapshot_does_not_start_a_process_or_restore_history_authority() {
    let root = tempfile::tempdir().unwrap();
    let (turns, store, sessions) = super::super::tests::plane_fixture(root.path());
    let agent = turns
        .agents
        .create(&json!({"name":"Synthetic","type":"a3s-code","model":"openai::fixture"}))
        .unwrap();
    let session = sessions
        .create(&json!({"agentId":agent.id,"workspace":{"type":"system"}}))
        .unwrap();
    assert!(store.messages(&session.id).is_empty());
    let snapshot = turns.goal_snapshot(&session.id).await.unwrap();
    assert_eq!(snapshot["status"], "unavailable");
    assert_eq!(snapshot["reason"], "runtime_unavailable");
    assert_eq!(snapshot["goal"], Value::Null);
    assert!(turns.runtimes.lock().await.is_empty());
    assert!(turns.goal_snapshot("foreign-session").await.is_err());
}

async fn warm_peer(name: &str) -> super::super::tests::Harness {
    use super::super::tests::{harness, message, until_done};
    let h = harness(name, "auto").expect("node fixture");
    let source = super::super::super::fake_acp::SCRIPT.replace(
        "if (m.method === '_a3s/goal/state') return send({ jsonrpc:'2.0',id:m.id,result:{definition:null,tick:null} })",
        "if (m.method === '_a3s/goal/state') { const p=path.join(path.dirname(process.argv[1]),'goal-count.json'); let v={active:0,max:0,total:0}; try {v=JSON.parse(fs.readFileSync(p,'utf8'))}catch(_){}; v.active++;v.total++;v.max=Math.max(v.max,v.active);fs.writeFileSync(p,JSON.stringify(v));setTimeout(()=>{let q=JSON.parse(fs.readFileSync(p,'utf8'));q.active--;fs.writeFileSync(p,JSON.stringify(q));send({jsonrpc:'2.0',id:m.id,result:{definition:null,tick:null}})},60);return; }",
    );
    std::fs::write(h.dir.join("fake-acp"), source).unwrap();
    let mut events = h.hub.subscribe();
    h.turns
        .open(&h.session_id, &message("warm fixture"))
        .unwrap();
    until_done(&mut events, |_| {}).await;
    h
}

#[tokio::test]
async fn native_alignment_goal_reads_do_not_overlap_for_the_same_runtime() {
    use futures_util::FutureExt;
    let h = warm_peer("native-goal-serial").await;
    let result = std::panic::AssertUnwindSafe(async {
        let (a, b) = tokio::join!(
            h.turns.goal_snapshot(&h.session_id),
            h.turns.goal_snapshot(&h.session_id)
        );
        assert_eq!(a.unwrap()["status"], "known");
        assert_eq!(b.unwrap()["status"], "known");
        let count: Value =
            serde_json::from_slice(&std::fs::read(h.dir.join("goal-count.json")).unwrap()).unwrap();
        assert_eq!(
            count["max"], 1,
            "a delayed older RPC cannot publish after a newer RPC"
        );
    })
    .catch_unwind()
    .await;
    h.turns.close_test_runtimes().await;
    let _ = std::fs::remove_dir_all(h.dir);
    if let Err(error) = result {
        std::panic::resume_unwind(error)
    }
}

#[tokio::test]
async fn native_alignment_goal_response_cannot_cross_an_idle_nonlocal_admission() {
    use futures_util::FutureExt;
    let h = warm_peer("native-goal-admission").await;
    let result = std::panic::AssertUnwindSafe(async {
        let count = || {
            serde_json::from_slice::<Value>(&std::fs::read(h.dir.join("goal-count.json")).unwrap())
                .unwrap()
        };
        let before = count()["total"].as_u64().unwrap();
        let turns = h.turns.clone();
        let id = h.session_id.clone();
        let read = tokio::spawn(async move { turns.goal_snapshot(&id).await.unwrap() });
        tokio::time::timeout(Duration::from_secs(1), async {
            while count()["total"].as_u64().unwrap() == before {
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        })
        .await
        .unwrap();
        let next = Execution {
            session_id: h.session_id.clone(),
            topic_id: format!("agent-session:{}", h.session_id),
            execution_id: "nonlocal-admission".into(),
            attempt_id: 1,
            anchor_id: "new-anchor".into(),
        };
        h.turns.reset_goal_snapshot(&next);
        assert_eq!(read.await.unwrap()["reason"], "stale_binding");
        assert_eq!(
            h.turns
                .cached_goal_snapshot(&h.session_id, "nonlocal-admission")["reason"],
            "runtime_unavailable"
        );
        let calls = count()["total"].as_u64().unwrap();
        assert_eq!(
            h.turns.goal_snapshot(&h.session_id).await.unwrap()["reason"],
            "runtime_unavailable"
        );
        assert_eq!(
            count()["total"],
            calls,
            "retained old local process is not authority after nonlocal admission"
        );
    })
    .catch_unwind()
    .await;
    h.turns.close_test_runtimes().await;
    let _ = std::fs::remove_dir_all(h.dir);
    if let Err(error) = result {
        std::panic::resume_unwind(error)
    }
}

#[tokio::test]
async fn native_alignment_goal_read_deadline_includes_the_runtime_registry_lock() {
    let h = warm_peer("native-goal-bounded-lock").await;
    use futures_util::FutureExt;
    let result = std::panic::AssertUnwindSafe(async {
        let guard = h.turns.runtimes.lock().await;
        let result = tokio::time::timeout(
            Duration::from_millis(1400),
            h.turns.goal_snapshot(&h.session_id),
        )
        .await;
        assert!(
            result.is_ok(),
            "registry contention is part of the bounded read budget"
        );
        let snapshot = result.unwrap().unwrap();
        assert_eq!(snapshot["status"], "unavailable");
        assert_eq!(snapshot["reason"], "request_failed");
        drop(guard);
    })
    .catch_unwind()
    .await;
    h.turns.close_test_runtimes().await;
    let _ = std::fs::remove_dir_all(h.dir);
    if let Err(error) = result {
        std::panic::resume_unwind(error)
    }
}

#[test]
fn native_alignment_goal_plan_revision_requires_an_explicit_nullable_field() {
    let mut input = wire("active", json!("needs_input"));
    input["definition"]
        .as_object_mut()
        .unwrap()
        .remove("plan_revision");
    assert_eq!(
        project(input.clone(), "studio", "ACP", None)["reason"],
        "invalid_snapshot"
    );
    input["definition"]["plan_revision"] = Value::Null;
    assert_eq!(project(input, "studio", "ACP", None)["status"], "known");
}
