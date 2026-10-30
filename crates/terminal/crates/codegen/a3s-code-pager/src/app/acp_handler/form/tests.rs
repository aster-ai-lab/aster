use super::*;
use crate::views::elicitation_view::{ElicitationStage, ElicitationViewState, FieldValueUi};

const FORM: &str = r#"{"root":"root","elements":{"root":{"type":"Card","props":{"title":"Details"},"children":["name","amount","ready","choice","notes","submit"]},"name":{"type":"Input","props":{"label":"Name","type":"text","required":true,"value":{"$bindState":"/form/name"}}},"amount":{"type":"Input","props":{"label":"Amount","type":"number","min":1,"max":5,"value":{"$bindState":"/form/amount"}}},"ready":{"type":"Checkbox","props":{"label":"Ready","required":true,"checked":{"$bindState":"/form/ready"}}},"choice":{"type":"Select","props":{"label":"Choice","options":["alpha","beta"],"required":true,"value":{"$bindState":"/form/choice"}}},"notes":{"type":"Textarea","props":{"label":"Notes","value":{"$bindState":"/form/notes"}}},"submit":{"type":"Button","props":{"action":"submit","label":"Continue"}}},"state":{"form":{"name":"","amount":3,"ready":false,"choice":"beta","notes":""}}}"#;

fn view(compiled: &Compiled) -> ElicitationViewState {
    ElicitationViewState::from_request(
        McpElicitExtRequest {
            session_id: "session-1".into(),
            tool_call_id: "form-1".into(),
            server_name: "Aster".into(),
            message: compiled.message.clone(),
            require_nonempty: true,
            mode: McpElicitModeFields::Form {
                requested_schema: Some(compiled.schema.clone()),
            },
        },
        None,
        None,
    )
}

#[test]
fn existing_native_form_keeps_invalid_fields_open_and_returns_typed_values() {
    let compiled = compile(FORM, "Fill in the details").expect("valid form");
    let mut view = view(&compiled);
    let ElicitationStage::Form(form) = &view.stage else {
        panic!("native form")
    };
    assert_eq!(
        form.fields
            .iter()
            .map(|field| field.spec.name.as_str())
            .collect::<Vec<_>>(),
        ["name", "amount", "ready", "choice", "notes"]
    );
    assert!(
        view.try_accept().is_none(),
        "required fields must block submission"
    );
    let ElicitationStage::Form(form) = &mut view.stage else {
        panic!("native form")
    };
    assert!(
        form.fields
            .iter()
            .filter(|field| field.error.is_some())
            .count()
            >= 2
    );
    for field in &mut form.fields {
        match field.spec.name.as_str() {
            "name" => {
                field.value = FieldValueUi::Text {
                    draft: "   ".into(),
                }
            }
            "ready" => field.value = FieldValueUi::Toggle { on: true },
            _ => {}
        }
    }
    assert!(
        view.try_accept().is_none(),
        "whitespace cannot satisfy a required input"
    );
    let ElicitationStage::Form(form) = &mut view.stage else {
        panic!("native form")
    };
    for field in &mut form.fields {
        if field.spec.name == "name" {
            field.value = FieldValueUi::Text {
                draft: "Ada".into(),
            };
        }
        if field.spec.name == "amount" {
            field.value = FieldValueUi::Text { draft: "6".into() };
        }
    }
    assert!(
        view.try_accept().is_none(),
        "numeric bounds must be enforced"
    );
    let ElicitationStage::Form(form) = &mut view.stage else {
        panic!("native form")
    };
    for field in &mut form.fields {
        if field.spec.name == "amount" {
            field.value = FieldValueUi::Text { draft: "4".into() };
        }
    }
    let response = view.try_accept().expect("valid input submits");
    let raw = serde_json::value::to_raw_value(&response).unwrap();
    let response = adapt_response(acp::ExtResponse::new(raw.into()), compiled.defaults).unwrap();
    let payload: Value = serde_json::from_str(response.0.get()).unwrap();
    assert_eq!(payload["values"]["name"], "Ada");
    assert_eq!(payload["values"]["amount"].as_f64(), Some(4.0));
    assert_eq!(payload["values"]["ready"], true);
    assert_eq!(payload["values"]["choice"], "beta");
    assert_eq!(
        payload["values"]["notes"], "",
        "optional fields retain their empty value"
    );
}

