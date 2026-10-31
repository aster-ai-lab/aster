//! `_aster/form/request` — the agent asks the client to render a JSON Render
//! form and waits for the submitted values. Mirrors the document bridge's
//! ext-method round trip, but open-ended: a form can stay on screen until the
//! user submits or the session dies, so there is no call timeout.

use agent_client_protocol::{AgentSideConnection, Client, ExtRequest};
use serde_json::{json, Value};

pub const FORM_REQUEST_METHOD: &str = "aster/form/request";

/// Send one form request. Returns the submitted values JSON, or `None` when
/// the client cannot serve forms or the session ended first.
pub async fn request_form(
    client: &std::rc::Rc<AgentSideConnection>,
    question_id: &str,
    question: &str,
    form: &str,
) -> Option<Value> {
    request_form_inner(client, None, question_id, question, form)
        .await
        .ok()
}

pub async fn request_form_in_session(
    client: &std::rc::Rc<AgentSideConnection>,
    session_id: &str,
    question_id: &str,
    question: &str,
    form: &str,
) -> Result<Value, String> {
    request_form_inner(client, Some(session_id), question_id, question, form).await
}

async fn request_form_inner(
    client: &std::rc::Rc<AgentSideConnection>,
    session_id: Option<&str>,
    question_id: &str,
    question: &str,
    form: &str,
) -> Result<Value, String> {
    let mut params = json!({
        "formId": question_id,
        "prompt": question,
        "spec": form,
        "format": "json-render",
    });
    if let Some(session_id) = session_id {
        params["sessionId"] = session_id.into();
    }
    let params = serde_json::value::to_raw_value(&params).map_err(|_| "invalid form request")?;
    let response = client
        .ext_method(ExtRequest::new(FORM_REQUEST_METHOD, params.into()))
        .await
        .map_err(|error| format!("form request failed: {}", error.message))?;
    let values = response.0.get();
    submitted_values(values)
}

fn submitted_values(response: &str) -> Result<Value, String> {
    let parsed: Value = serde_json::from_str(response).map_err(|_| "invalid form response")?;
    if matches!(
        parsed.get("outcome").and_then(Value::as_str),
        Some("cancel" | "decline")
    ) {
        return Err("form was cancelled by the user".into());
    }
    parsed
        .get("values")
        .filter(|values| values.is_object())
        .cloned()
        .ok_or_else(|| "the client returned no form values".into())
}

#[cfg(test)]
#[path = "form_request_tests.rs"]
mod tests;
