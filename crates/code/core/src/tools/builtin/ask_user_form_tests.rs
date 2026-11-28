#![cfg(test)]
use super::*;

#[test]
fn malformed_documents_are_rejected_before_waiting_for_an_answer() {
    for form in [
        "",
        "not JSON",
        "root = Input(\"name\", \"Name\")",
        r#"<Card />"#,
        r#"{"root":"root","elements":{"root":{"type":"Widget","props":{}}}}"#,
    ] {
        assert!(check_form_shape(form).is_err());
    }
    assert!(check_form_shape(FORM).is_ok());
    assert!(check_form_shape(&"x".repeat(MAX_FORM_BYTES + 1)).is_err());
}

#[test]
fn tool_description_uses_the_shared_json_component_catalog() {
    let description = AskUserFormTool.description();
    assert!(description.contains(&aster_form_core::json_render_component_catalog().to_string()));
    assert!(description.contains("$bindState"));
    assert!(description.contains("root, elements"));
}

#[test]
fn model_definition_preserves_every_component_property_and_children_flag() {
    let tool = AskUserFormTool;
    let definition = tool.definition();
    let catalog = aster_form_core::json_render_component_catalog();
    assert_eq!(definition.parameters, tool.parameters());
    assert!(definition.description.starts_with(FORM_GUIDANCE));
    assert!(definition.description.len() < tool.description().len());

    let (_, catalog_text) = definition
        .description
        .split_once("Containers (children allowed): ")
        .unwrap();
    let (containers, leaves) = catalog_text.split_once(". Leaves: ").unwrap();
    let mut decoded = serde_json::Map::new();
    for (group, children) in [(containers, true), (leaves.trim_end_matches('.'), false)] {
        for entry in group.split_whitespace() {
            let (name, props) = entry.strip_suffix(')').unwrap().split_once('(').unwrap();
            let props: Vec<_> = props.split_terminator(',').collect();
            assert!(decoded
                .insert(
                    name.to_owned(),
                    serde_json::json!({"props": props, "children": children})
                )
                .is_none());
        }
    }
    assert_eq!(serde_json::Value::Object(decoded), catalog);
}

const FORM: &str = "{\"root\":\"root\",\"elements\":{\"root\":{\"type\":\"Card\",\"props\":{\"title\":\"Details\"},\"children\":[\"name\",\"go\"]},\"name\":{\"type\":\"Input\",\"props\":{\"label\":\"Name\",\"value\":{\"$bindState\":\"/form/name\"}}},\"go\":{\"type\":\"Button\",\"props\":{\"action\":\"submit\",\"label\":\"Continue\"}}},\"state\":{\"form\":{\"name\":\"\"}}}";

async fn run_form(answer: &'static str) -> ToolOutput {
    let root = tempfile::tempdir().unwrap();
    let session = format!("form-{}", uuid::Uuid::new_v4());
    let (event_tx, mut events) = tokio::sync::broadcast::channel(4);
    let ctx = ToolContext::new(root.path().to_path_buf())
        .with_session_id(&session)
        .with_agent_event_tx(event_tx);
    let host = tokio::spawn(async move {
        match events.recv().await.unwrap() {
            AgentEvent::UserQuestion {
                question_id, form, ..
            } => {
                assert_eq!(form.as_deref(), Some(FORM));
                ask_user::answer(&question_id, answer);
            }
            other => panic!("expected a form question, got {other:?}"),
        }
    });
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        AskUserFormTool.execute(&serde_json::json!({ "form": FORM }), &ctx),
    )
    .await
    .expect("the form call returns once the host answers")
    .unwrap();
    host.await.unwrap();
    output
}

#[tokio::test]
async fn submitted_values_are_the_tool_result() {
    let output = run_form(r#"{"name":"e2e","include":true}"#).await;
    assert!(output.success, "{}", output.content);
    let result: serde_json::Value = serde_json::from_str(&output.content).unwrap();
    assert_eq!(result["status"], "answered");
    assert_eq!(
        result["values"],
        serde_json::json!({"name":"e2e","include":true})
    );
    assert_eq!(result["permission_grant"], false);
}

#[tokio::test]
async fn a_host_without_values_fails_the_call_instead_of_hanging() {
    let output = run_form("null").await;
    assert!(!output.success);
    assert!(
        output.content.contains("no form values"),
        "{}",
        output.content
    );
}

#[tokio::test]
async fn a_rejected_renderer_returns_the_reason_without_claiming_submission() {
    let output = run_form(
        r#"["aster.code.user-form-error.v1","unsupported terminal form component: Metric"]"#,
    )
    .await;
    assert!(!output.success);
    assert!(output
        .content
        .contains("unsupported terminal form component: Metric"));
}