#[test]
fn cancellation_never_becomes_a_submission() {
    let compiled = compile(FORM, "Details").unwrap();
    let raw = serde_json::value::to_raw_value(&McpElicitExtResponse::Cancel).unwrap();
    let response = adapt_response(acp::ExtResponse::new(raw.into()), compiled.defaults).unwrap();
    let payload: Value = serde_json::from_str(response.0.get()).unwrap();
    assert_eq!(payload["outcome"], "cancel");
    assert!(payload.get("values").is_none());
}

#[tokio::test]
async fn late_and_duplicate_form_answers_keep_their_request_and_session_owner() {
    use crate::app::acp_handler::tests::{
        insert_agent, interaction_resolved_ext, make_app_with_agent, switch_active_to,
    };
    use crate::app::agent::AgentId;
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    let mut app = make_app_with_agent("sess-A");
    insert_agent(&mut app, AgentId(1), Some("sess-B"));
    app.agents
        .get_mut(&AgentId(0))
        .unwrap()
        .prompt
        .set_text("draft A");
    app.agents
        .get_mut(&AgentId(1))
        .unwrap()
        .prompt
        .set_text("draft B");
    let mut old_a = request(&mut app, "sess-A", "old-A");
    let late = app
        .agents
        .get_mut(&AgentId(0))
        .unwrap()
        .elicitation_view
        .as_mut()
        .unwrap()
        .take_response_tx()
        .unwrap();
    switch_active_to(&mut app, AgentId(1));
    let mut current_a = request(&mut app, "sess-A", "current-A");
    let mut current_b = request(&mut app, "sess-B", "current-B");

    let raw = serde_json::value::to_raw_value(&McpElicitExtResponse::Accept {
        content: Some(json!({"value":"LATE-A"})),
    })
    .unwrap();
    late.send(Ok(acp::ExtResponse::new(raw.into()))).unwrap();
    assert_eq!(
        form_answer(&mut old_a).await,
        json!({"values":{"value":"LATE-A"}})
    );
    assert!(matches!(
        current_a.try_recv(),
        Err(tokio::sync::oneshot::error::TryRecvError::Empty)
    ));
    assert!(matches!(
        current_b.try_recv(),
        Err(tokio::sync::oneshot::error::TryRecvError::Empty)
    ));

    for _ in 0..2 {
        super::super::handle_session_notification(
            &interaction_resolved_ext("sess-A", "old-A"),
            &mut app,
        );
    }
    assert_eq!(
        app.agents[&AgentId(0)]
            .elicitation_view
            .as_ref()
            .unwrap()
            .tool_call_id,
        "current-A"
    );
    assert_eq!(
        app.agents[&AgentId(1)]
            .elicitation_view
            .as_ref()
            .unwrap()
            .tool_call_id,
        "current-B"
    );

    for (id, text, receiver) in [
        (AgentId(0), "ANSWER-A-中文", &mut current_a),
        (AgentId(1), "ANSWER-B", &mut current_b),
    ] {
        switch_active_to(&mut app, id);
        for code in std::iter::once(KeyCode::Enter)
            .chain(text.chars().map(KeyCode::Char))
            .chain([KeyCode::Enter, KeyCode::Tab, KeyCode::Enter])
        {
            let _ = app.handle_input(&Event::Key(KeyEvent::new(code, KeyModifiers::NONE)));
        }
        assert_eq!(
            form_answer(receiver).await,
            json!({"values":{"value":text}})
        );
        assert!(matches!(
            receiver.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Closed)
        ));
        assert!(app.agents[&id].elicitation_view.is_none());
    }
    assert_eq!(app.agents[&AgentId(0)].prompt.text(), "draft A");
    assert_eq!(app.agents[&AgentId(1)].prompt.text(), "draft B");
}

