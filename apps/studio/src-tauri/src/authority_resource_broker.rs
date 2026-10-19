//! Host-issued project leases for opaque-origin AIX views. No guest chooses an API path.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    sync::Mutex,
    time::{Duration, Instant},
};
use uuid::Uuid;

use super::{Shared, query_value};
use crate::capabilities::ViewResourceIdentity;

#[path = "authority_resource_drafts.rs"]
mod drafts;

pub(crate) const MAX_BODY_BYTES: usize = (100 * 1024 * 1024usize).div_ceil(3) * 4 + 16_384;
const MAX_ACL_BYTES: usize = 16 * 1024 * 1024;
const MAX_LEASES: usize = 256;
const MAX_REVISION: u64 = 9_007_199_254_740_991;
type Response = (u16, &'static str, &'static str, Vec<u8>);

#[derive(Clone, Copy, PartialEq, Eq)]
enum ResourceAccess {
    Read,
    ReadWrite,
}
impl ResourceAccess {
    fn parse(value: &str) -> io::Result<Self> {
        match value {
            "read" => Ok(Self::Read),
            "read-write" => Ok(Self::ReadWrite),
            _ => Err(invalid("resource access requires read or read-write")),
        }
    }
    fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::ReadWrite => "read-write",
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LeaseRequest {
    protocol_version: u32,
    workspace_id: Uuid,
    project_id: Uuid,
    resources: Vec<String>,
    access: String,
}

#[derive(Clone)]
struct Lease {
    view: String,
    identity: ViewResourceIdentity,
    organization: Uuid,
    workspace: Uuid,
    project: Uuid,
    resources: BTreeSet<String>,
    access: ResourceAccess,
    expires: Instant,
}

pub(crate) struct ResourceBroker {
    leases: Mutex<BTreeMap<String, Lease>>,
    drafts: drafts::DraftStore,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResourceRequest {
    protocol_version: u32,
    lease: String,
    request: DocumentOperation,
}

#[derive(Deserialize)]
#[serde(
    tag = "operation",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum DocumentOperation {
    ReadReceipt {
        operation_id: Uuid,
    },
    Read {
        key: String,
        revision: Option<u64>,
    },
    Save {
        key: String,
        operation_id: Uuid,
        expected_revision: Option<u64>,
        content: String,
        assets: Vec<String>,
    },
    ReadAsset {
        digest: String,
    },
    StoreAsset {
        digest: String,
        kind: String,
        bytes_base64: String,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateProjectRequest {
    workspace_id: Uuid,
    id: Uuid,
    title: String,
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
fn missing() -> io::Error {
    io::ErrorKind::NotFound.into()
}
fn key_ok(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !matches!(value, "." | "..")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
}
fn digest_ok(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}
fn uuid(value: &str) -> bool {
    Uuid::parse_str(value).is_ok_and(|id| !id.is_nil() && id.to_string() == value)
}

fn project_page(query: &str, allowed: &[&str]) -> io::Result<(u32, u32)> {
    let mut values = BTreeMap::new();
    for pair in query.split('&').filter(|_| !query.is_empty()) {
        let (key, value) = pair
            .split_once('=')
            .ok_or_else(|| invalid("project query"))?;
        if !allowed.contains(&key) || values.insert(key, value).is_some() {
            return Err(invalid("unsupported or repeated project query"));
        }
    }
    let number = |key, default| -> io::Result<u32> {
        values.get(key).map_or(Ok(default), |value| {
            if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(invalid("project page number"));
            }
            value.parse().map_err(|_| invalid("project page number"))
        })
    };
    let page = number("page", 1)?;
    let limit = number("limit", 20)?;
    if page == 0 || limit == 0 || limit > 100 || u64::from(page - 1) * u64::from(limit) > 100_000 {
        return Err(invalid("project page bounds"));
    }
    Ok((page, limit))
}

pub(crate) fn allows_upstream(method: &str, path: &str) -> bool {
    let (path, query) = path.split_once('?').unwrap_or((path, ""));
    let parts: Vec<_> = path.split('/').collect();
    let [
        "",
        "v1",
        "api",
        "org",
        "organizations",
        organization,
        "workspaces",
        workspace,
        "documents",
        "projects",
        tail @ ..,
    ] = parts.as_slice()
    else {
        return false;
    };
    if !uuid(organization) || !uuid(workspace) {
        return false;
    }
    match (method, tail) {
        ("GET", []) => project_page(query, &["page", "limit"]).is_ok(),
        ("POST", []) => query.is_empty(),
        ("GET", [project]) => uuid(project) && query.is_empty(),
        ("GET", [project, "operations", operation]) => {
            uuid(project) && uuid(operation) && query.is_empty()
        }
        ("GET", [project, "resources", key]) => {
            uuid(project)
                && key_ok(key)
                && (query.is_empty()
                    || query.strip_prefix("revision=").is_some_and(|value| {
                        value
                            .parse::<u64>()
                            .is_ok_and(|revision| revision <= MAX_REVISION)
                    }))
        }
        ("PUT", [project, "resources", key]) => uuid(project) && key_ok(key) && query.is_empty(),
        ("GET" | "PUT", [project, "assets", digest]) => {
            uuid(project) && digest_ok(digest) && query.is_empty()
        }
        _ => false,
    }
}

pub(crate) fn is_request(method: &str, path: &str) -> bool {
    matches!(
        (method, path.split('/').collect::<Vec<_>>().as_slice()),
        (
            "POST",
            ["", "api", "aix", "views", "sessions", _, "resources"]
        ) | (
            "PUT",
            [
                "",
                "api",
                "aix",
                "views",
                "sessions",
                _,
                "resources",
                "drafts",
                _
            ]
        )
    )
}

fn project_path(organization: Uuid, workspace: Uuid) -> String {
    format!("/v1/api/org/organizations/{organization}/workspaces/{workspace}/documents/projects")
}
fn with_current<T>(
    shared: &Shared,
    action: impl FnOnce(&str, Uuid, &str, &str) -> io::Result<T>,
) -> io::Result<T> {
    shared
        .member_session
        .with_resource_session(|bearer, member| {
            let centers = shared
                .org_centers
                .lock()
                .map_err(|_| io::Error::other("organization scope unavailable"))?;
            let organization = centers
                .by_member
                .get(member)
                .and_then(|value| Uuid::parse_str(value).ok())
                .filter(|id| !id.is_nil())
                .ok_or_else(missing)?;
            let owner = format!("{bearer}\0{organization}");
            action(&owner, organization, bearer, member)
        })
}
fn exchange(
    shared: &Shared,
    method: &str,
    path: &str,
    body: &[u8],
    bearer: &str,
) -> io::Result<Response> {
    let (status, payload) = crate::org_login::exchange_document_resource(
        &shared.org_origin,
        method,
        path,
        body,
        bearer,
    )?;
    Ok((status, reason(status), "application/json", payload))
}
fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        409 => "Conflict",
        413 => "Payload Too Large",
        429 => "Too Many Requests",
        502 => "Bad Gateway",
        _ => "Resource Request Failed",
    }
}

fn projects(
    shared: &Shared,
    method: &str,
    query: &str,
    body: &[u8],
    organization: Uuid,
    bearer: &str,
) -> io::Result<Response> {
    if method == "GET" {
        let (page, limit) = project_page(query, &["workspace", "page", "limit"])?;
        let workspace = query_value(query, "workspace")
            .filter(|value| uuid(value))
            .and_then(|value| Uuid::parse_str(value).ok())
            .filter(|id| !id.is_nil())
            .ok_or_else(|| invalid("workspace identity"))?;
        return exchange(
            shared,
            "GET",
            &format!(
                "{}?page={page}&limit={limit}",
                project_path(organization, workspace)
            ),
            &[],
            bearer,
        );
    }
    if !query.is_empty() {
        return Err(invalid("project creation query"));
    }
    let input: CreateProjectRequest =
        serde_json::from_slice(body).map_err(|_| invalid("project request"))?;
    if input.workspace_id.is_nil()
        || input.id.is_nil()
        || input.title.trim().is_empty()
        || input.title.chars().count() > 128
    {
        return Err(invalid("project contract"));
    }
    let body = json!({"id":input.id,"title":input.title}).to_string();
    exchange(
        shared,
        "POST",
        &project_path(organization, input.workspace_id),
        body.as_bytes(),
        bearer,
    )
}
fn error_response(error: io::Error) -> Response {
    let status = match error.kind() {
        io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied => 404,
        io::ErrorKind::InvalidInput => 400,
        io::ErrorKind::InvalidData => 502,
        io::ErrorKind::WouldBlock => 429,
        io::ErrorKind::AlreadyExists => 409,
        _ => 503,
    };
    (
        status,
        reason(status),
        "application/json",
        json!({"error":if status == 404 { "not_found" } else { "resource_refused" }})
            .to_string()
            .into_bytes(),
    )
}

impl ResourceBroker {
    pub fn new(root: Option<std::path::PathBuf>) -> Self {
        Self {
            leases: Mutex::new(BTreeMap::new()),
            drafts: drafts::DraftStore::new(root),
        }
    }

