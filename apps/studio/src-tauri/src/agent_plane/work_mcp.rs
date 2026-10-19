//! Built-in MCP server ToBot uses to read the signed-in member's tasks.
//! a3s-code reaches it over loopback with the plane bearer. The member token
//! stays in this process and is attached only on the call to a3s-api.

use std::sync::{Arc, Mutex};

#[allow(unused_imports)]
use a3s_boot::{Result, body, controller, get, post};
use serde_json::{Value, json};

pub(crate) fn work_mcp_server(base_url: &str, token: &str) -> Value {
    let base = base_url.trim_end_matches('/');
    json!({
        "type": "http",
        "name": "member-work",
        "url": format!("{base}/mcp/member-work"),
        "headers": [{ "name": "Authorization", "value": format!("Bearer {token}") }],
    })
}

pub trait WorkReader: Send + Sync {
    fn summary(&self, org_id: &str) -> std::result::Result<Value, String>;
    fn list(
        &self,
        org_id: &str,
        box_name: &str,
        kind: &str,
        query: &str,
    ) -> std::result::Result<Value, String>;
    fn read(&self, org_id: &str, id: &str) -> std::result::Result<Value, String>;
}

pub struct HttpWorkReader {
    pub origin: String,
    pub slug: String,
}

impl WorkReader for HttpWorkReader {
    fn summary(&self, org_id: &str) -> std::result::Result<Value, String> {
        self.get(&team_path(
            "/v1/api/org/members/work-items/summary",
            org_id,
        )?)
    }

    fn list(
        &self,
        org_id: &str,
        box_name: &str,
        kind: &str,
        query: &str,
    ) -> std::result::Result<Value, String> {
        let box_name = box_ok(box_name)?;
        if !kind.is_empty() {
            kind_ok(kind)?;
        }
        self.get(&team_path(&list_path(box_name, kind, query), org_id)?)
    }

    fn read(&self, org_id: &str, id: &str) -> std::result::Result<Value, String> {
        if !uuid_ok(id) {
            return Err("id must be a uuid".to_owned());
        }
        for box_name in BOX_NAMES {
            let page = self.list(org_id, box_name, "", "")?;
            if let Some(found) = page.as_array().and_then(|items| {
                items
                    .iter()
                    .find(|item| item.get("id").and_then(Value::as_str) == Some(id))
                    .cloned()
            }) {
                return Ok(found);
            }
        }
        Err("work item not found".to_owned())
    }
}

impl HttpWorkReader {
    fn get(&self, path: &str) -> std::result::Result<Value, String> {
        let token = crate::member_session::MemberSessionStore::open(&self.slug)
            .token()
            .ok_or_else(|| "member session missing".to_owned())?;
        let (status, _, _, payload) =
            crate::org_login::forward_member_api(&self.origin, "GET", path, b"", &token);
        unwrap_data(status, &payload)
    }
}

pub struct DisabledWork;

impl WorkReader for DisabledWork {
    fn summary(&self, _org_id: &str) -> std::result::Result<Value, String> {
        Err("tasks are unavailable".to_owned())
    }

    fn list(
        &self,
        _org_id: &str,
        _box_name: &str,
        _kind: &str,
        _query: &str,
    ) -> std::result::Result<Value, String> {
        Err("tasks are unavailable".to_owned())
    }

    fn read(&self, _org_id: &str, _id: &str) -> std::result::Result<Value, String> {
        Err("tasks are unavailable".to_owned())
    }
}

pub struct WorkMcpController {
    pub reader: Arc<dyn WorkReader>,
}

#[controller("/mcp/member-work")]
impl WorkMcpController {
    #[post("/")]
    async fn rpc(&self, #[body] body: Value) -> Result<Value> {
        let reader = Arc::clone(&self.reader);
        let value = tokio::task::spawn_blocking(move || handle_rpc(reader.as_ref(), &body))
            .await
            .map_err(|error| a3s_boot::BootError::Internal(error.to_string()))?;
        Ok(value)
    }

    #[get("/policy")]
    async fn read_policy(&self) -> Result<Value> {
        Ok(policy_json(current_policy()))
    }

    #[post("/policy")]
    async fn write_policy(&self, #[body] body: Value) -> Result<Value> {
        Ok(policy_json(store_policy(&body)))
    }
}

const KIND_NAMES: [&str; 8] = [
    "join_request",
    "approval",
    "leave",
    "travel",
    "expense",
    "meeting",
    "seal",
    "notice",
];
const BOX_NAMES: [&str; 4] = ["todo", "done", "cc", "sent"];
/// Every task kind. The member can narrow the set after allowing management.
const DEFAULT_KINDS: u8 = 0b1111_1111;

#[derive(Clone, Copy)]
struct BotPolicy {
    allow: bool,
    list: bool,
    read: bool,
    kinds: u8,
}