fn request(
    app: &mut AppView,
    session: &str,
    form: &str,
) -> tokio::sync::oneshot::Receiver<a3s_acp_lib::AcpResult<acp::ExtResponse>> {
    let spec = json!({"root":"value","elements":{
        "value":{"type":"Input","props":{"label":"Value","required":true,"value":{"$bindState":"/form/value"}}}
    },"state":{"form":{"value":""}}});
    let params = serde_json::value::to_raw_value(&json!({
        "sessionId":session,"formId":form,"prompt":"Answer","format":"json-render","spec":spec.to_string()
    })).unwrap();
    let (response_tx, response_rx) = tokio::sync::oneshot::channel();
    handle_form_request(
        a3s_acp_lib::AcpArgs {
            request: acp::ExtRequest::new("a3s/form/request", params.into()),
            response_tx,
        },
        app,
    );
    response_rx
}

async fn form_answer(
    receiver: &mut tokio::sync::oneshot::Receiver<a3s_acp_lib::AcpResult<acp::ExtResponse>>,
) -> Value {
    let response = tokio::time::timeout(std::time::Duration::from_secs(2), receiver)
        .await
        .expect("bounded response")
        .unwrap()
        .unwrap();
    serde_json::from_str(response.0.get()).unwrap()
}

#[test]
fn unrepresentable_or_ambiguous_forms_are_rejected_before_opening_ui() {
    for source in [
        "{\"root\":\"root\",\"elements\":{\"root\":{\"type\":\"Stack\",\"props\":{},\"children\":[\"root\"]}}}",
        "{\"root\":\"root\",\"elements\":{\"root\":{\"type\":\"Card\",\"props\":{\"title\":\"x\"},\"children\":[\"missing\"]}}}",
        "root = Widget(\"x\")",
        "root = Input(\"name\", \"Name\")\nother = env(\"SECRET\")",
        "{\"root\":\"root\",\"elements\":{\"root\":{\"type\":\"Card\",\"props\":{\"title\":\"x\"},\"children\":[\"a\",\"b\"]},\"a\":{\"type\":\"Input\",\"props\":{\"label\":\"A\",\"value\":{\"$bindState\":\"/form/name\"}}},\"b\":{\"type\":\"Input\",\"props\":{\"label\":\"B\",\"value\":{\"$bindState\":\"/form/name\"}}}},\"state\":{\"form\":{\"name\":\"\"}}}",
        "{\"root\":\"root\",\"elements\":{\"root\":{\"type\":\"Input\",\"props\":{\"label\":\"Name\",\"value\":{\"$bindState\":\"/form/constructor\"}}}},\"state\":{\"form\":{\"constructor\":\"\"}}}",
        "{\"root\":\"root\",\"elements\":{\"root\":{\"type\":\"Input\",\"props\":{\"label\":\"Name\",\"value\":{\"$bindState\":\"/form/name\"}}}},\"state\":{\"form\":{\"name\":false}}}",
        "{\"root\":\"root\",\"elements\":{\"root\":{\"type\":\"Input\",\"props\":{\"label\":\"Name\",\"type\":true,\"value\":{\"$bindState\":\"/form/name\"}}}},\"state\":{\"form\":{\"name\":\"\"}}}",
        "{\"root\":\"root\",\"elements\":{\"root\":{\"type\":\"Input\",\"props\":{\"label\":\"Name\",\"type\":\"password\",\"value\":{\"$bindState\":\"/form/name\"}}}},\"state\":{\"form\":{\"name\":\"\"}}}",
        "{\"root\":\"root\",\"elements\":{\"root\":{\"type\":\"Select\",\"props\":{\"label\":\"Name\",\"options\":[\"good\"],\"value\":{\"$bindState\":\"/form/name\"}}}},\"state\":{\"form\":{\"name\":\"bad\"}}}",
        "{\"root\":\"root\",\"elements\":{\"root\":{\"type\":\"Input\",\"props\":{\"label\":\"Count\",\"type\":\"number\",\"min\":10,\"max\":1,\"value\":{\"$bindState\":\"/form/count\"}}}},\"state\":{\"form\":{\"count\":3}}}",
        "{\"root\":\"root\",\"elements\":{\"root\":{\"type\":\"Button\",\"props\":{\"action\":\"open-url\",\"label\":\"Open\"}}}}",
        "root = Metric(\"Total\", {amount * 3})",
    ] {
        assert!(compile(source, "Details").is_err(), "must reject {source}");
    }
    assert!(compile(&"x".repeat(65_537), "Details").is_err());
}
