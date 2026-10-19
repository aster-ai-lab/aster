//! Member sign-in for the studio shell.
//! The page uses the same `/v1/api/org/members` routes as the web console.
//! Phone-code and password login consume a slide-captcha token first.
//! Puzzle images are read from the console's `public/captcha` directory.

use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

const MAX_RESPONSE: usize = 1024 * 1024;
const MAX_BUNDLE: usize = 16 * 1024 * 1024;
/// Same cap a3s-api applies to IM attachments.
pub const MAX_IM_ATTACHMENT: usize = 20 * 1024 * 1024;
/// Handbook preview shares the document open budget. Larger objects stay in storage.
pub const MAX_HUB_PREVIEW: usize = 64 * 1024 * 1024;
const UPSTREAM_TIMEOUT: Duration = Duration::from_secs(10);

pub fn default_origin() -> String {
    if let Ok(value) = std::env::var("A3S_ORG_API_ORIGIN") {
        if parse_loopback(&value).is_some() {
            return value;
        }
    }
    "http://127.0.0.1:3000".to_owned()
}

type Response = (u16, &'static str, &'static str, Vec<u8>);

/// IM 转发臂的路径段白名单：uuid、已知端点词与安全符号；防路径注入。
pub fn im_segment_ok(segment: &str) -> bool {
    if segment.is_empty() || segment.len() > 64 {
        return false;
    }
    segment
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

pub fn route(
    method: &str,
    parts: &[&str],
    body: &[u8],
    origin: &str,
    computer_id: &str,
) -> Option<Response> {
    match (method, parts) {
        ("GET", ["api", "publications", slug]) if publication_slug(slug) => Some(forward_get(
            origin,
            &format!("/v1/api/legal/{slug}/document"),
        )),
        ("POST", ["v1", "api", "org", "members", "phone-codes"]) => Some(forward_member(
            origin,
            "/v1/api/org/members/phone-codes",
            body,
            true,
        )),
        ("POST", ["v1", "api", "org", "members", "code-sessions"]) => Some(forward_member(
            origin,
            "/v1/api/org/members/code-sessions",
            &stamp_computer(body, computer_id),
            false,
        )),
        ("POST", ["v1", "api", "org", "members", "sessions"]) => Some(forward_member(
            origin,
            "/v1/api/org/members/sessions",
            &stamp_computer(body, computer_id),
            true,
        )),
        ("POST", ["v1", "api", "captcha", "challenges"]) => Some(forward_json(
            origin,
            "/v1/api/admin/captcha/challenges",
            body,
        )),
        ("POST", ["v1", "api", "captcha", "challenges", id, "verifications"])
            if challenge_id(id) =>
        {
            let path = format!("/v1/api/admin/captcha/challenges/{id}/verifications");
            Some(forward_json(origin, &path, body))
        }
        _ => None,
    }
}

/// Puzzle image for an `<img>` on the login page. Not under `/api`, so the
/// browser can load it without the shell bearer.
pub fn asset(method: &str, parts: &[&str]) -> Option<Response> {
    let ("GET", ["captcha", name]) = (method, parts) else {
        return None;
    };
    if !captcha_name(name) {
        return Some(not_found());
    }
    let Some(dir) = captcha_dir() else {
        return Some(not_found());
    };
    let path = dir.join(name);
    let Ok(bytes) = std::fs::read(&path) else {
        return Some(not_found());
    };
    if bytes.is_empty() || bytes.len() > 2 * 1024 * 1024 {
        return Some(not_found());
    }
    let content_type = if name.ends_with(".png") {
        "image/png"
    } else {
        "image/jpeg"
    };
    Some((200, "OK", content_type, bytes))
}

/// Names this machine on a member login so both doors share one computer.
pub fn stamp_computer(body: &[u8], computer_id: &str) -> Vec<u8> {
    let Ok(mut value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return body.to_vec();
    };
    let Some(object) = value.as_object_mut() else {
        return body.to_vec();
    };
    object.insert(
        "computer_id".to_owned(),
        serde_json::Value::String(computer_id.to_owned()),
    );
    serde_json::to_vec(&value).unwrap_or_else(|_| body.to_vec())
}

fn forward_member(origin: &str, path: &str, body: &[u8], require_captcha: bool) -> Response {
    if require_captcha {
        match consume_captcha(origin, body) {
            Ok(next) => forward_json(origin, path, &next),
            Err(response) => response,
        }
    } else {
        forward_json(origin, path, body)
    }
}

fn consume_captcha(origin: &str, body: &[u8]) -> Result<Vec<u8>, Response> {
    let mut value: serde_json::Value =
        serde_json::from_slice(body).unwrap_or(serde_json::Value::Null);
    let Some(obj) = value.as_object_mut() else {
        return Err(captcha_required());
    };
    let token = obj
        .get("captchaToken")
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .trim()
        .to_owned();
    if token.is_empty() || token.len() > 512 || token.chars().any(|ch| ch.is_control()) {
        return Err(captcha_required());
    }
    obj.remove("captchaToken");
    let payload = serde_json::to_vec(&serde_json::json!({ "token": token })).unwrap_or_default();
    let (status, raw) = forward(
        origin,
        "/v1/api/admin/captcha/token-verifications",
        &payload,
    )
    .map_err(|_| gateway())?;
    if status >= 500 {
        return Err(gateway());
    }
    if status >= 300 || !token_is_valid(&raw) {
        return Err(captcha_required());
    }
    serde_json::to_vec(&value).map_err(|_| captcha_required())
}

fn token_is_valid(raw: &[u8]) -> bool {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(raw) else {
        return false;
    };
    value.get("valid").and_then(|item| item.as_bool()) == Some(true)
        || value.pointer("/data/valid").and_then(|item| item.as_bool()) == Some(true)
}

fn publication_slug(slug: &str) -> bool {
    let bytes = slug.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 64
        && bytes[0] != b'-'
        && *bytes.last().unwrap_or(&b'-') != b'-'
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

fn forward_get(origin: &str, path: &str) -> Response {
    match forward_request(origin, "GET", path, b"", None) {
        Ok((status, payload)) => (status, reason(status), "application/json", payload),
        Err(_) => gateway(),
    }
}

fn forward_json(origin: &str, path: &str, body: &[u8]) -> Response {
    match forward(origin, path, body) {
        Ok((status, payload)) => (status, reason(status), "application/json", payload),
        Err(_) => gateway(),
    }
}

fn captcha_required() -> Response {
    (
        400,
        "Bad Request",
        "application/json",
        br#"{"code":400,"message":"captcha required","data":null}"#.to_vec(),
    )
}

fn gateway() -> Response {
    (
        502,
        "Bad Gateway",
        "application/json",
        br#"{"code":502,"message":"HTTP 502","data":null}"#.to_vec(),
    )
}

fn not_found() -> Response {
    (
        404,
        "Not Found",
        "application/json",
        br#"{"error":"not_found"}"#.to_vec(),
    )
}

fn challenge_id(id: &str) -> bool {
    (32..=36).contains(&id.len())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
        && id.bytes().filter(|byte| *byte == b'-').count() <= 4
}

fn captcha_name(name: &str) -> bool {
    let Some(stem) = name
        .strip_prefix("bg-")
        .and_then(|rest| rest.strip_suffix(".jpg"))
        .or_else(|| {
            name.strip_prefix("piece-")
                .and_then(|rest| rest.strip_suffix(".png"))
        })
    else {
        return false;
    };
    !stem.is_empty()
        && stem.len() <= 32
        && stem
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

fn captcha_dir() -> Option<PathBuf> {
    for key in ["A3S_CAPTCHA_DIR", "CAPTCHA_MANIFEST_PATH"] {
        if let Some(path) = std::env::var_os(key) {
            let path = PathBuf::from(path);
            if let Some(dir) = manifest_dir(&path) {
                return Some(dir);
            }
        }
    }
    let mut roots = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            roots.push(parent.to_path_buf());
        }
    }
    for root in roots {
        for ancestor in root.ancestors() {
            let candidate = ancestor.join("apps/web/public/captcha");
            if candidate.join("manifest.json").is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn manifest_dir(path: &Path) -> Option<PathBuf> {
    if path.is_dir() && path.join("manifest.json").is_file() {
        return Some(path.to_path_buf());
    }
    if path.is_file() {
        return path.parent().map(Path::to_path_buf);
    }
    None
}

/// The platform console reads. The upstream admits only the official platform team.
const PLATFORM_READS: &[&str] = &[
    "overview",
    "customers",
    "partners",
    "suppliers",
    "suppliers/page",
    "models",
    "models/page",
    "model-routes",
    "services",
    "workloads",
    "gateway",
    "fleet",
    "logs",
    "logs/levels",
    "usage",
    "orders",
    "tickets",
    "system",
    "sign-in",
];

fn allowed_platform_path(rest: &str) -> bool {
    if PLATFORM_READS.contains(&rest) {
        return true;
    }
    match rest.split('/').collect::<Vec<_>>().as_slice() {
        ["suppliers", id, "models" | "dependencies", "page"] => {
            uuid::Uuid::parse_str(id).is_ok_and(|value| !value.is_nil())
        }
        ["customers", id, "people"]
        | ["suppliers", id, "models"]
        | ["functions", id, "releases"]
        | ["agent-runtimes", id]
        | ["agent-runtimes", id, "logs"] => uuid_id_ok(id),
        ["cells", project, environment, application, "revisions"] => {
            uuid_id_ok(project) && uuid_id_ok(environment) && uuid_id_ok(application)
        }
        _ => false,
    }
}

/// Team list, one department tree, the org directory the 组织架构 page
/// manages, and the 能力扩展 catalog with its install / uninstall actions.
/// Anything else stays off this proxy.
pub fn allowed_member_path(method: &str, path: &str) -> bool {
    if let Some(rest) = path.strip_prefix("/v1/api/org/im/") {
        return allowed_im_path(method, rest);
    }
    if let Some(rest) = path.strip_prefix("/v1/api/org/repositories") {
        return repositories::allowed_repository_path(method, rest);
    }
    if let Some(rest) = path.strip_prefix("/v1/api/org/repository-namespaces/") {
        return repositories::allowed_namespace_path(method, rest);
    }
    if let Some(rest) = path.strip_prefix("/v1/api/org/pipelines/") {
        return repositories::allowed_pipeline_path(method, rest);
    }
    if path == "/v1/api/org/members/orgs" {
        return method == "GET" || method == "POST";
    }
    if path == "/v1/api/org/members/me" {
        return method == "GET" || method == "PUT";
    }
    if path == "/v1/api/org/catalog-agents"
        || path == "/v1/api/org/capabilities"
        || path == "/v1/api/org/capabilities/installations"
        || path == "/v1/api/org/models"
    {
        return method == "GET";
    }
    if let Some(rest) = path.strip_prefix("/v1/api/org/capabilities/") {
        if method == "GET" {
            return rest.split_once('/').is_some_and(|(slug, action)| {
                capability_slug_ok(slug) && matches!(action, "icon" | "cover" | "bundle")
            });
        }
        if let Some(slug) = rest.strip_prefix("installations/") {
            return matches!(method, "PUT" | "DELETE")
                && !slug.contains('/')
                && capability_slug_ok(slug);
        }
        return false;
    }
    if method == "PUT" && path == "/v1/api/org/members/password" {
        return true;
    }
    if path == "/v1/api/org/members/discoverable-orgs" {
        return method == "GET";
    }
    // The member's own identity verification; reviews stay on the operator plane.
    if path == "/v1/api/org/members/verification" {
        return method == "GET" || method == "POST";
    }
    // The member's own access tokens, and phone invites an admin issues.
    if path == "/v1/api/org/members/access-tokens" || path == "/v1/api/org/members/invites" {
        return method == "GET" || method == "POST";
    }
    if let Some(id) = path
        .strip_prefix("/v1/api/org/members/access-tokens/")
        .or_else(|| path.strip_prefix("/v1/api/org/members/invites/"))
    {
        return method == "DELETE" && uuid_id_ok(id);
    }
    if path == "/v1/api/org/members/work-items" {
        return method == "GET" || method == "POST";
    }
    if path == "/v1/api/org/members/work-items/summary" {
        return method == "GET";
    }
    // Process windows preview and start through the engine. The shell attaches
    // the stored member session. A GET would read a chart the page does not own.
    if path == "/v1/api/org/members/work-items/process-preview"
        || path == "/v1/api/org/members/work-items/process"
    {
        return method == "POST";
    }
    if path == "/v1/api/org/members/work-items/approvers"
        || path == "/v1/api/org/members/work-items/travel-links"
    {
        return method == "GET";
    }
    if let Some(id) = path
        .strip_prefix("/v1/api/org/members/work-items/")
        .and_then(|rest| rest.strip_suffix("/schedule"))
    {
        return method == "POST" && uuid_id_ok(id);
    }
    if let Some(id) = path
        .strip_prefix("/v1/api/org/members/work-items/")
        .and_then(|rest| rest.strip_suffix("/decision"))
    {
        return method == "POST" && uuid_id_ok(id);
    }
    if method == "POST" {
        if let Some(code) = path
            .strip_prefix("/v1/api/org/members/join-codes/")
            .and_then(|rest| rest.strip_suffix("/memberships"))
        {
            return !code.is_empty()
                && code.len() <= 64
                && !code.contains('/')
                && code.bytes().all(|byte| byte.is_ascii_alphanumeric());
        }
    }
    if path == "/v1/api/org/hub/files" {
        return method == "GET";
    }
    if let Some(rest) = path.strip_prefix("/v1/api/org/hub/files/") {
        // Preview bytes only. Upload and delete stay off this proxy.
        return method == "GET"
            && rest
                .split_once('/')
                .is_some_and(|(id, tail)| tail == "content" && uuid_id_ok(id));
    }
    if let Some(rest) = path.strip_prefix("/v1/api/org/drive/") {
        return allowed_drive_path(method, rest);
    }
    if path == "/v1/api/org/tobot/memory/reflects" || path == "/v1/api/org/tobot/memory/retains" {
        return method == "POST";
    }
    if path == "/v1/api/org/mail/mailbox" {
        return method == "GET";
    }
    if path == "/v1/api/org/mail/colleagues" {
        return method == "GET" || method == "POST";
    }
    if path == "/v1/api/org/mail/messages" {
        return method == "GET" || method == "POST";
    }
    if let Some(rest) = path.strip_prefix("/v1/api/org/mail/messages/") {
        if let Some(id) = rest.strip_suffix("/state") {
            return method == "POST" && !id.contains('/') && uuid_id_ok(id);
        }
        return method == "GET" && !rest.contains('/') && uuid_id_ok(rest);
    }
    if let Some(rest) = path.strip_prefix("/v1/api/org/members/platform/") {
        if rest == "suppliers" {
            return method == "GET" || method == "POST";
        }
        if rest == "suppliers/access" {
            return method == "GET";
        }
        if let Some(intent) = rest
            .strip_prefix("suppliers/intents/")
            .and_then(|value| value.strip_suffix("/resolve"))
        {
            return method == "POST"
                && uuid::Uuid::parse_str(intent).is_ok_and(|value| !value.is_nil());
        }
        if let Some(id) = rest.strip_prefix("suppliers/") {
            if uuid::Uuid::parse_str(id).is_ok_and(|value| !value.is_nil()) {
                return matches!(method, "GET" | "PUT" | "DELETE");
            }
        }
        return method == "GET" && allowed_platform_path(rest);
    }
    let Some(rest) = path.strip_prefix("/v1/api/org/members/orgs/") else {
        return false;
    };
    let Some((id, tail)) = rest.split_once('/') else {
        return (method == "PUT" || method == "DELETE") && org_id_ok(rest);
    };
    if !org_id_ok(id) {
        return false;
    }
    match tail {
        "membership" => return method == "DELETE",
        "transfer-candidates" | "entitlements" | "audit" | "runs" => return method == "GET",
        "subscription" | "capability-policy" => return method == "GET" || method == "PUT",
        "verification" => return method == "GET" || method == "POST",
        "ownership-transfers" => return method == "POST",
        "roles" | "positions" if method == "POST" => return true,
        _ => {}
    }
    if let Some(item) = tail
        .strip_prefix("roles/")
        .and_then(|rest| rest.strip_suffix("/audit"))
    {
        return method == "GET" && uuid_id_ok(item);
    }
    if let Some(run_tail) = tail.strip_prefix("runs/") {
        return match (method, run_tail.split_once('/')) {
            ("GET", Some((run, "logs"))) | ("POST", Some((run, "stop"))) => uuid_id_ok(run),
            _ => false,
        };
    }
    if let Some(item) = tail
        .strip_prefix("roles/")
        .or_else(|| tail.strip_prefix("positions/"))
    {
        return (method == "PUT" || method == "DELETE") && uuid_id_ok(item);
    }
    if tail == "departments" {
        return method == "GET" || method == "POST";
    }
    if let Some(department_id) = tail.strip_prefix("departments/") {
        return (method == "PUT" || method == "DELETE") && uuid_id_ok(department_id);
    }
    if let Some(member_tail) = tail.strip_prefix("members/") {
        return match (method, member_tail.split_once('/')) {
            ("GET", Some((user_id, "role-audit"))) => uuid_id_ok(user_id),
            ("PUT", Some((user_id, "role" | "position" | "department" | "status"))) => {
                uuid_id_ok(user_id)
            }
            ("DELETE", None) => uuid_id_ok(member_tail),
            _ => false,
        };
    }
    if matches!(tail, "members" | "roles" | "positions" | "join-invite") {
        return method == "GET" || (method == "PUT" && tail == "join-invite");
    }
    // GET lists applicants for an admin. POST is the signed-in person asking to join.
    if tail == "join-requests" {
        return method == "GET" || method == "POST";
    }
    if let Some(request_tail) = tail.strip_prefix("join-requests/") {
        return method == "PUT"
            && request_tail
                .split_once('/')
                .is_some_and(|(member_id, action)| {
                    uuid_id_ok(member_id) && matches!(action, "approval" | "rejection")
                });
    }
    if tail == "usage" {
        return method == "GET";
    }
    if tail == "schedule/events" {
        return method == "GET" || method == "POST";
    }
    if let Some(event_id) = tail.strip_prefix("schedule/events/") {
        return matches!(method, "PUT" | "DELETE")
            && !event_id.contains('/')
            && uuid_id_ok(event_id);
    }
    false
}

/// The member IM routes of a3s-api (`docs/IM.md` §6.1), below `/v1/api/org/im/`.
fn allowed_im_path(method: &str, rest: &str) -> bool {
    let segments: Vec<&str> = rest.split('/').collect();
    let Some((org, tail)) = segments.split_first() else {
        return false;
    };
    if !uuid_id_ok(org) {
        return false;
    }
    match (method, tail) {
        ("GET", ["conversations" | "directory" | "departments" | "me" | "presence"]) => true,
        ("GET", ["conversations", id]) => uuid_id_ok(id),
        ("GET", ["conversations", id, "messages"]) => uuid_id_ok(id),
        ("GET", ["attachments", id]) => uuid_id_ok(id),
        ("POST", ["direct-conversations" | "group-conversations"]) => true,
        ("PUT", ["file-transfer" | "presence"]) => true,
        ("POST", ["conversations", id, "members" | "messages" | "typing"]) => uuid_id_ok(id),
        ("PUT", ["conversations", id]) => uuid_id_ok(id),
        (
            "PUT",
            [
                "conversations",
                id,
                "muted" | "pinned" | "read-cursor" | "unread-cursor" | "remark" | "nickname"
                | "speech" | "owner",
            ],
        ) => uuid_id_ok(id),
        ("PUT", ["conversations", id, "members", user, "role" | "speech"]) => {
            uuid_id_ok(id) && uuid_id_ok(user)
        }
        ("DELETE", ["conversations", id, "membership"]) => uuid_id_ok(id),
        ("DELETE", ["conversations", id]) => uuid_id_ok(id),
        ("DELETE", ["messages", id]) => uuid_id_ok(id),
        ("POST", ["message-syncs" | "stream-tickets" | "attachments"]) => true,
        ("DELETE", ["conversations", id, "members", user]) => uuid_id_ok(id) && uuid_id_ok(user),
        _ => false,
    }
}

fn allowed_drive_path(method: &str, rest: &str) -> bool {
    let segments: Vec<&str> = rest
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    let Some((org, tail)) = segments.split_first() else {
        return false;
    };
    if !uuid_id_ok(org) {
        return false;
    }
    match (method, tail) {
        ("GET", ["nodes" | "files"]) => true,
        ("POST", ["nodes" | "files" | "grants"]) => true,
        ("GET" | "PUT", ["files", id, "content"]) => uuid_id_ok(id),
        ("PUT", ["files", id, "name" | "parent"]) => uuid_id_ok(id),
        ("GET", ["files", id, "encyclopedia"]) => uuid_id_ok(id),
        ("POST", ["files", id, "parse"]) => uuid_id_ok(id),
        ("GET", ["pipeline"]) => true,
        ("DELETE", ["files", id]) => uuid_id_ok(id),
        ("PUT", ["nodes", id, "name" | "parent" | "encyclopedia"]) => uuid_id_ok(id),
        ("DELETE", ["nodes", id]) => uuid_id_ok(id),
        ("DELETE", ["grants", dept, user]) => uuid_id_ok(dept) && uuid_id_ok(user),
        ("GET", ["encyclopedia"]) => true,
        ("GET", ["encyclopedia", id]) => uuid_id_ok(id),
        ("POST", ["encyclopedia"]) => true,
        (
            "POST",
            [
                "encyclopedia",
                id,
                "publish" | "return" | "retry" | "detach",
            ],
        ) => uuid_id_ok(id),
        ("POST", ["encyclopedia-questions" | "encyclopedia-passages"]) => true,
        _ => false,
    }
}

pub fn org_id_ok(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

/// Department / member / role ids are UUIDs upstream; keep proxy sub-ids to
/// that shape.
pub fn uuid_id_ok(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 36
        && id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
}

/// Same shape the OS accepts for a capability slug.
pub fn capability_slug_ok(slug: &str) -> bool {
    let bytes = slug.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 64
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

fn forward(origin: &str, path: &str, body: &[u8]) -> io::Result<(u16, Vec<u8>)> {
    forward_request(origin, "POST", path, body, None)
}

/// Member-plane call using the session stored by the shell. The renderer
/// never receives that `a3s_ms_` token; it only presents the shell bearer.
pub fn forward_member_api(
    origin: &str,
    method: &str,
    path: &str,
    body: &[u8],
    bearer: &str,
) -> Response {
    if !matches!(method, "GET" | "POST" | "PUT" | "DELETE") {
        return (
            405,
            "Method Not Allowed",
            "application/json",
            br#"{"error":"method"}"#.to_vec(),
        );
    }
    if !allowed_member_path(method, path.split('?').next().unwrap_or(path)) {
        return (
            404,
            "Not Found",
            "application/json",
            br#"{"error":"not_found"}"#.to_vec(),
        );
    }
    match forward_request(origin, method, path, body, Some(bearer)) {
        Ok((status, payload)) => (status, reason(status), "application/json", payload),
        Err(_) => gateway(),
    }
}

/// Capability icon bytes. The renderer requests this shell path with the shell bearer.
/// The gateway requires the member session, so the shell attaches the stored one.
pub fn forward_member_icon(origin: &str, path: &str, bearer: &str) -> Response {
    let path_only = path.split('?').next().unwrap_or(path);
    if !allowed_member_path("GET", path_only) {
        return not_found();
    }
    match exchange(origin, "GET", path, &[], Some(bearer), MAX_RESPONSE) {
        Ok((status, payload)) => {
            let media = if (200..300).contains(&status) {
                icon_media_type(&payload)
            } else {
                "application/json"
            };
            (status, reason(status), media, payload)
        }
        Err(_) => gateway(),
    }
}

fn icon_media_type(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        "image/jpeg"
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else if bytes
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .is_some_and(|index| bytes[index..].starts_with(b"<"))
    {
        "image/svg+xml"
    } else {
        "application/octet-stream"
    }
}

/// Drive file bytes. Lists stay on the JSON proxy. Upload and download use this lane
/// so the main window can store files with the shell bearer.
pub fn is_drive_file_byte_route(method: &str, path: &str) -> bool {
    let Some(rest) = path.strip_prefix("/api/drive/") else {
        return false;
    };
    let segments: Vec<&str> = rest
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    let Some((_, tail)) = segments.split_first() else {
        return false;
    };
    matches!(
        (method, tail),
        ("POST", ["files"]) | ("GET" | "PUT", ["files", _, "content"])
    )
}

pub fn relay_drive_file_bytes(
    origin: &str,
    method: &str,
    path: &str,
    query: &str,
    body: &[u8],
    content_type: Option<&str>,
    bearer: Option<&str>,
) -> Response {
    let Some(rest) = path.strip_prefix("/api/drive/") else {
        return not_found();
    };
    if !allowed_drive_path(method, rest) {
        return not_found();
    }
    let Some(token) = bearer.filter(|token| !token.is_empty()) else {
        return (
            401,
            "Unauthorized",
            "application/json",
            br#"{"error":"unauthorized"}"#.to_vec(),
        );
    };
    if query
        .bytes()
        .any(|byte| byte.is_ascii_control() || byte == b' ')
    {
        return (
            400,
            "Bad Request",
            "application/json",
            br#"{"error":"bad_request"}"#.to_vec(),
        );
    }
    let upstream = if query.is_empty() {
        format!("/v1/api/org/drive/{rest}")
    } else {
        format!("/v1/api/org/drive/{rest}?{query}")
    };
    let exchanged = if method == "GET" {
        exchange(origin, "GET", &upstream, &[], Some(token), MAX_HUB_PREVIEW)
    } else {
        let Some(content_type) = content_type.filter(|value| {
            value.starts_with("multipart/form-data")
                && value.len() <= 256
                && !value.bytes().any(|byte| byte.is_ascii_control())
        }) else {
            return (
                400,
                "Bad Request",
                "application/json",
                br#"{"error":"bad_request"}"#.to_vec(),
            );
        };
        exchange_typed(
            origin,
            method,
            &upstream,
            body,
            Some(token),
            Some(content_type),
            MAX_HUB_PREVIEW,
        )
    };
    match exchanged {
        Ok((status, payload)) if (200..300).contains(&status) && method == "GET" => {
            (status, reason(status), "application/octet-stream", payload)
        }
        Ok((status, payload)) if (200..300).contains(&status) => {
            (status, reason(status), "application/json", payload)
        }
        Ok((status, payload)) => (status, reason(status), "application/json", payload),
        Err(_) => gateway(),
    }
}

/// Handbook file bytes. The list stays on the JSON proxy; this path is the object.
pub fn forward_hub_content(origin: &str, path: &str, bearer: &str) -> Response {
    let path_only = path.split('?').next().unwrap_or(path);
    if !allowed_member_path("GET", path_only) {
        return not_found();
    }
    match exchange(origin, "GET", path, &[], Some(bearer), MAX_HUB_PREVIEW) {
        Ok((status, payload)) if (200..300).contains(&status) => {
            (status, reason(status), "application/octet-stream", payload)
        }
        Ok((status, payload)) => (status, reason(status), "application/json", payload),
        Err(_) => gateway(),
    }
}

/// An IM attachment upload (raw bytes in) or download (raw bytes out). Errors come
/// back as the API's JSON envelope.
pub fn forward_member_binary(
    origin: &str,
    method: &str,
    path: &str,
    body: &[u8],
    bearer: &str,
) -> Response {
    let path_only = path.split('?').next().unwrap_or(path);
    let attachment = path_only
        .strip_prefix("/v1/api/org/im/")
        .and_then(|rest| rest.split_once('/'))
        .is_some_and(|(_, tail)| match method {
            "POST" => tail == "attachments",
            "GET" => tail.starts_with("attachments/"),
            _ => false,
        });
    if !attachment || !allowed_member_path(method, path_only) {
        return not_found();
    }
    let content_type = (method == "POST").then_some("application/octet-stream");
    match exchange_typed(
        origin,
        method,
        path,
        body,
        Some(bearer),
        content_type,
        MAX_IM_ATTACHMENT,
    ) {
        Ok((status, payload)) if (200..300).contains(&status) && method == "GET" => {
            (status, reason(status), "application/octet-stream", payload)
        }
        Ok((status, payload)) => (status, reason(status), "application/json", payload),
        Err(_) => gateway(),
    }
}

/// An open upstream SSE response: status, whether the body is chunked, the bytes
/// already read past the headers, and the socket to keep reading.
pub struct UpstreamStream {
    pub status: u16,
    pub chunked: bool,
    pub prelude: Vec<u8>,
    pub stream: TcpStream,
}

/// Opens `GET path` (a one-time stream ticket in the query) without a bearer.
pub fn open_event_stream(origin: &str, path: &str) -> io::Result<UpstreamStream> {
    let address = parse_loopback(origin).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "org api origin must be loopback",
        )
    })?;
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))?;
    stream.set_read_timeout(Some(UPSTREAM_TIMEOUT))?;
    stream.set_write_timeout(Some(UPSTREAM_TIMEOUT))?;
    let head = format!(
        "GET {path} HTTP/1.1\r\nHost: {address}\r\nAccept: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(head.as_bytes())?;
    stream.flush()?;
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    let split = loop {
        if buf.len() > 64 * 1024 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "headers"));
        }
        let read = stream.read(&mut tmp)?;
        if read == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "headers"));
        }
        buf.extend_from_slice(&tmp[..read]);
        if let Some(pos) = find_bytes(&buf, b"\r\n\r\n") {
            break pos;
        }
    };
    let head = String::from_utf8_lossy(&buf[..split]).into_owned();
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "status"))?;
    let chunked = head.split("\r\n").skip(1).any(|line| {
        line.split_once(':').is_some_and(|(name, value)| {
            name.eq_ignore_ascii_case("transfer-encoding")
                && value.trim().eq_ignore_ascii_case("chunked")
        })
    });
    Ok(UpstreamStream {
        status,
        chunked,
        prelude: buf[split + 4..].to_vec(),
        stream,
    })
}

