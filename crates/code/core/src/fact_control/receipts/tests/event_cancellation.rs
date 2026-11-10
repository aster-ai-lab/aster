#![cfg(test)]

use super::*;
use crate::agent::AgentEvent;

#[tokio::test]
async fn settled_tool_receipts_survive_cancellation() {
    for (failed, incomplete) in [(false, false), (true, false), (false, true)] {
        let root = tempfile::tempdir().unwrap();
        let (tools, calls) = executor_tools(root.path());
        let prepared = prepare_tools(
            Arc::clone(&tools),
            request(
                "settled-after-cancel",
                json!({"settle_on_cancel": true, "write": true,
                    "fail": failed, "incomplete_evidence": incomplete}),
            ),
            Default::default(),
        )
        .await
        .unwrap();
        let original = prepared.prepared.clone();
        let cancellation = tools.context.cancellation_token();
        let invocation = tokio::spawn(execute_prepared(Arc::clone(&tools), prepared));
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while calls.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(!root.path().join("changed.txt").exists());
        cancellation.cancel();
        let result = tokio::time::timeout(std::time::Duration::from_secs(2), invocation)
            .await
            .unwrap()
            .unwrap();
        result.ensure_prepared(&original).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            std::fs::read(root.path().join("changed.txt")).unwrap(),
            b"exact bytes"
        );
        assert_eq!(
            result.outcome,
            if failed {
                ToolExecutionOutcome::Failed
            } else {
                ToolExecutionOutcome::Succeeded
            }
        );
        assert!(result
            .effects
            .changes
            .iter()
            .any(|change| change.path == "changed.txt"));
        assert_eq!(
            !result.effects.unresolved_external_operations.is_empty(),
            incomplete
        );
        assert_eq!(
            tools
                .ledger
                .as_ref()
                .unwrap()
                .lock()
                .unwrap()
                .has_uncertain_tool_invocations(),
            incomplete
        );
    }
}

#[tokio::test]
async fn tool_event_backpressure_ends_on_the_exact_run_token_and_keeps_fifo_order() {
    let root = tempfile::tempdir().unwrap();
    let (mut tools, _) = executor_tools(root.path());
    let (events, mut receiver) = tokio::sync::mpsc::channel(1);
    events
        .send(AgentEvent::TextDelta {
            text: "queued prefix".into(),
        })
        .await
        .unwrap();
    Arc::get_mut(&mut tools).unwrap().events = Some(events);
    let cancellation = tools.context.cancellation_token();
    let mut pending = Box::pin(tools.emit(AgentEvent::TextDelta {
        text: "blocked tail".into(),
    }));
    assert!(futures::poll!(&mut pending).is_pending());
    cancellation.cancel();
    tokio::time::timeout(std::time::Duration::from_millis(50), pending)
        .await
        .unwrap();
    assert!(
        matches!(receiver.recv().await, Some(AgentEvent::TextDelta { text }) if text == "queued prefix")
    );
    assert!(receiver.try_recv().is_err());
}

#[tokio::test]
async fn a_ready_tool_event_still_reaches_the_host_after_run_cancellation() {
    let root = tempfile::tempdir().unwrap();
    let (mut tools, _) = executor_tools(root.path());
    let (events, mut receiver) = tokio::sync::mpsc::channel(1);
    Arc::get_mut(&mut tools).unwrap().events = Some(events);
    tools.context.cancellation_token().cancel();
    tools
        .emit(AgentEvent::TextDelta {
            text: "available slot".into(),
        })
        .await;
    assert!(
        matches!(receiver.recv().await, Some(AgentEvent::TextDelta { text }) if text == "available slot")
    );
}