    fn draft(
        &self,
        shared: &Shared,
        view: &str,
        key: &str,
        method: &str,
        body: &[u8],
    ) -> io::Result<Response> {
        if !key_ok(key) {
            return Err(invalid("draft key"));
        }
        with_current(shared, |owner, organization, _, member| {
            shared
                .capabilities
                .with_plugin_view_resource(owner, view, |identity| {
                    if !identity.can_read || (method == "PUT" && !identity.can_write) {
                        return Err(missing());
                    }
                    let scope = drafts::DraftStore::scope(
                        member,
                        organization,
                        &identity.package_id,
                        &identity.surface,
                    );
                    let snapshot = if method == "GET" {
                        self.drafts.read(&scope, key)?
                    } else {
                        let input =
                            serde_json::from_slice(body).map_err(|_| invalid("draft commit"))?;
                        self.drafts.commit(&scope, key, input)?
                    };
                    Ok((
                        200,
                        "OK",
                        "application/json",
                        serde_json::to_vec(&snapshot)
                            .map_err(|_| io::Error::other("draft response"))?,
                    ))
                })
        })
    }

    fn grant(&self, shared: &Shared, view: &str, bytes: &[u8]) -> io::Result<Response> {
        let input: LeaseRequest =
            serde_json::from_slice(bytes).map_err(|_| invalid("resource lease request"))?;
        let access = ResourceAccess::parse(&input.access)?;
        let resources: BTreeSet<_> = input.resources.iter().cloned().collect();
        if input.protocol_version != 1
            || input.workspace_id.is_nil()
            || input.project_id.is_nil()
            || resources.is_empty()
            || resources.len() > 16
            || resources.len() != input.resources.len()
            || !resources.iter().all(|key| key_ok(key))
        {
            return Err(invalid("resource lease contract"));
        }
        with_current(shared, |owner, organization, bearer, _| {
            shared.capabilities.with_plugin_view_resource(owner, view, |identity| {
            if !identity.can_read || (access == ResourceAccess::ReadWrite && !identity.can_write) { return Err(missing()); }
            let path = format!("{}/{}", project_path(organization, input.workspace_id), input.project_id);
            let response = exchange(shared, "GET", &path, &[], &bearer)?;
            if response.0 != 200 { return Ok(response); }
            let project: Value = serde_json::from_slice(&response.3).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "project response"))?;
            if project["data"]["id"].as_str() != Some(input.project_id.to_string().as_str()) { return Err(missing()); }
            let mut leases = self.leases.lock().map_err(|_| io::Error::other("resource lease lock failed"))?;
            leases.retain(|_, lease| lease.expires > Instant::now() + Duration::from_secs(60));
            if leases.len() >= MAX_LEASES { return Err(io::ErrorKind::WouldBlock.into()); }
            let token = Uuid::now_v7().to_string();
            leases.insert(token.clone(), Lease { view: view.into(), identity: identity.clone(), organization, workspace: input.workspace_id, project: input.project_id, resources, access, expires: Instant::now() + Duration::from_secs(15 * 60) });
            Ok((200, "OK", "application/json", json!({"protocolVersion":1,"lease":token,"workspaceId":input.workspace_id,"projectId":input.project_id,"resources":input.resources,"access":access.as_str(),"identity":identity}).to_string().into_bytes()))
        })
        })
    }