/// Skill bundle bytes for an installed capability. Never routed to the page: only the
/// shell's local materializer reads bundles.
pub fn fetch_capability_bundle(origin: &str, slug: &str, bearer: &str) -> io::Result<Vec<u8>> {
    if !capability_slug_ok(slug) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "capability slug",
        ));
    }
    let path = format!("/v1/api/org/capabilities/{slug}/bundle");
    let (status, bytes) = exchange(origin, "GET", &path, &[], Some(bearer), MAX_BUNDLE)?;
    if status != 200 {
        return Err(io::Error::other(format!(
            "bundle download returned HTTP {status}"
        )));
    }
    Ok(bytes)
}

/// Same bundle fetch, reporting bytes received against `Content-Length` or the
/// catalog's package size. A transfer with no known total reports nothing.
pub fn fetch_capability_bundle_with_progress(
    origin: &str,
    slug: &str,
    bearer: &str,
    known_total: Option<u64>,
    mut on_progress: impl FnMut(u64, u64),
) -> io::Result<Vec<u8>> {
    if !capability_slug_ok(slug) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "capability slug",
        ));
    }
    let path = format!("/v1/api/org/capabilities/{slug}/bundle");
    let (status, bytes) = exchange_reporting(
        origin,
        "GET",
        &path,
        bearer,
        MAX_BUNDLE,
        known_total,
        &mut on_progress,
    )?;
    if status != 200 {
        return Err(io::Error::other(format!(
            "bundle download returned HTTP {status}"
        )));
    }
    Ok(bytes)
}

