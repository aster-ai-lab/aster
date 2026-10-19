//! In-process paper processor for the signed-in member. It does not call PaperLoom.

use std::path::Path;

use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub(crate) const UPLOAD_LIMIT: usize = 20 * 1024 * 1024;
pub(crate) type Response = (u16, &'static str, &'static str, Vec<u8>);

#[path = "paperloom_provenance.rs"]
mod provenance;
pub(crate) use provenance::route as provenance_route;

#[path = "paperloom_native.rs"]
mod native;

pub(crate) fn processor_revision() -> String {
    hex_digest(b"a3s-native-paper")
}

/// A request from an old member's still-closing view cannot act as the newly signed-in member.
pub(crate) fn member_query(member: &str, query: &str) -> Option<String> {
    let pairs: Vec<_> = url::form_urlencoded::parse(query.as_bytes()).collect();
    let owners: Vec<_> = pairs
        .iter()
        .filter(|(key, _)| key == "studio_member")
        .collect();
    if owners.len() != 1 || owners[0].1 != member {
        return None;
    }
    let mut remaining = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in pairs {
        if key != "studio_member" {
            remaining.append_pair(&key, &value);
        }
    }
    Some(remaining.finish())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SaveConnection {
    endpoint: String,
    api_key: Option<String>,
    expected_revision: String,
}

fn hex_digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(crate) fn connection(_root: &Path, member: &str, method: &str, body: &[u8]) -> Response {
    if uuid::Uuid::parse_str(member).is_err() {
        return json_response(400, json!({"error":"invalid_connection"}));
    }
    let revision = processor_revision();
    let ready = json!({"endpoint":"","apiKeyConfigured":true,"revision":revision});
    if method == "GET" {
        return json_response(200, ready);
    }
    if method != "PUT" {
        return json_response(400, json!({"error":"invalid_connection"}));
    }
    let Ok(input) = serde_json::from_slice::<SaveConnection>(body) else {
        return json_response(400, json!({"error":"invalid_connection"}));
    };
    if !input.expected_revision.is_empty() && input.expected_revision != revision {
        return json_response(409, json!({"error":"connection_changed"}));
    }
    if !input.endpoint.trim().is_empty() || input.api_key.is_some() {
        return json_response(400, json!({"error":"invalid_connection"}));
    }
    json_response(200, ready)
}

/// Exact public route shapes. Internal worker, arbitrary download URL and filesystem APIs stay private.
fn public_route(method: &str, parts: &[&str]) -> bool {
    if parts.iter().any(|p| {
        p.is_empty()
            || p.len() > 128
            || !p
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
            || *p == "."
            || *p == ".."
    }) {
        return false;
    }
    match (method, parts) {
        ("GET", ["health"]) => true,
        ("GET", ["documents"]) => true,
        ("GET" | "POST", ["glossaries"] | ["jobs"]) => true,
        // The workspace owns files; the renderer cannot import service paths or delete its content index.
        ("GET" | "PATCH", ["documents", _]) => true,
        ("GET" | "PATCH" | "DELETE", ["glossaries", _]) => true,
        (
            "GET",
            [
                "documents",
                _,
                "source.pdf" | "jobs" | "metadata-suggestions" | "agent-versions",
            ],
        ) => true,
        ("POST", ["documents", _, "translate" | "ocr" | "metadata-suggestions"]) => true,
        ("GET", ["search"] | ["providers", "ocr"] | ["jobs", _]) => true,
        (
            "GET",
            [
                "jobs",
                _,
                "events"
                | "stage-actions"
                | "resume-plan"
                | "artifacts-manifest"
                | "pdf"
                | "docx"
                | "download"
                | "markdown"
                | "normalized-document",
            ],
        ) => true,
        (
            "GET",
            ["jobs", _, "reader", "regions" | "metadata"]
            | ["jobs", _, "markdown", "document" | "export"]
            | ["jobs", _, "pdf", "side-by-side"]
            | ["jobs", _, "artifacts", _],
        ) => true,
        ("POST", ["jobs", _, "cancel" | "resume"]) => true,
        ("POST", ["ai", "ask"]) => true,
        ("GET" | "POST", ["ai", "conversations"]) => true,
        ("GET" | "PATCH" | "DELETE", ["ai", "conversations", _]) => true,
        ("GET", ["integrations", "obsidian" | "zotero"]) => true,
        _ => false,
    }
}

pub(crate) fn proxy(
    root: &Path,
    member: &str,
    method: &str,
    parts: &[&str],
    query: &str,
    body: &[u8],
) -> Response {
    native::handle(root, member, method, parts, query, body)
}

fn json_response(status: u16, value: Value) -> Response {
    (
        status,
        if status < 400 {
            "OK"
        } else {
            "PaperLoom Request Failed"
        },
        "application/json",
        value.to_string().into_bytes(),
    )
}

#[cfg(test)]
#[path = "paperloom_tests.rs"]
mod tests;
