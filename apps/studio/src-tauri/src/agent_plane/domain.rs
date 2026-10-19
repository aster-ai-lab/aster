//! Entities mirror the renderer's DataApi schemas (`src/shared/data/api/schemas/agent*.ts`).
//! Those schemas are strict: optional fields must be absent, never `null`.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const AGENT_TYPES: [&str; 4] = ["claude-code", "pi", "dsh", "a3s-code"];
pub const TOPIC_PREFIX: &str = "agent-session:";

/// Platform a3s-code agent. The ids match `a3s_cloud_contracts` builtin
/// `a3s.os.agent.a3s-code` and `a3s.os.agent.a3s-code.release`.
pub const BUILTIN_CODE_AGENT_ID: &str = "a3s-code";
pub const BUILTIN_CODE_AGENT_ASSET_ID: &str = "b0aaf8f3-2d7b-51a6-b48f-44fcad1230fc";
pub const BUILTIN_CODE_AGENT_RELEASE_ID: &str = "ad580d5a-93d9-5ed8-81cf-0e9548b4b543";
pub const BUILTIN_CODE_PROVIDER_KIND: &str = "a3s.code";
/// Long-lived session the messages ToBot thread keeps warm.
/// An empty tenant slug still uses this id. A signed-in member uses `tobot-{uuid}`.
pub const BUILTIN_TOBOT_SESSION_ID: &str = "tobot";

/// Lowercase UUID `8-4-4-4-12`. Phone numbers and uppercase ids are not members.
pub fn member_uuid_ok(id: &str) -> bool {
    let bytes = id.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    let groups = [8usize, 4, 4, 4, 12];
    let mut index = 0;
    for (group, length) in groups.iter().enumerate() {
        if group > 0 {
            if bytes[index] != b'-' {
                return false;
            }
            index += 1;
        }
        for _ in 0..*length {
            let byte = bytes[index];
            if !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase() {
                return false;
            }
            index += 1;
        }
    }
    true
}

pub fn member_tobot_session_id(member_id: &str) -> Option<String> {
    let id = member_id.trim();
    member_uuid_ok(id).then(|| format!("tobot-{id}"))
}

pub fn tobot_member_of(session_id: &str) -> Option<&str> {
    let rest = session_id.strip_prefix("tobot-")?;
    member_uuid_ok(rest).then_some(rest)
}

pub fn is_tobot_session(session_id: &str) -> bool {
    session_id == BUILTIN_TOBOT_SESSION_ID || tobot_member_of(session_id).is_some()
}

/// Long-lived session the messages ToBoss thread keeps warm.
/// An empty tenant slug still uses this id. A signed-in member uses `toboss-{uuid}`.
pub const BUILTIN_TOBOSS_SESSION_ID: &str = "toboss";

pub fn member_toboss_session_id(member_id: &str) -> Option<String> {
    let id = member_id.trim();
    member_uuid_ok(id).then(|| format!("toboss-{id}"))
}

pub fn toboss_member_of(session_id: &str) -> Option<&str> {
    let rest = session_id.strip_prefix("toboss-")?;
    member_uuid_ok(rest).then_some(rest)
}

pub fn is_toboss_session(session_id: &str) -> bool {
    session_id == BUILTIN_TOBOSS_SESSION_ID || toboss_member_of(session_id).is_some()
}

/// A group transcript. `toboss-room-{uuid}` has no signed-in member.
/// `toboss-{member}-room-{room}` belongs to that member and that group only.
pub fn is_toboss_room_session(session_id: &str) -> bool {
    toboss_room_parts(session_id).is_some()
}

pub fn toboss_room_parts(session_id: &str) -> Option<(Option<&str>, &str)> {
    if let Some(room) = session_id.strip_prefix("toboss-room-") {
        return member_uuid_ok(room).then_some((None, room));
    }
    let rest = session_id.strip_prefix("toboss-")?;
    let (member, room) = rest.split_once("-room-")?;
    (member_uuid_ok(member) && member_uuid_ok(room)).then_some((Some(member), room))
}

pub fn member_toboss_room_session_id(member_id: &str, room_id: &str) -> Option<String> {
    let room = room_id.trim();
    if !member_uuid_ok(room) {
        return None;
    }
    let member = member_id.trim();
    if member.is_empty() {
        return Some(format!("toboss-room-{room}"));
    }
    member_uuid_ok(member).then(|| format!("toboss-{member}-room-{room}"))
}

/// Compile sessions stay inside the knowledge dialog. This exact description is not a task.
pub const KNOWLEDGE_COMPILE_SESSION: &str = "knowledge-compile";

/// A compile session, including one stored before the description existed.
/// The personal vault path is the same rule the compile command accepts.
/// The display name is not a signal.
pub fn is_knowledge_compile_session(session: &Session) -> bool {
    session.description.as_deref() == Some(KNOWLEDGE_COMPILE_SESSION)
        || a3s_okf::is_personal_vault(Path::new(&session.workspace.path))
}

/// ToBot, private ToBoss, and each group ToBoss stay out of the task list.
pub fn is_resident_session(session_id: &str) -> bool {
    is_tobot_session(session_id)
        || is_toboss_session(session_id)
        || is_toboss_room_session(session_id)
}

/// Private long-term memory for one resident session.
/// A group ToBoss room is absent here. That turn uses the room bank.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrivateMemory {
    /// `personal` is the member ToBot bank. `private` is the member ToBoss bank.
    pub scope: &'static str,
    pub label: &'static str,
}

