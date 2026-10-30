use a3s_code_tools::mcp_elicitation::{
    McpElicitExtRequest, McpElicitExtResponse, McpElicitModeFields, parse_form_schema,
};
use agent_client_protocol as acp;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::{AppView, handle_mcp_elicit, interaction_target_agent};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    session_id: String,
    form_id: String,
    prompt: String,
    spec: String,
    format: String,
}

pub(super) fn handle_form_request(
    ext: a3s_acp_lib::AcpArgs<acp::ExtRequest>,
    app: &mut AppView,
) -> bool {
    let prepared: Result<_, String> = (|| {
        let request: Request = serde_json::from_str(ext.request.params.get())
            .map_err(|_| "invalid form request".to_string())?;
        for id in [&request.session_id, &request.form_id] {
            if id.trim().is_empty() || id.len() > 128 || id.chars().any(char::is_control) {
                return Err("invalid form identity".into());
            }
        }
        if request.format != "json-render" {
            return Err("unsupported form format".into());
        }
        if interaction_target_agent(app, &request.session_id).is_none() {
            return Err("form session has no local view".into());
        }
        let compiled = compile(&request.spec, &request.prompt)?;
        let request = McpElicitExtRequest {
            session_id: request.session_id,
            tool_call_id: request.form_id,
            server_name: "Aster".into(),
            message: compiled.message,
            require_nonempty: true,
            mode: McpElicitModeFields::Form {
                requested_schema: Some(compiled.schema),
            },
        };
        let raw = serde_json::value::to_raw_value(&request).map_err(|_| "invalid form schema")?;
        Ok((raw, compiled.defaults))
    })();
    let (raw, defaults) = match prepared {
        Ok(prepared) => prepared,
        Err(message) => {
            let _ = ext.response_tx.send(Err(acp::Error::new(-32602, message)));
            return false;
        }
    };
    let (response_tx, response_rx) = tokio::sync::oneshot::channel();
    let mut original = ext.response_tx;
    tokio::spawn(async move {
        tokio::select! {
            _ = original.closed() => {}
            response = response_rx => {
                let response = match response {
                    Ok(Ok(response)) => adapt_response(response, defaults),
                    Ok(Err(error)) => Err(error),
                    Err(_) => Err(acp::Error::new(-32603, "form was closed before submission")),
                };
                let _ = original.send(response);
            }
        }
    });
    handle_mcp_elicit(
        a3s_acp_lib::AcpArgs {
            request: acp::ExtRequest::new("a3s/mcp/elicit", raw.into()),
            response_tx,
        },
        app,
    )
}

fn adapt_response(
    response: acp::ExtResponse,
    defaults: Map<String, Value>,
) -> a3s_acp_lib::AcpResult<acp::ExtResponse> {
    let response: McpElicitExtResponse = serde_json::from_str(response.0.get())
        .map_err(|_| acp::Error::new(-32603, "invalid form response"))?;
    let payload = match response {
        McpElicitExtResponse::Accept {
            content: Some(Value::Object(content)),
        } => {
            let mut values = defaults;
            values.extend(content);
            json!({ "values": values })
        }
        McpElicitExtResponse::Cancel => json!({ "outcome": "cancel" }),
        McpElicitExtResponse::Decline => json!({ "outcome": "decline" }),
        _ => return Err(acp::Error::new(-32603, "form returned no values")),
    };
    let raw =
        serde_json::value::to_raw_value(&payload).map_err(|_| acp::Error::internal_error())?;
    Ok(acp::ExtResponse::new(raw.into()))
}

struct Compiled {
    schema: Value,
    defaults: Map<String, Value>,
    message: String,
}

fn compile(source: &str, prompt: &str) -> Result<Compiled, String> {
    let document = a3s_form_core::parse_json_render(source).map_err(|error| error.to_string())?;
    if document.fields.is_empty() {
        return Err("form contains no fields".into());
    }
    let mut properties = Map::new();
    let mut defaults = Map::new();
    let mut required = Vec::new();
    let mut order = Vec::new();
    for field in document.fields {
        if field.required {
            required.push(field.name.clone());
        }
        order.push(field.name.clone());
        defaults.insert(field.name.clone(), field.initial);
        properties.insert(field.name, field.schema);
    }
    let schema = json!({"type":"object","properties":properties,"required":required,"x-a3s-field-order":order});
    parse_form_schema(&schema)?;
    let message = std::iter::once(prompt.to_owned())
        .chain(document.text)
        .collect::<Vec<_>>()
        .join(" · ");
    if message.chars().count() > 4096 {
        return Err("form description is too long".into());
    }
    Ok(Compiled {
        schema,
        defaults,
        message,
    })
}

#[cfg(test)]
mod tests;
