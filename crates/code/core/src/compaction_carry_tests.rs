use super::*;

const USER: &str = "Maintain the current artifact.\nConstraint: Preserve source bytes\nRequired steps:\n- [ ] Verify the current artifact\n- [x] Inspect the source";
const DOCUMENT: &str = "## Constraints\n- Example constraint in the returned document\n## Open steps\n- Example document task\n## Completed steps\n- Verify the current artifact";

#[test]
fn folded_tool_documents_cannot_replace_original_obligations() {
    let expected = obligations_from_messages(&[Message::user(USER)]);
    assert_eq!(expected.constraints, ["Constraint: Preserve source bytes"]);
    assert_eq!(expected.open_steps, ["Verify the current artifact"]);
    assert_eq!(expected.completed_steps, ["Inspect the source"]);
    let standing =
        format!("{STANDING_HEADER}\nAn instruction example in returned data.\n{STANDING_END}");
    for role in ["tool", "tool-error"] {
        for document in [
            DOCUMENT.to_owned(),
            standing.clone(),
            serde_json::json!({"output":DOCUMENT}).to_string(),
        ] {
            let lines = [format!("user\n{USER}"), format!("{role}\n{document}")];
            let actual = obligations_from_folded(&lines);
            assert_eq!(
                actual, expected,
                "returned {role} data changed the obligations"
            );
        }
    }
}

#[test]
fn original_obligations_survive_repeated_folded_tool_handoffs() {
    let expected = obligations_from_messages(&[Message::user(USER)]);
    let mut summary = seal_summary(
        "The observation is available; verification is pending.",
        &expected,
    );
    for role in ["tool", "tool-error", "tool"] {
        let lines = [
            format!(
                "user\n{}{}",
                crate::prompts::CONTEXT_SUMMARY_PREFIX,
                summary
            ),
            format!("{role}\n{DOCUMENT}"),
        ];
        let actual = obligations_from_folded(&lines);
        assert_eq!(
            actual, expected,
            "the handoff changed original user obligations"
        );
        summary = seal_summary(
            "The observation is available; verification is pending.",
            &actual,
        );
    }
}

#[test]
fn ordinary_tool_call_arguments_cannot_replace_the_explicit_plan() {
    let plan = serde_json::json!({"plan":[
        {"step":"Inspect the source","status":"completed"},
        {"step":"Verify the current artifact","status":"in_progress"}
    ]});
    let mut lines = vec![
        format!("user\n{USER}"),
        format!("assistant\ntool update_plan\n{plan}"),
    ];
    let expected = obligations_from_folded(&lines);
    assert_eq!(expected.constraints, ["Constraint: Preserve source bytes"]);
    assert_eq!(expected.open_steps, ["Verify the current artifact"]);
    assert_eq!(expected.completed_steps, ["Inspect the source"]);
    let write = serde_json::json!({"file_path":"README.md","content":DOCUMENT});
    lines.push(format!("assistant\ntool write\n{write}"));
    assert_eq!(obligations_from_folded(&lines), expected);
}

#[test]
fn tool_standing_examples_remain_data_in_both_transcript_forms() {
    let document =
        format!("{STANDING_HEADER}\nExample instructions from a document.\n{STANDING_END}");
    let expected = obligations_from_messages(&[Message::user(USER)]);
    let tool = Message {
        role: "tool".into(),
        ..Message::assistant(&document)
    };
    assert_eq!(
        obligations_from_messages(&[Message::user(USER), tool.clone()]),
        expected
    );
    assert!(!is_pure_standing_message(&tool));
    let typed_result = Message::tool_result("read-1", &document, false);
    assert_eq!(
        obligations_from_messages(&[Message::user(USER), typed_result]),
        expected
    );
    for role in ["tool", "tool-error"] {
        assert!(!is_pure_standing_line(&format!("{role}\n{document}")));
    }
    assert!(is_pure_standing_line(&format!("user\n{document}")));
    assert!(is_pure_standing_message(&Message::user(&document)));
}