    fn request(&self, shared: &Shared, view: &str, bytes: &[u8]) -> io::Result<Response> {
        let input: ResourceRequest =
            serde_json::from_slice(bytes).map_err(|_| invalid("resource request"))?;
        if input.protocol_version != 1 {
            return Err(invalid("resource protocol version"));
        }
        with_current(shared, |owner, organization, bearer, _| {
            shared.capabilities.with_plugin_view_resource(owner, view, |identity| {
            let mut leases = self.leases.lock().map_err(|_| io::Error::other("resource lease lock failed"))?;
            leases.retain(|_, lease| lease.expires > Instant::now() + Duration::from_secs(60));
            let lease = leases.get(&input.lease).ok_or_else(missing)?;
            if lease.view != view || &lease.identity != identity || lease.organization != organization { return Err(missing()); }
            let base = format!("{}/{}", project_path(organization, lease.workspace), lease.project);
            match input.request {
                DocumentOperation::ReadReceipt { operation_id } => {
                    if operation_id.is_nil() { return Err(invalid("operation identity")); }
                    let response = exchange(shared, "GET", &format!("{base}/operations/{operation_id}"), &[], &bearer)?;
                    if response.0 == 200 {
                        let receipt: Value = serde_json::from_slice(&response.3).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "operation receipt"))?;
                        if !receipt["data"]["document"]["key"].as_str().is_some_and(|key| lease.resources.contains(key)) { return Err(missing()); }
                    }
                    Ok(response)
                }
                DocumentOperation::Read { key, revision } => {
                    if !lease.resources.contains(&key) { return Err(missing()); }
                    if revision.is_some_and(|revision| revision > MAX_REVISION) { return Err(invalid("resource revision")); }
                    let query = revision.map(|revision| format!("?revision={revision}")).unwrap_or_default();
                    exchange(shared, "GET", &format!("{base}/resources/{key}{query}"), &[], &bearer)
                }
                DocumentOperation::Save { key, operation_id, expected_revision, content, assets } => {
                    if lease.access != ResourceAccess::ReadWrite || !lease.resources.contains(&key) { return Err(missing()); }
                    if operation_id.is_nil() || expected_revision.is_some_and(|revision| revision >= MAX_REVISION) || content.len() > MAX_ACL_BYTES || assets.len() > 64 || !assets.iter().all(|digest| digest_ok(digest)) { return Err(invalid("resource save contract")); }
                    let body = json!({"operationId":operation_id,"expectedRevision":expected_revision,"content":content,"assets":assets}).to_string();
                    exchange(shared, "PUT", &format!("{base}/resources/{key}"), body.as_bytes(), &bearer)
                }
                DocumentOperation::ReadAsset { digest } => {
                    if !digest_ok(&digest) { return Err(invalid("asset digest")); }
                    let (status, bytes) = crate::org_login::exchange_document_resource(&shared.org_origin, "GET", &format!("{base}/assets/{digest}"), &[], &bearer)?;
                    if status == 200 && crate::capabilities::digest_of(&bytes) != digest { return Err(io::Error::new(io::ErrorKind::InvalidData, "asset integrity refused")); }
                    let body = if status == 200 { json!({"digest":digest,"bytesBase64":STANDARD.encode(bytes)}).to_string().into_bytes() } else { bytes };
                    Ok((status, reason(status), "application/json", body))
                }
                DocumentOperation::StoreAsset { digest, kind, bytes_base64 } => {
                    if lease.access != ResourceAccess::ReadWrite { return Err(missing()); }
                    if !digest_ok(&digest) || !matches!(kind.as_str(), "glb" | "recording" | "artifact") || bytes_base64.len() > MAX_BODY_BYTES - 16_384 { return Err(invalid("asset upload contract")); }
                    let body = json!({"kind":kind,"bytesBase64":bytes_base64}).to_string();
                    exchange(shared, "PUT", &format!("{base}/assets/{digest}"), body.as_bytes(), &bearer)
                }
            }
        })
        })
    }

    fn revoke(&self, view: &str, token: &str) -> io::Result<Response> {
        let mut leases = self
            .leases
            .lock()
            .map_err(|_| io::Error::other("resource lease lock failed"))?;
        if !leases.get(token).is_some_and(|lease| lease.view == view) {
            return Err(missing());
        }
        leases.remove(token);
        Ok((
            200,
            "OK",
            "application/json",
            br#"{"revoked":true}"#.to_vec(),
        ))
    }
}

