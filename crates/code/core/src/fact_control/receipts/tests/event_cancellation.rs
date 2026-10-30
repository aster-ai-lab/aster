#![cfg(test)]

use super::*;
use crate::agent::AgentEvent;

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
