use super::*;

fn definition(name: &str) -> ToolDefinition {
    let mut properties = serde_json::Map::from_iter([
        (
            "mode".into(),
            json!({"enum": ["fast", "exact"], "description": "Select the working mode."}),
        ),
        (
            "offset".into(),
            json!({"type": "integer", "description": "0-based offset."}),
        ),
    ]);
    properties.extend((0..128).map(|index| (format!("field_{index}"), json!({"type": "string"}))));
    ToolDefinition {
        name: name.into(),
        description: format!("Read a record.\n{}", "Complete documentation.\n".repeat(8)),
        parameters: json!({"type": "object", "properties": properties, "required": ["mode"]}),
    }
}

#[test]
fn projection_is_deterministic_and_preserves_required_values_and_short_field_notes() {
    let source = [definition("read_record")];
    let expanded = BTreeSet::new();
    let mut tools = project(&source, &expanded).unwrap();
    assert_eq!(tools[0].parameters["required"], json!(["mode"]));
    assert_eq!(
        tools[0].parameters["properties"]["mode"]["enum"],
        json!(["fast", "exact"])
    );
    assert!(tools[0].description.contains("0-based offset."));
    assert!(tools[0].description.starts_with(&source[0].description));
    assert!(verify(&source, &tools));
    tools[0].name = "foreign_record".into();
    assert!(!verify(&source, &tools));
    assert!(project(&[definition(LOOKUP_NAME)], &expanded).is_none());
    assert!(project(&[], &expanded).is_none());
}

#[test]
fn savings_compare_against_the_existing_wire_projection() {
    let mut small = definition("read_record");
    small.description = "Read a record.".into();
    small.parameters = json!({
        "type": "object",
        "properties": {"offset": {"type": "integer"}},
        "examples": ["x".repeat(4096)],
    });
    assert!(
        project(&[small], &BTreeSet::new()).is_none(),
        "existing wire encoding already omits examples"
    );
}

#[test]
fn field_notes_that_do_not_fit_keep_the_exact_tool_without_expanding_its_neighbors() {
    let plain = definition("read_record");
    let mut rich = definition("read_report");
    rich.parameters["properties"]["offset"]["description"] =
        json!("0-based offset.\nFor archived reports, use the preceding cursor.");
    let source = [plain, rich];
    let tools = project(&source, &BTreeSet::new()).unwrap();
    assert_ne!(tools[0].parameters, source[0].parameters);
    assert_eq!(tools[1].description, source[1].description);
    assert_eq!(tools[1].parameters, source[1].parameters);
    assert!(verify(&source, &tools));
}

#[test]
fn lookup_expands_exact_current_definitions_only_in_the_native_tool_list() {
    let source = [definition("read_record"), definition("write_record")];
    let mut expanded = BTreeSet::new();
    let mut call = ToolCall {
        id: "lookup".into(),
        name: LOOKUP_NAME.into(),
        args: json!({"names": ["read_record", "hidden_record"]}),
    };
    assert!(lookup(&source, &call, &[], &mut expanded).is_none());
    assert!(expanded.is_empty());
    call.args = json!({"names": ["read_record"]});
    let result = lookup(&source, &call, &[], &mut expanded).unwrap();
    assert!(result.transcript_visibility.is_wire());
    let encoded = serde_json::to_value(result).unwrap();
    assert_eq!(encoded["content"][0]["tool_use_id"], "lookup");
    let acknowledgement = encoded["content"][0]["content"].as_str().unwrap();
    assert!(acknowledgement.contains("current tool list"));
    assert!(!acknowledgement.contains("Complete documentation."));
    let compact = project(&source, &BTreeSet::new()).unwrap();
    let mut projected = project(&source, &expanded).unwrap();
    assert_eq!(projected[0].description, source[0].description);
    assert_eq!(projected[0].parameters, source[0].parameters);
    assert_eq!(projected[1].description, compact[1].description);
    assert_eq!(projected[1].parameters, compact[1].parameters);
    assert!(verify(&source, &projected));
    projected[0].parameters["properties"]["mode"]["enum"] = json!(["altered"]);
    assert!(!verify(&source, &projected));
    assert!(lookup(&source, &call, &[], &mut expanded).is_none());

    // If expanding this definition uses the remaining saving, the real full
    // tool list takes its place. The earlier tool acknowledgement stays valid.
    assert!(project(&source[..1], &expanded).is_none());
}