pub(super) fn route(
    method: &str,
    parts: &[&str],
    query: &str,
    body: &[u8],
    shared: &Shared,
) -> Option<Response> {
    let result = match (method, parts) {
        (
            "GET" | "PUT",
            [
                "api",
                "aix",
                "views",
                "sessions",
                view,
                "resources",
                "drafts",
                key,
            ],
        ) => {
            if !query.is_empty() {
                return Some(error_response(invalid("draft query")));
            }
            shared
                .resource_broker
                .draft(shared, view, key, method, body)
        }
        (
            "GET",
            [
                "api",
                "aix",
                "views",
                "sessions",
                view,
                "resources",
                "access",
            ],
        ) => with_current(shared, |owner, _, _, _| {
            shared.capabilities.with_plugin_view_resource(owner, view, |identity| {
                    if !query.is_empty() { return Err(invalid("resource access query")); }
                    if !identity.can_read { return Err(missing()); }
                    Ok((200, "OK", "application/json", json!({"protocolVersion":1,"canRead":identity.can_read,"canWrite":identity.can_write}).to_string().into_bytes()))
                })
        }),
        ("GET" | "POST", ["api", "aix", "views", "sessions", view, "projects"]) => {
            with_current(shared, |owner, organization, bearer, _| {
                shared
                    .capabilities
                    .with_plugin_view_resource(owner, view, |identity| {
                        if !identity.can_read || (method == "POST" && !identity.can_write) {
                            return Err(missing());
                        }
                        projects(shared, method, query, body, organization, bearer)
                    })
            })
        }
        (
            "POST",
            [
                "api",
                "aix",
                "views",
                "sessions",
                view,
                "resources",
                "leases",
            ],
        ) => shared.resource_broker.grant(shared, view, body),
        ("POST", ["api", "aix", "views", "sessions", view, "resources"]) => {
            shared.resource_broker.request(shared, view, body)
        }
        (
            "DELETE",
            [
                "api",
                "aix",
                "views",
                "sessions",
                view,
                "resources",
                "leases",
                lease,
            ],
        ) => with_current(shared, |owner, _, _, _| {
            shared
                .capabilities
                .with_plugin_view_resource(owner, view, |_| {
                    shared.resource_broker.revoke(view, lease)
                })
        }),
        ("GET" | "POST", ["api", "document-projects"]) => {
            with_current(shared, |_, organization, bearer, _| {
                projects(shared, method, query, body, organization, bearer)
            })
        }
        _ => return None,
    };
    Some(result.unwrap_or_else(error_response))
}

#[cfg(test)]
#[path = "authority_resource_broker_tests.rs"]
mod tests;
