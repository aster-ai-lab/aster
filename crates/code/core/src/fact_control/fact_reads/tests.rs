use super::*;

#[test]
fn manual_compaction_keeps_public_tool_progress_without_duplicate_or_private_text() {
    let call = |id: &str, text: Option<&str>| aster_effect::ToolCall {
        id: id.into(),
        name: "read".into(),
        args: serde_json::json!({"file_path": format!("{id}.txt")}),
        needs_confirmation: false,
        text: text.map(str::to_owned),
        reasoning: Some("synthetic-private-reasoning".into()),
    };
    let decision = ModelDecision::Tool {
        call: call("first", Some("Observed copper=83, lavender=17.")),
        also: vec![
            call("second", Some("Observed copper=83, lavender=17.")),
            call("third", Some("Next inspect the remaining source.")),
            call("fourth", Some("")),
            call("fifth", None),
        ],
    };
    let fact = aster_effect::Fact {
        seq: 0,
        kind: "model.turn".into(),
        key: "public-progress".into(),
        cause: None,
        payload: serde_json::to_value(decision).unwrap(),
    };
    let messages = compaction_transcript_from_facts(&[fact]);
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].role, "assistant");
    let text = messages[0].text();
    assert_eq!(text.matches("Observed copper=83, lavender=17.").count(), 1);
    assert_eq!(
        text.matches("Next inspect the remaining source.").count(),
        1
    );
    assert!(!text.contains("synthetic-private-reasoning"));
    for id in ["first", "second", "third", "fourth", "fifth"] {
        assert!(text.contains(&format!("Tool call read ({id}):")));
    }
}
