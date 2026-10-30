#![cfg(test)]

use super::*;
use crate::agent::AgentEvent;

async fn pending_id(session: &AgentSession, previous: Option<&str>) -> String {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let pending: Vec<_> = session
                .pending_confirmations()
                .await
                .into_iter()
                .filter(|pending| previous != Some(pending.tool_id.as_str()))
                .collect();
            if let [request] = pending.as_slice() {
                return request.tool_id.clone();
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the actual original invocation must request confirmation")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_cancelled_runs_late_approval_cannot_authorize_a_reused_model_call_id() {
    reused_model_id(None, false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_answered_runs_old_approval_cannot_authorize_a_reused_model_call_id() {
    reused_model_id(Some(true), false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_rejected_runs_late_approval_cannot_authorize_a_reused_model_call_id() {
    reused_model_id(Some(false), false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unconfirmed_host_cleanup_cannot_let_an_abandoned_approval_authorize_a_new_run() {
    reused_model_id(None, true).await;
}

async fn reused_model_id(first_answer: Option<bool>, stalled_cleanup: bool) {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("source.txt"), "current source").unwrap();
    let (events, _) = broadcast::channel(8);
    let manager = ConfirmationManager::new(ConfirmationPolicy::enabled(), events);
    let (provider, stalled): (Arc<dyn ConfirmationProvider>, _) = if stalled_cleanup {
        let provider = Arc::new(StalledConfirmation {
            inner: manager,
            request: false,
            policy_stage: None,
            requires_calls: AtomicUsize::new(0),
            entered: AtomicBool::new(false),
            dropped: Arc::new(AtomicBool::new(false)),
            release: CancellationToken::new(),
        });
        (provider.clone(), Some(provider))
    } else {
        (Arc::new(manager), None)
    };
    let (agent, session, client) = fixture(
        root.path(),
        true,
        options("confirmation-identity")
            .with_permission_policy(
                crate::permissions::PermissionPolicy::new().ask("verify_source(*)"),
            )
            .with_confirmation_manager(provider),
    )
    .await;
    session
        .register_dynamic_tool(Arc::new(acceptance::VerifySource("current source".into())))
        .unwrap();
    acceptance::script(&client, ACHIEVED);
    let (mut first_events, first_worker) =
        session.stream("Inspect the source.", None).await.unwrap();
    let first_drain = tokio::spawn(async move {
        let mut observed = Vec::new();
        while let Some(event) = first_events.recv().await {
            observed.push(event);
        }
        observed
    });
    let old_id = pending_id(&session, None).await;
    if let Some(approved) = first_answer {
        assert!(session
            .confirm_tool_use(&old_id, approved, None)
            .await
            .unwrap());
    } else {
        assert!(session.cancel().await);
    }
    tokio::time::timeout(Duration::from_secs(5), first_worker)
        .await
        .unwrap()
        .unwrap();
    let first_events = first_drain.await.unwrap();
    if let Some(stalled) = &stalled {
        assert!(stalled.entered.load(Ordering::SeqCst));
        assert!(stalled.dropped.load(Ordering::SeqCst));
        assert_eq!(
            stalled.inner.pending_count().await,
            1,
            "a cleanup timeout cannot pretend the host retired its request"
        );
    }
    client.scripted.lock().unwrap().clear();
    acceptance::script(&client, ACHIEVED);

    let (mut next_events, mut next_worker) = session
        .stream("Inspect the source now.", None)
        .await
        .unwrap();
    let next_drain = tokio::spawn(async move {
        let mut observed = Vec::new();
        while let Some(event) = next_events.recv().await {
            observed.push(event);
        }
        observed
    });
    let current_id = pending_id(&session, Some(&old_id)).await;
    let late_accepted = session
        .confirm_tool_use(
            &old_id,
            true,
            Some("late answer to the cancelled Run".into()),
        )
        .await
        .unwrap();
    // Always settle the exact new owner, including a red run that accepted the old reply.
    let current_accepted = if !late_accepted {
        session
            .confirm_tool_use(
                &current_id,
                true,
                Some("approve the current invocation".into()),
            )
            .await
            .unwrap()
    } else {
        false
    };
    let joined = tokio::time::timeout(Duration::from_secs(5), &mut next_worker).await;
    if joined.is_err() {
        session.cancel().await;
        tokio::time::timeout(Duration::from_secs(3), next_worker)
            .await
            .unwrap()
            .unwrap();
    }
    let next_events = next_drain.await.unwrap();
    let facts =
        crate::fact_control::read_workspace_facts(root.path(), &session.session_id).unwrap();
    if let Some(stalled) = &stalled {
        stalled.release.cancel();
    }
    session.close().await;
    agent.close().await;
    eprintln!(
        "CONFIRMATION_IDENTITY_OBSERVATION reused_model_id=verify-current old_id={old_id} current_id={current_id} late_accepted={late_accepted} current_accepted={current_accepted}"
    );
    assert!(joined.is_ok_and(|joined| joined.is_ok()));
    assert_eq!(
        first_events
            .iter()
            .any(|event| matches!(event, AgentEvent::ToolExecutionStart { .. })),
        first_answer == Some(true)
    );
    assert!(next_events
        .iter()
        .any(|event| matches!(event, AgentEvent::ToolExecutionStart { .. })));
    assert_eq!(
        facts
            .iter()
            .filter(|fact| fact.kind == "tool.result"
                && fact.payload["receipt"]["outcome"] == "succeeded")
            .count(),
        1 + usize::from(first_answer == Some(true))
    );
    assert_ne!(
        old_id, current_id,
        "host approval identity must not be reused with the model ID"
    );
    assert!(
        !late_accepted,
        "a reply from an already cancelled Run granted new authority"
    );
    assert!(
        current_accepted,
        "a fresh approval must still let the new invocation finish"
    );
}
