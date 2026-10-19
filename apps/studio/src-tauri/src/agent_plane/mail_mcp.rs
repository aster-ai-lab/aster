//! Built-in MCP server ToBot uses to read the signed-in member's mailbox.
//! a3s-code reaches it over loopback with the plane bearer. The member token
//! stays in this process and is attached only on the call to a3s-api.

use std::sync::{Arc, Mutex};

#[allow(unused_imports)]
use a3s_boot::{Result, body, controller, get, post};
use serde_json::{Value, json};

pub struct MailMcpEndpoint {
    pub base_url: String,
    pub token: String,
    pub slug: String,
    pub origin: String,
}

pub(crate) fn mail_mcp_server(endpoint: &MailMcpEndpoint) -> Value {
    let base = endpoint.base_url.trim_end_matches('/');
    json!({
        "type": "http",
        "name": "member-mail",
        "url": format!("{base}/mcp/member-mail"),
        "headers": [{ "name": "Authorization", "value": format!("Bearer {}", endpoint.token) }],
    })
}

pub trait MailReader: Send + Sync {
    fn mailbox(&self, org_id: &str) -> std::result::Result<Value, String>;
    fn list(&self, org_id: &str, folder: &str, query: &str) -> std::result::Result<Value, String>;
    fn read(&self, org_id: &str, id: &str) -> std::result::Result<Value, String>;
}

pub struct HttpMailReader {
    pub origin: String,
    pub slug: String,
}

impl MailReader for HttpMailReader {
    fn mailbox(&self, org_id: &str) -> std::result::Result<Value, String> {
        self.get(&team_path("/v1/api/org/mail/mailbox", org_id)?)
    }

    fn list(&self, org_id: &str, folder: &str, query: &str) -> std::result::Result<Value, String> {
        let folder = folder_ok(folder)?;
        self.get(&team_path(&list_path(folder, query), org_id)?)
    }

    fn read(&self, org_id: &str, id: &str) -> std::result::Result<Value, String> {
        if !uuid_ok(id) {
            return Err("id must be a uuid".to_owned());
        }
        self.get(&team_path(
            &format!("/v1/api/org/mail/messages/{id}"),
            org_id,
        )?)
    }
}

impl HttpMailReader {
    fn get(&self, path: &str) -> std::result::Result<Value, String> {
        let token = crate::member_session::MemberSessionStore::open(&self.slug)
            .token()
            .ok_or_else(|| "member session missing".to_owned())?;
        let (status, _, _, payload) =
            crate::org_login::forward_member_api(&self.origin, "GET", path, b"", &token);
        unwrap_data(status, &payload)
    }
}

pub struct DisabledMail;

impl MailReader for DisabledMail {
    fn mailbox(&self, _org_id: &str) -> std::result::Result<Value, String> {
        Err("mail is unavailable".to_owned())
    }

    fn list(
        &self,
        _org_id: &str,
        _folder: &str,
        _query: &str,
    ) -> std::result::Result<Value, String> {
        Err("mail is unavailable".to_owned())
    }

    fn read(&self, _org_id: &str, _id: &str) -> std::result::Result<Value, String> {
        Err("mail is unavailable".to_owned())
    }
}

pub struct MailMcpController {
    pub reader: Arc<dyn MailReader>,
}

#[controller("/mcp/member-mail")]
impl MailMcpController {
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

const FOLDER_NAMES: [&str; 7] = [
    "inbox", "unread", "flagged", "drafts", "sent", "spam", "trash",
];
/// inbox, unread, flagged, and sent. Drafts, spam, and trash stay closed until chosen.
const DEFAULT_FOLDERS: u8 = 0b001_0111;

#[derive(Clone, Copy)]
struct BotPolicy {
    allow: bool,
    list: bool,
    read: bool,
    folders: u8,
}

impl BotPolicy {
    const fn closed() -> Self {
        Self {
            allow: false,
            list: true,
            read: false,
            folders: DEFAULT_FOLDERS,
        }
    }

    fn list_ok(self, folder: &str) -> bool {
        self.allow && self.list && folder_bit(folder).is_some_and(|bit| self.folders & bit != 0)
    }