impl BotPolicy {
    const fn closed() -> Self {
        Self {
            allow: false,
            list: true,
            read: false,
            kinds: DEFAULT_KINDS,
        }
    }

    fn any_list(self) -> bool {
        self.allow && self.list && self.kinds != 0
    }

    fn list_ok(self, kind: &str) -> bool {
        self.allow && self.list && kind_bit(kind).is_some_and(|bit| self.kinds & bit != 0)
    }

    fn read_ok(self, kind: &str) -> bool {
        self.allow && self.read && kind_bit(kind).is_some_and(|bit| self.kinds & bit != 0)
    }
}

fn policy_slot() -> &'static Mutex<BotPolicy> {
    static POLICY: Mutex<BotPolicy> = Mutex::new(BotPolicy::closed());
    &POLICY
}

fn current_policy() -> BotPolicy {
    *policy_slot()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

fn store_policy(body: &Value) -> BotPolicy {
    let next = policy_from_json(body);
    *policy_slot()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner()) = next;
    next
}

fn policy_from_json(body: &Value) -> BotPolicy {
    let mut kinds = 0_u8;
    if let Some(items) = body.get("kinds").and_then(Value::as_array) {
        for item in items {
            let Some(name) = item.as_str() else { continue };
            let Some(bit) = kind_bit(name) else { continue };
            kinds |= bit;
        }
    }
    if kinds == 0 {
        kinds = DEFAULT_KINDS;
    }
    BotPolicy {
        allow: body.get("allow").and_then(Value::as_bool).unwrap_or(false),
        list: body.get("list").and_then(Value::as_bool).unwrap_or(true),
        read: body.get("read").and_then(Value::as_bool).unwrap_or(false),
        kinds,
    }
}

fn policy_json(policy: BotPolicy) -> Value {
    let kinds: Vec<&str> = KIND_NAMES
        .iter()
        .enumerate()
        .filter(|(index, _)| policy.kinds & (1 << index) != 0)
        .map(|(_, name)| *name)
        .collect();
    json!({
        "allow": policy.allow,
        "list": policy.list,
        "read": policy.read,
        "kinds": kinds
    })
}

fn kind_bit(name: &str) -> Option<u8> {
    KIND_NAMES
        .iter()
        .position(|item| *item == name)
        .and_then(|index| u8::try_from(index).ok())
        .map(|index| 1 << index)
}

fn handle_rpc(reader: &dyn WorkReader, body: &Value) -> Value {
    handle_rpc_with(reader, body, current_policy())
}

fn handle_rpc_with(reader: &dyn WorkReader, body: &Value, policy: BotPolicy) -> Value {
    let method = body.get("method").and_then(Value::as_str).unwrap_or("");
    let id = body.get("id").cloned();
    if id.is_none() {
        return Value::Null;
    }
    let result = match method {
        "initialize" => json!({
            "protocolVersion": body
                .pointer("/params/protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or("2024-11-05"),
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "member-work", "version": "1.0.0" }
        }),
        "ping" => json!({}),
        "tools/list" => json!({ "tools": tools(policy) }),
        "tools/call" => call_tool(reader, body.get("params").unwrap_or(&Value::Null), policy),
        _ => {
            return json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": "method not found" }
            });
        }
    };
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn tools(policy: BotPolicy) -> Value {
    let mut items = Vec::new();
    let names = policy_json(policy)["kinds"].to_string();
    if policy.any_list() {
        items.push(json!({
            "name": "list_work",
            "description": format!("List the signed-in member's tasks. kind, when set, must be one of {names}. box is todo, done, cc, or sent. q matches the title. Do not approve, reject, cancel, or create a task."),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "org_id": { "type": "string", "format": "uuid" },
                    "box": { "type": "string" },
                    "kind": { "type": "string" },
                    "q": { "type": "string" }
                },
                "required": ["org_id"]
            }
        }));
    }
    if policy.allow && policy.read {
        items.push(json!({
            "name": "read_work",
            "description": format!("Read one task by id. Its kind must be one of {names}. Do not approve, reject, cancel, or create a task."),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "org_id": { "type": "string", "format": "uuid" },
                    "id": { "type": "string" }
                },
                "required": ["org_id", "id"]
            }
        }));
    }
    Value::Array(items)
}

fn call_tool(reader: &dyn WorkReader, params: &Value, policy: BotPolicy) -> Value {
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let outcome = match name {
        "list_work" => {
            if policy.any_list() {
                list_work(reader, &arguments, policy)
            } else {
                Err("ToBot is not allowed to list tasks".to_owned())
            }
        }
        "read_work" => {
            if policy.allow && policy.read {
                read_work(reader, &arguments, policy)
            } else {
                Err("ToBot is not allowed to read tasks".to_owned())
            }
        }
        _ => Err(format!("unknown tool {name}")),
    };
    match outcome {
        Ok(text) => json!({ "content": [{ "type": "text", "text": text }], "isError": false }),
        Err(message) => {
            json!({ "content": [{ "type": "text", "text": message }], "isError": true })
        }
    }
}

