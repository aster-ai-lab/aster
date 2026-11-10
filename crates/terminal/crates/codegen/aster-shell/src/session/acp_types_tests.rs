use super::*;

#[test]
fn partial_snapshots_preserve_availability() {
    let snapshot = ContextInfo::from_notification(50, 1_000);
    assert!(!snapshot.breakdown_available);
    assert!(snapshot.has_usage());
    let wire = serde_json::to_value(&snapshot).unwrap();
    assert_eq!(wire["breakdownAvailable"], false);
    let restored: ContextInfo = serde_json::from_value(wire).unwrap();
    assert!(!restored.breakdown_available);
    assert_eq!(restored.used, 50);
}

#[test]
fn full_snapshots_keep_reported_details() {
    let snapshot: ContextInfo = serde_json::from_value(serde_json::json!({
        "used": 50,
        "total": 1_000,
        "systemPromptTokens": 20,
        "messageTokens": 30
    }))
    .unwrap();
    assert!(snapshot.breakdown_available);
    assert_eq!(snapshot.system_prompt_tokens, 20);
    assert_eq!(snapshot.message_tokens, 30);
}

#[test]
fn zero_usage_requires_a_measurement() {
    let mut snapshot = ContextInfo::default();
    assert!(!snapshot.has_usage());
    snapshot.usage_available = Some(true);
    assert!(snapshot.has_usage());
    snapshot.usage_available = Some(false);
    assert!(!snapshot.has_usage());
    assert!(ContextInfo::from_notification(0, 1_000).has_usage());
}