    fn read_ok(self, folder: Option<&str>) -> bool {
        if !(self.allow && self.read) {
            return false;
        }
        match folder {
            Some(name) => folder_bit(name).is_some_and(|bit| self.folders & bit != 0),
            None => true,
        }
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
    let mut folders = 0_u8;
    if let Some(items) = body.get("folders").and_then(Value::as_array) {
        for item in items {
            let Some(name) = item.as_str() else { continue };
            let Some(bit) = folder_bit(name) else {
                continue;
            };
            folders |= bit;
        }
    }
    if folders == 0 {
        folders = DEFAULT_FOLDERS;
    }
    BotPolicy {
        allow: body.get("allow").and_then(Value::as_bool).unwrap_or(false),
        list: body.get("list").and_then(Value::as_bool).unwrap_or(true),
        read: body.get("read").and_then(Value::as_bool).unwrap_or(false),
        folders,
    }
}

fn policy_json(policy: BotPolicy) -> Value {
    let folders: Vec<&str> = FOLDER_NAMES
        .iter()
        .enumerate()
        .filter(|(index, _)| policy.folders & (1 << index) != 0)
        .map(|(_, name)| *name)
        .collect();
    json!({
        "allow": policy.allow,
        "list": policy.list,
        "read": policy.read,
        "folders": folders
    })
}

fn folder_bit(name: &str) -> Option<u8> {
    FOLDER_NAMES
        .iter()
        .position(|item| *item == name)
        .and_then(|index| u8::try_from(index).ok())
        .map(|index| 1 << index)
}

fn handle_rpc(reader: &dyn MailReader, body: &Value) -> Value {
    handle_rpc_with(reader, body, current_policy())
}

fn handle_rpc_with(reader: &dyn MailReader, body: &Value, policy: BotPolicy) -> Value {
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
            "serverInfo": { "name": "member-mail", "version": "1.0.0" }
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
    if policy.allow && policy.list {
        let names = policy_json(policy)["folders"].to_string();
        items.push(json!({
            "name": "list_mail",
            "description": format!("List the signed-in member's mailbox. folder must be one of {names}. q matches the subject or the sender. The result has the address, the unread count, and summaries without bodies."),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "org_id": { "type": "string", "format": "uuid" },
                    "folder": { "type": "string" },
                    "q": { "type": "string" }
                },
                "required": ["org_id"]
            }
        }));
    }
    if policy.allow && policy.read {
        items.push(json!({
            "name": "read_mail",
            "description": "Read one message by id from an allowed folder. Opening an unread message marks it read only when the team role permits changing mail state.",
            "inputSchema": {
                "type": "object",
                "properties": { "org_id": { "type": "string", "format": "uuid" },
                    "id": { "type": "string" } },
                "required": ["org_id", "id"]
            }
        }));
    }
    Value::Array(items)
}

fn call_tool(reader: &dyn MailReader, params: &Value, policy: BotPolicy) -> Value {
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let outcome = match name {
        "list_mail" => {
            let folder = arguments
                .get("folder")
                .and_then(Value::as_str)
                .unwrap_or("inbox");
            if policy.list_ok(folder) {
                list_mail(reader, &arguments)
            } else {
                Err("ToBot is not allowed to list that folder".to_owned())
            }
        }
        "read_mail" => {
            if policy.allow && policy.read {
                read_mail(reader, &arguments, policy)
            } else {
                Err("ToBot is not allowed to read this mailbox".to_owned())
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

fn list_mail(reader: &dyn MailReader, arguments: &Value) -> std::result::Result<String, String> {
    let folder = arguments
        .get("folder")
        .and_then(Value::as_str)
        .unwrap_or("inbox");
    let query = arguments.get("q").and_then(Value::as_str).unwrap_or("");
    let org_id = required_team(arguments)?;
    let mailbox = reader.mailbox(org_id)?;
    let page = reader.list(org_id, folder, query)?;
    let text = json!({ "mailbox": mailbox, "page": page }).to_string();
    Ok(clip(&text, 6000))
}

fn read_mail(
    reader: &dyn MailReader,
    arguments: &Value,
    policy: BotPolicy,
) -> std::result::Result<String, String> {
    let id = arguments
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "id is required".to_owned())?;
    let message = reader.read(required_team(arguments)?, id)?;
    let folder = message.get("folder").and_then(Value::as_str);
    if !policy.read_ok(folder) {
        return Err("ToBot is not allowed to read that folder".to_owned());
    }
    Ok(clip(&message.to_string(), 8000))
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
    Ok(format!("{path}{separator}org_id={org_id}"))
}

fn clip(text: &str, limit: usize) -> String {
    let mut out: String = text.chars().take(limit).collect();
    if text.chars().count() > limit {
        out.push_str("\n[truncated]");
    }
    out
}

pub(crate) fn list_path(folder: &str, query: &str) -> String {
    format!(
        "/v1/api/org/mail/messages?folder={}&q={}&limit=20",
        encode_query(folder),
        encode_query(query)
    )
}

fn folder_ok(folder: &str) -> std::result::Result<&str, String> {
    match folder {
        "inbox" | "unread" | "flagged" | "drafts" | "sent" | "spam" | "trash" => Ok(folder),
        _ => Err("folder is not valid".to_owned()),
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
        .unwrap_or_else(|| format!("mail api {status}"))
}

#[cfg(test)]
#[path = "mail_mcp_tests.rs"]
mod tests;