fn forward_request(
    origin: &str,
    method: &str,
    path: &str,
    body: &[u8],
    bearer: Option<&str>,
) -> io::Result<(u16, Vec<u8>)> {
    exchange(origin, method, path, body, bearer, MAX_RESPONSE)
}

pub(crate) fn exchange_document_resource(
    origin: &str,
    method: &str,
    path: &str,
    body: &[u8],
    bearer: &str,
) -> io::Result<(u16, Vec<u8>)> {
    if !crate::authority::resource_broker::allows_upstream(method, path) {
        return Err(io::ErrorKind::NotFound.into());
    }
    if parse_loopback(origin).is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "document origin must be loopback",
        ));
    }
    static CLIENT: std::sync::OnceLock<Result<reqwest::blocking::Client, String>> =
        std::sync::OnceLock::new();
    let client = CLIENT
        .get_or_init(|| {
            reqwest::blocking::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(2))
                .timeout(Duration::from_secs(30))
                .build()
                .map_err(|error| error.to_string())
        })
        .as_ref()
        .map_err(|_| io::Error::other("document transport unavailable"))?;
    let method =
        reqwest::Method::from_bytes(method.as_bytes()).map_err(|_| io::ErrorKind::InvalidInput)?;
    let response = client
        .request(method, format!("{origin}{path}"))
        .bearer_auth(bearer)
        .header("content-type", "application/json")
        .body(body.to_vec())
        .send()
        .map_err(|_| io::Error::other("document request failed"))?;
    let status = response.status().as_u16();
    let max_bytes = 100 * 1024 * 1024 + 16_384;
    if response
        .content_length()
        .is_some_and(|length| length > max_bytes)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "document response too large",
        ));
    }
    let mut bytes = Vec::new();
    response.take(max_bytes + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "document response too large",
        ));
    }
    Ok((status, bytes))
}

