use super::*;
use crate::hitl::{ConfirmationManager, ConfirmationPolicy};

#[tokio::test]
async fn missing_or_withdrawn_confirmation_with_bound_policy_is_a_non_execution() {
    let workspace = tempfile::tempdir().unwrap();
    let agent = agent().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let (events, _) = tokio::sync::broadcast::channel(8);
    let manager = Arc::new(ConfirmationManager::new(
        ConfirmationPolicy::enabled(),
        events,
    ));
    let session = agent
        .build_session(
            workspace.path().display().to_string(),
            Arc::new(CountingText {
                calls: calls.clone(),
            }),
            &options("policy-missing-confirmation")
                .with_work_policy(
                    crate::WorkPolicyV1::new("engineering", 1, "Controlled original-owner policy.")
                        .unwrap(),
                )
                .with_confirmation_manager(manager.clone()),
        )
        .unwrap();
    let sent = session
        .send("Bind the controlled original policy", None)
        .await;
    let before = read_workspace_facts(workspace.path(), "policy-missing-confirmation");
    let calls_before = calls.load(Ordering::SeqCst);
    let live_receiver = manager
        .request_confirmation(
            "live",
            "bash",
            &serde_json::json!({"command":"controlled-unused"}),
        )
        .await;
    let live = session.confirm_tool_use("live", true, None).await;
    let accepted = tokio::time::timeout(std::time::Duration::from_millis(100), live_receiver).await;
    let missing = session.confirm_tool_use("missing", true, None).await;
    let withdrawn_receiver = manager
        .request_confirmation(
            "withdrawn",
            "bash",
            &serde_json::json!({"command":"controlled-unused"}),
        )
        .await;
    let withdrawn = manager.cancel("withdrawn").await;
    let rejected = withdrawn_receiver.await;
    let late = session.confirm_tool_use("withdrawn", true, None).await;
    let after = read_workspace_facts(workspace.path(), "policy-missing-confirmation");
    let calls_after = calls.load(Ordering::SeqCst);
    session.close().await;
    agent.close().await;

    assert!(
        sent.is_ok(),
        "original owner must admit the real policy: {sent:?}"
    );
    let before = before.unwrap();
    assert!(before
        .iter()
        .any(|fact| fact.kind == "a3s.work-policy.admitted"));
    assert!(
        matches!(live, Ok(true)),
        "the bound session's live receiver still owns approval: {live:?}"
    );
    assert!(accepted.unwrap().unwrap().approved);
    assert!(withdrawn);
    assert!(!rejected.unwrap().approved);
    assert!(
        matches!(missing, Ok(false)),
        "missing id is not a new legacy owner: {missing:?}"
    );
    assert!(
        matches!(late, Ok(false)),
        "withdrawn id is not a new legacy owner: {late:?}"
    );
    assert_eq!(
        before,
        after.unwrap(),
        "no confirmation or execution fact may be appended"
    );
    assert_eq!(
        calls_before, calls_after,
        "no provider call may be made by a stale answer"
    );
}

#[tokio::test]
async fn unbound_legacy_fact_confirmation_survives_a_missing_manager_receiver() {
    for with_manager in [false, true] {
        let workspace = tempfile::tempdir().unwrap();
        let agent = agent().await;
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut options = options("legacy-pending-confirmation")
            .with_permission_policy(crate::permissions::PermissionPolicy::new());
        if with_manager {
            let (events, _) = tokio::sync::broadcast::channel(8);
            options = options.with_confirmation_manager(Arc::new(ConfirmationManager::new(
                ConfirmationPolicy::enabled(),
                events,
            )));
        }
        let session = agent
            .build_session(
                workspace.path().display().to_string(),
                Arc::new(ScriptedClient {
                    seen: seen.clone(),
                    responses: std::sync::Mutex::new(vec![
                        tool_response(
                            "legacy-pending",
                            "read",
                            serde_json::json!({"file_path":"unused.txt"}),
                        ),
                        text_response("done"),
                    ]),
                }),
                &options,
            )
            .unwrap();
        let parked = {
            let run = super::super::super::super::conversation_runtime::FactSession::from(&session)
                .open()
                .unwrap();
            run.user_text("Park a controlled legacy read").await
        };
        let before = read_workspace_facts(workspace.path(), "legacy-pending-confirmation").unwrap();
        let exact = session
            .confirm_tool_use("legacy-pending", false, None)
            .await;
        let after = read_workspace_facts(workspace.path(), "legacy-pending-confirmation");
        let requests = seen.lock().unwrap().len();
        session.close().await;
        agent.close().await;

        assert_eq!(parked.unwrap().view.phase, a3s_effect::CodingPhase::Confirm);
        assert!(
            matches!(exact, Ok(true)),
            "a valid unbound legacy wait must remain answerable: {exact:?}"
        );
        let after = after.unwrap();
        assert_eq!(after.len(), before.len() + 1);
        assert_eq!(after.last().unwrap().kind, "confirmation.answered");
        assert_eq!(after.last().unwrap().payload["approved"], false);
        assert!(after
            .iter()
            .all(|fact| fact.kind != "tool.prepared" && fact.kind != "tool.result"));
        assert_eq!(
            requests, 1,
            "denying the parked call must not invoke another provider"
        );
    }
}
