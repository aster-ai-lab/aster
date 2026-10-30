#![cfg(test)]

use super::*;

fn unresolved() -> AgentEvent {
    AgentEvent::ToolEffectUnresolved {
        error: crate::harness_loop::ToolEffectUnresolved {
            obligation_ids: vec!["original-native-call".into()],
        },
    }
}

#[tokio::test]
async fn unresolved_tool_effect_is_a_failed_terminal_with_retained_obligations() {
    let store = InMemoryRunStore::new();
    let created = store.create_run("session", "actual pending effect").await;
    let event = unresolved();
    let snapshot = store.record_event(&created.id, event).await.unwrap();
    assert_eq!(snapshot.status, RunStatus::Failed);
    let message = snapshot.error.as_deref().unwrap();
    assert!(message.starts_with("tool_effect_unresolved: 1 invocation obligation(s)"));
    assert!(message.contains("identity-bound reconciliation"));
    assert!(snapshot.result_text.is_none());
    assert_eq!(snapshot.event_count, 1);
    let events = store.events(&created.id).await;
    assert!(
        matches!(&events[0].event, AgentEvent::ToolEffectUnresolved { error } if error.obligation_ids == ["original-native-call"])
    );
}

#[tokio::test]
async fn late_unresolved_effect_cannot_rewrite_a_cancelled_terminal() {
    let store = InMemoryRunStore::new();
    let created = store.create_run("session", "cancelled work").await;
    store.mark_cancelled(&created.id).await.unwrap();
    let snapshot = store.record_event(&created.id, unresolved()).await.unwrap();
    assert_eq!(snapshot.status, RunStatus::Cancelled);
    assert!(snapshot.error.is_none());
    assert_eq!(snapshot.event_count, 1);
}