fn list_work(
    reader: &dyn WorkReader,
    arguments: &Value,
    policy: BotPolicy,
) -> std::result::Result<String, String> {
    let org_id = required_team(arguments)?;
    let box_name = arguments
        .get("box")
        .and_then(Value::as_str)
        .unwrap_or("todo");
    box_ok(box_name)?;
    let kind = arguments.get("kind").and_then(Value::as_str).unwrap_or("");
    if kind.is_empty() {
        if !policy.any_list() {
            return Err("ToBot is not allowed to list tasks".to_owned());
        }
    } else if !policy.list_ok(kind) {
        return Err("ToBot is not allowed to list that task".to_owned());
    }
    let query = arguments.get("q").and_then(Value::as_str).unwrap_or("");
    let summary = reader.summary(org_id)?;
    let page = reader.list(org_id, box_name, kind, query)?;
    let items = match page.as_array() {
        Some(rows) => Value::Array(
            rows.iter()
                .filter(|row| {
                    row.get("kind")
                        .and_then(Value::as_str)
                        .is_some_and(|name| policy.list_ok(name))
                })
                .cloned()
                .collect(),
        ),
        None => Value::Array(Vec::new()),
    };
    Ok(clip(
        &json!({ "summary": summary, "items": items }).to_string(),
        6000,
    ))
}

fn read_work(
    reader: &dyn WorkReader,
    arguments: &Value,
    policy: BotPolicy,
) -> std::result::Result<String, String> {
    let id = arguments
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "id is required".to_owned())?;
    let item = reader.read(required_team(arguments)?, id)?;
    let kind = item.get("kind").and_then(Value::as_str).unwrap_or("");
    if !policy.read_ok(kind) {
        return Err("ToBot is not allowed to read that task".to_owned());
    }
    Ok(clip(&item.to_string(), 8000))
}

fn required_team(arguments: &Value) -> std::result::Result<&str, String> {
    let id = arguments
        .get("org_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "org_id is required".to_owned())?;
    if !uuid_ok(id) {
        return Err("org_id must be a uuid".to_owned());
    }
    Ok(id)
}

fn team_path(path: &str, org_id: &str) -> std::result::Result<String, String> {
    if !uuid_ok(org_id) {
        return Err("org_id must be a uuid".to_owned());
    }
    let separator = if path.contains('?') { '&' } else { '?' };
    Ok(format!("{path}{separator}org={org_id}"))
}

fn clip(text: &str, limit: usize) -> String {
    let mut out: String = text.chars().take(limit).collect();
    if text.chars().count() > limit {
        out.push_str("\n[truncated]");
    }
    out
}

pub(crate) fn list_path(box_name: &str, kind: &str, query: &str) -> String {
    let mut path = format!(
        "/v1/api/org/members/work-items?box={}",
        encode_query(box_name)
    );
    if !kind.is_empty() {
        path.push_str("&kind=");
        path.push_str(&encode_query(kind));
    }
    if !query.is_empty() {
        path.push_str("&q=");
        path.push_str(&encode_query(query));
    }
    path
}

fn box_ok(name: &str) -> std::result::Result<&str, String> {
    match name {
        "todo" | "done" | "cc" | "sent" => Ok(name),
        _ => Err("box is not valid".to_owned()),
    }
}

fn kind_ok(name: &str) -> std::result::Result<&str, String> {
    if kind_bit(name).is_some() {
        Ok(name)
    } else {
        Err("kind is not valid".to_owned())
    }
}

fn uuid_ok(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => *byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
}

fn encode_query(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn unwrap_data(status: u16, payload: &[u8]) -> std::result::Result<Value, String> {
    if !(200..300).contains(&status) {
        return Err(error_text(payload, status));
    }
    let value: Value = serde_json::from_slice(payload).unwrap_or(Value::Null);
    if value.get("code").is_some() && value.get("data").is_some() {
        let code = value.get("code").and_then(Value::as_i64).unwrap_or(-1);
        let success = value
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let http_class = u16::try_from(code).ok();
        let ok =
            (code == 0 || http_class.is_some_and(|item| (200..300).contains(&item))) && success;
        if ok {
            return Ok(value.get("data").cloned().unwrap_or(Value::Null));
        }
        return Err(error_text(payload, status));
    }
    Ok(value)
}

fn error_text(payload: &[u8], status: u16) -> String {
    serde_json::from_slice::<Value>(payload)
        .ok()
        .and_then(|value| {
            value
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .filter(|message| !message.is_empty())
        .unwrap_or_else(|| format!("work api {status}"))
}

#[cfg(test)]
#[path = "work_mcp_tests.rs"]
mod tests;