fn exchange(
    origin: &str,
    method: &str,
    path: &str,
    body: &[u8],
    bearer: Option<&str>,
    max_body: usize,
) -> io::Result<(u16, Vec<u8>)> {
    exchange_typed(
        origin,
        method,
        path,
        body,
        bearer,
        Some("application/json"),
        max_body,
    )
}

fn exchange_typed(
    origin: &str,
    method: &str,
    path: &str,
    body: &[u8],
    bearer: Option<&str>,
    content_type: Option<&str>,
    max_body: usize,
) -> io::Result<(u16, Vec<u8>)> {
    let address = parse_loopback(origin).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "org api origin must be loopback",
        )
    })?;
    let auth = match bearer {
        Some(token)
            if !token.is_empty()
                && !token
                    .bytes()
                    .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control()) =>
        {
            format!("Authorization: Bearer {token}\r\n")
        }
        Some(_) => {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "member token"));
        }
        None => String::new(),
    };
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))?;
    stream.set_read_timeout(Some(UPSTREAM_TIMEOUT))?;
    stream.set_write_timeout(Some(UPSTREAM_TIMEOUT))?;
    let head = if method == "GET" {
        format!(
            "GET {path} HTTP/1.1\r\nHost: {address}\r\nAccept: */*\r\n{auth}Connection: close\r\n\r\n"
        )
    } else {
        format!(
            "{method} {path} HTTP/1.1\r\nHost: {address}\r\nAccept: application/json\r\nContent-Type: {}\r\nContent-Length: {}\r\n{auth}Connection: close\r\n\r\n",
            content_type.unwrap_or("application/json"),
            body.len()
        )
    };
    stream.write_all(head.as_bytes())?;
    if method != "GET" {
        stream.write_all(body)?;
    }
    stream.flush()?;
    read_response(&mut stream, max_body)
}

pub(crate) fn parse_loopback(origin: &str) -> Option<SocketAddr> {
    let rest = origin.strip_prefix("http://")?;
    if rest.is_empty() || rest.contains(['/', '@', ' ', '?', '#']) {
        return None;
    }
    let (host, port) = rest.rsplit_once(':')?;
    if host != "127.0.0.1" && host != "localhost" {
        return None;
    }
    let port = port.parse::<u16>().ok()?;
    if port == 0 {
        return None;
    }
    Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port))
}

fn read_response(stream: &mut TcpStream, max_body: usize) -> io::Result<(u16, Vec<u8>)> {
    read_response_reporting(stream, max_body, None, &mut |_, _| {})
}

fn exchange_reporting(
    origin: &str,
    method: &str,
    path: &str,
    bearer: &str,
    max_body: usize,
    known_total: Option<u64>,
    on_progress: &mut dyn FnMut(u64, u64),
) -> io::Result<(u16, Vec<u8>)> {
    if bearer.is_empty()
        || bearer
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
    {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "member token"));
    }
    let address = parse_loopback(origin).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "org api origin must be loopback",
        )
    })?;
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))?;
    stream.set_read_timeout(Some(Duration::from_secs(60)))?;
    stream.set_write_timeout(Some(Duration::from_secs(60)))?;
    let head = format!(
        "{method} {path} HTTP/1.1\r\nHost: {address}\r\nAccept: */*\r\nAuthorization: Bearer {bearer}\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(head.as_bytes())?;
    stream.flush()?;
    read_response_reporting(&mut stream, max_body, known_total, on_progress)
}

