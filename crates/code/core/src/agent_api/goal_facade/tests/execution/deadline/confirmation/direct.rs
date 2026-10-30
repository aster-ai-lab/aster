#![cfg(test)]

use super::*;
use crate::agent::AgentEvent;
use crate::tool_confirmation::{
    ToolConfirmationRequest, ToolConfirmationResolution, ToolConfirmationRuntime,
};
use tokio::sync::mpsc;

#[tokio::test]
async fn the_shared_runtime_cancels_both_pending_provider_stages_without_claiming_cleanup() {
    for request_stage in [true, false] {
        let (events, _) = broadcast::channel(8);
        let provider = StalledConfirmation {
            inner: ConfirmationManager::new(ConfirmationPolicy::enabled(), events),
            request: request_stage,
            policy_stage: None,
            requires_calls: AtomicUsize::new(0),
            entered: AtomicBool::new(false),
            dropped: Arc::new(AtomicBool::new(false)),
            release: CancellationToken::new(),
        };
        let cancellation = CancellationToken::new();
        let (events, mut receiver) = mpsc::channel(8);
        let runtime = ToolConfirmationRuntime::new(&provider, Some(&events));
        let args = serde_json::json!({});
        let mut pending = Box::pin(runtime.resolve_with_cancellation(
            ToolConfirmationRequest {
                tool_id: "owned",
                tool_name: "write",
                args: &args,
                timeout_ms: 30_000,
                timeout_action: crate::hitl::TimeoutAction::AutoApprove,
            },
            &cancellation,
        ));
        assert!(futures::poll!(&mut pending).is_pending());
        assert_eq!(provider.inner.pending_count().await, 1);
        cancellation.cancel();
        let resolution = tokio::time::timeout(Duration::from_millis(1_500), &mut pending)
            .await
            .unwrap();
        let ToolConfirmationResolution::Rejected { output } = resolution else {
            panic!("cancellation cannot approve")
        };
        assert!(provider.dropped.load(Ordering::SeqCst));
        assert_eq!(
            output.contains("confirmation.cancel_unresolved"),
            !request_stage
        );
        assert_eq!(
            provider.inner.pending_count().await,
            usize::from(!request_stage)
        );
        let mut observed = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            observed.push(event);
        }
        assert!(!observed
            .iter()
            .any(|event| matches!(event, AgentEvent::Error { .. })));
        if request_stage {
            assert!(
                observed.is_empty(),
                "an unfinished registration never announced a host request"
            );
        }
        if !request_stage {
            assert!(observed.iter().any(|event| matches!(event, AgentEvent::ConfirmationTimeout { action_taken, .. } if action_taken == "cancel_unresolved")));
            assert!(!observed
                .iter()
                .any(|event| matches!(event, AgentEvent::ConfirmationReceived { .. })));
        }
        drop(pending);
        provider.inner.cancel_all().await;
    }
}

#[tokio::test]
async fn confirmation_backpressure_is_cancellable_without_dropping_the_queued_prefix() {
    let (broadcast, _) = broadcast::channel(8);
    let manager = ConfirmationManager::new(ConfirmationPolicy::enabled(), broadcast);
    let (events, mut receiver) = mpsc::channel(1);
    events
        .send(AgentEvent::TextDelta {
            text: "queued prefix".into(),
        })
        .await
        .unwrap();
    let runtime = ToolConfirmationRuntime::new(&manager, Some(&events));
    let cancellation = CancellationToken::new();
    let args = serde_json::json!({});
    let mut pending = Box::pin(runtime.resolve_with_cancellation(
        ToolConfirmationRequest {
            tool_id: "owned",
            tool_name: "write",
            args: &args,
            timeout_ms: 30_000,
            timeout_action: crate::hitl::TimeoutAction::Reject,
        },
        &cancellation,
    ));
    assert!(futures::poll!(&mut pending).is_pending());
    assert_eq!(manager.pending_count().await, 1);
    cancellation.cancel();
    let resolution = tokio::time::timeout(Duration::from_millis(50), &mut pending)
        .await
        .unwrap();
    assert!(matches!(
        resolution,
        ToolConfirmationResolution::Rejected { .. }
    ));
    assert_eq!(manager.pending_count().await, 0);
    assert!(
        matches!(receiver.recv().await, Some(AgentEvent::TextDelta { text }) if text == "queued prefix")
    );
    assert!(receiver.try_recv().is_err());
}

#[tokio::test]
async fn an_already_cancelled_call_never_registers_or_cancels_an_existing_request() {
    let (events, _) = broadcast::channel(8);
    let manager = ConfirmationManager::new(ConfirmationPolicy::enabled(), events);
    let args = serde_json::json!({});
    let untouched = manager
        .request_confirmation("same-id", "write", &args)
        .await;
    let runtime = ToolConfirmationRuntime::new(&manager, None);
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let resolution = runtime
        .resolve_with_cancellation(
            ToolConfirmationRequest {
                tool_id: "same-id",
                tool_name: "write",
                args: &args,
                timeout_ms: 30_000,
                timeout_action: crate::hitl::TimeoutAction::AutoApprove,
            },
            &cancellation,
        )
        .await;
    assert!(matches!(
        resolution,
        ToolConfirmationResolution::Rejected { .. }
    ));
    assert_eq!(manager.pending_count().await, 1);
    manager.confirm("same-id", true, None).await.unwrap();
    assert!(untouched.await.unwrap().approved);
}
