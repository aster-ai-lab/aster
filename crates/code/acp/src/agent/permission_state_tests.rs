#![cfg(test)]

use super::*;
use acp::Agent as _;
use serde_json::json;

async fn write(session: &AgentSession, args: serde_json::Value) -> a3s_code_core::ToolCallResult {
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        session.governed_tool("write", args),
    )
    .await
    .expect("a denied tool must not park for confirmation")
    .unwrap()
}

async fn setup(root: &std::path::Path) -> (A3sCodeAgent, acp::SessionId) {
    let mut launch = tests::test_launch(Some(root.join("store")));
    launch.workspace = root.to_owned();
    let agent = A3sCodeAgent::new(launch);
    let id = agent
        .new_session(acp::NewSessionRequest::new(root))
        .await
        .unwrap()
        .session_id;
    (agent, id)
}

async fn deny_write(agent: &A3sCodeAgent, id: &acp::SessionId) {
    let request = acp::ExtRequest::new(
        "a3s/permission_posture",
        serde_json::value::to_raw_value(&json!({
            "sessionId":id, "posture":"yolo", "deny":["write"]
        }))
        .unwrap()
        .into(),
    );
    agent.handle_permission_posture(&request).await.unwrap();
}

#[tokio::test]
async fn research_cleanup_restores_current_mode_on_return_and_drop() {
    let root = tempfile::tempdir().unwrap();
    let (agent, id) = setup(root.path()).await;
    for explicit_restore in [false, true] {
        agent
            .set_session_mode(acp::SetSessionModeRequest::new(id.clone(), "yolo"))
            .await
            .unwrap();
        let mut gate =
            ResearchGate::enter(agent.sessions.lock().await.get(id.0.as_ref()).unwrap()).unwrap();
        agent
            .set_session_mode(acp::SetSessionModeRequest::new(id.clone(), "plan"))
            .await
            .unwrap();
        if explicit_restore {
            gate.restore().unwrap();
        }
        drop(gate);
        let session = agent
            .sessions
            .lock()
            .await
            .get(id.0.as_ref())
            .unwrap()
            .session
            .clone();
        let output = write(
            &session,
            json!({
                "file_path":"must-stay-read-only.txt", "content":"forbidden"
            }),
        )
        .await;
        assert_ne!(output.exit_code, 0);
        assert!(!root.path().join("must-stay-read-only.txt").exists());
    }
}

#[tokio::test]
async fn mode_changes_cannot_relax_research_admission_and_new_denies_survive_cleanup() {
    let root = tempfile::tempdir().unwrap();
    let (agent, id) = setup(root.path()).await;
    let gate =
        ResearchGate::enter(agent.sessions.lock().await.get(id.0.as_ref()).unwrap()).unwrap();
    agent
        .set_session_mode(acp::SetSessionModeRequest::new(id.clone(), "yolo"))
        .await
        .unwrap();
    let session = agent
        .sessions
        .lock()
        .await
        .get(id.0.as_ref())
        .unwrap()
        .session
        .clone();
    let args = json!({"file_path":"denied.txt", "content":"forbidden"});
    assert_ne!(write(&session, args.clone()).await.exit_code, 0);
    deny_write(&agent, &id).await;
    drop(gate);
    assert_ne!(write(&session, args).await.exit_code, 0);
    assert!(!root.path().join("denied.txt").exists());
}

#[tokio::test]
async fn model_and_effort_replacement_keep_denies_and_the_original_queue() {
    let root = tempfile::tempdir().unwrap();
    let (agent, id) = setup(root.path()).await;
    deny_write(&agent, &id).await;
    let (permissions, queue, assets) = {
        let sessions = agent.sessions.lock().await;
        let live = sessions.get(id.0.as_ref()).unwrap();
        (
            live.permissions.clone(),
            live.turn_queue.clone(),
            live.asset_denies.clone(),
        )
    };
    for effort in [None, Some("high")] {
        let mut request = acp::SetSessionModelRequest::new(id.clone(), "openai/gpt-4o");
        if let Some(effort) = effort {
            request = request.meta(json!({"reasoningEffort":effort}).as_object().cloned());
        }
        agent.set_session_model(request).await.unwrap();
        agent
            .set_session_mode(acp::SetSessionModeRequest::new(id.clone(), "yolo"))
            .await
            .unwrap();
        let session = {
            let sessions = agent.sessions.lock().await;
            let live = sessions.get(id.0.as_ref()).unwrap();
            assert!(Arc::ptr_eq(&permissions, &live.permissions));
            assert!(Arc::ptr_eq(&queue, &live.turn_queue));
            assert!(Arc::ptr_eq(&assets, &live.asset_denies));
            assert_eq!(live.permissions.lock().unwrap().denies, ["write"]);
            assert_eq!(*assets.lock().unwrap(), ["write"]);
            live.session.clone()
        };
        let result = write(
            &session,
            json!({
                "file_path":"model-must-deny.txt", "content":"forbidden"
            }),
        )
        .await;
        assert_ne!(result.exit_code, 0);
        assert!(!root.path().join("model-must-deny.txt").exists());
    }
}