fn read_response_reporting(
    stream: &mut TcpStream,
    max_body: usize,
    known_total: Option<u64>,
    on_progress: &mut dyn FnMut(u64, u64),
) -> io::Result<(u16, Vec<u8>)> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 8192];
    let split = loop {
        if buf.len() > 64 * 1024 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "headers"));
        }
        let read = stream.read(&mut tmp)?;
        if read == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "headers"));
        }
        buf.extend_from_slice(&tmp[..read]);
        if let Some(pos) = find_bytes(&buf, b"\r\n\r\n") {
            break pos;
        }
    };
    let head = String::from_utf8_lossy(&buf[..split]).into_owned();
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "status"))?;
    let total = content_length(&head)
        .map(|value| value as u64)
        .or(known_total)
        .filter(|value| *value > 0);
    let mut reported = 0u64;
    let mut report = |received: u64| {
        let Some(total) = total else {
            return;
        };
        let received = received.min(total);
        if received <= reported {
            return;
        }
        reported = received;
        on_progress(received, total);
    };
    let mut body = buf[split + 4..].to_vec();
    if !body.is_empty() {
        report(body.len() as u64);
    }
    if let Some(length) = content_length(&head) {
        if length > max_body {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "body"));
        }
        while body.len() < length {
            let read = stream.read(&mut tmp)?;
            if read == 0 {
                break;
            }
            body.extend_from_slice(&tmp[..read]);
            report(body.len() as u64);
        }
        if body.len() < length {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "body"));
        }
        body.truncate(length);
    } else {
        loop {
            let read = match stream.read(&mut tmp) {
                Ok(0) => break,
                Ok(n) => n,
                Err(err)
                    if err.kind() == io::ErrorKind::WouldBlock
                        || err.kind() == io::ErrorKind::TimedOut =>
                {
                    break;
                }
                Err(err) => return Err(err),
            };
            if body.len() + read > max_body {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "body"));
            }
            body.extend_from_slice(&tmp[..read]);
            report(body.len() as u64);
        }
    }
    Ok((status, body))
}

fn content_length(head: &str) -> Option<usize> {
    for line in head.split("\r\n").skip(1) {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.eq_ignore_ascii_case("content-length") {
            return value.trim().parse().ok();
        }
    }
    None
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        409 => "Conflict",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        _ => "Error",
    }
}

#[cfg(test)]
#[path = "org_login_role_history_tests.rs"]
mod role_history_tests;

#[cfg(test)]
#[path = "org_login_proxy_gap_tests.rs"]
mod proxy_gap_tests;

#[cfg(test)]
#[path = "org_login_platform_page_tests.rs"]
mod platform_page_tests;

#[cfg(test)]
#[path = "org_login_captcha_tests.rs"]
mod captcha_tests;

#[cfg(test)]
#[path = "org_login_team_admin_tests.rs"]
mod team_admin_tests;

