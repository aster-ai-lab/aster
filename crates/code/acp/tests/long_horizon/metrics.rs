#![cfg(test)]

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    System,
    Developer,
    User,
    Assistant,
    Tool,
}

impl Role {
    pub fn parse(value: &str) -> Result<Self, &'static str> {
        match value {
            "system" => Ok(Self::System),
            "developer" => Ok(Self::Developer),
            "user" => Ok(Self::User),
            "assistant" => Ok(Self::Assistant),
            "tool" => Ok(Self::Tool),
            _ => Err("unsupported observation role"),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Developer => "developer",
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct RequestSummary {
    pub sequence: usize,
    pub serialized_request_bytes: usize,
    pub messages: usize,
    pub message_bytes: usize,
    pub role_bytes: BTreeMap<Role, usize>,
    pub shape_complete: bool,
    pub system_sha256: String,
    pub system_prefix_sha256: BTreeMap<usize, String>,
    pub tools: usize,
    pub tool_schema_bytes: usize,
    pub tools_sha256: String,
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

impl RequestSummary {
    /// Passive sizes and hashes only: no prompt, tool result, reasoning or credential text.
    pub fn capture(sequence: usize, body: &Value) -> Self {
        let mut role_bytes = BTreeMap::new();
        let messages = body["messages"].as_array();
        let mut shape_complete = messages.is_some();
        let mut system = Vec::new();
        for message in messages.into_iter().flatten() {
            match message["role"]
                .as_str()
                .and_then(|role| Role::parse(role).ok())
            {
                Some(role) => {
                    *role_bytes.entry(role).or_default() += message.to_string().len();
                    if matches!(role, Role::System | Role::Developer) {
                        system.push(message);
                    }
                }
                None => shape_complete = false,
            }
        }
        let system = serde_json::to_vec(&system).expect("JSON values serialize");
        let tools = body.get("tools").map(Value::to_string).unwrap_or_default();
        Self {
            sequence,
            serialized_request_bytes: body.to_string().len(),
            messages: messages.map_or(0, Vec::len),
            message_bytes: body
                .get("messages")
                .map_or(0, |messages| messages.to_string().len()),
            role_bytes,
            shape_complete,
            system_sha256: hash(&system),
            system_prefix_sha256: [4096, 16384, 65536]
                .into_iter()
                .map(|limit| (limit, hash(&system[..system.len().min(limit)])))
                .collect(),
            tools: body["tools"].as_array().map_or(0, Vec::len),
            tool_schema_bytes: tools.len(),
            tools_sha256: hash(tools.as_bytes()),
        }
    }
}

#[path = "metrics_tests.rs"]
mod tests;