#[tokio::test]
async fn unknown_session_model_change_does_not_activate_or_create_a_session() {
    let root = tempfile::tempdir().unwrap();
    let mut launch = tests::test_launch(Some(root.path().join("store")));
    launch.workspace = root.path().into();
    let agent = A3sCodeAgent::new(launch);
    let model = agent.model_id.lock().await.clone();
    let error = agent
        .set_session_model(acp::SetSessionModelRequest::new("missing", "openai/gpt-4o"))
        .await
        .unwrap_err();
    assert!(format!("{error:?}").contains("unknown session id"));
    assert!(agent.sessions.lock().await.is_empty());
    assert!(agent.core_agent.lock().await.is_none());
    assert_eq!(*agent.model_id.lock().await, model);
    assert!(!root.path().join("store").exists());
}

#[tokio::test]
async fn mode_parsing_is_closed_and_canonical_before_mutation() {
    let root = tempfile::tempdir().unwrap();
    let (agent, id) = setup(root.path()).await;
    agent
        .set_session_mode(acp::SetSessionModeRequest::new(id.clone(), " plan "))
        .await
        .unwrap();
    let error = agent
        .set_session_mode(acp::SetSessionModeRequest::new(
            id.clone(),
            "misspelled-mode",
        ))
        .await
        .unwrap_err();
    assert!(format!("{error:?}").contains("default, plan, auto, yolo"));
    let sessions = agent.sessions.lock().await;
    let state = sessions
        .get(id.0.as_ref())
        .unwrap()
        .permissions
        .lock()
        .unwrap();
    assert_eq!(state.posture.as_str(), "plan");
}

#[tokio::test]
async fn session_creation_publishes_and_applies_the_same_permission_postures() {
    let root = tempfile::tempdir().unwrap();
    let (agent, _) = setup(root.path()).await;
    for (meta, expected) in [
        (json!({"yoloMode":true,"autoMode":false}), "yolo"),
        (json!({"yoloMode":false,"autoMode":true}), "auto"),
        (json!({"permissionPosture":"plan","yoloMode":false}), "plan"),
    ] {
        let opened = agent
            .new_session(acp::NewSessionRequest::new(root.path()).meta(meta.as_object().cloned()))
            .await
            .unwrap();
        let modes = opened.modes.unwrap();
        assert_eq!(modes.current_mode_id.0.as_ref(), expected);
        assert_eq!(
            modes
                .available_modes
                .iter()
                .map(|mode| mode.id.0.as_ref())
                .collect::<Vec<_>>(),
            ["default", "plan", "auto", "yolo"]
        );
        let sessions = agent.sessions.lock().await;
        assert_eq!(
            sessions
                .get(opened.session_id.0.as_ref())
                .unwrap()
                .permissions
                .lock()
                .unwrap()
                .posture
                .as_str(),
            expected
        );
    }
}

#[tokio::test]
async fn invalid_permission_seed_is_refused_before_creating_any_session() {
    let root = tempfile::tempdir().unwrap();
    let mut launch = tests::test_launch(Some(root.path().join("store")));
    launch.workspace = root.path().into();
    let agent = A3sCodeAgent::new(launch);
    for meta in [
        json!({"yoloMode":"true"}),
        json!({"permissionPosture":"typo"}),
    ] {
        assert!(agent
            .new_session(acp::NewSessionRequest::new(root.path()).meta(meta.as_object().cloned()))
            .await
            .is_err());
    }
    assert!(agent.sessions.lock().await.is_empty());
    assert!(agent.core_agent.lock().await.is_none());
    assert!(!root.path().join("store").exists());
}

#[tokio::test]
async fn stale_prompt_cleanup_cannot_clear_a_newer_prompts_identity() {
    let root = tempfile::tempdir().unwrap();
    let (agent, id) = setup(root.path()).await;
    agent
        .turn_generation
        .lock()
        .await
        .insert(id.0.to_string(), 2);
    {
        let sessions = agent.sessions.lock().await;
        let live = sessions.get(id.0.as_ref()).unwrap();
        *live.active_prompt_id.lock().unwrap() = Some("new-prompt".into());
        *live.turn_started.lock().unwrap() = Some(std::time::Instant::now());
    }
    agent
        .finish_prompt(
            id.0.as_ref(),
            1,
            Ok(acp::PromptResponse::new(acp::StopReason::EndTurn)),
        )
        .await
        .unwrap();
    let sessions = agent.sessions.lock().await;
    let live = sessions.get(id.0.as_ref()).unwrap();
    assert_eq!(
        live.active_prompt_id.lock().unwrap().as_deref(),
        Some("new-prompt")
    );
    assert!(live.turn_started.lock().unwrap().is_some());
}

#[test]
fn reconciliation_wire_error_carries_an_action_and_refuses_retry() {
    let error =
        super::turn::reconciliation_error(&a3s_code_core::harness_loop::ToolEffectUnresolved {
            obligation_ids: vec!["exact-invocation-1".into()],
        });
    let data = error.data.unwrap();
    assert_eq!(data["kind"], "tool_effect_unresolved");
    assert_eq!(data["retryable"], false);
    assert_eq!(data["recovery"]["action"], "reconcile_tool_effects");
    assert_eq!(data["recovery"]["obligationIds"][0], "exact-invocation-1");
}