#[path = "org_login_repositories.rs"]
mod repositories;

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use super::{asset, forward_member_api, route};

    fn read_request(stream: &mut std::net::TcpStream) -> String {
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        let mut buf = Vec::new();
        let mut tmp = [0u8; 2048];
        loop {
            match stream.read(&mut tmp) {
                Ok(0) => break,
                Ok(n) => {
                    buf.extend_from_slice(&tmp[..n]);
                    if let Some(split) = buf.windows(4).position(|window| window == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&buf[..split]);
                        let length = head.lines().skip(1).find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().ok())
                                .flatten()
                        });
                        if buf.len() >= split + 4 + length.unwrap_or(0) {
                            break;
                        }
                    }
                }
                Err(_) => break,
            }
        }
        String::from_utf8_lossy(&buf).into_owned()
    }

    fn write_json(stream: &mut std::net::TcpStream, payload: &[u8]) {
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            payload.len()
        );
        stream.write_all(response.as_bytes()).unwrap();
        stream.write_all(payload).unwrap();
    }

    #[test]
    fn published_notice_reads_the_v1_api_document() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let raw = read_request(&mut stream);
            write_json(
                &mut stream,
                br#"{"code":0,"data":{"url":"http://127.0.0.1:3000/v1/api/legal/terms"}}"#,
            );
            raw
        });
        let origin = format!("http://{address}");
        let (status, _, _, _) = route(
            "GET",
            &["api", "publications", "terms"],
            b"",
            &origin,
            "computer",
        )
        .expect("route");
        assert_eq!(status, 200);
        let raw = server.join().unwrap();
        assert!(raw.contains("GET /v1/api/legal/terms/document "), "{raw}");
    }

    #[test]
    fn forwards_the_phone_code_after_the_captcha_token_is_consumed() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let mut seen = Vec::new();
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let raw = read_request(&mut stream);
                let payload: &[u8] = if raw.contains("/v1/api/admin/captcha/token-verifications") {
                    br#"{"code":0,"message":"ok","data":{"valid":true}}"#
                } else {
                    br#"{"code":0,"message":"ok","data":{"ok":true,"cooldown_seconds":60,"dev_code":"654321"}}"#
                };
                write_json(&mut stream, payload);
                seen.push(raw);
            }
            seen
        });
        let origin = format!("http://{address}");
        let body = br#"{"phone":"13800138000","captchaToken":"captcha_v1.example"}"#;
        let (status, _, content_type, payload) = route(
            "POST",
            &["v1", "api", "org", "members", "phone-codes"],
            body,
            &origin,
            "computer-1",
        )
        .unwrap();
        assert_eq!(status, 200);
        assert_eq!(content_type, "application/json");
        let text = String::from_utf8(payload).unwrap();
        assert!(text.contains("\"dev_code\":\"654321\""), "{text}");
        let seen = server.join().unwrap();
        assert!(
            seen[0].contains("POST /v1/api/admin/captcha/token-verifications HTTP/1.1"),
            "{seen:?}"
        );
        assert!(seen[0].contains("captcha_v1.example"), "{seen:?}");
        assert!(
            seen[1].contains("POST /v1/api/org/members/phone-codes HTTP/1.1"),
            "{seen:?}"
        );
        assert!(seen[1].contains("\"phone\":\"13800138000\""), "{seen:?}");
        assert!(!seen[1].contains("captchaToken"), "{seen:?}");
        assert!(
            !seen[1].to_ascii_lowercase().contains("authorization"),
            "{seen:?}"
        );
    }

    #[test]
    fn phone_code_without_a_captcha_does_not_reach_the_api() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let (status, _, _, payload) = route(
            "POST",
            &["v1", "api", "org", "members", "phone-codes"],
            br#"{"phone":"13800138000"}"#,
            &origin,
            "computer-1",
        )
        .unwrap();
        assert_eq!(status, 400);
        let text = String::from_utf8(payload).unwrap();
        assert!(text.contains("captcha required"), "{text}");
        assert!(listener.accept().unwrap_err().kind() == std::io::ErrorKind::WouldBlock);
    }

    #[test]
    fn serves_a_local_captcha_image_and_rejects_other_names() {
        let (status, _, content_type, bytes) =
            asset("GET", &["captcha", "bg-n060.jpg"]).expect("route");
        assert_eq!(status, 200, "{content_type}");
        assert_eq!(content_type, "image/jpeg");
        assert!(bytes.starts_with(&[0xff, 0xd8, 0xff]));
        let (missing, _, _, _) = asset("GET", &["captcha", "../manifest.json"]).unwrap();
        assert_eq!(missing, 404);
        assert!(asset("GET", &["mark.png"]).is_none());
    }

    #[test]
    fn refuses_to_forward_anywhere_but_loopback() {
        let (status, _, _, payload) = route(
            "POST",
            &["v1", "api", "org", "members", "sessions"],
            br#"{"identifier":"13800138000","password":"secret-secret","captchaToken":"captcha_v1.example"}"#,
            "http://example.com:3000",
            "computer-1",
        )
        .unwrap();
        assert_eq!(status, 502);
        let text = String::from_utf8(payload).unwrap();
        assert!(text.contains("HTTP 502"), "{text}");
        assert!(!text.contains("secret-secret"), "{text}");
        assert!(!text.contains("captcha_v1.example"), "{text}");
        assert!(
            route(
                "GET",
                &["v1", "api", "org", "members", "me"],
                b"",
                "http://127.0.0.1:3000",
                "computer-1",
            )
            .is_none()
        );
    }

    #[test]
    fn forwards_the_member_org_list_with_the_stored_bearer() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let raw = read_request(&mut stream);
            write_json(
                &mut stream,
                br#"{"code":0,"message":"ok","data":[{"id":"org-1","name":"Tongyou"}]}"#,
            );
            raw
        });
        let origin = format!("http://{address}");
        let (status, _, content_type, payload) = forward_member_api(
            &origin,
            "GET",
            "/v1/api/org/members/orgs",
            b"",
            "a3s_ms_session.token",
        );
        assert_eq!(status, 200);
        assert_eq!(content_type, "application/json");
        let text = String::from_utf8(payload).unwrap();
        assert!(text.contains("Tongyou"), "{text}");
        let seen = server.join().unwrap();
        assert!(
            seen.contains("GET /v1/api/org/members/orgs HTTP/1.1"),
            "{seen}"
        );
        assert!(
            seen.contains("Authorization: Bearer a3s_ms_session.token"),
            "{seen}"
        );
        let (rejected, _, _, _) = forward_member_api(
            &origin,
            "GET",
            "/v1/api/org/members/verifications/reviews",
            b"",
            "a3s_ms_session.token",
        );
        assert_eq!(rejected, 404);
        let (broken, _, _, _) = forward_member_api(
            &origin,
            "GET",
            "/v1/api/org/members/orgs",
            b"",
            "bad\r\ntoken",
        );
        assert_eq!(broken, 502);
    }

    #[test]
    fn platform_routes_keep_closed_resource_admission() {
        for resource in super::PLATFORM_READS {
            let path = format!("/v1/api/org/members/platform/{resource}");
            assert!(super::allowed_member_path("GET", &path), "{path}");
            assert_eq!(
                super::allowed_member_path("POST", &path),
                *resource == "suppliers",
                "{path}"
            );
        }
        let people =
            "/v1/api/org/members/platform/customers/7f8c1c62-6a32-4e7d-9a1b-3a2f0c4d5e6f/people";
        assert!(super::allowed_member_path("GET", people));
        assert!(!super::allowed_member_path("DELETE", people));
        assert!(!super::allowed_member_path(
            "GET",
            "/v1/api/org/members/platform/customers/../people"
        ));
        assert!(!super::allowed_member_path(
            "GET",
            "/v1/api/org/members/platform/customers/7f8c1c62-6a32-4e7d-9a1b-3a2f0c4d5e6f/orders"
        ));
        assert!(!super::allowed_member_path(
            "GET",
            "/v1/api/org/members/platform/secrets"
        ));
        assert!(!super::allowed_member_path(
            "GET",
            "/v1/api/org/members/platform/logs/extra"
        ));
        let id = "7f8c1c62-6a32-4e7d-9a1b-3a2f0c4d5e6f";
        for tail in [
            format!("suppliers/{id}/models"),
            format!("functions/{id}/releases"),
            format!("agent-runtimes/{id}"),
            format!("agent-runtimes/{id}/logs"),
            format!("cells/{id}/{id}/{id}/revisions"),
        ] {
            let path = format!("/v1/api/org/members/platform/{tail}");
            assert!(super::allowed_member_path("GET", &path), "{path}");
            assert!(!super::allowed_member_path("POST", &path), "{path}");
        }
        for tail in [
            format!("agent-runtimes/{id}/stop"),
            "agent-runtimes/../logs".to_owned(),
            format!("functions/{id}/runs"),
            format!("suppliers/{id}/credential"),
            "suppliers/deepseek/models".to_owned(),
            "model-routes/deepseek".to_owned(),
            format!("usage/{id}"),
            "logs/extra".to_owned(),
            "logs/levels/extra".to_owned(),
            format!("cells/{id}/{id}/revisions"),
            format!("cells/{id}/{id}/x/revisions"),
        ] {
            let path = format!("/v1/api/org/members/platform/{tail}");
            assert!(!super::allowed_member_path("GET", &path), "{path}");
        }
    }

    #[test]
    fn allows_only_the_team_list_and_its_departments() {
        assert!(super::allowed_member_path(
            "GET",
            "/v1/api/org/members/orgs"
        ));
        assert!(super::allowed_member_path(
            "POST",
            "/v1/api/org/members/orgs"
        ));
        assert!(super::allowed_member_path(
            "GET",
            "/v1/api/org/members/orgs/org-1/departments"
        ));
        assert!(super::allowed_member_path(
            "GET",
            "/v1/api/org/members/orgs/org-1/usage"
        ));
        assert!(!super::allowed_member_path(
            "POST",
            "/v1/api/org/members/orgs/org-1/usage"
        ));
        assert!(!super::allowed_member_path(
            "GET",
            "/v1/api/org/members/orgs/org-1/usage/extra"
        ));
        assert!(!super::allowed_member_path(
            "GET",
            "/v1/api/org/members/orgs/../departments"
        ));
        assert!(super::allowed_member_path(
            "GET",
            "/v1/api/org/members/work-items"
        ));
        assert!(super::allowed_member_path(
            "POST",
            "/v1/api/org/members/work-items"
        ));
        assert!(super::allowed_member_path(
            "GET",
            "/v1/api/org/members/work-items/summary"
        ));
        assert!(super::allowed_member_path(
            "POST",
            "/v1/api/org/members/work-items/process-preview"
        ));
        assert!(super::allowed_member_path(
            "POST",
            "/v1/api/org/members/work-items/process"
        ));
        assert!(!super::allowed_member_path(
            "GET",
            "/v1/api/org/members/work-items/process-preview"
        ));
        assert!(!super::allowed_member_path(
            "GET",
            "/v1/api/org/members/work-items/process"
        ));
        assert!(super::allowed_member_path(
            "GET",
            "/v1/api/org/members/work-items/approvers"
        ));
        assert!(super::allowed_member_path(
            "GET",
            "/v1/api/org/members/work-items/travel-links"
        ));
        assert!(super::allowed_member_path(
            "POST",
            "/v1/api/org/members/work-items/11111111-1111-4111-8111-111111111111/schedule"
        ));
        assert!(super::allowed_member_path(
            "POST",
            "/v1/api/org/members/work-items/11111111-1111-4111-8111-111111111111/decision"
        ));
        assert!(!super::allowed_member_path(
            "DELETE",
            "/v1/api/org/members/work-items"
        ));
        assert!(!super::allowed_member_path(
            "POST",
            "/v1/api/org/members/work-items/not-a-uuid/decision"
        ));
        assert!(super::allowed_member_path("GET", "/v1/api/org/members/me"));
        assert!(super::allowed_member_path("PUT", "/v1/api/org/members/me"));
        assert!(!super::allowed_member_path(
            "POST",
            "/v1/api/org/members/me"
        ));
        assert!(!super::allowed_member_path(
            "POST",
            "/v1/api/org/members/logout"
        ));
        assert!(!super::org_id_ok("org/1"));
    }

    #[test]
    fn allows_the_org_directory_routes_the_members_page_drives() {
        let org = "/v1/api/org/members/orgs/org-1";
        assert!(super::allowed_member_path(
            "POST",
            &format!("{org}/departments")
        ));
        assert!(super::allowed_member_path(
            "PUT",
            &format!("{org}/departments/0197e5a2-89f1-7d33-a11c-5f53d9b3af10")
        ));
        assert!(super::allowed_member_path(
            "DELETE",
            &format!("{org}/departments/0197e5a2-89f1-7d33-a11c-5f53d9b3af10")
        ));
        assert!(super::allowed_member_path("GET", &format!("{org}/members")));
        assert!(super::allowed_member_path("GET", &format!("{org}/roles")));
        assert!(super::allowed_member_path(
            "GET",
            &format!("{org}/positions")
        ));
        assert!(super::allowed_member_path(
            "PUT",
            &format!("{org}/members/0197e5a2-89f1-7d33-a11c-5f53d9b3af10/role")
        ));
        assert!(super::allowed_member_path(
            "PUT",
            &format!("{org}/members/0197e5a2-89f1-7d33-a11c-5f53d9b3af10/position")
        ));
        assert!(super::allowed_member_path(
            "DELETE",
            &format!("{org}/members/0197e5a2-89f1-7d33-a11c-5f53d9b3af10")
        ));
        assert!(super::allowed_member_path(
            "GET",
            "/v1/api/org/members/discoverable-orgs"
        ));
        assert!(!super::allowed_member_path(
            "POST",
            "/v1/api/org/members/discoverable-orgs"
        ));
        assert!(super::allowed_member_path(
            "GET",
            &format!("{org}/join-requests")
        ));
        assert!(super::allowed_member_path(
            "POST",
            &format!("{org}/join-requests")
        ));
        assert!(!super::allowed_member_path(
            "DELETE",
            &format!("{org}/join-requests")
        ));
        assert!(super::allowed_member_path(
            "PUT",
            &format!("{org}/join-requests/0197e5a2-89f1-7d33-a11c-5f53d9b3af10/approval")
        ));
        assert!(super::allowed_member_path(
            "PUT",
            &format!("{org}/join-requests/0197e5a2-89f1-7d33-a11c-5f53d9b3af10/rejection")
        ));
        assert!(super::allowed_member_path(
            "GET",
            &format!("{org}/join-invite")
        ));
        assert!(super::allowed_member_path(
            "PUT",
            &format!("{org}/join-invite")
        ));

        assert!(!super::allowed_member_path("POST", org));
        assert!(!super::allowed_member_path(
            "POST",
            &format!("{org}/departments/0197e5a2-89f1-7d33-a11c-5f53d9b3af10")
        ));
        assert!(!super::allowed_member_path(
            "PUT",
            &format!("{org}/departments/not-a-uuid")
        ));
        assert!(!super::allowed_member_path(
            "PUT",
            &format!("{org}/members/0197e5a2-89f1-7d33-a11c-5f53d9b3af10/admin")
        ));
        assert!(!super::allowed_member_path(
            "DELETE",
            &format!("{org}/members/0197e5a2-89f1-7d33-a11c-5f53d9b3af10/role")
        ));
        assert!(!super::allowed_member_path(
            "POST",
            &format!("{org}/join-requests/0197e5a2-89f1-7d33-a11c-5f53d9b3af10/renew")
        ));
        assert!(!super::allowed_member_path(
            "DELETE",
            &format!("{org}/join-invite")
        ));
        assert!(!super::allowed_member_path(
            "PUT",
            &format!("{org}/join-requests")
        ));
        assert!(!super::uuid_id_ok("0197e5a2/89f1"));
        assert!(!super::uuid_id_ok(&"a".repeat(37)));
    }

    #[test]
    fn allows_the_catalog_and_team_settings_routes() {
        let org = "/v1/api/org/members/orgs/org-1";
        let id = "0197e5a2-89f1-7d33-a11c-5f53d9b3af10";
        for tail in ["roles", "positions"] {
            assert!(super::allowed_member_path("POST", &format!("{org}/{tail}")));
            assert!(super::allowed_member_path(
                "PUT",
                &format!("{org}/{tail}/{id}")
            ));
            assert!(super::allowed_member_path(
                "DELETE",
                &format!("{org}/{tail}/{id}")
            ));
            assert!(!super::allowed_member_path(
                "POST",
                &format!("{org}/{tail}/{id}")
            ));
            assert!(!super::allowed_member_path(
                "PUT",
                &format!("{org}/{tail}/not-a-uuid")
            ));
        }
        assert!(super::allowed_member_path(
            "GET",
            &format!("{org}/schedule/events")
        ));
        assert!(super::allowed_member_path(
            "POST",
            &format!("{org}/schedule/events")
        ));
        assert!(super::allowed_member_path(
            "PUT",
            &format!("{org}/schedule/events/{id}")
        ));
        assert!(super::allowed_member_path(
            "DELETE",
            &format!("{org}/schedule/events/{id}")
        ));
        assert!(!super::allowed_member_path(
            "PATCH",
            &format!("{org}/schedule/events/{id}")
        ));
        assert!(!super::allowed_member_path(
            "DELETE",
            &format!("{org}/schedule/events/not-a-uuid")
        ));
        assert!(super::allowed_member_path(
            "GET",
            "/v1/api/org/mail/mailbox"
        ));
        assert!(super::allowed_member_path(
            "GET",
            "/v1/api/org/mail/colleagues"
        ));
        assert!(super::allowed_member_path(
            "POST",
            "/v1/api/org/mail/colleagues"
        ));
        assert!(!super::allowed_member_path(
            "DELETE",
            "/v1/api/org/mail/colleagues"
        ));
        assert!(super::allowed_member_path(
            "GET",
            "/v1/api/org/mail/messages"
        ));
        assert!(super::allowed_member_path(
            "POST",
            "/v1/api/org/mail/messages"
        ));
        assert!(super::allowed_member_path(
            "GET",
            &format!("/v1/api/org/mail/messages/{id}")
        ));
        assert!(super::allowed_member_path(
            "POST",
            &format!("/v1/api/org/mail/messages/{id}/state")
        ));
        assert!(!super::allowed_member_path(
            "DELETE",
            "/v1/api/org/mail/messages"
        ));
        assert!(!super::allowed_member_path(
            "POST",
            "/v1/api/org/mail/messages/not-a-uuid/state"
        ));
        assert!(!super::allowed_member_path("GET", "/v1/api/org/mail/other"));
        assert!(super::allowed_member_path(
            "POST",
            "/v1/api/org/tobot/memory/reflects"
        ));
        assert!(super::allowed_member_path(
            "POST",
            "/v1/api/org/tobot/memory/retains"
        ));
        assert!(!super::allowed_member_path(
            "GET",
            "/v1/api/org/tobot/memory/reflects"
        ));
        assert!(!super::allowed_member_path(
            "POST",
            "/v1/api/org/tobot/memory/recalls"
        ));
        assert!(!super::allowed_member_path(
            "POST",
            "/v1/api/org/tobot/memory"
        ));
        assert!(super::allowed_member_path("PUT", org));
        assert!(super::allowed_member_path("DELETE", org));
        assert!(!super::allowed_member_path(
            "DELETE",
            "/v1/api/org/members/orgs/a.b"
        ));
        assert!(super::allowed_member_path(
            "DELETE",
            &format!("{org}/membership")
        ));
        assert!(super::allowed_member_path(
            "GET",
            &format!("{org}/transfer-candidates")
        ));
        assert!(super::allowed_member_path(
            "POST",
            &format!("{org}/ownership-transfers")
        ));
        assert!(!super::allowed_member_path(
            "GET",
            &format!("{org}/activity")
        ));
        assert!(!super::allowed_member_path(
            "GET",
            &format!("{org}/membership")
        ));
        assert!(!super::allowed_member_path(
            "GET",
            &format!("{org}/transfer")
        ));
        assert!(!super::allowed_member_path(
            "DELETE",
            &format!("{org}/activity")
        ));
    }

    #[test]
    fn forwards_password_setup_and_join_accept_for_the_team_story() {
        assert!(!super::allowed_member_path(
            "POST",
            "/v1/api/org/members/register"
        ));
        assert!(!super::allowed_member_path(
            "GET",
            "/v1/api/org/members/verifications/reviews"
        ));
        assert!(!super::allowed_member_path(
            "PUT",
            "/v1/api/org/members/invites"
        ));
        let join = "/v1/api/org/members/join-codes/0123456789abcdef0123456789abcdef/memberships";
        let mut missing = Vec::new();
        if !super::allowed_member_path("PUT", "/v1/api/org/members/password") {
            missing.push("PUT /v1/api/org/members/password");
        }
        if !super::allowed_member_path("POST", join) {
            missing.push("POST /v1/api/org/members/join-codes/{code}/memberships");
        }
        assert!(
            missing.is_empty(),
            "step shell: the team story needs these paths forwarded: {}",
            missing.join(", ")
        );
    }

    #[test]
    fn allows_reading_the_published_model_catalog_only() {
        assert!(super::allowed_member_path("GET", "/v1/api/org/models"));
        assert!(!super::allowed_member_path("POST", "/v1/api/org/models"));
        assert!(!super::allowed_member_path("GET", "/v1/api/org/models/x"));
    }

    #[test]
    fn allows_the_capability_catalog_and_its_install_actions() {
        assert!(super::allowed_member_path(
            "GET",
            "/v1/api/org/capabilities"
        ));
        assert!(super::allowed_member_path(
            "GET",
            "/v1/api/org/capabilities/installations"
        ));
        assert!(!super::allowed_member_path(
            "POST",
            "/v1/api/org/capabilities/installations"
        ));
        assert!(super::allowed_member_path(
            "GET",
            "/v1/api/org/capabilities/notion/icon"
        ));
        assert!(super::allowed_member_path(
            "GET",
            "/v1/api/org/capabilities/a3s-review/cover"
        ));
        assert!(super::allowed_member_path(
            "GET",
            "/v1/api/org/capabilities/notion/bundle"
        ));
        assert!(!super::allowed_member_path(
            "POST",
            "/v1/api/org/capabilities"
        ));
        assert!(super::allowed_member_path(
            "PUT",
            "/v1/api/org/capabilities/installations/notion"
        ));
        assert!(super::allowed_member_path(
            "DELETE",
            "/v1/api/org/capabilities/installations/ppt-builder"
        ));
        assert!(!super::allowed_member_path(
            "GET",
            "/v1/api/org/capabilities/installations/notion"
        ));
        assert!(!super::allowed_member_path(
            "POST",
            "/v1/api/org/capabilities/installations/notion"
        ));
        assert!(!super::allowed_member_path(
            "PUT",
            "/v1/api/org/capabilities/installations/../notion"
        ));
        assert!(!super::allowed_member_path(
            "PUT",
            "/v1/api/org/capabilities/installations/Notion"
        ));
        assert!(!super::capability_slug_ok("-notion"));
        assert!(!super::capability_slug_ok(&"a".repeat(65)));
    }

    const ORG: &str = "0b9d2a4e-1c3f-4a5b-8d6e-7f8091a2b3c4";
    const ID: &str = "9f8e7d6c-5b4a-4392-8170-6f5e4d3c2b1a";

    #[test]
    fn allows_the_team_encyclopedia_routes() {
        let drive = |tail: &str| format!("/v1/api/org/drive/{ORG}/{tail}");
        for (method, tail) in [
            ("GET", "encyclopedia".to_owned()),
            ("GET", format!("encyclopedia/{ID}")),
            ("POST", "encyclopedia".to_owned()),
            ("POST", format!("encyclopedia/{ID}/publish")),
            ("POST", format!("encyclopedia/{ID}/return")),
            ("POST", format!("encyclopedia/{ID}/retry")),
            ("POST", format!("encyclopedia/{ID}/detach")),
            ("POST", "encyclopedia-questions".to_owned()),
            ("POST", "encyclopedia-passages".to_owned()),
            ("PUT", format!("nodes/{ID}/encyclopedia")),
            ("PUT", format!("files/{ID}/name")),
            ("PUT", format!("files/{ID}/parent")),
            ("DELETE", format!("files/{ID}")),
            ("GET", format!("files/{ID}/content")),
            ("PUT", format!("files/{ID}/content")),
            ("POST", "files".to_owned()),
        ] {
            assert!(
                super::allowed_member_path(method, &drive(&tail)),
                "{method} {tail}"
            );
        }
        for (method, path) in [
            ("DELETE", drive("encyclopedia")),
            ("POST", drive(&format!("encyclopedia/{ID}/delete"))),
            ("GET", drive("encyclopedia/not-a-uuid")),
            ("GET", "/v1/api/org/drive/personal/encyclopedia".to_owned()),
            ("GET", drive(&format!("nodes/{ID}/encyclopedia"))),
            ("PUT", drive("nodes/not-a-uuid/encyclopedia")),
            ("GET", drive("encyclopedia-passages")),
            ("POST", drive("encyclopedia/not-a-uuid/detach")),
        ] {
            assert!(
                !super::allowed_member_path(method, &path),
                "{method} {path}"
            );
        }
    }

    #[test]
    fn drive_file_bytes_cover_upload_download_and_replace() {
        assert!(super::is_drive_file_byte_route(
            "POST",
            &format!("/api/drive/{ORG}/files")
        ));
        assert!(super::is_drive_file_byte_route(
            "GET",
            &format!("/api/drive/{ORG}/files/{ID}/content")
        ));
        assert!(super::is_drive_file_byte_route(
            "PUT",
            &format!("/api/drive/{ORG}/files/{ID}/content")
        ));
        assert!(!super::is_drive_file_byte_route(
            "GET",
            &format!("/api/drive/{ORG}/nodes")
        ));
        assert!(!super::is_drive_file_byte_route(
            "PUT",
            &format!("/api/drive/{ORG}/files/{ID}/name")
        ));
        assert!(!super::is_drive_file_byte_route(
            "DELETE",
            &format!("/api/drive/org-1/files/{ID}")
        ));
    }

    #[test]
    fn allows_handbook_file_reads_only() {
        assert!(super::allowed_member_path("GET", "/v1/api/org/hub/files"));
        assert!(super::allowed_member_path(
            "GET",
            &format!("/v1/api/org/hub/files/{ID}/content")
        ));
        assert!(!super::allowed_member_path("POST", "/v1/api/org/hub/files"));
        assert!(!super::allowed_member_path(
            "DELETE",
            &format!("/v1/api/org/hub/files/{ID}")
        ));
        assert!(!super::allowed_member_path(
            "GET",
            "/v1/api/org/hub/files/not-a-uuid/content"
        ));
        assert!(!super::allowed_member_path(
            "GET",
            "/v1/api/org/hub/files/extra"
        ));
    }

    #[test]
    fn allows_exactly_the_member_im_routes() {
        let im = |tail: &str| format!("/v1/api/org/im/{ORG}/{tail}");
        for (method, tail) in [
            ("GET", "conversations".to_owned()),
            ("GET", "directory".to_owned()),
            ("GET", "departments".to_owned()),
            ("GET", "me".to_owned()),
            ("GET", format!("conversations/{ID}")),
            ("GET", format!("conversations/{ID}/messages")),
            ("GET", format!("attachments/{ID}")),
            ("POST", "direct-conversations".to_owned()),
            ("PUT", "file-transfer".to_owned()),
            ("POST", "group-conversations".to_owned()),
            ("PUT", format!("conversations/{ID}/read-cursor")),
            ("PUT", format!("conversations/{ID}/muted")),
            ("PUT", format!("conversations/{ID}/pinned")),
            ("PUT", format!("conversations/{ID}/unread-cursor")),
            ("PUT", format!("conversations/{ID}")),
            ("PUT", format!("conversations/{ID}/remark")),
            ("PUT", format!("conversations/{ID}/nickname")),
            ("PUT", format!("conversations/{ID}/speech")),
            ("PUT", format!("conversations/{ID}/owner")),
            ("PUT", format!("conversations/{ID}/members/{ID}/role")),
            ("PUT", format!("conversations/{ID}/members/{ID}/speech")),
            ("POST", format!("conversations/{ID}/typing")),
            ("PUT", "presence".to_owned()),
            ("GET", "presence".to_owned()),
            ("POST", format!("conversations/{ID}/messages")),
            ("DELETE", format!("conversations/{ID}/membership")),
            ("DELETE", format!("conversations/{ID}")),
            ("DELETE", format!("messages/{ID}")),
            ("POST", "message-syncs".to_owned()),
            ("POST", "stream-tickets".to_owned()),
            ("POST", "attachments".to_owned()),
            ("DELETE", format!("conversations/{ID}/members/{ID}")),
        ] {
            assert!(
                super::allowed_member_path(method, &im(&tail)),
                "{method} {tail}"
            );
        }
        for (method, path) in [
            ("GET", im("stream")),
            ("POST", im(&format!("conversations/{ID}/archive"))),
            (
                "PUT",
                im(&format!("conversations/{ID}/members/not-a-uuid/role")),
            ),
            ("PUT", im(&format!("conversations/not-a-uuid/nickname"))),
            ("GET", im("conversations/not-a-uuid")),
            ("GET", "/v1/api/org/im/personal/conversations".to_owned()),
            ("GET", format!("/v1/api/admin/im/{ORG}/service-identities")),
        ] {
            assert!(
                !super::allowed_member_path(method, &path),
                "{method} {path}"
            );
        }
    }

    #[test]
    fn im_reads_keep_their_query_upstream() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let mut seen = Vec::new();
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                seen.push(read_request(&mut stream));
                write_json(&mut stream, br#"{"code":0,"data":[]}"#);
            }
            seen
        });
        let origin = format!("http://{address}");
        for path in [
            format!("/v1/api/org/im/{ORG}/conversations?limit=100"),
            format!("/v1/api/org/im/{ORG}/conversations/{ID}/messages?before_sequence=51&limit=50"),
        ] {
            let response = super::forward_member_api(&origin, "GET", &path, b"", "a3s_ms_token");
            assert_eq!(response.0, 200, "{path}");
        }
        let seen = server.join().unwrap();
        assert!(
            seen[0].starts_with(&format!(
                "GET /v1/api/org/im/{ORG}/conversations?limit=100 "
            )),
            "{}",
            seen[0]
        );
        assert!(
            seen[1].contains("messages?before_sequence=51&limit=50 "),
            "{}",
            seen[1]
        );
        let refused = super::forward_member_api(
            &origin,
            "GET",
            &format!("/v1/api/org/im/{ORG}/stream?ticket=x"),
            b"",
            "a3s_ms_token",
        );
        assert_eq!(refused.0, 404);
    }

    #[test]
    fn im_attachments_travel_as_raw_bytes_both_ways() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let mut seen = Vec::new();
            for index in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                seen.push(read_request(&mut stream));
                if index == 0 {
                    write_json(&mut stream, br#"{"code":0,"data":{"object_key":"k"}}"#);
                } else {
                    let bytes = [0x89_u8, b'P', b'N', b'G', 0, 1, 2];
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        bytes.len()
                    );
                    stream.write_all(head.as_bytes()).unwrap();
                    stream.write_all(&bytes).unwrap();
                }
            }
            seen
        });
        let origin = format!("http://{address}");
        let upload = super::forward_member_binary(
            &origin,
            "POST",
            &format!("/v1/api/org/im/{ORG}/attachments?name=a.png&mime=image%2Fpng"),
            &[1, 2, 3],
            "a3s_ms_token",
        );
        assert_eq!(upload.0, 200);
        assert_eq!(upload.2, "application/json");
        let download = super::forward_member_binary(
            &origin,
            "GET",
            &format!("/v1/api/org/im/{ORG}/attachments/{ID}"),
            b"",
            "a3s_ms_token",
        );
        assert_eq!(download.0, 200);
        assert_eq!(download.2, "application/octet-stream");
        assert_eq!(download.3, vec![0x89, b'P', b'N', b'G', 0, 1, 2]);
        let seen = server.join().unwrap();
        assert!(
            seen[0].contains("Content-Type: application/octet-stream"),
            "{}",
            seen[0]
        );
        assert!(
            seen[0].contains("Authorization: Bearer a3s_ms_token"),
            "{}",
            seen[0]
        );
        assert!(seen[0].ends_with("\u{1}\u{2}\u{3}"), "{:?}", seen[0]);
        let refused = super::forward_member_binary(
            &origin,
            "POST",
            &format!("/v1/api/org/im/{ORG}/message-syncs"),
            b"{}",
            "a3s_ms_token",
        );
        assert_eq!(refused.0, 404);
    }

    #[test]
    fn bundle_download_reports_increasing_bytes_against_the_known_total() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let body = vec![b'a'; 20_000];
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let _ = read_request(&mut stream);
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/zip\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(head.as_bytes()).unwrap();
            stream.write_all(&body).unwrap();
        });
        let mut points = Vec::new();
        let bytes = super::fetch_capability_bundle_with_progress(
            &format!("http://{address}"),
            "pdf",
            "a3s_ms_token",
            Some(20_000),
            |received, total| points.push((received, total)),
        )
        .unwrap();
        server.join().unwrap();
        assert_eq!(bytes.len(), 20_000);
        assert!(points.len() >= 2, "{points:?}");
        assert!(
            points.iter().all(|(_, total)| *total == 20_000),
            "{points:?}"
        );
        assert!(
            points.windows(2).all(|pair| pair[0].0 < pair[1].0),
            "{points:?}"
        );
        assert!(
            points.iter().any(|(received, total)| received < total),
            "{points:?}"
        );
    }
}