pub fn private_memory(session_id: &str) -> Option<PrivateMemory> {
    if !is_resident_session(session_id) || is_toboss_room_session(session_id) {
        return None;
    }
    if is_toboss_session(session_id) {
        Some(PrivateMemory {
            scope: "private",
            label: "ToBot",
        })
    } else if is_tobot_session(session_id) {
        Some(PrivateMemory {
            scope: "personal",
            label: "ToBot",
        })
    } else {
        None
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub small_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcps: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub knowledge_base_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled_tools: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration: Option<Value>,
    pub model_name: Option<String>,
    pub order_key: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<String>,
}

impl Agent {
    pub fn permission_mode(&self) -> &str {
        self.configuration
            .as_ref()
            .and_then(|config| config.get("permission_mode"))
            .and_then(Value::as_str)
            .unwrap_or("default")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default = "default_order_key")]
    pub order_key: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

fn default_order_key() -> String {
    "a0".to_owned()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub agent_id: Option<String>,
    pub name: String,
    pub is_name_manually_edited: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub workspace_id: String,
    pub workspace: Workspace,
    pub order_key: String,
    pub last_activity_at: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<String>,
}

impl Session {
    pub fn topic_id(&self) -> String {
        format!("{TOPIC_PREFIX}{}", self.id)
    }
}

/// A pin promotes one entity. Session pins are the workbench 置顶 marker.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Pin {
    pub id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub order_key: String,
    pub created_at: String,
    pub updated_at: String,
}

/// A workspace file attached to a scheduled task.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledTaskFile {
    pub path: String,
    pub name: String,
}

/// Persisted scheduled task. `workspace_path` is the resolved folder for a user
/// workspace; the public read model omits it and returns `workspace` as stored.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledTask {
    pub id: String,
    pub agent_id: String,
    pub name: String,
    pub prompt: String,
    pub trigger: Value,
    pub timeout_minutes: u32,
    pub workspace: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_name: Option<String>,
    pub reuse_session: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reuse_session_id: Option<String>,
    /// Session the next fire must continue after the owning process disappeared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume_session_id: Option<String>,
    /// `providerId::modelId`. Absent means the agent's model when the task fires.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Skill names applied when the task fires.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<String>,
    /// Workspace files the agent should read when the task fires.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<ScheduledTaskFile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_run: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_run: Option<String>,
    pub enabled: bool,
    #[serde(default)]
    pub fire_count: u32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRun {
    pub id: String,
    pub task_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    pub started_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// 1 is the first try. A following process interruption increments it.
    #[serde(default = "first_attempt")]
    pub attempt: u32,
}

fn first_attempt() -> u32 {
    1
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub data: Value,
    pub status: String,
    pub model_id: Option<String>,
    pub message_snapshot: Option<Value>,
    pub stats: Option<Value>,
    pub delivery: Option<Value>,
    pub searchable_text: String,
    pub runtime_resume_token: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl Message {
    pub fn parts(&self) -> &[Value] {
        self.data
            .get("parts")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// The renderer's reserved-message shape (`CherryUIMessage`).
    pub fn to_ui_message(&self) -> Value {
        serde_json::json!({
            "id": self.id,
            "role": self.role,
            "parts": self.parts(),
            "metadata": {
                "createdAt": self.created_at,
                "status": self.status,
                "modelId": self.model_id,
                "messageSnapshot": self.message_snapshot,
            },
        })
    }
}

/// Text the session search index and auto-title read: text parts only.
pub fn searchable_text(parts: &[Value]) -> String {
    parts
        .iter()
        .filter(|part| part.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

/// `providerId::modelId` → (`providerId`, `modelId`).
pub fn split_model_id(model: &str) -> Option<(&str, &str)> {
    let (provider, model) = model.split_once("::")?;
    (!provider.is_empty() && !model.is_empty()).then_some((provider, model))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_unique_model_ids() {
        assert_eq!(split_model_id("openai::gpt-4o"), Some(("openai", "gpt-4o")));
        assert_eq!(
            split_model_id("bailian::qwen/qwen3"),
            Some(("bailian", "qwen/qwen3"))
        );
        assert_eq!(split_model_id("gpt-4o"), None);
        assert_eq!(split_model_id("::gpt"), None);
    }

    #[test]
    fn group_rooms_do_not_touch_private_memory() {
        let member = "8089eba1-5b69-435d-ba4b-727e4af5b91a";
        let room = "11111111-1111-4111-8111-111111111111";
        let personal = member_tobot_session_id(member).unwrap();
        let boss = member_toboss_session_id(member).unwrap();
        let group = member_toboss_room_session_id(member, room).unwrap();
        assert_eq!(private_memory(&personal).unwrap().scope, "personal");
        assert_eq!(private_memory(&boss).unwrap().scope, "private");
        assert!(private_memory(&group).is_none());
        assert!(private_memory("toboss-room-11111111-1111-4111-8111-111111111111").is_none());
    }

    #[test]
    fn searchable_text_reads_only_text_parts() {
        let parts = vec![
            serde_json::json!({ "type": "reasoning", "text": "hidden" }),
            serde_json::json!({ "type": "text", "text": "hello" }),
            serde_json::json!({ "type": "text", "text": "world" }),
        ];
        assert_eq!(searchable_text(&parts), "hello\nworld");
    }
}
