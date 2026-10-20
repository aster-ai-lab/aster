//! In-process loopback authority for the Studio window.
//! The listener and the Tauri process share one lifetime.

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::json;

use crate::tenant::{self, TenantBrand};

const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_BODY_BYTES: usize = 1024 * 1024;
const COMPOSER_FILE_LIMIT: usize = 20 * 1024 * 1024;
const STREAM_WAIT: Duration = Duration::from_secs(3);
const INDEX_HTML: &str = include_str!("../../src/renderer/shell/index.html");
const CLIENT_JS: &str = include_str!("../../src/renderer/shell/studioClient.mjs");

fn shell_client_source(packaged: bool) -> String {
    if !packaged {
        return CLIENT_JS.to_owned();
    }
    CLIENT_JS.replace(
        "http://127.0.0.1:4318/windows/main/index.html",
        crate::main_renderer_url(true),
    )
}

#[cfg(test)]
mod shell_client {
    use super::shell_client_source;

    #[test]
    fn packaged_client_points_at_the_bundle() {
        let dev = shell_client_source(false);
        assert!(dev.contains("http://127.0.0.1:4318/windows/main/index.html"));
        let packaged = shell_client_source(true);
        assert!(packaged.contains("tauri://localhost/windows/main/index.html"));
        assert!(!packaged.contains("127.0.0.1:4318"));
        assert_eq!(
            shell_client_source(crate::packaged_shell()).contains("127.0.0.1:4318"),
            !crate::packaged_shell()
        );
    }
}

const WECHAT_QR: &[u8] = include_bytes!("../../src/renderer/shell/wechat-qr.png");
const LOGIN_BG: &[u8] = include_bytes!("../../src/renderer/shell/login-bg.jpg");

struct Conversation {
    title: String,
    cancelled: bool,
    stream_id: Option<String>,
}

struct State {
    session_id: String,
    conversations: HashMap<String, Conversation>,
}

#[path = "authority_resource_broker.rs"]
pub(crate) mod resource_broker;

struct Shared {
    shutdown: Arc<AtomicBool>,
    token: String,
    tenant: String,
    name: String,
    product: String,
    logo_png: Vec<u8>,
    mark_png: Vec<u8>,
    icon_png: Vec<u8>,
    state: Mutex<State>,
    providers: Mutex<crate::providers::ProviderStore>,
    workspace_snapshots: Arc<crate::workspace_snapshot::WorkspaceSnapshots>,
    workspace_documents: Arc<crate::workspace_document::WorkspaceDocuments>,
    resource_broker: resource_broker::ResourceBroker,
    member_session: Arc<crate::member_session::MemberSessionStore>,
    applet_products: crate::applet_product::AppletProductStore,
    applet_access: Arc<crate::applet_runtime::AppletAccess>,
    capabilities: crate::capabilities::LocalCapabilities,
    /// Warm edge boxes AIX views call. `None` when its runtime failed to start.
    aix_agents: Option<crate::agent_plane::aix_episodes::AixEpisodes>,
    org_origin: String,
    /// Unified gateway (`/api` → a3s-api). Capability catalog, install, and icons go here.
    gateway_origin: String,
    /// Bound `http://127.0.0.1:<port>`. Only this origin, or a missing Origin, may read the member token.
    shell_origin: String,
    /// Loopback base URL of the agent plane; `None` when it failed to start.
    agent_plane_url: Option<String>,
    /// Selected team for each signed-in member. Applet windows cannot read renderer storage.
    org_centers: Mutex<OrgCenters>,
    wake: Condvar,
}

/// `by_member` stays after logout so the same person returns to their team.
/// `pending` is only used before `/me` has stored a member id.
#[derive(Default)]
struct OrgCenters {
    by_member: HashMap<String, String>,
    pending: String,
}

/// Each tenant's primary Studio keeps its own loopback port pair across launches,
/// so primary windows of different tenants run side by side.
const PRIMARY_PORT_BASE: u16 = 59850;
const PRIMARY_PORT_PAIRS: u32 = 64;

fn primary_ports(slug: &str) -> (u16, u16) {
    let hash = slug.bytes().fold(0x811c_9dc5_u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    });
    let authority = PRIMARY_PORT_BASE + 2 * (hash % PRIMARY_PORT_PAIRS) as u16;
    (authority, authority + 1)
}

fn authority_listen_port(slug: &str, instance: Option<&str>) -> u16 {
    if instance.is_some() {
        0
    } else {
        primary_ports(slug).0
    }
}

fn agent_plane_listen_port(slug: &str, instance: Option<&str>) -> u16 {
    if instance.is_some() {
        0
    } else {
        primary_ports(slug).1
    }
}

/// A busy preferred port costs only URL stability, so this launch takes a system port.
pub(crate) fn bind_loopback_preferring(port: u16, label: &str) -> io::Result<TcpListener> {
    match TcpListener::bind((Ipv4Addr::LOCALHOST, port)) {
        Err(error) if port != 0 && error.kind() == io::ErrorKind::AddrInUse => {
            eprintln!(
                "studio: {label} 127.0.0.1:{port} is in use; this launch uses a system-assigned port"
            );
            TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        }
        Err(error) if port != 0 => Err(io::Error::new(
            error.kind(),
            format!("{label} 127.0.0.1:{port}: {error}"),
        )),
        other => other,
    }
}

fn bind_loopback(port: u16) -> io::Result<TcpListener> {
    let listener = bind_loopback_preferring(port, "studio listener")?;
    let address = listener.local_addr()?;
    if address.ip() != IpAddr::V4(Ipv4Addr::LOCALHOST) {
        return Err(io::Error::new(
            io::ErrorKind::AddrNotAvailable,
            "studio authority must bind 127.0.0.1",
        ));
    }
    Ok(listener)
}

pub struct StudioAuthority {
    address: SocketAddr,
    token: String,
    slug: String,
    name: String,
    product: String,
    logo_path: PathBuf,
    theme_light: (u8, u8, u8),
    shared: Arc<Shared>,
    shutdown: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl StudioAuthority {
    pub fn start() -> io::Result<Self> {
        Self::open(
            tenant::load()?,
            crate::org_login::default_origin(),
            crate::providers::gateway_origin(),
            None,
        )
    }

    #[cfg(test)]
    pub fn start_with_org_origin(origin: &str) -> io::Result<Self> {
        Self::open(tenant::load()?, origin.to_owned(), origin.to_owned(), None)
    }

    /// An authority whose installed packages live under `capabilities`, so a
    /// test can install through the catalog and open real views.
    #[cfg(test)]
    pub fn start_with_store(origin: &str, capabilities: PathBuf) -> io::Result<Self> {
        Self::open(
            tenant::load()?,
            origin.to_owned(),
            origin.to_owned(),
            Some(capabilities),
        )
    }

    fn open(
        brand: TenantBrand,
        org_origin: String,
        gateway_origin: String,
        capabilities: Option<PathBuf>,
    ) -> io::Result<Self> {
        let instance = crate::tenant::instance_name()?;
        let listener = bind_loopback(if cfg!(test) {
            0
        } else {
            authority_listen_port(&brand.slug, instance.as_deref())
        })?;
        let address = listener.local_addr()?;
        let token = hex_encode(&random_bytes::<32>()?);
        let session_id = hex_encode(&random_bytes::<16>()?);
        let shutdown = Arc::new(AtomicBool::new(false));
        let slug = brand.slug.clone();
        let storage = crate::tenant::storage_slug(&slug)?;
        let name = brand.name.clone();
        let product = crate::tenant::window_product(&brand.product, instance.as_deref());
        let logo_path = brand.logo_path.clone();
        let theme_light = brand.theme_light;
        let shell_origin = format!("http://{address}");
        let workspace_snapshots =
            Arc::new(crate::workspace_snapshot::WorkspaceSnapshots::default());
        let workspace_documents = Arc::new(crate::workspace_document::WorkspaceDocuments::new(
            Arc::clone(&workspace_snapshots),
        ));
        let agent_plane_url = match crate::agent_plane::start(
            &storage,
            &token,
            Arc::clone(&workspace_documents),
            if cfg!(test) {
                0
            } else {
                agent_plane_listen_port(&slug, instance.as_deref())
            },
        ) {
            Ok(url) => Some(url),
            Err(err) => {
                eprintln!("studio: agent plane did not start: {err}");
                None
            }
        };
        let local_capabilities = match capabilities {
            Some(root) => crate::capabilities::LocalCapabilities::open_at(Some(root)),
            None => crate::capabilities::LocalCapabilities::open(&storage),
        };
        let resource_broker = resource_broker::ResourceBroker::new(local_capabilities.drafts_dir());
        let shared = Arc::new(Shared {
            shutdown: Arc::clone(&shutdown),
            token: token.clone(),
            tenant: slug.clone(),
            name: name.clone(),
            product: product.clone(),
            logo_png: brand.logo_png,
            mark_png: brand.mark_png,
            icon_png: brand.icon_png,
            state: Mutex::new(State {
                session_id,
                conversations: HashMap::new(),
            }),
            providers: Mutex::new(crate::providers::ProviderStore::open(&storage)),
            workspace_documents,
            resource_broker,
            workspace_snapshots,
            member_session: Arc::new(crate::member_session::MemberSessionStore::open(&storage)),
            applet_products: crate::applet_product::AppletProductStore::open(&storage),
            applet_access: Arc::new(crate::applet_runtime::AppletAccess::open(&storage)),
            capabilities: local_capabilities,
            aix_agents: crate::agent_plane::aix_episodes::AixEpisodes::new()
                .map_err(|err| eprintln!("studio: AIX view agents are disabled: {err}"))
                .ok(),
            org_origin,
            gateway_origin,
            shell_origin,
            agent_plane_url,
            org_centers: Mutex::new(OrgCenters::default()),
            wake: Condvar::new(),
        });
        let startup_shared = Arc::clone(&shared);
        thread::Builder::new()
            .name("studio-capabilities".to_owned())
            .spawn(move || {
                if let Err(err) = sync_capabilities(&startup_shared) {
                    eprintln!("studio: capabilities were not applied on startup: {err}");
                }
            })?;
        #[cfg(not(test))]
        crate::desktop_settings::boot(&storage, &product, &logo_path);
        #[cfg(not(test))]
        if let Some(url) = shared.agent_plane_url.clone() {
            crate::channel_gateway::boot(&token, &url);
        }
        let reaped = Arc::downgrade(&shared);
        thread::Builder::new()
            .name("studio-aix-agents".to_owned())
            .spawn(move || reap_aix_agents(reaped))?;
        let thread_shared = Arc::clone(&shared);
        let thread_shutdown = Arc::clone(&shutdown);
        let thread = thread::Builder::new()
            .name("studio-authority".to_owned())
            .spawn(move || accept_loop(listener, thread_shared, thread_shutdown))?;
        Ok(Self {
            address,
            token,
            slug,
            name,
            product,
            logo_path,
            theme_light,
            shared,
            shutdown,
            thread: Some(thread),
        })
    }

    pub fn shutdown(&mut self) {
        if self.shutdown.swap(true, Ordering::SeqCst) {
            return;
        }
        self.shared.wake.notify_all();
        // The accept loop blocks in accept; a connection wakes it to observe shutdown.
        let _ = TcpStream::connect_timeout(&self.address, Duration::from_secs(1));
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
        if let Some(agents) = &self.shared.aix_agents {
            agents.shutdown();
        }
    }

    #[cfg(test)]
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn api_base_url(&self) -> String {
        format!("http://{}", self.address)
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn applet_access(&self) -> Arc<crate::applet_runtime::AppletAccess> {
        Arc::clone(&self.shared.applet_access)
    }

    pub(crate) fn preference_member_session(
        &self,
    ) -> Arc<crate::member_session::MemberSessionStore> {
        Arc::clone(&self.shared.member_session)
    }

    #[cfg(test)]
    pub fn mint_applet_ticket(&self, app_id: &str, label: &str) -> Result<String, String> {
        self.shared.applet_access.mint(app_id, label, &self.token)
    }

    pub fn slug(&self) -> &str {
        &self.slug
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn product(&self) -> &str {
        &self.product
    }

    pub fn logo_path(&self) -> &std::path::Path {
        &self.logo_path
    }

    pub fn theme_light(&self) -> (u8, u8, u8) {
        self.theme_light
    }

    #[cfg(test)]
    pub fn session_id(&self) -> String {
        self.shared
            .state
            .lock()
            .expect("studio state")
            .session_id
            .clone()
    }
}

impl Drop for StudioAuthority {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn accept_loop(listener: TcpListener, shared: Arc<Shared>, shutdown: Arc<AtomicBool>) {
    while !shutdown.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, peer)) => {
                if shutdown.load(Ordering::SeqCst) {
                    break;
                }
                if peer.ip() != IpAddr::V4(Ipv4Addr::LOCALHOST) {
                    continue;
                }
                let shared = Arc::clone(&shared);
                thread::spawn(move || {
                    if let Err(err) = handle_connection(stream, &shared) {
                        let _ = err;
                    }
                });
            }
            Err(err) if err.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
}

fn handle_connection(mut stream: TcpStream, shared: &Shared) -> io::Result<()> {
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let (head, prelude) = match read_headers(&mut stream) {
        Ok(parts) => parts,
        Err(_) => {
            let _ = write_response(
                &mut stream,
                400,
                "Bad Request",
                "application/json",
                b"{\"error\":\"bad_request\"}",
                None,
            );
            return Ok(());
        }
    };
    let Some((method, path, query)) = request_target(&head) else {
        let _ = write_response(
            &mut stream,
            400,
            "Bad Request",
            "application/json",
            b"{\"error\":\"bad_request\"}",
            None,
        );
        return Ok(());
    };
    if let Some(resource) = path.strip_prefix("/aix-view/") {
        let result = resource
            .split_once('/')
            .filter(|_| method == "GET")
            .and_then(|(token, relative)| {
                plugin_view_owner(shared).map(|owner| (owner, token, relative))
            })
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
            .and_then(|(owner, token, relative)| {
                shared
                    .capabilities
                    .plugin_view_asset(&owner, token, &percent_decode(relative))
            });
        return write_plugin_view_response(&mut stream, result);
    }
    if let Some(resource) = path.strip_prefix("/presenton-ui/") {
        let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
        let result = if method == "GET" {
            presenton_ui_file(&percent_decode(resource))
                .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
        } else {
            Err(io::Error::from(io::ErrorKind::NotFound))
        };
        return write_plugin_view_response(&mut stream, result);
    }
    let origin = header_value(&head, "origin");
    let allow_origin = match origin.as_deref() {
        None => None,
        Some(value) if is_loopback_origin(value) => Some(value.to_owned()),
        Some(_) => {
            let _ = write_response(
                &mut stream,
                403,
                "Forbidden",
                "application/json",
                b"{\"error\":\"forbidden\"}",
                None,
            );
            return Ok(());
        }
    };

    if method == "OPTIONS" && shell_api(path) {
        let _ = write_response(
            &mut stream,
            204,
            "No Content",
            "text/plain",
            b"",
            allow_origin.as_deref(),
        );
        return Ok(());
    }

    let api = shell_api(path);
    if path == "/api/plugin-documents" {
        let bearer = header_value(&head, "authorization")
            .and_then(|value| value.strip_prefix("Bearer ").map(str::to_owned));
        if !bearer
            .as_deref()
            .is_some_and(|value| shared.workspace_documents.allows_bearer(value))
        {
            return write_response(
                &mut stream,
                404,
                "Document Request Refused",
                "application/json",
                br#"{"error":"document_refused"}"#,
                allow_origin.as_deref(),
            );
        }
    } else if let Some(app_id) = crate::applet_runtime::runtime_app_id(path) {
        let bearer = header_value(&head, "authorization").and_then(|value| {
            value
                .strip_prefix("Bearer ")
                .or_else(|| value.strip_prefix("bearer "))
                .map(str::trim)
        });
        let ready_preview = shell_ready_process_preview(method, path)
            && bearer.is_some_and(|token| tokens_match(token, &shared.token));
        if !ready_preview
            && !bearer
                .is_some_and(|token| shared.applet_access.allows(token, app_id, &shared.token))
        {
            let _ = write_response(
                &mut stream,
                401,
                "Unauthorized",
                "application/json",
                b"{\"error\":\"unauthorized\"}",
                allow_origin.as_deref(),
            );
            return Ok(());
        }
    } else if api && !authorized(&head, &shared.token) {
        let _ = write_response(
            &mut stream,
            401,
            "Unauthorized",
            "application/json",
            b"{\"error\":\"unauthorized\"}",
            allow_origin.as_deref(),
        );
        return Ok(());
    }

    if method == "GET" {
        if let Some(org) = im_stream_org(path) {
            return stream_im(stream, org, query, shared, allow_origin.as_deref());
        }
        if path == "/api/local-documents/changes" {
            return stream_document_changes(stream, query, allow_origin.as_deref());
        }
    }

    let composer_upload = method == "POST" && path.starts_with("/api/composer-files/");
    let skill_upload = method == "POST" && path == "/api/skills/upload";
    let im_upload = method == "POST" && im_attachment_upload(path);
    let workspace_write = method == "PUT"
        && matches!(
            path,
            "/api/workspace-file/raw" | "/api/workspace-file/snapshot"
        );
    let workspace_import = method == "POST" && path == "/api/workspace-file/import";
    let workspace_document =
        method == "POST" && matches!(path, "/api/workspace-documents" | "/api/plugin-documents");
    let resource_request = resource_broker::is_request(method, path);
    let drive_bytes = crate::org_login::is_drive_file_byte_route(method, path);
    if skill_upload {
        if let Some(name) = header_value(&head, "x-skill-filename") {
            let name = name.trim().trim_matches('"').to_ascii_lowercase();
            if !name.ends_with(".zip") && !name.ends_with(".skill") {
                let _ = write_response(
                    &mut stream,
                    400,
                    "Bad Request",
                    "application/json",
                    br#"{"error":"bad_format"}"#,
                    allow_origin.as_deref(),
                );
                return Ok(());
            }
        }
    }
    let paperloom_upload = method == "POST" && path == "/api/paperloom/uploads";
    let body_limit = if paperloom_upload {
        crate::paperloom::UPLOAD_LIMIT
    } else if composer_upload {
        COMPOSER_FILE_LIMIT
    } else if skill_upload {
        crate::capabilities::MAX_SKILL_ARCHIVE
    } else if im_upload {
        crate::org_login::MAX_IM_ATTACHMENT
    } else if resource_request {
        resource_broker::MAX_BODY_BYTES
    } else if workspace_write || workspace_import || workspace_document {
        crate::workspace_file::RAW_LIMIT as usize
    } else if drive_bytes {
        crate::org_login::MAX_HUB_PREVIEW
    } else {
        MAX_BODY_BYTES
    };
    if paperloom_upload
        || composer_upload
        || skill_upload
        || im_upload
        || workspace_write
        || workspace_import
        || workspace_document
        || resource_request
        || drive_bytes
    {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
    }
    let content_length =
        header_value(&head, "content-length").and_then(|value| value.parse::<usize>().ok());
    if content_length.unwrap_or(0) > body_limit {
        let _ = write_response(
            &mut stream,
            413,
            "Payload Too Large",
            "application/json",
            b"{\"error\":\"payload_too_large\"}",
            allow_origin.as_deref(),
        );
        return Ok(());
    }
    // DELETE carries a body too: member-plane deletes send `expected_revision`.
    let body = if matches!(method, "POST" | "PUT" | "PATCH" | "DELETE") {
        read_body(
            &mut stream,
            &prelude,
            content_length.unwrap_or(0),
            body_limit,
        )?
    } else {
        Vec::new()
    };

    if method == "POST" && path == "/api/workspace-folder" {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(180)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(180)));
    }
    if method == "PUT" {
        if let Some(slug) = capability_install_slug(path) {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(60)));
            let _ = stream.set_write_timeout(Some(Duration::from_secs(60)));
            return apply_capability_install(
                &mut stream,
                slug,
                &body,
                shared,
                allow_origin.as_deref(),
            );
        }
    }
    if path == "/api/workspace-file/snapshot" {
        let result = workspace_snapshot(
            method,
            query,
            &body,
            header_value(&head, "if-match").as_deref(),
            &shared.workspace_snapshots,
        );
        return match result {
            Ok(snapshot) => write_response_with_etag(
                &mut stream,
                200,
                "OK",
                "application/octet-stream",
                &snapshot.bytes,
                allow_origin.as_deref(),
                Some(&snapshot.revision),
            ),
            Err(status) => write_response(
                &mut stream,
                status,
                "Workspace Snapshot Refused",
                "application/json",
                b"{\"error\":\"snapshot_refused\"}",
                allow_origin.as_deref(),
            ),
        };
    }
    if drive_bytes {
        let response = crate::org_login::relay_drive_file_bytes(
            shared.org_origin.as_str(),
            method,
            path,
            query,
            &body,
            header_value(&head, "content-type").as_deref(),
            shared.member_session.token().as_deref(),
        );
        return write_response(
            &mut stream,
            response.0,
            response.1,
            response.2,
            &response.3,
            allow_origin.as_deref(),
        );
    }
    let (status, reason, content_type, payload) = if path == "/api/plugin-documents" {
        let bearer = header_value(&head, "authorization")
            .and_then(|value| value.strip_prefix("Bearer ").map(str::to_owned));
        plugin_document_response(
            method,
            &body,
            bearer.as_deref().unwrap_or(""),
            &shared.workspace_documents,
        )
    } else {
        dispatch(method, path, query, &body, origin.as_deref(), shared)
    };
    write_response(
        &mut stream,
        status,
        reason,
        content_type,
        &payload,
        allow_origin.as_deref(),
    )
}

fn plugin_document_response(
    method: &str,
    body: &[u8],
    bearer: &str,
    documents: &crate::workspace_document::WorkspaceDocuments,
) -> (u16, &'static str, &'static str, Vec<u8>) {
    let result = if method != "POST" || !documents.allows_bearer(bearer) {
        Err(404)
    } else {
        serde_json::from_slice::<crate::workspace_document::DocumentRequest>(body)
            .map_err(|_| 400u16)
            .and_then(|request| {
                documents
                    .request_scoped(bearer, request)
                    .map_err(|error| error.status())
            })
    };
    match result {
        Ok(body) => (200, "OK", "application/json", body.into_bytes()),
        Err(status) => (
            status,
            "Document Request Refused",
            "application/json",
            br#"{"error":"document_refused"}"#.to_vec(),
        ),
    }
}

#[cfg(test)]
#[path = "authority_document_capability_tests.rs"]
mod document_capability_tests;

fn dispatch(
    method: &str,
    path: &str,
    query: &str,
    body: &[u8],
    origin: Option<&str>,
    shared: &Shared,
) -> (u16, &'static str, &'static str, Vec<u8>) {
    let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    if let Some(response) = resource_broker::route(method, &parts, query, body, shared) {
        return response;
    }
    if let (
        "POST",
        [
            "api",
            "aix",
            "views",
            "sessions",
            token,
            "agents",
            agent,
            "turns",
        ],
    ) = (method, parts.as_slice())
    {
        return aix_agent_turn(shared, token, agent, body);
    }
    if matches!(
        parts.as_slice(),
        ["api", "aix", "views"]
            | ["api", "aix", "views", "sessions"]
            | ["api", "aix", "views", "sessions", _]
            | ["api", "aix", "views", "requests", _]
    ) {
        let result = plugin_view_owner(shared)
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
            .and_then(|owner| match (method, parts.as_slice()) {
                ("GET", ["api", "aix", "views"]) => shared.capabilities.plugin_views(),
                ("POST", ["api", "aix", "views", "sessions"]) => {
                    shared.capabilities.open_plugin_view(&owner, body)
                }
                ("DELETE", ["api", "aix", "views", "requests", id]) => shared
                    .capabilities
                    .cancel_plugin_view_request(&owner, id, body),
                ("DELETE", ["api", "aix", "views", "sessions", token]) => shared
                    .capabilities
                    .close_plugin_view(&owner, token)
                    .map(|_| {
                        if let Some(agents) = &shared.aix_agents {
                            agents.end_view(token);
                        }
                        b"{}".to_vec()
                    }),
                _ => Err(io::ErrorKind::NotFound.into()),
            });
        return match result {
            Ok(body) => (200, "OK", "application/json", body),
            Err(_) => (
                404,
                "Not Found",
                "application/json",
                br#"{"error":"not_found"}"#.to_vec(),
            ),
        };
    }
    if let Some(response) =
        crate::providers::route(method, parts.as_slice(), body, &shared.providers)
    {
        return response;
    }
    if let Some(response) = crate::org_login::route(
        method,
        parts.as_slice(),
        body,
        &shared.org_origin,
        shared.member_session.computer_id(),
    ) {
        if matches!(
            (method, parts.as_slice()),
            (
                "POST",
                ["v1", "api", "org", "members", "sessions" | "code-sessions"]
            )
        ) && (200..300).contains(&response.0)
        {
            note_sign_in(shared, &response.3);
        }
        return response;
    }
    if let Some(response) = crate::org_login::asset(method, parts.as_slice()) {
        return response;
    }
    if let Some(response) = member_orgs(method, parts.as_slice(), query, body, shared) {
        return response;
    }
    if let ("GET", ["api", "capabilities", slug, "ui", surface]) = (method, parts.as_slice()) {
        return match shared.capabilities.plugin_ui(slug, surface) {
            Ok(body) => (200, "OK", "application/json", body),
            Err(_) => (
                404,
                "Not Found",
                "application/json",
                br#"{"error":"not_found"}"#.to_vec(),
            ),
        };
    }
    if let Some(response) = applet_member_work(method, parts.as_slice(), query, body, shared) {
        return response;
    }
    if method == "GET" && parts.first().copied() == Some("review-picker") {
        let relative = parts[1..].join("/");
        return crate::review_plugin::picker_http_path(&relative);
    }
    let outcome = match (method, parts.as_slice()) {
        ("GET", []) | ("GET", ["index.html"]) => {
            let deck = std::env::var("A3S_PRESENTATION_DECK").ok();
            if let Some(page) = presentation_deck_page(deck.as_deref()) {
                return (200, "OK", "text/html; charset=utf-8", page.into_bytes());
            }
            return (
                200,
                "OK",
                "text/html; charset=utf-8",
                INDEX_HTML.as_bytes().to_vec(),
            );
        }
        ("GET", ["slides"]) => {
            let deck = std::env::var("A3S_PRESENTATION_DECK").ok();
            let Some(page) = presentation_deck_page(deck.as_deref()) else {
                return (
                    404,
                    "Not Found",
                    "text/html; charset=utf-8",
                    b"not found".to_vec(),
                );
            };
            return (200, "OK", "text/html; charset=utf-8", page.into_bytes());
        }
        ("GET", ["studioClient.mjs"]) => {
            return (
                200,
                "OK",
                "text/javascript; charset=utf-8",
                shell_client_source(crate::packaged_shell()).into_bytes(),
            );
        }
        ("GET", ["logo.png"]) => {
            return (200, "OK", "image/png", shared.logo_png.clone());
        }
        ("GET", ["mark.png"]) => {
            return (200, "OK", "image/png", shared.mark_png.clone());
        }
        ("GET", ["wechat-qr.png"]) => {
            return (200, "OK", "image/png", WECHAT_QR.to_vec());
        }
        ("GET", ["login-bg.jpg"]) => {
            return (200, "OK", "image/jpeg", LOGIN_BG.to_vec());
        }
        ("GET", ["favicon.ico"]) => {
            return (200, "OK", "image/png", shared.icon_png.clone());
        }
        ("GET", ["api", "bootstrap"]) => bootstrap(shared),
        ("GET", ["api", "presentation-deck"]) => {
            presentation_deck_json(std::env::var("A3S_PRESENTATION_DECK").ok().as_deref())
        }
        ("POST", ["api", "workspace-documents"]) => {
            serde_json::from_slice::<crate::workspace_document::DocumentRequest>(body)
                .map_err(|_| 400u16)
                .and_then(|request| {
                    shared
                        .workspace_documents
                        .request(request)
                        .map_err(|error| error.status())
                })
        }
        ("GET", ["api", "workspace-file", "raw"]) => {
            let decode = crate::local_documents::decode_query_component;
            let target = query_value(query, "root")
                .zip(query_value(query, "path"))
                .ok_or(400u16)
                .and_then(|(root, path)| Ok((decode(root)?, decode(path)?)))
                .and_then(|(root, path)| crate::workspace_file::raw_bytes(&root, &path));
            match target {
                Ok(bytes) => return (200, "OK", "application/octet-stream", bytes),
                Err(status) => Err(status),
            }
        }
        ("POST", ["api", "workspace-file", "import"]) => {
            let decode = crate::local_documents::decode_query_component;
            let saved = query_value(query, "root")
                .zip(query_value(query, "directory"))
                .zip(query_value(query, "name"))
                .ok_or(400u16)
                .and_then(|((root, directory), name)| {
                    Ok((decode(root)?, decode(directory)?, decode(name)?))
                })
                .and_then(
                    |(root, directory, name)| match query_value(query, "replace") {
                        Some("false") => shared
                            .workspace_snapshots
                            .import_new(&root, &directory, &name, body),
                        None | Some("true") => shared
                            .workspace_snapshots
                            .import(&root, &directory, &name, body),
                        _ => Err(400),
                    },
                );
            match saved {
                Ok(path) => Ok(json!({ "ok": true, "path": path }).to_string()),
                Err(status) => Err(status),
            }
        }
        ("PUT", ["api", "workspace-file", "raw"]) => {
            let decode = crate::local_documents::decode_query_component;
            let saved = query_value(query, "root")
                .zip(query_value(query, "path"))
                .ok_or(400u16)
                .and_then(|(root, path)| Ok((decode(root)?, decode(path)?)))
                .and_then(|(root, path)| {
                    shared
                        .workspace_snapshots
                        .replace_legacy(&root, &path, body)
                });
            match saved {
                Ok(()) => Ok(json!({ "ok": true }).to_string()),
                Err(status) => Err(status),
            }
        }
        ("GET", ["api", "health"]) => Ok(json!({ "ok": true }).to_string()),
        ("GET", ["api", "agent-plane"]) => match &shared.agent_plane_url {
            Some(url) => Ok(json!({ "baseUrl": url }).to_string()),
            None => Err(503),
        },
        ("GET", ["api", "local-documents"]) => match query_value(query, "root") {
            Some(raw) => match crate::local_documents::decode_query_component(raw) {
                Ok(root) => crate::local_documents::list_json(&root),
                Err(code) => Err(code),
            },
            None => Err(400),
        },
        ("GET", ["api", "local-documents", "entries"]) => match query_value(query, "root") {
            Some(raw_root) => match crate::local_documents::decode_query_component(raw_root) {
                Ok(root) => match query_value(query, "path") {
                    Some(raw_path) => {
                        match crate::local_documents::decode_query_component(raw_path) {
                            Ok(path) => crate::local_documents::entries_json(&root, &path),
                            Err(code) => Err(code),
                        }
                    }
                    None => crate::local_documents::entries_json(&root, ""),
                },
                Err(code) => Err(code),
            },
            None => Err(400),
        },
        ("GET", ["api", "local-skills"]) => Ok(local_skills_json(shared)),
        ("GET", ["api", "review-plugin"]) => {
            return (
                200,
                "OK",
                "application/json",
                crate::review_plugin::status_json().into_bytes(),
            );
        }
        ("POST", ["api", "skills", "upload"]) => return skill_upload_http(body),
        ("POST", ["api", "skills", "builtin", name]) => return skill_builtin_http(name),
        ("GET", ["api", "workspace-search"]) => workspace_search(query),
        ("GET", ["api", "workspace-file"]) => {
            workspace_file(query, crate::workspace_file::metadata_json)
        }
        ("GET", ["api", "workspace-file", "text"]) => {
            workspace_file(query, crate::workspace_file::text_json)
        }
        ("POST", ["api", "workspace-file", "open"]) => crate::workspace_file::open_json(body),
        ("POST", ["api", "open-external"]) => crate::external_open::open_external_json(body),
        ("POST", ["api", "workspace-file", "reveal"]) => crate::workspace_file::reveal_json(body),
        ("POST", ["api", "local-documents", "trash"]) => crate::local_documents::trash_json(body),
        ("POST", ["api", "local-documents", "open"]) => crate::local_documents::open_json(body),
        ("POST", ["api", "local-documents", "entries"]) => {
            crate::local_documents::create_json(body)
        }
        ("POST", ["api", "local-documents", "rename"]) => crate::local_documents::rename_json(body),
        ("POST", ["api", "local-documents", "move"]) => crate::local_documents::move_json(body),
        ("POST", ["api", "local-documents", "remove"]) => crate::local_documents::remove_json(body),
        ("GET", ["api", "os-gateway", "credential"]) => {
            let Some(token) = shared.member_session.token() else {
                return (
                    401,
                    "Unauthorized",
                    "application/json",
                    br#"{"error":"unauthorized"}"#.to_vec(),
                );
            };
            return (
                200,
                "OK",
                "application/json",
                serde_json::json!({
                    "origin": crate::providers::gateway_origin(),
                    "token": token,
                })
                .to_string()
                .into_bytes(),
            );
        }
        ("GET", ["api", "member-session", "profile"]) => Ok(shared.member_session.profile_json()),
        ("GET", ["api", "member-session"]) => Ok(member_session_json(origin, shared)),
        ("POST", ["api", "member-session"]) => {
            let previous = shared.member_session.token();
            let saved = shared.member_session.save(body);
            if saved.is_ok() && previous.as_deref() != shared.member_session.token().as_deref() {
                shared
                    .org_centers
                    .lock()
                    .expect("org center")
                    .pending
                    .clear();
            }
            saved
        }
        ("POST", ["api", "member-session", "logout"]) => {
            let cleared = shared.member_session.clear();
            if cleared.is_ok() {
                shared
                    .org_centers
                    .lock()
                    .expect("org center")
                    .pending
                    .clear();
            }
            cleared
        }
        ("POST", ["api", "applet-products"]) => shared.applet_products.create(body),
        ("POST", ["api", "applet-products", id, "builds"])
            if crate::applet_product::applet_id_ok(id) =>
        {
            shared.applet_products.build(id, body)
        }
        ("POST", ["api", "applet-products", id, "publish"])
            if crate::applet_product::applet_id_ok(id) =>
        {
            shared.applet_products.publish(id, body)
        }
        ("GET", ["api", "applet-products", id, "package", channel])
            if crate::applet_product::applet_id_ok(id) =>
        {
            shared.applet_products.package(id, channel)
        }
        ("GET", ["api", "applet-products", id]) if crate::applet_product::applet_id_ok(id) => {
            shared.applet_products.metadata(id)
        }
        ("GET", ["api", "applet-runtime", id]) if crate::applet_product::is_office_app(id) => {
            shared.applet_products.metadata(id)
        }
        ("GET", ["api", "applet-runtime", id, "package", channel])
            if crate::applet_product::is_office_app(id)
                && matches!(*channel, "release" | "trial") =>
        {
            shared.applet_products.package(id, channel)
        }
        ("GET", ["api", "applet-runtime", id, "session"])
            if crate::applet_product::is_office_app(id) =>
        {
            Ok(shared.member_session.applet_session_json())
        }
        ("GET", ["api", "applet-runtime", id, "profile"])
            if crate::applet_product::is_office_app(id) =>
        {
            refresh_applet_profile(shared);
            Ok(shared.member_session.applet_profile_json())
        }
        ("POST", ["api", "applet-runtime", id, "submissions"])
            if crate::applet_product::is_office_app(id) =>
        {
            shared
                .applet_access
                .submit(id, &shared.applet_products, body)
        }
        ("POST", ["api", "conversations"]) => create_conversation(shared, body),
        ("GET", ["api", "conversations"]) => list_conversations(shared),
        ("GET", ["api", "conversations", id]) if is_id(id) => get_conversation(shared, id),
        ("POST", ["api", "conversations", id, "stream"]) if is_id(id) => open_stream(shared, id),
        ("GET", ["api", "conversations", id, "stream"]) if is_id(id) => read_stream(shared, id),
        ("POST", ["api", "conversations", id, "cancel"]) if is_id(id) => cancel_stream(shared, id),
        ("POST", ["api", "composer-files", name]) => save_composer_file(name, &body),
        ("POST", ["api", "agent-channels", "sync"]) => {
            crate::channel_gateway::apply(&shared.token, shared.agent_plane_url.as_deref(), body)
        }
        ("POST", ["api", "desktop-settings"]) => {
            match crate::desktop_settings::settings_from_body(body) {
                Ok(settings) => {
                    crate::desktop_settings::update(settings);
                    Ok(json!({ "ok": true }).to_string())
                }
                Err(status) => Err(status),
            }
        }
        ("POST", ["api", "workspace-folder"]) => {
            match crate::folder_dialog::directory_request_from_body(body) {
                Ok(request) => Ok(crate::folder_dialog::directory_response(
                    crate::folder_dialog::choose_directory(&request.default_path, &request.title),
                )),
                Err(status) => Err(status),
            }
        }
        ("POST", ["api", "workspace-entries"]) => crate::workspace_entries::apply_json(body),
        _ => Err(404),
    };
    match outcome {
        Ok(text) => (200, "OK", "application/json", text.into_bytes()),
        Err(404) => (
            404,
            "Not Found",
            "application/json",
            b"{\"error\":\"not_found\"}".to_vec(),
        ),
        Err(400) => (
            400,
            "Bad Request",
            "application/json",
            b"{\"error\":\"bad_request\"}".to_vec(),
        ),
        Err(status) => (
            status,
            "Error",
            "application/json",
            format!("{{\"error\":\"status_{status}\"}}").into_bytes(),
        ),
    }
}

fn workspace_snapshot(
    method: &str,
    query: &str,
    body: &[u8],
    expected: Option<&str>,
    snapshots: &crate::workspace_snapshot::WorkspaceSnapshots,
) -> Result<crate::workspace_snapshot::Snapshot, u16> {
    let decode = crate::local_documents::decode_query_component;
    let (root, path) = query_value(query, "root")
        .zip(query_value(query, "path"))
        .ok_or(400u16)?;
    let (root, path) = (decode(root)?, decode(path)?);
    match method {
        "GET" => snapshots.read(&root, &path),
        "PUT" => {
            let revision = expected
                .ok_or(428u16)?
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
                .ok_or(400u16)?;
            let mut snapshot = snapshots.write(&root, &path, revision, body)?;
            snapshot.bytes.clear();
            Ok(snapshot)
        }
        _ => Err(405),
    }
}

fn workspace_search(query: &str) -> Result<String, u16> {
    let decode = crate::local_documents::decode_query_component;
    let root = decode(query_value(query, "root").ok_or(400u16)?)?;
    let text = match query_value(query, "q") {
        Some(raw) => decode(raw)?,
        None => String::new(),
    };
    let limit = query_value(query, "limit").and_then(|raw| raw.parse::<usize>().ok());
    crate::workspace_search::search_json(&root, &text, limit)
}

fn workspace_file(
    query: &str,
    handler: fn(&str, &str) -> Result<String, u16>,
) -> Result<String, u16> {
    let decode = crate::local_documents::decode_query_component;
    let root = decode(query_value(query, "root").ok_or(400u16)?)?;
    let path = decode(query_value(query, "path").ok_or(400u16)?)?;
    handler(&root, &path)
}

fn local_skills_json(shared: &Shared) -> String {
    let roots: Vec<_> = [
        shared
            .capabilities
            .skills_dir()
            .map(|dir| (dir, crate::local_skills::Source::Installed)),
        crate::local_skills::agents_root().map(|dir| (dir, crate::local_skills::Source::User)),
        crate::local_skills::user_root().map(|dir| (dir, crate::local_skills::Source::User)),
    ]
    .into_iter()
    .flatten()
    .collect();
    crate::local_skills::list_json(&roots)
}

fn skill_upload_http(body: &[u8]) -> (u16, &'static str, &'static str, Vec<u8>) {
    match crate::capabilities::import_uploaded_skill(body) {
        Ok(name) => (
            200,
            "OK",
            "application/json",
            json!({ "name": name }).to_string().into_bytes(),
        ),
        Err(err) => skill_import_http(&err),
    }
}

fn skill_builtin_http(name: &str) -> (u16, &'static str, &'static str, Vec<u8>) {
    match crate::capabilities::prepare_builtin_skill_at_home(name) {
        Ok(()) => (
            200,
            "OK",
            "application/json",
            json!({ "name": name }).to_string().into_bytes(),
        ),
        Err(err) => skill_import_http(&err),
    }
}

fn skill_import_http(err: &std::io::Error) -> (u16, &'static str, &'static str, Vec<u8>) {
    let code = crate::capabilities::skill_import_code(err);
    let status = match code {
        "unavailable" => 503,
        "too_large" => 413,
        "skill_exists" => 409,
        "missing_builtin" => 404,
        "failed" => 500,
        _ => 400,
    };
    let reason = match status {
        413 => "Payload Too Large",
        409 => "Conflict",
        404 => "Not Found",
        503 => "Service Unavailable",
        500 => "Internal Server Error",
        _ => "Bad Request",
    };
    (
        status,
        reason,
        "application/json",
        json!({ "error": code }).to_string().into_bytes(),
    )
}

/// The shell page often omits Origin. Every other loopback origin gets the profile body.
fn member_session_json(origin: Option<&str>, shared: &Shared) -> String {
    match origin {
        None => shared.member_session.get_json(),
        Some(value) if value == shared.shell_origin => shared.member_session.get_json(),
        Some(_) => shared.member_session.profile_json(),
    }
}

/// `GET/PUT/DELETE /api/capabilities/custom` stores connectors the member authored.
fn local_custom_connectors(
    method: &str,
    parts: &[&str],
    body: &[u8],
    shared: &Shared,
) -> Option<(u16, &'static str, &'static str, Vec<u8>)> {
    let response = match (method, parts) {
        ("GET", ["api", "capabilities", "custom"]) => match shared.capabilities.custom_json() {
            Ok(json) => (200, "OK", "application/json", json.into_bytes()),
            Err(err) => custom_connector_http(err),
        },
        ("PUT", ["api", "capabilities", "custom"]) => match shared.capabilities.save_custom(body) {
            Ok(()) => (200, "OK", "application/json", br#"{"ok":true}"#.to_vec()),
            Err(err) => custom_connector_http(err),
        },
        ("DELETE", ["api", "capabilities", "custom", name]) => {
            match shared.capabilities.remove_custom(name) {
                Ok(()) => (200, "OK", "application/json", br#"{"ok":true}"#.to_vec()),
                Err(err) => custom_connector_http(err),
            }
        }
        _ => return None,
    };
    Some(response)
}

fn custom_connector_http(err: io::Error) -> (u16, &'static str, &'static str, Vec<u8>) {
    let code = crate::capabilities::custom_connector_code(&err);
    if err.kind() == io::ErrorKind::NotFound {
        return (
            503,
            "Service Unavailable",
            "application/json",
            br#"{"error":"unavailable"}"#.to_vec(),
        );
    }
    if err.kind() == io::ErrorKind::InvalidInput {
        return (
            400,
            "Bad Request",
            "application/json",
            json!({ "error": code }).to_string().into_bytes(),
        );
    }
    (
        500,
        "Error",
        "application/json",
        br#"{"error":"failed"}"#.to_vec(),
    )
}

/// `GET/PUT /api/capabilities/config` reads and writes this device's connector values.
fn local_connector_config(
    method: &str,
    parts: &[&str],
    body: &[u8],
    shared: &Shared,
) -> Option<(u16, &'static str, &'static str, Vec<u8>)> {
    match (method, parts) {
        ("GET", ["api", "capabilities", "config"]) => Some((
            200,
            "OK",
            "application/json",
            shared.capabilities.config_json().into_bytes(),
        )),
        ("PUT", ["api", "capabilities", "config", slug])
            if crate::org_login::capability_slug_ok(slug) =>
        {
            Some(match shared.capabilities.save_config(slug, body) {
                Ok(()) => (200, "OK", "application/json", br#"{"ok":true}"#.to_vec()),
                Err(err) if err.kind() == io::ErrorKind::InvalidInput => (
                    400,
                    "Bad Request",
                    "application/json",
                    json!({ "error": "invalid_config" })
                        .to_string()
                        .into_bytes(),
                ),
                Err(_) => (
                    500,
                    "Error",
                    "application/json",
                    br#"{"error":"config_failed"}"#.to_vec(),
                ),
            })
        }
        _ => None,
    }
}

/// Installed plugin surfaces this machine can call.
fn local_plugin_callable(
    method: &str,
    parts: &[&str],
    shared: &Shared,
) -> Option<(u16, &'static str, &'static str, Vec<u8>)> {
    let ("GET", ["api", "capabilities", slug, "callable"]) = (method, parts) else {
        return None;
    };
    if !crate::org_login::capability_slug_ok(slug) {
        return None;
    }
    Some(plugin_okf_http(shared.capabilities.callable_plugin(slug)))
}

/// Plugin OKF pages stay on this machine. `GET/PUT /api/capabilities/{slug}/okf`.
fn local_plugin_okf(
    method: &str,
    parts: &[&str],
    query: &str,
    body: &[u8],
    shared: &Shared,
) -> Option<(u16, &'static str, &'static str, Vec<u8>)> {
    let slug_ok = |slug: &str| crate::org_login::capability_slug_ok(slug);
    let outcome = match (method, parts) {
        ("GET", ["api", "capabilities", slug, "okf"]) if slug_ok(slug) => {
            shared.capabilities.list_plugin_okf(slug)
        }
        ("GET", ["api", "capabilities", slug, "okf", "page"]) if slug_ok(slug) => {
            let Some(raw) = query_value(query, "path") else {
                return Some(plugin_okf_invalid());
            };
            let Ok(relative) = crate::local_documents::decode_query_component(raw) else {
                return Some(plugin_okf_invalid());
            };
            shared.capabilities.read_plugin_okf(slug, &relative)
        }
        ("PUT", ["api", "capabilities", slug, "okf", "page"]) if slug_ok(slug) => {
            shared.capabilities.save_plugin_okf(slug, body)
        }
        ("POST", ["api", "capabilities", slug, "okf", "seed"]) if slug_ok(slug) => {
            shared.capabilities.seed_plugin_okf(slug, body)
        }
        _ => return None,
    };
    Some(plugin_okf_http(outcome))
}

fn plugin_okf_invalid() -> (u16, &'static str, &'static str, Vec<u8>) {
    (
        400,
        "Bad Request",
        "application/json",
        br#"{"error":"invalid"}"#.to_vec(),
    )
}

fn plugin_okf_http(outcome: io::Result<Vec<u8>>) -> (u16, &'static str, &'static str, Vec<u8>) {
    match outcome {
        Ok(body) => (200, "OK", "application/json", body),
        Err(err)
            if err.kind() == io::ErrorKind::NotFound && err.to_string() == "missing_plugin" =>
        {
            (
                404,
                "Not Found",
                "application/json",
                br#"{"error":"missing_plugin"}"#.to_vec(),
            )
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => (
            503,
            "Service Unavailable",
            "application/json",
            br#"{"error":"unavailable"}"#.to_vec(),
        ),
        Err(err) if err.kind() == io::ErrorKind::InvalidInput => (
            400,
            "Bad Request",
            "application/json",
            br#"{"error":"invalid"}"#.to_vec(),
        ),
        Err(_) => (
            500,
            "Error",
            "application/json",
            br#"{"error":"index_failed"}"#.to_vec(),
        ),
    }
}

/// `PUT /api/knowledge/wiki` projects saved pages into the tenant wiki directory.
fn local_wiki(
    method: &str,
    parts: &[&str],
    body: &[u8],
    shared: &Shared,
) -> Option<(u16, &'static str, &'static str, Vec<u8>)> {
    if !(method == "PUT" && parts == ["api", "knowledge", "wiki"]) {
        return None;
    }
    Some(match shared.capabilities.project_wiki(body) {
        Ok(()) => (200, "OK", "application/json", br#"{"ok":true}"#.to_vec()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => (
            503,
            "Service Unavailable",
            "application/json",
            br#"{"error":"unavailable"}"#.to_vec(),
        ),
        Err(err) if err.kind() == io::ErrorKind::InvalidInput => (
            400,
            "Bad Request",
            "application/json",
            br#"{"error":"invalid"}"#.to_vec(),
        ),
        Err(_) => (
            500,
            "Error",
            "application/json",
            br#"{"error":"failed"}"#.to_vec(),
        ),
    })
}

/// Process windows preview and start through the engine. They do not create a single-step item.
fn process_window(id: &str) -> bool {
    matches!(
        id,
        "leave" | "expense" | "travel" | "correction" | "probation" | "ai-resource" | "clock"
    )
}

/// A ready process preview from the shell bearer uses the stored member session.
/// The page never receives that bearer. Every other applet route still requires its ticket.
fn shell_ready_process_preview(method: &str, path: &str) -> bool {
    if method != "POST" {
        return false;
    }
    let mut parts = path.split('/').filter(|part| !part.is_empty());
    matches!(
        (
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next()
        ),
        (
            Some("api"),
            Some("applet-runtime"),
            Some(id),
            Some("work-items"),
            Some("process-preview"),
            None
        ) if process_window(id)
    )
}

/// Work inbox for an applet window.
///
/// The page presents its own ticket. The shell attaches the stored member
/// session and never puts that session, or the shell bearer, on the page.
/// Listing and decisions stay on the inbox ticket. An office applet may create
/// one item of its own kind. The selected team is readable by those same windows.
fn applet_member_work(
    method: &str,
    parts: &[&str],
    query: &str,
    body: &[u8],
    shared: &Shared,
) -> Option<(u16, &'static str, &'static str, Vec<u8>)> {
    let office = |id: &str| id == "inbox" || crate::applet_product::is_office_app(id);
    match (method, parts) {
        ("POST", ["api", "member-center"]) => Some(store_org_center(shared, body)),
        ("GET", ["api", "applet-runtime", id, "center"]) if office(id) => {
            Some(org_center_json(shared))
        }
        ("GET", ["api", "applet-runtime", "inbox", "work-items", "summary"]) => {
            Some(forward_scoped_work(shared, true, ""))
        }
        ("GET", ["api", "applet-runtime", "inbox", "work-items"]) => {
            Some(forward_scoped_work(shared, false, query))
        }
        ("GET", ["api", "applet-runtime", id, "work-items", "approvers"])
            if office(id) && id != &"notice" =>
        {
            Some(forward_scoped_suffix(shared, "approvers", query))
        }
        (
            "GET",
            [
                "api",
                "applet-runtime",
                "expense",
                "work-items",
                "travel-links",
            ],
        ) => Some(forward_scoped_suffix(shared, "travel-links", query)),
        ("POST", ["api", "applet-runtime", id, "work-items"]) if office(id) => Some(
            forward_member_work(shared, "POST", "/v1/api/org/members/work-items", body),
        ),
        ("POST", ["api", "applet-runtime", id, "work-items", "process-preview"])
            if process_window(id) =>
        {
            Some(forward_member_work(
                shared,
                "POST",
                "/v1/api/org/members/work-items/process-preview",
                body,
            ))
        }
        ("POST", ["api", "applet-runtime", id, "work-items", "process"]) if process_window(id) => {
            Some(forward_member_work(
                shared,
                "POST",
                "/v1/api/org/members/work-items/process",
                body,
            ))
        }
        (
            "POST",
            [
                "api",
                "applet-runtime",
                "inbox",
                "work-items",
                id,
                "decision",
            ],
        ) if crate::org_login::uuid_id_ok(id) => Some(forward_member_work(
            shared,
            "POST",
            &format!("/v1/api/org/members/work-items/{id}/decision"),
            body,
        )),
        (
            "POST",
            [
                "api",
                "applet-runtime",
                "inbox",
                "work-items",
                id,
                "schedule",
            ],
        ) if crate::org_login::uuid_id_ok(id) => Some(forward_member_work(
            shared,
            "POST",
            &format!("/v1/api/org/members/work-items/{id}/schedule"),
            body,
        )),
        _ => None,
    }
}

fn store_org_center(shared: &Shared, body: &[u8]) -> (u16, &'static str, &'static str, Vec<u8>) {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return plain_status(400);
    };
    let Some(object) = value.as_object() else {
        return plain_status(400);
    };
    if object.len() != 1 || !object.contains_key("id") {
        return plain_status(400);
    }
    let Some(id) = object.get("id").and_then(serde_json::Value::as_str) else {
        return plain_status(400);
    };
    let id = id.trim();
    if id != "personal" && !crate::org_login::org_id_ok(id) {
        return plain_status(400);
    }
    let member = shared.member_session.member_id();
    let mut centers = shared.org_centers.lock().expect("org center");
    if let Some(member) = member {
        centers.by_member.insert(member, id.to_owned());
        centers.pending.clear();
    } else {
        centers.pending = id.to_owned();
    }
    (200, "OK", "application/json", br#"{"ok":true}"#.to_vec())
}

fn org_center_json(shared: &Shared) -> (u16, &'static str, &'static str, Vec<u8>) {
    let id = current_org_scope(shared);
    (
        200,
        "OK",
        "application/json",
        json!({ "id": id }).to_string().into_bytes(),
    )
}

/// The signed-in member's team, or `personal` when none is stored.
/// Another member's selection is never returned.
fn current_org_scope(shared: &Shared) -> String {
    let member = shared.member_session.member_id();
    let centers = shared.org_centers.lock().expect("org center");
    let chosen = if let Some(member) = member.as_deref() {
        centers
            .by_member
            .get(member)
            .map(String::as_str)
            .unwrap_or("personal")
    } else if centers.pending.is_empty() {
        "personal"
    } else {
        centers.pending.as_str()
    };
    if chosen == "personal" || crate::org_login::org_id_ok(chosen) {
        chosen.to_owned()
    } else {
        "personal".to_owned()
    }
}

fn forward_member_work(
    shared: &Shared,
    method: &str,
    path: &str,
    body: &[u8],
) -> (u16, &'static str, &'static str, Vec<u8>) {
    let Some(token) = shared.member_session.token() else {
        return plain_status(401);
    };
    crate::org_login::forward_member_api(&shared.org_origin, method, path, body, &token)
}

/// Replaces any `org` the page sent with the team stored for the current member.
fn forward_scoped_work(
    shared: &Shared,
    summary: bool,
    query: &str,
) -> (u16, &'static str, &'static str, Vec<u8>) {
    let org = current_org_scope(shared);
    let scoped = match scoped_work_query(query, &org) {
        Ok(value) => value,
        Err(status) => return plain_status(status),
    };
    let path = if summary {
        format!("/v1/api/org/members/work-items/summary?{scoped}")
    } else {
        format!("/v1/api/org/members/work-items?{scoped}")
    };
    forward_member_work(shared, "GET", &path, &[])
}

fn forward_scoped_suffix(
    shared: &Shared,
    suffix: &str,
    query: &str,
) -> (u16, &'static str, &'static str, Vec<u8>) {
    let org = current_org_scope(shared);
    let scoped = match scoped_work_query(query, &org) {
        Ok(value) => value,
        Err(status) => return plain_status(status),
    };
    forward_member_work(
        shared,
        "GET",
        &format!("/v1/api/org/members/work-items/{suffix}?{scoped}"),
        &[],
    )
}

fn scoped_work_query(query: &str, org: &str) -> Result<String, u16> {
    if query.len() > 480
        || query
            .bytes()
            .any(|byte| matches!(byte, b'\0' | b'\r' | b'\n' | b' '))
    {
        return Err(400);
    }
    let mut out = String::new();
    for part in query.split('&') {
        if part.is_empty() || part == "org" || part.starts_with("org=") {
            continue;
        }
        if !out.is_empty() {
            out.push('&');
        }
        out.push_str(part);
    }
    if !out.is_empty() {
        out.push('&');
    }
    out.push_str("org=");
    out.push_str(org);
    if out.len() > 512 {
        return Err(400);
    }
    Ok(out)
}

fn plain_status(status: u16) -> (u16, &'static str, &'static str, Vec<u8>) {
    match status {
        400 => (
            400,
            "Bad Request",
            "application/json",
            br#"{"error":"bad_request"}"#.to_vec(),
        ),
        401 => (
            401,
            "Unauthorized",
            "application/json",
            br#"{"error":"unauthorized"}"#.to_vec(),
        ),
        _ => (
            status,
            "Error",
            "application/json",
            br#"{"error":"failed"}"#.to_vec(),
        ),
    }
}

/// Teams and 能力扩展 for the signed-in member. The shell bearer authorizes
/// these routes; the stored `a3s_ms_` token is attached only upstream.
fn member_orgs(
    method: &str,
    parts: &[&str],
    query: &str,
    body: &[u8],
    shared: &Shared,
) -> Option<(u16, &'static str, &'static str, Vec<u8>)> {
    if let ["api", "paperloom", rest @ ..] = parts {
        let Some(member) = shared.member_session.member_id() else {
            return Some(plain_status(401));
        };
        let Some(query) = crate::paperloom::member_query(&member, query) else {
            return Some(plain_status(404));
        };
        let installation = shared
            .capabilities
            .callable_plugin("a3s-paperloom")
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
        let Some(installation) = installation.filter(|value| {
            value.get("packageId").and_then(serde_json::Value::as_str) == Some("use/a3s/paperloom")
        }) else {
            return Some(plain_status(404));
        };
        let Some(root) = shared
            .capabilities
            .drafts_dir()
            .and_then(|path| path.parent().map(PathBuf::from))
        else {
            return Some(plain_status(503));
        };
        if let Some(response) = crate::paperloom::provenance_route(
            &root,
            &member,
            installation
                .get("packageDigest")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            method,
            rest,
            &query,
            body,
        ) {
            return Some(response);
        }
        return Some(if *rest == ["connection"] {
            crate::paperloom::connection(&root, &member, method, body)
        } else {
            crate::paperloom::proxy(&root, &member, method, rest, &query, body)
        });
    }
    // Connector secrets stay on this machine. They never go upstream.
    if let Some(response) = local_custom_connectors(method, parts, body, shared) {
        return Some(response);
    }
    if let Some(response) = local_connector_config(method, parts, body, shared) {
        return Some(response);
    }
    if let Some(response) = local_wiki(method, parts, body, shared) {
        return Some(response);
    }
    if let Some(response) = local_plugin_callable(method, parts, shared) {
        return Some(response);
    }
    if let Some(response) = local_plugin_okf(method, parts, query, body, shared) {
        return Some(response);
    }
    // The page cannot choose another team's inbox. The shell writes `org`.
    if method == "GET" && parts == ["api", "member", "work-items", "summary"] {
        return Some(forward_scoped_work(shared, true, ""));
    }
    if method == "GET" && parts == ["api", "member", "work-items"] {
        return Some(forward_scoped_work(shared, false, query));
    }
    if method == "GET" && parts == ["api", "member", "work-items", "approvers"] {
        return Some(forward_scoped_suffix(shared, "approvers", query));
    }
    if method == "GET" && parts == ["api", "member", "work-items", "travel-links"] {
        return Some(forward_scoped_suffix(shared, "travel-links", query));
    }
    let upstream = match (method, parts) {
        ("GET", ["api", "member-orgs"]) => ("GET", "/v1/api/org/members/orgs".to_owned()),
        ("POST", ["api", "member-orgs"]) => ("POST", "/v1/api/org/members/orgs".to_owned()),
        ("GET", ["api", "member-departments", id]) if crate::org_login::org_id_ok(id) => {
            ("GET", format!("/v1/api/org/members/orgs/{id}/departments"))
        }
        // Mail is per member, including the personal center. `/api/mail/...`
        // mirrors `/v1/api/org/mail/...`; allowed_member_path gates it.
        (method, ["api", "mail", rest @ ..])
            if matches!(method, "GET" | "POST")
                && !rest.is_empty()
                && rest.len() <= 3
                && rest
                    .iter()
                    .all(|segment| crate::org_login::im_segment_ok(segment)) =>
        {
            (method, format!("/v1/api/org/mail/{}", rest.join("/")))
        }
        // 组织架构 page: the loopback `/api/member/...` tree mirrors the
        // upstream `/v1/api/org/members/...` tree; allowed_member_path gates it.
        (method, ["api", "member", rest @ ..])
            if matches!(method, "GET" | "POST" | "PUT" | "DELETE") && !rest.is_empty() =>
        {
            (method, format!("/v1/api/org/members/{}", rest.join("/")))
        }
        // Team repositories: `/api/repositories/...` mirrors
        // `/v1/api/org/repositories/...`; allowed_member_path gates the shape.
        (method, ["api", "repositories"]) if matches!(method, "GET" | "POST") => {
            (method, "/v1/api/org/repositories".to_owned())
        }
        (method, ["api", "repositories", rest @ ..])
            if matches!(method, "GET" | "POST" | "PUT") && rest.len() <= 6 =>
        {
            (
                method,
                format!("/v1/api/org/repositories/{}", rest.join("/")),
            )
        }
        ("GET", ["api", "repository-namespaces", org]) if crate::org_login::uuid_id_ok(org) => {
            ("GET", format!("/v1/api/org/repository-namespaces/{org}"))
        }
        ("GET", ["api", "pipelines", kind @ ("repos" | "runs"), id]) => {
            ("GET", format!("/v1/api/org/pipelines/{kind}/{id}"))
        }
        // Handbook library. List is JSON; content bytes use the larger preview cap.
        ("GET", ["api", "hub", "files"]) => ("GET", "/v1/api/org/hub/files".to_owned()),
        ("GET", ["api", "hub", "files", id, "content"]) if crate::org_login::uuid_id_ok(id) => {
            ("GET", format!("/v1/api/org/hub/files/{id}/content"))
        }
        // Drive lane: loopback `/api/drive/{org}/...` mirrors upstream
        // `/v1/api/org/drive/{org}/...`（apps/api docs/DRIVE.md §4；multipart 直连不经此道）。
        (method, ["api", "drive", org, rest @ ..])
            if matches!(method, "GET" | "POST" | "PUT" | "DELETE")
                && crate::org_login::org_id_ok(org)
                && rest.len() <= 6
                && rest
                    .iter()
                    .all(|segment| crate::org_login::im_segment_ok(segment)) =>
        {
            (
                method,
                format!("/v1/api/org/drive/{org}/{}", rest.join("/")),
            )
        }
        // IM REST lane: loopback `/api/im/{org}/...` mirrors upstream
        // `/v1/api/org/im/{org}/...`（apps/api docs/IM.md §6.1；SSE 流通道另议）。
        (method, ["api", "im", org, rest @ ..])
            if matches!(method, "GET" | "POST" | "PUT" | "DELETE")
                && crate::org_login::uuid_id_ok(org)
                && !rest.is_empty()
                && rest.len() <= 6
                && rest
                    .iter()
                    .all(|segment| crate::org_login::im_segment_ok(segment)) =>
        {
            (method, format!("/v1/api/org/im/{org}/{}", rest.join("/")))
        }
        ("GET", ["api", "catalog-agents"]) => ("GET", "/v1/api/org/catalog-agents".to_owned()),
        ("GET", ["api", "capabilities"]) => ("GET", "/v1/api/org/capabilities".to_owned()),
        ("GET", ["api", "capabilities", slug, action])
            if crate::org_login::capability_slug_ok(slug)
                && matches!(*action, "icon" | "cover") =>
        {
            ("GET", format!("/v1/api/org/capabilities/{slug}/{action}"))
        }
        ("GET", ["v1", "api", "org", "models"]) => ("GET", "/v1/api/org/models".to_owned()),
        ("PUT", ["api", "capabilities", "installations", slug])
            if crate::org_login::capability_slug_ok(slug) =>
        {
            (
                "PUT",
                format!("/v1/api/org/capabilities/installations/{slug}"),
            )
        }
        ("DELETE", ["api", "capabilities", "installations", slug])
            if crate::org_login::capability_slug_ok(slug) =>
        {
            (
                "DELETE",
                format!("/v1/api/org/capabilities/installations/{slug}"),
            )
        }
        _ => return None,
    };
    let Some(token) = shared.member_session.token() else {
        return Some((
            401,
            "Unauthorized",
            "application/json",
            br#"{"error":"unauthorized"}"#.to_vec(),
        ));
    };
    let upstream_path = if query.is_empty() {
        upstream.1.clone()
    } else {
        format!("{}?{}", upstream.1, query)
    };
    let origin = if upstream.1.starts_with("/v1/api/org/capabilities") {
        shared.gateway_origin.as_str()
    } else {
        shared.org_origin.as_str()
    };
    if upstream.0 == "GET"
        && upstream.1.starts_with("/v1/api/org/hub/files/")
        && upstream.1.ends_with("/content")
    {
        return Some(crate::org_login::forward_hub_content(
            origin,
            &upstream_path,
            &token,
        ));
    }
    if upstream.1.ends_with("/icon") || upstream.1.ends_with("/cover") {
        return Some(crate::org_login::forward_member_icon(
            origin,
            &upstream_path,
            &token,
        ));
    }
    if is_im_attachment(&upstream.1) {
        return Some(crate::org_login::forward_member_binary(
            &shared.org_origin,
            upstream.0,
            &upstream_path,
            body,
            &token,
        ));
    }
    let response =
        crate::org_login::forward_member_api(origin, upstream.0, &upstream_path, body, &token);
    if matches!(upstream.0, "GET" | "PUT")
        && upstream.1 == "/v1/api/org/members/me"
        && (200..300).contains(&response.0)
    {
        note_member_profile(shared, &response.3);
    }
    if !upstream.1.starts_with("/v1/api/org/capabilities") || !(200..300).contains(&response.0) {
        return Some(response);
    }
    if upstream.0 == "GET" {
        return Some(response);
    }
    match sync_capabilities(shared) {
        Ok(()) => Some(response),
        Err(err) => Some((
            502,
            "Bad Gateway",
            "application/json",
            json!({
                "code": 502,
                "message": format!("saved to your account, but this device could not apply it: {err}"),
                "data": null,
            })
            .to_string()
            .into_bytes(),
        )),
    }
}

/// Copies the OS member name and photo into the saved session.
/// The phone and the token in that payload are ignored.
fn note_member_profile(shared: &Shared, payload: &[u8]) {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(payload) else {
        return;
    };
    let member = value.get("data").unwrap_or(&value);
    let name = member
        .get("display_name")
        .and_then(serde_json::Value::as_str);
    let avatar = member.get("avatar").and_then(serde_json::Value::as_str);
    if shared
        .member_session
        .remember_profile(name, avatar)
        .is_err()
    {
        eprintln!("studio: saved member profile was not updated");
    }
    if let Some(id) = member.get("id").and_then(serde_json::Value::as_str) {
        if shared.member_session.remember_member_id(id).is_err() {
            eprintln!("studio: saved member id was not updated");
        }
    }
}

fn note_sign_in(shared: &Shared, payload: &[u8]) {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(payload) else {
        return;
    };
    let auth = value.get("data").unwrap_or(&value);
    let token = auth.get("token").and_then(serde_json::Value::as_str);
    let member = auth
        .get("member")
        .and_then(|member| member.get("id"))
        .and_then(serde_json::Value::as_str);
    if let (Some(token), Some(member)) = (token, member) {
        shared.member_session.note_sign_in(token, member);
    }
}

fn refresh_applet_profile(shared: &Shared) {
    let Some(token) = shared.member_session.token() else {
        return;
    };
    let response = crate::org_login::forward_member_api(
        &shared.org_origin,
        "GET",
        "/v1/api/org/members/me",
        &[],
        &token,
    );
    if !(200..300).contains(&response.0) {
        return;
    }
    note_member_profile(shared, &response.3);
}

/// Mirrors the member's installed capabilities onto disk. One installed-set read,
/// not the browsable shelf. Studio does not install plugins on its own.
fn sync_capabilities(shared: &Shared) -> io::Result<()> {
    let Some(token) = shared.member_session.token() else {
        return Ok(());
    };
    let response = crate::org_login::forward_member_api(
        &shared.gateway_origin,
        "GET",
        "/v1/api/org/capabilities/installations",
        &[],
        &token,
    );
    if response.0 != 200 {
        return Err(io::Error::other(format!(
            "capability catalog returned HTTP {}",
            response.0
        )));
    }
    let entries = installed_entries(&response.3)?;
    shared.capabilities.reconcile(&entries, |slug| {
        crate::org_login::fetch_capability_bundle(&shared.gateway_origin, slug, &token)
    })
}

fn capability_install_slug(path: &str) -> Option<&str> {
    let slug = path.strip_prefix("/api/capabilities/installations/")?;
    if slug.contains('/') || !crate::org_login::capability_slug_ok(slug) {
        return None;
    }
    Some(slug)
}

enum LocalPackage {
    Skill { size: u64 },
    Other,
    Broken(&'static str),
}

fn package_for(entries: &[serde_json::Value], slug: &str) -> LocalPackage {
    let Some(entry) = entries
        .iter()
        .find(|entry| entry.get("slug").and_then(serde_json::Value::as_str) == Some(slug))
    else {
        return LocalPackage::Other;
    };
    let Some(install) = entry.get("install") else {
        return LocalPackage::Other;
    };
    let kind = install.get("type").and_then(serde_json::Value::as_str);
    if kind != Some("skill") && kind != Some("plugin") {
        return LocalPackage::Other;
    }
    let digest = install
        .get("digest")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let size = install
        .get("size")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if !crate::capabilities::digest_ok(digest) || size == 0 {
        return LocalPackage::Broken("package is not installable");
    }
    if kind == Some("plugin")
        && install
            .get("packageType")
            .is_some_and(|v| !matches!(v.as_str(), Some("native" | "aix")))
    {
        return LocalPackage::Broken("unsupported plugin package type");
    }
    LocalPackage::Skill { size }
}

fn apply_capability_install(
    stream: &mut TcpStream,
    slug: &str,
    body: &[u8],
    shared: &Shared,
    allow_origin: Option<&str>,
) -> io::Result<()> {
    let Some(token) = shared.member_session.token() else {
        return write_response(
            stream,
            401,
            "Unauthorized",
            "application/json",
            br#"{"error":"unauthorized"}"#,
            allow_origin,
        );
    };
    let put_path = format!("/v1/api/org/capabilities/installations/{slug}");
    let response = crate::org_login::forward_member_api(
        &shared.gateway_origin,
        "PUT",
        &put_path,
        body,
        &token,
    );
    if !(200..300).contains(&response.0) {
        return write_response(
            stream,
            response.0,
            response.1,
            response.2,
            &response.3,
            allow_origin,
        );
    }
    let listed = crate::org_login::forward_member_api(
        &shared.gateway_origin,
        "GET",
        "/v1/api/org/capabilities/installations",
        &[],
        &token,
    );
    if listed.0 != 200 {
        return device_apply_failed(
            stream,
            &format!("capability catalog returned HTTP {}", listed.0),
            allow_origin,
        );
    }
    let entries = match installed_entries(&listed.3) {
        Ok(entries) => entries,
        Err(err) => {
            return device_apply_failed(stream, &err.to_string(), allow_origin);
        }
    };
    match package_for(&entries, slug) {
        LocalPackage::Skill { size } => {
            stream_skill_install(stream, slug, size, &entries, shared, &token, allow_origin)
        }
        LocalPackage::Other => {
            finish_spec_install(stream, &entries, &response, shared, &token, allow_origin)
        }
        LocalPackage::Broken(message) => device_apply_failed(stream, message, allow_origin),
    }
}

fn finish_spec_install(
    stream: &mut TcpStream,
    entries: &[serde_json::Value],
    response: &(u16, &'static str, &'static str, Vec<u8>),
    shared: &Shared,
    token: &str,
    allow_origin: Option<&str>,
) -> io::Result<()> {
    match shared.capabilities.reconcile(entries, |name| {
        crate::org_login::fetch_capability_bundle(&shared.gateway_origin, name, token)
    }) {
        Ok(()) => write_response(
            stream,
            response.0,
            response.1,
            response.2,
            &response.3,
            allow_origin,
        ),
        Err(err) => device_apply_failed(stream, &err.to_string(), allow_origin),
    }
}

fn stream_skill_install(
    stream: &mut TcpStream,
    slug: &str,
    size: u64,
    entries: &[serde_json::Value],
    shared: &Shared,
    token: &str,
    allow_origin: Option<&str>,
) -> io::Result<()> {
    begin_ndjson(stream, allow_origin)?;
    let mut client_gone = false;
    let downloaded = crate::org_login::fetch_capability_bundle_with_progress(
        &shared.gateway_origin,
        slug,
        token,
        Some(size),
        |received, total| {
            if client_gone {
                return;
            }
            let line = format!("{}\n", json!({ "received": received, "total": total }));
            if write_chunk(stream, line.as_bytes()).is_err() {
                client_gone = true;
            }
        },
    );
    let bytes = match downloaded {
        Ok(bytes) => bytes,
        Err(err) => {
            let _ = write_ndjson(
                stream,
                &json!({ "error": device_apply_message(&err.to_string()) }),
            );
            let _ = write_chunk(stream, b"");
            return Ok(());
        }
    };
    let cached_slug = slug.to_owned();
    let applied = shared.capabilities.reconcile(entries, |name| {
        if name == cached_slug {
            return Ok(bytes.clone());
        }
        crate::org_login::fetch_capability_bundle(&shared.gateway_origin, name, token)
    });
    match applied {
        Ok(()) => {
            let _ = write_ndjson(stream, &json!({ "installed": true }));
            let _ = write_chunk(stream, b"");
            Ok(())
        }
        Err(err) => {
            let _ = write_ndjson(
                stream,
                &json!({ "error": device_apply_message(&err.to_string()) }),
            );
            let _ = write_chunk(stream, b"");
            Ok(())
        }
    }
}

fn device_apply_message(message: &str) -> String {
    format!("saved to your account, but this device could not apply it: {message}")
}

fn device_apply_failed(
    stream: &mut TcpStream,
    message: &str,
    allow_origin: Option<&str>,
) -> io::Result<()> {
    let payload = json!({
        "code": 502,
        "message": device_apply_message(message),
        "data": null,
    })
    .to_string();
    write_response(
        stream,
        502,
        "Bad Gateway",
        "application/json",
        payload.as_bytes(),
        allow_origin,
    )
}

fn begin_ndjson(stream: &mut TcpStream, allow_origin: Option<&str>) -> io::Result<()> {
    let mut head = String::from(
        "HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nCache-Control: no-store\r\nConnection: close\r\nTransfer-Encoding: chunked\r\n",
    );
    if let Some(origin) = allow_origin.filter(|origin| is_loopback_origin(origin)) {
        head.push_str("Access-Control-Allow-Origin: ");
        head.push_str(origin);
        head.push_str("\r\nVary: Origin\r\nAccess-Control-Allow-Headers: authorization, content-type, x-skill-filename\r\nAccess-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS\r\n");
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.flush()
}

fn write_ndjson(stream: &mut TcpStream, value: &serde_json::Value) -> io::Result<()> {
    let line = format!("{value}\n");
    write_chunk(stream, line.as_bytes())
}

fn installed_entries(payload: &[u8]) -> io::Result<Vec<serde_json::Value>> {
    let envelope: serde_json::Value = serde_json::from_slice(payload)?;
    let data = envelope.get("data").unwrap_or(&envelope);
    if let Some(items) = data.as_array() {
        return Ok(items.clone());
    }
    data.get("items")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "capability catalog"))
}

/// `/api/im/{org}/stream`: the shell trades its stored member session for a
/// one-time ticket and relays the OS event stream, so the page never holds
/// the `a3s_ms_` token.
fn im_stream_org(path: &str) -> Option<&str> {
    let org = path.strip_prefix("/api/im/")?.strip_suffix("/stream")?;
    crate::org_login::uuid_id_ok(org).then_some(org)
}

fn im_attachment_upload(path: &str) -> bool {
    path.strip_prefix("/api/im/")
        .and_then(|rest| rest.strip_suffix("/attachments"))
        .is_some_and(crate::org_login::uuid_id_ok)
}

fn is_im_attachment(upstream: &str) -> bool {
    upstream
        .strip_prefix("/v1/api/org/im/")
        .and_then(|rest| rest.split_once('/'))
        .is_some_and(|(_, tail)| tail == "attachments" || tail.starts_with("attachments/"))
}

fn query_value<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    query.split('&').find_map(|pair| {
        let (name, value) = pair.split_once('=')?;
        (name == key).then_some(value)
    })
}

fn device_id(query: &str) -> String {
    query
        .split('&')
        .find_map(|pair| pair.strip_prefix("device="))
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        })
        .unwrap_or("studio")
        .to_owned()
}

fn stream_document_changes(
    mut client: TcpStream,
    query: &str,
    allow_origin: Option<&str>,
) -> io::Result<()> {
    let root = match query_value(query, "root") {
        Some(raw) => match crate::local_documents::decode_query_component(raw) {
            Ok(root) => root,
            Err(status) => return reject_document_stream(&mut client, status, allow_origin),
        },
        None => return reject_document_stream(&mut client, 400, allow_origin),
    };
    let mut cursor = match query_value(query, "after") {
        None => 0,
        Some(raw) => match raw.parse::<u64>() {
            Ok(after) => after,
            Err(_) => return reject_document_stream(&mut client, 400, allow_origin),
        },
    };
    let pump = match crate::workspace_watch::ChangePump::open(&root) {
        Ok(pump) => pump,
        Err(status) => return reject_document_stream(&mut client, status, allow_origin),
    };

    client.set_write_timeout(Some(Duration::from_secs(10)))?;
    let mut head = String::from(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nConnection: close\r\nTransfer-Encoding: chunked\r\n",
    );
    if let Some(origin) = allow_origin.filter(|origin| is_loopback_origin(origin)) {
        head.push_str("Access-Control-Allow-Origin: ");
        head.push_str(origin);
        head.push_str("\r\nVary: Origin\r\n");
    }
    head.push_str("\r\n");
    client.write_all(head.as_bytes())?;
    client.flush()?;

    loop {
        let frame = match pump.poll(cursor, Duration::from_secs(15)) {
            Ok(Some(value)) => {
                let revision = value
                    .get("revision")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(cursor);
                cursor = revision;
                format!("event: workspace.files.changed\nid: {revision}\ndata: {value}\n\n")
            }
            Ok(None) => ": keep-alive\n\n".to_owned(),
            Err(_) => break,
        };
        if write_chunk(&mut client, frame.as_bytes()).is_err() {
            break;
        }
    }
    Ok(())
}

fn reject_document_stream(
    stream: &mut TcpStream,
    status: u16,
    allow_origin: Option<&str>,
) -> io::Result<()> {
    let (reason, body) = match status {
        400 => ("Bad Request", "{\"error\":\"bad_request\"}".to_owned()),
        404 => ("Not Found", "{\"error\":\"not_found\"}".to_owned()),
        _ => ("Error", format!("{{\"error\":\"status_{status}\"}}")),
    };
    write_response(
        stream,
        status,
        reason,
        "application/json",
        body.as_bytes(),
        allow_origin,
    )
}

fn write_chunk(stream: &mut TcpStream, payload: &[u8]) -> io::Result<()> {
    write!(stream, "{:x}\r\n", payload.len())?;
    stream.write_all(payload)?;
    stream.write_all(b"\r\n")?;
    stream.flush()
}

fn stream_im(
    mut client: TcpStream,
    org: &str,
    query: &str,
    shared: &Shared,
    allow_origin: Option<&str>,
) -> io::Result<()> {
    let fail = |client: &mut TcpStream, status: u16, message: &str| {
        let body = json!({ "code": status, "message": message, "data": null }).to_string();
        write_response(
            client,
            status,
            "Error",
            "application/json",
            body.as_bytes(),
            allow_origin,
        )
    };
    let Some(token) = shared.member_session.token() else {
        return fail(&mut client, 401, "member session missing");
    };
    let ticket_body = json!({ "device_id": device_id(query) }).to_string();
    let (status, _, _, payload) = crate::org_login::forward_member_api(
        &shared.org_origin,
        "POST",
        &format!("/v1/api/org/im/{org}/stream-tickets"),
        ticket_body.as_bytes(),
        &token,
    );
    if !(200..300).contains(&status) {
        return write_response(
            &mut client,
            status,
            "Error",
            "application/json",
            &payload,
            allow_origin,
        );
    }
    let ticket = serde_json::from_slice::<serde_json::Value>(&payload)
        .ok()
        .and_then(|value| {
            value
                .pointer("/data/ticket")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .filter(|ticket| {
            !ticket.is_empty()
                && ticket.len() <= 128
                && ticket
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        });
    let Some(ticket) = ticket else {
        return fail(&mut client, 502, "stream ticket missing");
    };
    let upstream = match crate::org_login::open_event_stream(
        &shared.org_origin,
        &format!("/v1/api/org/im/{org}/stream?ticket={ticket}"),
    ) {
        Ok(upstream) => upstream,
        Err(_) => return fail(&mut client, 502, "event stream unavailable"),
    };
    if upstream.status != 200 {
        return fail(&mut client, upstream.status, "event stream refused");
    }
    let mut source = upstream.stream;
    // Heartbeats arrive every 25s; a silent minute means the stream is gone.
    source.set_read_timeout(Some(Duration::from_secs(60)))?;
    client.set_write_timeout(Some(Duration::from_secs(10)))?;
    let mut head = String::from(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nConnection: close\r\n",
    );
    if upstream.chunked {
        head.push_str("Transfer-Encoding: chunked\r\n");
    }
    if let Some(origin) = allow_origin.filter(|origin| is_loopback_origin(origin)) {
        head.push_str("Access-Control-Allow-Origin: ");
        head.push_str(origin);
        head.push_str("\r\nVary: Origin\r\n");
    }
    head.push_str("\r\n");
    client.write_all(head.as_bytes())?;
    client.write_all(&upstream.prelude)?;
    client.flush()?;
    let mut buf = [0u8; 8192];
    loop {
        let read = match source.read(&mut buf) {
            Ok(0) => return Ok(()),
            Ok(read) => read,
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return Ok(()),
        };
        if client.write_all(&buf[..read]).is_err() || client.flush().is_err() {
            return Ok(());
        }
    }
}

const PRESENTON_UI_LIMIT: usize = 16 * 1024 * 1024;
const PRESENTON_VIEW_DOCUMENT: &str =
    "b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1";

fn presenton_workbench_root() -> Option<std::path::PathBuf> {
    let candidate = std::env::var("A3S_PRESENTON_WORKBENCH")
        .ok()
        .filter(|path| !path.trim().is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../apps/use-registry/packages/presenton/ui/workbench"
            ))
        });
    let root = candidate.canonicalize().ok()?;
    if root.join("index.html").is_file() && root.join("app.js").is_file() {
        Some(root)
    } else {
        None
    }
}

fn presenton_ui_mime(path: &std::path::Path) -> Option<&'static str> {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("html") => Some("text/html; charset=utf-8"),
        Some("js") => Some("text/javascript; charset=utf-8"),
        Some("css") => Some("text/css; charset=utf-8"),
        Some("svg") => Some("image/svg+xml"),
        Some("png") => Some("image/png"),
        Some("jpg" | "jpeg") => Some("image/jpeg"),
        Some("webp") => Some("image/webp"),
        Some("woff2") => Some("font/woff2"),
        _ => None,
    }
}

/// Files of the vendored Presenton workbench. Anything outside that directory is refused.
fn presenton_ui_file(relative: &str) -> Option<(&'static str, Vec<u8>)> {
    if relative.is_empty()
        || relative.contains('\0')
        || relative.starts_with('/')
        || relative
            .split(['/', '\\'])
            .any(|part| part.is_empty() || part == "..")
    {
        return None;
    }
    let root = presenton_workbench_root()?;
    let path = root.join(relative).canonicalize().ok()?;
    if !path.starts_with(&root) || !path.is_file() {
        return None;
    }
    let mime = presenton_ui_mime(&path)?;
    let bytes = std::fs::read(&path).ok()?;
    if bytes.is_empty() || bytes.len() > PRESENTON_UI_LIMIT {
        return None;
    }
    Some((mime, bytes))
}

fn json_for_html_script(value: &serde_json::Value) -> String {
    serde_json::to_string(value)
        .unwrap_or_else(|_| "null".to_owned())
        .replace('<', "\\u003c")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

/// The view deck the Presenton editor restores. Flattened heading text is not a canvas.
fn presenton_view_deck(value: &serde_json::Value) -> Option<&serde_json::Value> {
    let deck = value.get("deck")?;
    let id = deck.get("id")?.as_str()?.trim();
    if id.is_empty() || !id.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-') {
        return None;
    }
    if deck
        .get("template")
        .and_then(serde_json::Value::as_str)
        .is_none_or(|template| template.trim().is_empty())
    {
        return None;
    }
    let slides = deck.get("slides")?.as_array()?;
    if slides.len() < 2 {
        return None;
    }
    for slide in slides {
        if slide
            .get("group")
            .and_then(serde_json::Value::as_str)
            .is_none_or(|group| group.trim().is_empty())
            || slide
                .get("layout")
                .and_then(serde_json::Value::as_str)
                .is_none_or(|layout| layout.trim().is_empty())
            || !slide
                .get("content")
                .is_some_and(serde_json::Value::is_object)
        {
            return None;
        }
    }
    Some(deck)
}

fn presentation_host_page(title: &str, deck: &serde_json::Value) -> String {
    let mut page = String::new();
    page.push_str("<!doctype html><html lang=\"zh-CN\"><head><meta charset=\"utf-8\"><title>");
    page.push_str(&escape_html(title));
    page.push_str(
        "</title><style>html,body{margin:0;height:100%;background:#f4f4f5}body{display:flex;flex-direction:column}.chrome{height:38px;flex:none}iframe{flex:1 1 auto;width:100%;min-height:0;border:0}</style></head><body><div class=\"chrome\" data-tauri-drag-region=\"true\"></div><iframe id=\"view\" sandbox=\"allow-scripts\" src=\"/presenton-ui/index.html#aix-document=",
    );
    page.push_str(PRESENTON_VIEW_DOCUMENT);
    page.push_str("\"></iframe><script type=\"application/json\" id=\"presenton-deck\">");
    page.push_str(&json_for_html_script(deck));
    page.push_str("</script><script>\nconst doc=\"");
    page.push_str(PRESENTON_VIEW_DOCUMENT);
    page.push_str(
        "\";\nconst deck=JSON.parse(document.getElementById(\"presenton-deck\").textContent);\naddEventListener(\"message\",async(event)=>{\n  const data=event.data;\n  if(!data||data.protocolVersion!==1||data.type!==\"aix.document.ready\"||data.documentId!==doc)return;\n  const port=event.ports&&event.ports[0];\n  if(!port)return;\n  port.onmessage=(message)=>{\n    const body=message.data;\n    if(!body||body.type!==\"aix.view.store\")return;\n    port.postMessage({protocolVersion:1,type:\"aix.view.stored\",id:body.id});\n  };\n  port.start();\n  const payload=JSON.stringify({route:{pathname:\"/presentation\",search:\"?id=\"+deck.id},deck});\n  const bytes=new Uint8Array(await new Response(new Blob([payload]).stream().pipeThrough(new CompressionStream(\"gzip\"))).arrayBuffer());\n  let binary=\"\";\n  for(let offset=0;offset<bytes.length;offset+=32768)binary+=String.fromCharCode(...bytes.subarray(offset,offset+32768));\n  port.postMessage({protocolVersion:1,type:\"aix.view.connect\",stateJSON:JSON.stringify({v:2,gz:btoa(binary)}),environment:{theme:\"light\",locale:\"zh-CN\",direction:\"ltr\",tokens:{}}});\n});\n</script></body></html>",
    );
    page
}

/// One launch can show a finished deck without a member session.
/// An unset path, a missing file, or a deck with fewer than two complete slides is not found.
fn presentation_deck_json(path: Option<&str>) -> Result<String, u16> {
    let Some(path) = path.map(str::trim).filter(|path| !path.is_empty()) else {
        return Err(404);
    };
    let bytes = std::fs::read(path).map_err(|_| 404u16)?;
    if bytes.is_empty() || bytes.len() > 1_000_000 {
        return Err(404);
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| 404u16)?;
    let slides = value
        .get("slides")
        .and_then(serde_json::Value::as_array)
        .ok_or(404u16)?;
    if slides.len() < 2 {
        return Err(404);
    }
    for slide in slides {
        let heading = slide
            .get("heading")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .trim();
        let body = slide
            .get("body")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .trim();
        if heading.is_empty() || body.is_empty() {
            return Err(404);
        }
    }
    String::from_utf8(bytes).map_err(|_| 404u16)
}

/// The Presenton editor for one finished deck. This document is not the login shell.
pub(crate) fn presentation_deck_page(path: Option<&str>) -> Option<String> {
    let raw = presentation_deck_json(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let deck = presenton_view_deck(&value)?;
    if presenton_workbench_root().is_none() {
        return None;
    }
    let title = value
        .get("title")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .trim();
    Some(presentation_host_page(title, deck))
}

fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

fn bootstrap(shared: &Shared) -> Result<String, u16> {
    let state = shared.state.lock().expect("studio state");
    Ok(json!({
        "sessionId": state.session_id,
        "tenant": shared.tenant,
        "name": shared.name,
        "product": shared.product,
        "orgOrigin": shared.org_origin,
    })
    .to_string())
}

fn create_conversation(shared: &Shared, body: &[u8]) -> Result<String, u16> {
    let request: CreateConversation = serde_json::from_slice(body).map_err(|_| 400u16)?;
    let title = request.title.trim();
    if title.is_empty() || title.chars().count() > 1024 {
        return Err(400);
    }
    let id = hex_encode(&random_bytes::<16>().map_err(|_| 500u16)?);
    let mut state = shared.state.lock().expect("studio state");
    state.conversations.insert(
        id.clone(),
        Conversation {
            title: title.to_owned(),
            cancelled: false,
            stream_id: None,
        },
    );
    Ok(json!({ "id": id, "title": title }).to_string())
}

fn list_conversations(shared: &Shared) -> Result<String, u16> {
    let state = shared.state.lock().expect("studio state");
    let conversations: Vec<_> = state
        .conversations
        .iter()
        .map(|(id, conversation)| json!({ "id": id, "title": conversation.title }))
        .collect();
    Ok(json!({ "conversations": conversations }).to_string())
}

fn get_conversation(shared: &Shared, id: &str) -> Result<String, u16> {
    let state = shared.state.lock().expect("studio state");
    let conversation = state.conversations.get(id).ok_or(404u16)?;
    Ok(json!({ "id": id, "title": conversation.title }).to_string())
}

fn open_stream(shared: &Shared, id: &str) -> Result<String, u16> {
    let mut state = shared.state.lock().expect("studio state");
    let conversation = state.conversations.get_mut(id).ok_or(404u16)?;
    if conversation.stream_id.is_none() {
        conversation.stream_id = Some(hex_encode(&random_bytes::<16>().map_err(|_| 500u16)?));
    }
    let stream_id = conversation.stream_id.clone().ok_or(500u16)?;
    Ok(json!({ "streamId": stream_id, "status": "open" }).to_string())
}

fn read_stream(shared: &Shared, id: &str) -> Result<String, u16> {
    let mut state = shared.state.lock().expect("studio state");
    if !state.conversations.contains_key(id) {
        return Err(404);
    }
    let started = Instant::now();
    loop {
        if state
            .conversations
            .get(id)
            .is_some_and(|conversation| conversation.cancelled)
        {
            return Ok(json!({ "event": "cancelled" }).to_string());
        }
        if shared.shutdown.load(Ordering::SeqCst) {
            return Ok(json!({ "event": "done" }).to_string());
        }
        let remaining = STREAM_WAIT.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return Ok(json!({ "event": "done" }).to_string());
        }
        let (next, timeout) = shared
            .wake
            .wait_timeout(state, remaining)
            .expect("studio state");
        state = next;
        if !state.conversations.contains_key(id) {
            return Err(404);
        }
        if timeout.timed_out() && started.elapsed() >= STREAM_WAIT {
            return Ok(json!({ "event": "done" }).to_string());
        }
    }
}

fn cancel_stream(shared: &Shared, id: &str) -> Result<String, u16> {
    let mut state = shared.state.lock().expect("studio state");
    let conversation = state.conversations.get_mut(id).ok_or(404u16)?;
    conversation.cancelled = true;
    shared.wake.notify_all();
    Ok(json!({ "ok": true }).to_string())
}

#[derive(Deserialize)]
struct CreateConversation {
    title: String,
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&value[index + 1..index + 3], 16) {
                out.push(byte);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn sanitize_upload_name(raw: &str) -> String {
    let decoded = percent_decode(raw);
    let base = decoded.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = base
        .chars()
        .filter(|ch| *ch != '\0' && *ch != '/' && *ch != '\\')
        .take(80)
        .collect();
    let trimmed = cleaned.trim().trim_start_matches('.');
    if trimmed.is_empty() {
        "file.bin".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn save_composer_file(name: &str, body: &[u8]) -> Result<String, u16> {
    if body.len() > COMPOSER_FILE_LIMIT {
        return Err(413);
    }
    let dir = std::env::temp_dir().join("a3s-studio-composer");
    std::fs::create_dir_all(&dir).map_err(|_| 500u16)?;
    let id = hex_encode(&random_bytes::<8>().map_err(|_| 500u16)?);
    let safe_name = sanitize_upload_name(name);
    let path = dir.join(format!("{id}_{safe_name}"));
    std::fs::write(&path, body).map_err(|_| 500u16)?;
    Ok(json!({
        "path": path.to_string_lossy(),
        "size": body.len(),
        "name": safe_name,
    })
    .to_string())
}

/// Loopback `/api/*` and the platform prefix `/v1/api/*` both require the shell bearer.
fn shell_api(path: &str) -> bool {
    path.starts_with("/api/") || path.starts_with("/v1/api/")
}

fn authorized(head: &str, token: &str) -> bool {
    let Some(value) = header_value(head, "authorization") else {
        return false;
    };
    let Some(presented) = value
        .strip_prefix("Bearer ")
        .or_else(|| value.strip_prefix("bearer "))
    else {
        return false;
    };
    tokens_match(presented.trim(), token)
}

pub(crate) fn tokens_match(presented: &str, expected: &str) -> bool {
    let presented = presented.as_bytes();
    let expected = expected.as_bytes();
    if presented.len() != expected.len() {
        return false;
    }
    let mut diff = 0u8;
    for (left, right) in presented.iter().zip(expected.iter()) {
        diff |= left ^ right;
    }
    diff == 0
}

fn is_loopback_origin(origin: &str) -> bool {
    let Some(port) = origin.strip_prefix("http://127.0.0.1:") else {
        return false;
    };
    matches!(port.parse::<u16>(), Ok(value) if value != 0)
}

fn is_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn request_target(head: &str) -> Option<(&str, &str, &str)> {
    let line = head.split("\r\n").next()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?;
    let target = parts.next()?;
    let (path, query) = match target.split_once('?') {
        Some((path, query)) => (path, query),
        None => (target, ""),
    };
    if !path.starts_with('/') {
        return None;
    }
    Some((method, path, query))
}

fn header_value<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    for line in head.split("\r\n").skip(1) {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if key.eq_ignore_ascii_case(name) {
            return Some(value.trim());
        }
    }
    None
}

fn read_headers(stream: &mut TcpStream) -> io::Result<(String, Vec<u8>)> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    loop {
        if buf.len() > MAX_HEADER_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "headers too large",
            ));
        }
        let read = match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => n,
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(err),
        };
        buf.extend_from_slice(&tmp[..read]);
        if let Some(end) = find_subsequence(&buf, b"\r\n\r\n") {
            let rest = buf[end + 4..].to_vec();
            let head = String::from_utf8_lossy(&buf[..end]).into_owned();
            return Ok((head, rest));
        }
    }
    Err(io::Error::new(io::ErrorKind::UnexpectedEof, "headers"))
}

fn read_body(
    stream: &mut TcpStream,
    prelude: &[u8],
    len: usize,
    limit: usize,
) -> io::Result<Vec<u8>> {
    if len > limit {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "body too large"));
    }
    let mut body = Vec::with_capacity(len);
    body.extend_from_slice(&prelude[..prelude.len().min(len)]);
    while body.len() < len {
        let mut tmp = [0u8; 8192];
        let read = stream.read(&mut tmp)?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&tmp[..read]);
    }
    if body.len() < len {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "body"));
    }
    body.truncate(len);
    Ok(body)
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// One call from a live AIX view to an edge agent of its own package. The
/// member bearer stays on the host; the box only receives model responses.
fn aix_agent_turn(
    shared: &Shared,
    token: &str,
    agent: &str,
    body: &[u8],
) -> (u16, &'static str, &'static str, Vec<u8>) {
    use crate::agent_plane::aix_episodes::{EpisodeError, ModelAccess, ViewTurn};
    let refused = |status, reason, error: &str| {
        (
            status,
            reason,
            "application/json",
            json!({ "error": error }).to_string().into_bytes(),
        )
    };
    let bound = shared.aix_agents.as_ref().and_then(|agents| {
        if !crate::org_login::capability_slug_ok(agent) {
            return None;
        }
        let owner = plugin_view_owner(shared)?;
        let bound = shared
            .capabilities
            .plugin_view_agent(&owner, token, agent)
            .ok()?;
        Some((agents, bound))
    });
    let Some((agents, bound)) = bound else {
        return refused(404, "Not Found", "not_found");
    };
    let Some(turn) = ViewTurn::parse(body) else {
        return refused(400, "Bad Request", "invalid_request");
    };
    let model = ModelAccess {
        origin: shared.gateway_origin.clone(),
        bearer: shared.member_session.token(),
    };
    match agents.turn(token, agent, &bound, &turn, &model) {
        Ok(text) => (
            200,
            "OK",
            "application/json",
            json!({ "text": text }).to_string().into_bytes(),
        ),
        Err(EpisodeError::Busy) => refused(409, "Conflict", "busy"),
        Err(EpisodeError::Capacity) => refused(429, "Too Many Requests", "capacity"),
        Err(EpisodeError::Failed(message)) => {
            eprintln!("studio: AIX view agent {agent} failed: {message}");
            refused(502, "Bad Gateway", "agent_failed")
        }
    }
}

const AIX_AGENT_REAP_INTERVAL: Duration = Duration::from_secs(10);

/// Ends warm boxes whose view closed, expired, changed owner, or was upgraded.
fn reap_aix_agents(shared: std::sync::Weak<Shared>) {
    loop {
        thread::sleep(AIX_AGENT_REAP_INTERVAL);
        let Some(shared) = shared.upgrade() else {
            return;
        };
        if shared.shutdown.load(Ordering::SeqCst) {
            return;
        }
        let Some(agents) = &shared.aix_agents else {
            return;
        };
        if !agents.has_episodes() {
            continue;
        }
        let owner = plugin_view_owner(&shared);
        agents.reap(|token, agent| {
            owner.as_deref().is_some_and(|owner| {
                shared
                    .capabilities
                    .plugin_view_agent(owner, token, agent)
                    .is_ok()
            })
        });
    }
}

fn plugin_view_owner(shared: &Shared) -> Option<String> {
    shared
        .member_session
        .token()
        .map(|token| format!("{token}\0{}", current_org_scope(shared)))
}

/// A view may compile WebAssembly bytes its own scripts carry. It still fetches
/// nothing: no network, no worker, no Studio API.
const PLUGIN_VIEW_CSP: &str = "sandbox allow-scripts; default-src 'none'; script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'none'; worker-src 'none'; base-uri 'none'; form-action 'none'; frame-src 'none'; object-src 'none'";

fn write_plugin_view_response(
    stream: &mut TcpStream,
    result: io::Result<(&str, Vec<u8>)>,
) -> io::Result<()> {
    let (status, reason, content_type, bytes) = match result {
        Ok((content_type, bytes)) => (200, "OK", content_type, bytes),
        Err(_) => (
            404,
            "Not Found",
            "application/json",
            br#"{"error":"not_found"}"#.to_vec(),
        ),
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nAccess-Control-Allow-Origin: *\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: {PLUGIN_VIEW_CSP}\r\n\r\n",
        bytes.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(&bytes)?;
    stream.flush()
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    content_type: &str,
    body: &[u8],
    allow_origin: Option<&str>,
) -> io::Result<()> {
    write_response_with_etag(
        stream,
        status,
        reason,
        content_type,
        body,
        allow_origin,
        None,
    )
}

fn write_response_with_etag(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    content_type: &str,
    body: &[u8],
    allow_origin: Option<&str>,
    revision: Option<&str>,
) -> io::Result<()> {
    let mut head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n",
        body.len()
    );
    if let Some(origin) = allow_origin {
        if !is_loopback_origin(origin) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "refusing a non-loopback allow-origin",
            ));
        }
        head.push_str("Access-Control-Allow-Origin: ");
        head.push_str(origin);
        head.push_str("\r\nVary: Origin\r\nAccess-Control-Allow-Headers: authorization, content-type, x-skill-filename, if-match\r\nAccess-Control-Expose-Headers: etag\r\nAccess-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS\r\n");
    }
    if let Some(revision) = revision {
        head.push_str(&format!("ETag: \"{revision}\"\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()?;
    Ok(())
}

fn random_bytes<const N: usize>() -> io::Result<[u8; N]> {
    let mut bytes = [0u8; N];
    getrandom::getrandom(&mut bytes)
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err.to_string()))?;
    Ok(bytes)
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod presentation_deck {
    use super::{presentation_deck_json, presentation_deck_page};

    #[test]
    fn an_unset_path_is_not_a_deck() {
        assert_eq!(presentation_deck_json(None), Err(404));
        assert_eq!(presentation_deck_json(Some("  ")), Err(404));
    }

    #[test]
    fn a_finished_deck_file_is_returned_unchanged() {
        let path =
            std::env::temp_dir().join(format!("a3s-presentation-deck-{}.json", std::process::id()));
        let body = r#"{"id":"deck-1","title":"量子计算入门","slides":[{"heading":"量子计算入门","body":"叠加与纠缠"},{"heading":"第 2 部分","body":"测量会改变状态"}]}"#;
        std::fs::write(&path, body).unwrap();
        let read = presentation_deck_json(Some(path.to_str().unwrap()));
        let _ = std::fs::remove_file(&path);
        assert_eq!(read.as_deref(), Ok(body));
    }

    fn five_slide_deck() -> String {
        r#"{"title":"量子计算入门","slides":[{"heading":"量子计算入门","body":"叠加与纠缠"},{"heading":"第 2 部分","body":"测量会改变状态"},{"heading":"第 3 部分","body":"第三页正文"},{"heading":"第 4 部分","body":"第四页正文"},{"heading":"第 5 部分","body":"第五页正文"}],"deck":{"id":"deck-1","template":"general","title":"量子计算入门","slides":[{"id":"s1","group":"general","layout":"title","content":{"text":"量子计算入门"}},{"id":"s2","group":"general","layout":"body","content":{"text":"第 2 部分"}},{"id":"s3","group":"general","layout":"body","content":{"text":"第三页正文"}},{"id":"s4","group":"general","layout":"body","content":{"text":"第 4 部分"}},{"id":"s5","group":"general","layout":"body","content":{"text":"第五页正文"}}]}}"#.to_owned()
    }

    #[test]
    fn a_finished_deck_opens_the_presenton_editor_and_not_the_login_shell() {
        let path = std::env::temp_dir().join(format!(
            "a3s-presentation-slides-{}.json",
            std::process::id()
        ));
        std::fs::write(&path, five_slide_deck()).unwrap();
        let page = presentation_deck_page(path.to_str()).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(page.contains("/presenton-ui/index.html#aix-document="));
        assert!(page.contains("aix.view.connect"));
        assert!(page.contains("sandbox=\"allow-scripts\""));
        assert!(page.contains("量子计算入门"));
        assert!(page.contains("第 2 部分"));
        assert!(page.contains("第 4 部分"));
        assert!(page.contains("第五页正文"));
        assert!(!page.contains("data-presentation-slide="));
        assert!(!page.contains("line-clamp"));
        assert!(!page.contains("pre-wrap"));
        assert!(!page.contains("box-shadow"));
        assert!(!page.contains("anywhere"));
        assert!(!page.contains("id=\"login\""));
        assert!(!page.contains("手机号登录"));
        assert!(!page.contains("studioClient"));
        assert_eq!(super::PRESENTON_VIEW_DOCUMENT.len(), 64);
        let served = super::presenton_ui_file("index.html").unwrap();
        assert!(served.0.starts_with("text/html"));
        assert!(super::presenton_ui_file("../Cargo.toml").is_none());
        assert!(super::presenton_ui_file("index.html/../../Cargo.toml").is_none());
    }

    #[test]
    fn a_text_dump_without_a_view_deck_is_not_a_page() {
        let path =
            std::env::temp_dir().join(format!("a3s-presentation-dump-{}.json", std::process::id()));
        std::fs::write(
            &path,
            r#"{"title":"量子计算入门","slides":[{"heading":"量子计算入门","body":"叠加与纠缠"},{"heading":"第 2 部分","body":"测量会改变状态"}]}"#,
        )
        .unwrap();
        let page = presentation_deck_page(path.to_str());
        let _ = std::fs::remove_file(&path);
        assert!(page.is_none());
    }

    #[test]
    fn a_heading_without_a_body_is_refused() {
        let path = std::env::temp_dir().join(format!(
            "a3s-presentation-deck-empty-{}.json",
            std::process::id()
        ));
        std::fs::write(
            &path,
            r#"{"slides":[{"heading":"只有标题","body":"  "},{"heading":"第二页","body":"有正文"}]}"#,
        )
        .unwrap();
        let read = presentation_deck_json(Some(path.to_str().unwrap()));
        let _ = std::fs::remove_file(&path);
        assert_eq!(read, Err(404));
    }
}

#[cfg(test)]
#[path = "authority_workspace_snapshot_tests.rs"]
mod workspace_snapshot_tests;

#[cfg(test)]
#[path = "authority_review_observation_tests.rs"]
mod review_observation_tests;

#[cfg(test)]
mod tests {
    use std::io::Read;
    use std::net::{Ipv4Addr, SocketAddr, TcpStream};
    use std::process::Command;
    use std::time::Duration;

    use super::{PLUGIN_VIEW_CSP, StudioAuthority};

    fn exchange(port: u16, request: &str, timeout: Duration) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        stream.set_read_timeout(Some(timeout)).unwrap();
        stream.set_write_timeout(Some(timeout)).unwrap();
        std::io::Write::write_all(&mut stream, request.as_bytes()).unwrap();
        let mut buf = Vec::new();
        let mut tmp = [0u8; 2048];
        loop {
            match stream.read(&mut tmp) {
                Ok(0) => break,
                Ok(n) => buf.extend_from_slice(&tmp[..n]),
                Err(err)
                    if err.kind() == std::io::ErrorKind::WouldBlock
                        || err.kind() == std::io::ErrorKind::TimedOut =>
                {
                    break;
                }
                Err(err) => panic!("{err}"),
            }
        }
        String::from_utf8_lossy(&buf).into_owned()
    }

    pub(super) fn split_response(raw: &str) -> (u16, String, String) {
        let (head, body) = raw.split_once("\r\n\r\n").unwrap_or((raw, ""));
        let status = head
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        (status, head.to_ascii_lowercase(), body.to_owned())
    }

    pub(super) fn request(
        port: u16,
        method: &str,
        path: &str,
        token: &str,
        origin: Option<&str>,
        body: &str,
    ) -> String {
        let origin_line = origin
            .map(|value| format!("Origin: {value}\r\n"))
            .unwrap_or_default();
        let auth = if token.is_empty() {
            String::new()
        } else {
            format!("Authorization: Bearer {token}\r\n")
        };
        let raw = format!(
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n{auth}{origin_line}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        exchange(port, &raw, Duration::from_secs(5))
    }

    #[test]
    fn each_tenant_primary_keeps_its_own_stable_loopback_ports() {
        let tenants = ["toyou", "a3s", "kense", "towork-cua"];
        let mut seen = std::collections::HashSet::new();
        for slug in tenants {
            let authority = super::authority_listen_port(slug, None);
            let plane = super::agent_plane_listen_port(slug, None);
            assert_eq!(authority, super::authority_listen_port(slug, None));
            assert_eq!(plane, authority + 1);
            assert!((59850..59850 + 128).contains(&authority));
            assert!(
                seen.insert(authority),
                "{slug} shares a port with another tenant"
            );
            assert_eq!(super::authority_listen_port(slug, Some("fork1")), 0);
            assert_eq!(super::agent_plane_listen_port(slug, Some("fork1")), 0);
        }
    }

    // Catch a view policy that lets a document compile JavaScript or reach the network.
    #[test]
    fn a_view_compiles_wasm_but_reaches_nothing() {
        let policy = PLUGIN_VIEW_CSP;
        assert!(policy.starts_with("sandbox allow-scripts; default-src 'none'; "));
        assert!(policy.contains("script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval';"));
        assert!(!policy.contains(" 'unsafe-eval'"));
        assert!(policy.contains("connect-src 'none';"));
        assert!(policy.contains("worker-src 'none';"));
        assert!(!policy.contains("allow-same-origin"));
        assert!(!policy.contains("http"));
    }

    // Catch a view agent call reaching a box without a live, bound view session.
    #[test]
    fn a_view_agent_turn_without_a_bound_view_is_not_found() {
        let authority = StudioAuthority::start().unwrap();
        let port = authority.address().port();
        let token = "0".repeat(64);
        for agent in ["lab", "Lab", "x".repeat(65).as_str()] {
            let raw = request(
                port,
                "POST",
                &format!("/api/aix/views/sessions/{token}/agents/{agent}/turns"),
                authority.token(),
                None,
                r#"{"prompt":"go"}"#,
            );
            let (status, _, body) = split_response(&raw);
            assert_eq!(status, 404, "{raw}");
            assert!(body.contains("not_found"), "{raw}");
        }
        let raw = request(
            port,
            "POST",
            &format!("/api/aix/views/sessions/{token}/agents/lab/turns"),
            "wrong",
            None,
            r#"{"prompt":"go"}"#,
        );
        assert_eq!(split_response(&raw).0, 401, "{raw}");
    }

    #[test]
    fn a_busy_preferred_port_still_opens_a_loopback_listener() {
        let holder = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let busy = holder.local_addr().unwrap().port();
        let listener = super::bind_loopback(busy).unwrap();
        let address = listener.local_addr().unwrap();
        assert_eq!(address.ip(), std::net::IpAddr::V4(Ipv4Addr::LOCALHOST));
        assert_ne!(address.port(), busy);
        assert_ne!(address.port(), 0);
    }

    #[test]
    fn binds_loopback_and_rejects_a_bad_bearer_before_the_body() {
        let authority = StudioAuthority::start().unwrap();
        let address = authority.address();
        assert_eq!(address.ip(), std::net::IpAddr::V4(Ipv4Addr::LOCALHOST));
        assert_ne!(address.port(), 0);
        assert!(authority.api_base_url().starts_with("http://127.0.0.1:"));
        assert_eq!(authority.token().len(), 64);
        assert!(
            authority
                .token()
                .chars()
                .all(|character| character.is_ascii_hexdigit())
        );

        let raw = request(
            address.port(),
            "POST",
            "/api/conversations",
            "nope",
            Some("http://127.0.0.1:9"),
            "not-json",
        );
        let (status, head, body) = split_response(&raw);
        assert_eq!(status, 401, "{raw}");
        assert!(!head.contains("access-control-allow-origin: *"));
        assert!(!head.contains("set-cookie"));
        assert!(head.contains("access-control-allow-origin: http://127.0.0.1:9"));
        assert!(!body.contains("not-json"));
        assert!(body.contains("unauthorized"));
    }

    #[test]
    fn open_external_refuses_a_non_http_page() {
        let authority = StudioAuthority::start().unwrap();
        let raw = request(
            authority.address().port(),
            "POST",
            "/api/open-external",
            authority.token(),
            Some("http://127.0.0.1:4318"),
            r#"{"url":"javascript:alert(1)"}"#,
        );
        let (status, _, body) = split_response(&raw);
        assert_eq!(status, 400, "{raw}");
        assert!(body.contains("bad_request"));
    }

    #[test]
    fn lists_local_skills_only_with_the_bearer() {
        let authority = StudioAuthority::start().unwrap();
        let port = authority.address().port();
        let origin = Some("http://127.0.0.1:9");

        let raw = request(port, "GET", "/api/local-skills", "", origin, "");
        assert_eq!(split_response(&raw).0, 401, "{raw}");

        let raw = request(
            port,
            "GET",
            "/api/local-skills",
            authority.token(),
            origin,
            "",
        );
        let (status, _, body) = split_response(&raw);
        assert_eq!(status, 200, "{raw}");
        assert_eq!(body, r#"{"skills":[]}"#);
    }

    #[test]
    fn rejects_a_non_loopback_origin_without_a_wildcard() {
        let authority = StudioAuthority::start().unwrap();
        let port = authority.address().port();
        let raw = request(
            port,
            "GET",
            "/api/bootstrap",
            authority.token(),
            Some("https://example.com"),
            "",
        );
        let (status, head, _) = split_response(&raw);
        assert_eq!(status, 403, "{raw}");
        assert!(!head.contains("access-control-allow-origin"));
        assert!(!head.contains("set-cookie"));
    }

    #[test]
    fn returns_the_stored_session_for_the_matching_bearer() {
        let authority = StudioAuthority::start().unwrap();
        let port = authority.address().port();
        let origin = format!("http://127.0.0.1:{port}");
        let first = request(
            port,
            "GET",
            "/api/bootstrap",
            authority.token(),
            Some(&origin),
            "",
        );
        let second = request(
            port,
            "GET",
            "/api/bootstrap",
            authority.token(),
            Some(&origin),
            "",
        );
        let (status, head, body) = split_response(&first);
        assert_eq!(status, 200, "{first}");
        assert!(head.contains(&format!(
            "access-control-allow-origin: {}",
            origin.to_ascii_lowercase()
        )));
        assert!(!head.contains("access-control-allow-origin: *"));
        assert!(!head.contains("set-cookie"));
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            parsed["sessionId"].as_str(),
            Some(authority.session_id().as_str())
        );
        assert_eq!(parsed["name"].as_str(), Some(authority.name()));
        assert_eq!(parsed["tenant"].as_str(), Some(authority.slug()));
        assert_eq!(parsed["product"].as_str(), Some(authority.product()));
        assert!(!parsed["name"].as_str().unwrap_or("").is_empty());
        assert!(!parsed["product"].as_str().unwrap_or("").is_empty());
        let again: serde_json::Value = serde_json::from_str(&split_response(&second).2).unwrap();
        assert_eq!(again["sessionId"], parsed["sessionId"]);
    }

    #[test]
    fn two_authorities_mint_different_sessions() {
        let first = StudioAuthority::start().unwrap();
        let second = StudioAuthority::start().unwrap();
        assert_ne!(first.session_id(), second.session_id());
        assert_ne!(first.token(), second.token());
        assert_ne!(first.address().port(), second.address().port());
    }

    #[test]
    fn stores_a_conversation_and_cancels_its_stream() {
        let authority = StudioAuthority::start().unwrap();
        let port = authority.address().port();
        let origin = format!("http://127.0.0.1:{port}");
        let title = format!("probe-{}", authority.session_id());
        let created = request(
            port,
            "POST",
            "/api/conversations",
            authority.token(),
            Some(&origin),
            &format!("{{\"title\":\"{title}\"}}"),
        );
        let (status, _, body) = split_response(&created);
        assert_eq!(status, 200, "{created}");
        let created_json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(created_json["title"].as_str(), Some(title.as_str()));
        let id = created_json["id"].as_str().unwrap().to_owned();

        let listed = request(
            port,
            "GET",
            "/api/conversations",
            authority.token(),
            Some(&origin),
            "",
        );
        let listed_json: serde_json::Value =
            serde_json::from_str(&split_response(&listed).2).unwrap();
        let titles: Vec<&str> = listed_json["conversations"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|item| item["title"].as_str())
            .collect();
        assert!(titles.contains(&title.as_str()), "{listed}");

        let opened = request(
            port,
            "POST",
            &format!("/api/conversations/{id}/stream"),
            authority.token(),
            Some(&origin),
            "",
        );
        let opened_json: serde_json::Value =
            serde_json::from_str(&split_response(&opened).2).unwrap();
        assert_eq!(opened_json["status"].as_str(), Some("open"), "{opened}");
        assert!(opened_json["streamId"].as_str().unwrap().len() >= 16);

        let token = authority.token().to_owned();
        let path = format!("/api/conversations/{id}/stream");
        let origin_for_reader = origin.clone();
        let reader = std::thread::spawn(move || {
            request(port, "GET", &path, &token, Some(&origin_for_reader), "")
        });
        std::thread::sleep(Duration::from_millis(150));
        let cancelled = request(
            port,
            "POST",
            &format!("/api/conversations/{id}/cancel"),
            authority.token(),
            Some(&origin),
            "",
        );
        assert_eq!(split_response(&cancelled).0, 200, "{cancelled}");
        let streamed = reader.join().unwrap();
        let event: serde_json::Value = serde_json::from_str(&split_response(&streamed).2).unwrap();
        assert_eq!(event["event"].as_str(), Some("cancelled"), "{streamed}");

        let again = request(
            port,
            "GET",
            &format!("/api/conversations/{id}/stream"),
            authority.token(),
            Some(&origin),
            "",
        );
        let immediate: serde_json::Value = serde_json::from_str(&split_response(&again).2).unwrap();
        assert_eq!(immediate["event"].as_str(), Some("cancelled"), "{again}");
    }

    #[test]
    fn shutdown_closes_the_listener() {
        let mut authority = StudioAuthority::start().unwrap();
        let port = authority.address().port();
        authority.shutdown();
        let mut last = Ok(());
        for _ in 0..20 {
            last = TcpStream::connect_timeout(
                &SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
                Duration::from_millis(100),
            )
            .map(|_| ());
            if last.is_err() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(last.is_err(), "listener still accepted connections");
    }

    #[test]
    fn serves_the_shell_without_an_iframe_or_electron_webview() {
        let authority = StudioAuthority::start().unwrap();
        let port = authority.address().port();
        let page = request(port, "GET", "/", "", None, "");
        let (status, _, body) = split_response(&page);
        assert_eq!(status, 200, "{page}");
        assert!(body.contains("/studioClient.mjs"));
        assert!(body.contains("data-theme=\"light\""));
        assert!(body.contains("微信扫码登录"));
        assert!(!body.contains("配置其他服务商"));
        assert!(body.contains("id=\"provider-setup\""));
        assert!(body.contains("选择服务商"));
        assert!(!body.contains("/mark.png"));
        assert!(body.contains("/wechat-qr.png"));
        assert!(body.contains("/login-bg.jpg"));
        assert!(!body.contains("打开预览"));
        assert!(!body.contains("欢迎使用"));
        assert!(!body.contains("管理后台"));
        assert!(!body.contains("<header"));
        assert!(!body.contains("id=\"app-name\""));
        assert!(!body.to_ascii_lowercase().contains("<iframe"));
        assert!(!body.to_ascii_lowercase().contains("<webview"));
        let client = request(port, "GET", "/studioClient.mjs", "", None, "");
        let (_, _, source) = split_response(&client);
        assert!(source.contains("请先启用一个服务商"));
        assert!(source.contains("开发验证码"));
        assert!(source.contains("http://127.0.0.1:4318/windows/main/index.html"));
        assert!(!source.contains("create_webview"));
        assert!(!source.contains("打开预览"));
        assert!(!source.to_ascii_lowercase().contains("<iframe"));
        assert!(!source.to_ascii_lowercase().contains("<webview"));
        let logo = request(port, "GET", "/logo.png", "", None, "");
        let (logo_status, logo_head, logo_body) = split_response(&logo);
        assert_eq!(logo_status, 200, "{logo}");
        assert!(logo_head.contains("content-type: image/png"));
        assert!(!logo_body.is_empty());
        let icon = request(port, "GET", "/favicon.ico", "", None, "");
        let (icon_status, icon_head, icon_body) = split_response(&icon);
        assert_eq!(icon_status, 200, "{icon}");
        assert!(icon_head.contains("content-type: image/png"));
        assert!(!icon_body.is_empty());
        assert_ne!(logo_body, icon_body);
        let mark = request(port, "GET", "/mark.png", "", None, "");
        let (mark_status, mark_head, mark_body) = split_response(&mark);
        assert_eq!(mark_status, 200, "{mark}");
        assert!(mark_head.contains("content-type: image/png"));
        assert!(!mark_body.is_empty());
        assert_ne!(mark_body, logo_body);
        let qr = request(port, "GET", "/wechat-qr.png", "", None, "");
        let (qr_status, qr_head, qr_body) = split_response(&qr);
        assert_eq!(qr_status, 200, "{qr}");
        assert!(qr_head.contains("content-type: image/png"));
        assert!(!qr_body.is_empty());
        let bg = request(port, "GET", "/login-bg.jpg", "", None, "");
        let (bg_status, bg_head, bg_body) = split_response(&bg);
        assert_eq!(bg_status, 200, "{bg}");
        assert!(bg_head.contains("content-type: image/jpeg"));
        assert!(!bg_body.is_empty());
    }

    #[test]
    fn forwards_a_member_phone_code_when_the_shell_bearer_matches() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let upstream = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let mut seen = Vec::new();
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut buf = Vec::new();
                let mut tmp = [0u8; 2048];
                loop {
                    match stream.read(&mut tmp) {
                        Ok(0) => break,
                        Ok(n) => {
                            buf.extend_from_slice(&tmp[..n]);
                            let Some(split) =
                                buf.windows(4).position(|window| window == b"\r\n\r\n")
                            else {
                                continue;
                            };
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
                        Err(_) => break,
                    }
                }
                let raw = String::from_utf8_lossy(&buf).into_owned();
                let payload: &[u8] = if raw.contains("token-verifications") {
                    br#"{"code":0,"message":"ok","data":{"valid":true}}"#
                } else {
                    br#"{"code":0,"message":"ok","data":{"ok":true,"cooldown_seconds":60,"dev_code":"246810"}}"#
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    payload.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
                stream.write_all(payload).unwrap();
                seen.push(raw);
            }
            seen
        });
        let authority =
            StudioAuthority::start_with_org_origin(&format!("http://{upstream}")).unwrap();
        let port = authority.address().port();
        let origin = format!("http://127.0.0.1:{port}");
        let preflight = request(
            port,
            "OPTIONS",
            "/v1/api/org/members/phone-codes",
            "",
            Some(&origin),
            "",
        );
        assert_eq!(split_response(&preflight).0, 204, "{preflight}");
        let denied = request(
            port,
            "POST",
            "/v1/api/org/members/phone-codes",
            "nope",
            Some(&origin),
            "{\"phone\":\"13800138000\"}",
        );
        assert_eq!(split_response(&denied).0, 401, "{denied}");
        let sent = request(
            port,
            "POST",
            "/v1/api/org/members/phone-codes",
            authority.token(),
            Some(&origin),
            "{\"phone\":\"13800138000\",\"captchaToken\":\"captcha_v1.example\"}",
        );
        let (status, _, body) = split_response(&sent);
        assert_eq!(status, 200, "{sent}");
        assert!(body.contains("\"dev_code\":\"246810\""), "{sent}");
        let seen = server.join().unwrap();
        let phone = seen
            .iter()
            .find(|raw| raw.contains("phone-codes"))
            .expect("phone-codes");
        assert!(
            !phone.to_ascii_lowercase().contains("authorization"),
            "{phone}"
        );
        assert!(!phone.contains("captchaToken"), "{phone}");
        let missing = request(
            port,
            "POST",
            "/v1/api/org/members/logout",
            authority.token(),
            Some(&origin),
            "{}",
        );
        assert_eq!(split_response(&missing).0, 404, "{missing}");
    }

    #[test]
    fn org_models_forward_keeps_the_canonical_path() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let upstream = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = Vec::new();
            let mut chunk = [0_u8; 1024];
            while !buf.windows(4).any(|window| window == b"\r\n\r\n") {
                let read = stream.read(&mut chunk).unwrap();
                if read == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..read]);
            }
            let payload = br#"{"code":0,"message":"ok","data":{"items":[]}}"#;
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                payload.len()
            );
            stream.write_all(head.as_bytes()).unwrap();
            stream.write_all(payload).unwrap();
            String::from_utf8_lossy(&buf).into_owned()
        });
        let authority =
            StudioAuthority::start_with_org_origin(&format!("http://{upstream}")).unwrap();
        let port = authority.address().port();
        let origin = format!("http://127.0.0.1:{port}");
        let token = authority.token();
        let saved = request(
            port,
            "POST",
            "/api/member-session",
            token,
            Some(&origin),
            "{\"token\":\"a3s_ms_session.token\",\"display_name\":\"林\",\"phone\":\"13800138000\"}",
        );
        assert_eq!(split_response(&saved).0, 200, "{saved}");
        let alias = request(port, "GET", "/api/org-models", token, Some(&origin), "");
        assert_eq!(split_response(&alias).0, 404, "{alias}");
        let listed = request(port, "GET", "/v1/api/org/models", token, Some(&origin), "");
        assert_eq!(split_response(&listed).0, 200, "{listed}");
        let seen = server.join().unwrap();
        assert!(
            seen.starts_with("GET /v1/api/org/models HTTP/1.1"),
            "{seen}"
        );
        assert!(
            seen.contains("Authorization: Bearer a3s_ms_session.token"),
            "{seen}"
        );
    }

    #[test]
    fn member_delete_forwards_its_revision_body() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let upstream = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut buf = Vec::new();
            let mut chunk = [0_u8; 1024];
            loop {
                let read = stream.read(&mut chunk).unwrap_or(0);
                if read == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..read]);
                let Some(split) = buf.windows(4).position(|window| window == b"\r\n\r\n") else {
                    continue;
                };
                let head = String::from_utf8_lossy(&buf[..split]).to_ascii_lowercase();
                let length = head
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length:"))
                    .and_then(|value| value.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                if buf.len() >= split + 4 + length {
                    break;
                }
            }
            let payload = br#"{"code":0,"message":"ok","data":{"ok":true}}"#;
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                payload.len()
            );
            stream.write_all(head.as_bytes()).unwrap();
            stream.write_all(payload).unwrap();
            String::from_utf8_lossy(&buf).into_owned()
        });
        let authority =
            StudioAuthority::start_with_org_origin(&format!("http://{upstream}")).unwrap();
        let port = authority.address().port();
        let origin = format!("http://127.0.0.1:{port}");
        let token = authority.token();
        let saved = request(
            port,
            "POST",
            "/api/member-session",
            token,
            Some(&origin),
            "{\"token\":\"a3s_ms_session.token\",\"display_name\":\"林\",\"phone\":\"13800138000\"}",
        );
        assert_eq!(split_response(&saved).0, 200, "{saved}");
        let revision = "a".repeat(64);
        let revoked = request(
            port,
            "DELETE",
            "/api/member/invites/2587f901-e50d-4f90-871a-0ad1f14668ec",
            token,
            Some(&origin),
            &format!("{{\"expected_revision\":\"{revision}\"}}"),
        );
        assert_eq!(split_response(&revoked).0, 200, "{revoked}");
        let seen = server.join().unwrap();
        assert!(
            seen.starts_with(
                "DELETE /v1/api/org/members/invites/2587f901-e50d-4f90-871a-0ad1f14668ec HTTP/1.1"
            ),
            "{seen}"
        );
        assert!(
            seen.ends_with(&format!("{{\"expected_revision\":\"{revision}\"}}")),
            "{seen}"
        );
    }

    #[test]
    fn capabilities_forward_with_the_stored_member_session() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let upstream = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let mut seen = Vec::new();
            for _ in 0..3 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buf = Vec::new();
                let mut chunk = [0_u8; 1024];
                while !buf.windows(4).any(|window| window == b"\r\n\r\n") {
                    let read = stream.read(&mut chunk).unwrap();
                    if read == 0 {
                        break;
                    }
                    buf.extend_from_slice(&chunk[..read]);
                }
                let payload = br#"{"code":0,"message":"ok","data":[]}"#;
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    payload.len()
                );
                stream.write_all(head.as_bytes()).unwrap();
                stream.write_all(payload).unwrap();
                seen.push(String::from_utf8_lossy(&buf).into_owned());
            }
            seen
        });
        let authority =
            StudioAuthority::start_with_org_origin(&format!("http://{upstream}")).unwrap();
        let port = authority.address().port();
        let origin = format!("http://127.0.0.1:{port}");
        let token = authority.token();
        let denied = request(port, "GET", "/api/capabilities", "nope", Some(&origin), "");
        assert_eq!(split_response(&denied).0, 401, "{denied}");
        let signed_out = request(port, "GET", "/api/capabilities", token, Some(&origin), "");
        assert_eq!(split_response(&signed_out).0, 401, "{signed_out}");
        let saved = request(
            port,
            "POST",
            "/api/member-session",
            token,
            Some(&origin),
            "{\"token\":\"a3s_ms_session.token\",\"display_name\":\"林\",\"phone\":\"13800138000\"}",
        );
        assert_eq!(split_response(&saved).0, 200, "{saved}");
        let listed = request(port, "GET", "/api/capabilities", token, Some(&origin), "");
        assert_eq!(split_response(&listed).0, 200, "{listed}");
        let installed = request(
            port,
            "PUT",
            "/api/capabilities/installations/notion",
            token,
            Some(&origin),
            "",
        );
        assert_eq!(split_response(&installed).0, 200, "{installed}");
        let bad_slug = request(
            port,
            "PUT",
            "/api/capabilities/installations/Notion",
            token,
            Some(&origin),
            "",
        );
        assert_eq!(split_response(&bad_slug).0, 404, "{bad_slug}");
        let seen = server.join().unwrap();
        assert!(
            seen[0].starts_with("GET /v1/api/org/capabilities HTTP/1.1"),
            "{seen:?}"
        );
        assert!(
            seen[1].starts_with("PUT /v1/api/org/capabilities/installations/notion HTTP/1.1"),
            "{seen:?}"
        );
        assert!(
            seen[2].starts_with("GET /v1/api/org/capabilities/installations HTTP/1.1"),
            "install re-reads the installed set to apply it locally: {seen:?}"
        );
        assert!(
            !seen.iter().any(|raw| raw.contains("a3s-home-")),
            "studio does not install catalog plugins on its own: {seen:?}"
        );
        assert!(
            seen.iter()
                .all(|raw| raw.contains("Authorization: Bearer a3s_ms_session.token")),
            "{seen:?}"
        );
    }

    #[test]
    fn skill_install_streams_download_progress_before_it_is_applied() {
        use std::io::{Read, Write};
        let body = vec![b'a'; 20_000];
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let upstream = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let mut seen = Vec::new();
            for _ in 0..3 {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut buf = Vec::new();
                let mut chunk = [0_u8; 2048];
                while !buf.windows(4).any(|window| window == b"\r\n\r\n") {
                    let read = stream.read(&mut chunk).unwrap();
                    if read == 0 {
                        break;
                    }
                    buf.extend_from_slice(&chunk[..read]);
                }
                let raw = String::from_utf8_lossy(&buf).into_owned();
                if raw.contains("GET /v1/api/org/capabilities/pdf/bundle ") {
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/zip\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    stream.write_all(head.as_bytes()).unwrap();
                    stream.write_all(&body).unwrap();
                } else if raw.starts_with("GET /v1/api/org/capabilities/installations ") {
                    let payload = br#"{"code":0,"message":"ok","data":[{"slug":"pdf","installed":true,"install":{"type":"skill","digest":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","size":20000}}]}"#;
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        payload.len()
                    );
                    stream.write_all(head.as_bytes()).unwrap();
                    stream.write_all(payload).unwrap();
                } else {
                    let payload = br#"{"code":0,"message":"ok","data":[]}"#;
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        payload.len()
                    );
                    stream.write_all(head.as_bytes()).unwrap();
                    stream.write_all(payload).unwrap();
                }
                seen.push(raw);
            }
            seen
        });
        let authority =
            StudioAuthority::start_with_org_origin(&format!("http://{upstream}")).unwrap();
        let port = authority.address().port();
        let origin = format!("http://127.0.0.1:{port}");
        let token = authority.token();
        let saved = request(
            port,
            "POST",
            "/api/member-session",
            token,
            Some(&origin),
            "{\"token\":\"a3s_ms_session.token\",\"display_name\":\"林\",\"phone\":\"13800138000\"}",
        );
        assert_eq!(split_response(&saved).0, 200, "{saved}");
        let installed = request(
            port,
            "PUT",
            "/api/capabilities/installations/pdf",
            token,
            Some(&origin),
            "",
        );
        let (status, head, raw_body) = split_response(&installed);
        assert_eq!(status, 200, "{installed}");
        assert!(
            head.contains("content-type: application/x-ndjson"),
            "{head}"
        );
        let lines = ndjson_events(&raw_body);
        let points: Vec<(u64, u64)> = lines
            .iter()
            .filter_map(|line| {
                let value: serde_json::Value = serde_json::from_str(line).ok()?;
                let received = value.get("received")?.as_u64()?;
                let total = value.get("total")?.as_u64()?;
                Some((received, total))
            })
            .collect();
        assert!(points.len() >= 2, "{lines:?}");
        assert!(
            points.windows(2).all(|pair| pair[0].0 < pair[1].0),
            "{points:?}"
        );
        assert!(
            points.iter().all(|(_, total)| *total == 20_000),
            "{points:?}"
        );
        assert!(
            points.iter().any(|(received, total)| received < total),
            "{points:?}"
        );
        let installed_at = lines
            .iter()
            .position(|line| line.contains("\"installed\":true"))
            .expect("installed");
        let last_progress = lines
            .iter()
            .rposition(|line| line.contains("\"received\""))
            .expect("progress");
        assert!(last_progress < installed_at, "{lines:?}");
        let seen = server.join().unwrap();
        assert!(
            seen[0].starts_with("PUT /v1/api/org/capabilities/installations/pdf "),
            "{seen:?}"
        );
        assert!(
            seen[1].starts_with("GET /v1/api/org/capabilities/installations "),
            "{seen:?}"
        );
        assert!(
            seen[2].starts_with("GET /v1/api/org/capabilities/pdf/bundle "),
            "{seen:?}"
        );
        assert!(
            !seen.iter().any(|raw| raw.contains("a3s-home-")),
            "studio does not install catalog plugins on its own: {seen:?}"
        );
    }

    fn ndjson_events(body: &str) -> Vec<String> {
        let bytes = body.as_bytes();
        let mut index = 0;
        let mut events = Vec::new();
        while index < bytes.len() {
            let rest = &bytes[index..];
            let Some(line_end) = rest.windows(2).position(|window| window == b"\r\n") else {
                break;
            };
            let size =
                usize::from_str_radix(std::str::from_utf8(&rest[..line_end]).unwrap().trim(), 16)
                    .unwrap();
            index += line_end + 2;
            if size == 0 {
                break;
            }
            let payload = std::str::from_utf8(&bytes[index..index + size]).unwrap();
            events.extend(
                payload
                    .lines()
                    .filter(|line| !line.is_empty())
                    .map(str::to_owned),
            );
            index += size;
            if bytes.get(index..index + 2) == Some(b"\r\n") {
                index += 2;
            }
        }
        events
    }

    #[test]
    fn relays_the_im_event_stream_with_a_ticket_the_page_never_sees() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let upstream = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let mut seen = Vec::new();
            for index in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buf = Vec::new();
                let mut chunk = [0_u8; 1024];
                while !buf.windows(4).any(|window| window == b"\r\n\r\n") {
                    let read = stream.read(&mut chunk).unwrap();
                    if read == 0 {
                        break;
                    }
                    buf.extend_from_slice(&chunk[..read]);
                }
                // Closing with an unread body resets the socket before the reply lands.
                let head_end = buf
                    .windows(4)
                    .position(|w| w == b"\r\n\r\n")
                    .map_or(buf.len(), |p| p + 4);
                let length = String::from_utf8_lossy(&buf[..head_end])
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())?
                    })
                    .unwrap_or(0);
                while buf.len() < head_end + length {
                    let read = stream.read(&mut chunk).unwrap();
                    if read == 0 {
                        break;
                    }
                    buf.extend_from_slice(&chunk[..read]);
                }
                if index == 0 {
                    let payload = br#"{"code":0,"message":"ok","data":{"ticket":"a3s_imt_abc123","expires_in_secs":60}}"#;
                    let head = format!(
                        "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        payload.len()
                    );
                    stream.write_all(head.as_bytes()).unwrap();
                    stream.write_all(payload).unwrap();
                } else {
                    let frame = "event: im.facts.message.created\ndata: {\"sequence\":7}\n\n";
                    let body = format!("{:x}\r\n{frame}\r\n0\r\n\r\n", frame.len());
                    stream
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n")
                        .unwrap();
                    stream.write_all(body.as_bytes()).unwrap();
                }
                seen.push(String::from_utf8_lossy(&buf).into_owned());
            }
            seen
        });
        let authority =
            StudioAuthority::start_with_org_origin(&format!("http://{upstream}")).unwrap();
        let port = authority.address().port();
        let origin = format!("http://127.0.0.1:{port}");
        let token = authority.token();
        let saved = request(
            port,
            "POST",
            "/api/member-session",
            token,
            Some(&origin),
            "{\"token\":\"a3s_ms_stream.token\",\"display_name\":\"林\",\"phone\":\"13800138000\"}",
        );
        assert_eq!(split_response(&saved).0, 200, "{saved}");
        let org = "0b9d2a4e-1c3f-4a5b-8d6e-7f8091a2b3c4";
        let raw = request(
            port,
            "GET",
            &format!("/api/im/{org}/stream?device=dev-1"),
            token,
            Some(&origin),
            "",
        );
        let (status, head, body) = split_response(&raw);
        assert_eq!(status, 200, "{raw}");
        assert!(head.contains("content-type: text/event-stream"), "{head}");
        assert!(head.contains("transfer-encoding: chunked"), "{head}");
        assert!(body.contains("event: im.facts.message.created"), "{body}");
        assert!(!raw.contains("a3s_ms_stream.token"), "{raw}");
        let seen = server.join().unwrap();
        assert!(
            seen[0].starts_with(&format!(
                "POST /v1/api/org/im/{org}/stream-tickets HTTP/1.1"
            )),
            "{seen:?}"
        );
        assert!(
            seen[0].contains("Authorization: Bearer a3s_ms_stream.token"),
            "{seen:?}"
        );
        assert!(
            seen[1].starts_with(&format!(
                "GET /v1/api/org/im/{org}/stream?ticket=a3s_imt_abc123 HTTP/1.1"
            )),
            "{seen:?}"
        );
        assert!(
            !seen[1].to_ascii_lowercase().contains("authorization"),
            "{seen:?}"
        );
    }

    #[test]
    fn composer_upload_name_drops_path_segments() {
        assert_eq!(super::sanitize_upload_name("../secret.txt"), "secret.txt");
        assert_eq!(super::sanitize_upload_name(".."), "file.bin");
        assert_eq!(super::sanitize_upload_name(""), "file.bin");
        assert_eq!(
            super::sanitize_upload_name("%E6%88%AA%E5%9B%BE.png"),
            "截图.png"
        );
    }

    #[test]
    fn composer_file_is_written_under_the_temp_dir() {
        let authority = StudioAuthority::start().unwrap();
        let port = authority.address().port();
        let origin = format!("http://127.0.0.1:{port}");
        let token = authority.token();
        let body = "hello-image";
        let raw = format!(
            "POST /api/composer-files/note.txt HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAuthorization: Bearer {token}\r\nOrigin: {origin}\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let response = exchange(port, &raw, Duration::from_secs(2));
        let (status, _, payload) = split_response(&response);
        assert_eq!(status, 200, "{response}");
        assert!(payload.contains("\"size\":11"), "{payload}");
        let marker = "\"path\":\"";
        let start = payload.find(marker).expect("path") + marker.len();
        let end = payload[start..].find('"').expect("path end") + start;
        let path = payload[start..end].to_owned();
        assert!(path.contains("a3s-studio-composer"), "{path}");
        assert_eq!(std::fs::read(&path).unwrap(), b"hello-image");
        let _ = std::fs::remove_file(&path);
        let denied = request(
            port,
            "POST",
            "/api/composer-files/note.txt",
            "nope",
            Some(&origin),
            "x",
        );
        assert_eq!(split_response(&denied).0, 401, "{denied}");
    }

    #[test]
    fn member_session_is_empty_until_this_process_saves_one() {
        let authority = StudioAuthority::start().unwrap();
        let port = authority.address().port();
        let origin = format!("http://127.0.0.1:{port}");
        let token = authority.token();
        let empty = request(port, "GET", "/api/member-session", token, Some(&origin), "");
        let (status, _, body) = split_response(&empty);
        assert_eq!(status, 200, "{empty}");
        assert_eq!(body, "{\"token\":null}");
        let unsigned = request(
            port,
            "GET",
            "/api/member-session/profile",
            token,
            Some("http://127.0.0.1:4318"),
            "",
        );
        let (status, _, body) = split_response(&unsigned);
        assert_eq!(status, 200, "{unsigned}");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&body).unwrap(),
            serde_json::json!({
                "signedIn": false,
                "display_name": "",
                "phone": "",
                "avatar": "",
            })
        );
        assert!(!body.contains("token"), "{body}");
        let rejected = request(
            port,
            "POST",
            "/api/member-session",
            token,
            Some(&origin),
            "{\"token\":\"nope\"}",
        );
        assert_eq!(split_response(&rejected).0, 400, "{rejected}");
        let saved = request(
            port,
            "POST",
            "/api/member-session",
            token,
            Some(&origin),
            "{\"token\":\"a3s_ms_session.token\",\"display_name\":\"林\",\"phone\":\"13800138000\"}",
        );
        assert_eq!(split_response(&saved).0, 200, "{saved}");
        let loaded = request(port, "GET", "/api/member-session", token, Some(&origin), "");
        let (status, _, body) = split_response(&loaded);
        assert_eq!(status, 200, "{loaded}");
        assert!(
            body.contains("\"token\":\"a3s_ms_session.token\""),
            "{loaded}"
        );
        let renderer_origin = "http://127.0.0.1:4318";
        let redacted = request(
            port,
            "GET",
            "/api/member-session",
            token,
            Some(renderer_origin),
            "",
        );
        let (status, _, body) = split_response(&redacted);
        assert_eq!(status, 200, "{redacted}");
        let profile: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(profile["signedIn"], true, "{body}");
        assert_eq!(profile["display_name"], "林", "{body}");
        assert_eq!(profile["phone"], "13800138000", "{body}");
        assert!(profile.get("token").is_none(), "{body}");
        assert!(!body.contains("a3s_ms_"), "{body}");
        let profile_route = request(
            port,
            "GET",
            "/api/member-session/profile",
            token,
            Some(renderer_origin),
            "",
        );
        let (status, _, body) = split_response(&profile_route);
        assert_eq!(status, 200, "{profile_route}");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&body).unwrap(),
            serde_json::json!({
                "signedIn": true,
                "display_name": "林",
                "phone": "13800138000",
                "avatar": "",
            })
        );
        assert!(!body.contains("a3s_ms_"), "{body}");
        let shell_profile = request(
            port,
            "GET",
            "/api/member-session/profile",
            token,
            Some(&origin),
            "",
        );
        let (status, _, body) = split_response(&shell_profile);
        assert_eq!(status, 200, "{shell_profile}");
        assert!(!body.contains("a3s_ms_"), "{body}");
        assert!(body.contains("\"signedIn\":true"), "{body}");
        let missing = request(port, "GET", "/api/member-session", token, None, "");
        let (status, _, body) = split_response(&missing);
        assert_eq!(status, 200, "{missing}");
        assert!(
            body.contains("\"token\":\"a3s_ms_session.token\""),
            "{missing}"
        );
        let denied = request(
            port,
            "GET",
            "/api/member-session",
            "nope",
            Some(&origin),
            "",
        );
        assert_eq!(split_response(&denied).0, 401, "{denied}");
        let logged_out = request(
            port,
            "POST",
            "/api/member-session/logout",
            token,
            Some(&origin),
            "{}",
        );
        let (status, _, body) = split_response(&logged_out);
        assert_eq!(status, 200, "{logged_out}");
        assert_eq!(body, "{\"token\":null}");
        let after = request(port, "GET", "/api/member-session", token, Some(&origin), "");
        assert_eq!(split_response(&after).2, "{\"token\":null}", "{after}");
        let after_profile = request(
            port,
            "GET",
            "/api/member-session/profile",
            token,
            Some(renderer_origin),
            "",
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&split_response(&after_profile).2).unwrap()["signedIn"],
            false
        );
    }

    #[test]
    fn applet_product_release_is_served_without_a_token() {
        let authority = StudioAuthority::start().unwrap();
        let port = authority.address().port();
        let origin = format!("http://127.0.0.1:{port}");
        let token = authority.token();
        let loaded = request(
            port,
            "GET",
            "/api/applet-products/leave",
            token,
            Some("http://127.0.0.1:4318"),
            "",
        );
        let (status, _, body) = split_response(&loaded);
        assert_eq!(status, 200, "{loaded}");
        let meta: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(meta["release"]["version"], "1.1.0");
        assert!(meta.get("token").is_none(), "{body}");
        assert!(!body.contains("a3s_ms_"), "{body}");
        let package = request(
            port,
            "GET",
            "/api/applet-products/leave/package/release",
            token,
            Some("http://127.0.0.1:4318"),
            "",
        );
        let (status, _, body) = split_response(&package);
        assert_eq!(status, 200, "{package}");
        assert_eq!(
            crate::applet_product::sha256_hex(body.as_bytes()),
            meta["release"]["digest"].as_str().unwrap()
        );
        assert!(body.contains("\"version\":\"1.1.0\""), "{body}");
        assert!(body.contains("\"fields\""), "{body}");
        assert!(body.contains("\"name\":\"title\""), "{body}");
        assert!(body.contains("说明"), "{body}");
        assert!(!body.contains("token"), "{body}");
        let denied = request(
            port,
            "GET",
            "/api/applet-products/leave/package/release",
            "nope",
            Some(&origin),
            "",
        );
        assert_eq!(split_response(&denied).0, 401, "{denied}");

        let created = request(
            port,
            "POST",
            "/api/applet-products",
            token,
            Some(&origin),
            "{\"appId\":\"widget\",\"name\":\"挂件\"}",
        );
        assert_eq!(split_response(&created).0, 200, "{created}");
        let built = request(
            port,
            "POST",
            "/api/applet-products/widget/builds",
            token,
            Some(&origin),
            "{\"version\":\"1.0.0\"}",
        );
        assert_eq!(split_response(&built).0, 200, "{built}");
        let trial = request(
            port,
            "POST",
            "/api/applet-products/widget/publish",
            token,
            Some(&origin),
            "{\"version\":\"1.0.0\",\"channel\":\"trial\"}",
        );
        assert_eq!(split_response(&trial).0, 200, "{trial}");
        let unreleased = request(
            port,
            "GET",
            "/api/applet-products/widget/package/release",
            token,
            Some("http://127.0.0.1:4318"),
            "",
        );
        assert_eq!(split_response(&unreleased).0, 404, "{unreleased}");
        let published = request(
            port,
            "POST",
            "/api/applet-products/widget/publish",
            token,
            Some(&origin),
            "{\"version\":\"1.0.0\",\"channel\":\"release\"}",
        );
        assert_eq!(split_response(&published).0, 200, "{published}");
        let release = request(
            port,
            "GET",
            "/api/applet-products/widget/package/release",
            token,
            Some("http://127.0.0.1:4318"),
            "",
        );
        let (status, _, body) = split_response(&release);
        assert_eq!(status, 200, "{release}");
        assert!(body.contains("\"version\":\"1.0.0\""), "{body}");
        assert!(!body.contains("a3s_ms_"), "{body}");
    }

    #[test]
    fn applet_runtime_accepts_its_ticket_and_rejects_the_shell_token() {
        let authority = StudioAuthority::start().unwrap();
        let port = authority.address().port();
        let origin = "http://127.0.0.1:4318";
        let shell = authority.token().to_owned();
        let ticket = authority
            .mint_applet_ticket("leave", "applet-leave")
            .unwrap();
        let denied = request(
            port,
            "GET",
            "/api/applet-runtime/leave/package/release",
            &shell,
            Some(origin),
            "",
        );
        assert_eq!(split_response(&denied).0, 401, "{denied}");
        let other = request(
            port,
            "GET",
            "/api/applet-runtime/travel/package/release",
            &ticket,
            Some(origin),
            "",
        );
        assert_eq!(split_response(&other).0, 401, "{other}");
        let created = request(
            port,
            "POST",
            "/api/applet-products",
            &ticket,
            Some(origin),
            "{\"appId\":\"widget\",\"name\":\"挂件\"}",
        );
        assert_eq!(split_response(&created).0, 401, "{created}");
        let release = request(
            port,
            "GET",
            "/api/applet-runtime/leave/package/release",
            &ticket,
            Some(origin),
            "",
        );
        let (status, _, package) = split_response(&release);
        assert_eq!(status, 200, "{release}");
        assert!(package.contains("\"fields\""), "{package}");
        assert!(!package.contains(&shell), "{package}");
        assert!(!package.contains("phone"), "{package}");
        let digest = crate::applet_product::sha256_hex(package.as_bytes());
        let saved = request(
            port,
            "POST",
            "/api/member-session",
            &shell,
            Some(origin),
            "{\"token\":\"a3s_ms_runtime.token\",\"display_name\":\"林\",\"phone\":\"13800138000\"}",
        );
        assert_eq!(split_response(&saved).0, 200, "{saved}");
        let session = request(
            port,
            "GET",
            "/api/applet-runtime/leave/session",
            &ticket,
            Some(origin),
            "",
        );
        let (status, _, body) = split_response(&session);
        assert_eq!(status, 200, "{session}");
        let profile: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(profile["signedIn"], true);
        assert_eq!(profile["displayName"], "林");
        assert!(profile.get("phone").is_none(), "{body}");
        assert!(!body.contains("13800138000"), "{body}");
        assert!(!body.contains("a3s_ms_"), "{body}");
        let submit = format!(
            "{{\"version\":\"1.1.0\",\"digest\":\"{digest}\",\"idempotencyKey\":\"11111111-1111-4111-8111-111111111111\",\"values\":{{\"title\":\"明天\",\"detail\":\"事假\"}}}}"
        );
        let first = request(
            port,
            "POST",
            "/api/applet-runtime/leave/submissions",
            &ticket,
            Some(origin),
            &submit,
        );
        let again = request(
            port,
            "POST",
            "/api/applet-runtime/leave/submissions",
            &ticket,
            Some(origin),
            &submit,
        );
        let (status, _, first_body) = split_response(&first);
        assert_eq!(status, 200, "{first}");
        let (status, _, again_body) = split_response(&again);
        assert_eq!(status, 200, "{again}");
        let first_json: serde_json::Value = serde_json::from_str(&first_body).unwrap();
        let again_json: serde_json::Value = serde_json::from_str(&again_body).unwrap();
        assert_eq!(first_json["id"], again_json["id"]);
        assert!(!first_body.contains("13800138000"));
        assert!(!first_body.contains(&shell));
    }

    #[test]
    fn shell_ready_leave_preview_returns_the_forwarded_chart() {
        use std::io::{Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let upstream = listener.local_addr().unwrap();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let seen_thread = std::sync::Arc::clone(&seen);
        let server = std::thread::spawn(move || {
            for _ in 0..4 {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut buf = Vec::new();
                let mut tmp = [0u8; 2048];
                while !buf.windows(4).any(|window| window == b"\r\n\r\n") {
                    match stream.read(&mut tmp) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => buf.extend_from_slice(&tmp[..n]),
                    }
                }
                let raw = String::from_utf8_lossy(&buf).into_owned();
                let head_len = raw.find("\r\n\r\n").map_or(buf.len(), |at| at + 4);
                let body_len = raw
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())?
                    })
                    .unwrap_or(0);
                while buf.len() < head_len + body_len {
                    match stream.read(&mut tmp) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => buf.extend_from_slice(&tmp[..n]),
                    }
                }
                let raw = String::from_utf8_lossy(&buf).into_owned();
                let preview = raw.contains("POST /v1/api/org/members/work-items/process-preview ");
                if preview {
                    seen_thread.lock().expect("seen").push(raw);
                }
                let body = if preview {
                    "{\"version\":3,\"nodes\":[{\"name\":\"直属主管\"}]}"
                        .as_bytes()
                        .to_vec()
                } else {
                    br#"{"error":"no"}"#.to_vec()
                };
                let status = if preview { "200 OK" } else { "404 Not Found" };
                let head = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(&body);
            }
        });
        let authority =
            StudioAuthority::start_with_org_origin(&format!("http://{upstream}")).unwrap();
        let port = authority.address().port();
        let origin = "http://127.0.0.1:4318";
        let shell = authority.token().to_owned();
        let member = "a3s_ms_preview.token";
        let saved = request(
            port,
            "POST",
            "/api/member-session",
            &shell,
            Some(origin),
            &format!(
                "{{\"token\":\"{member}\",\"display_name\":\"林\",\"phone\":\"13800138000\",\"member_id\":\"33333333-3333-4333-8333-333333333333\"}}"
            ),
        );
        assert_eq!(split_response(&saved).0, 200, "{saved}");
        let body = "{\"org_id\":\"org-1\",\"process_key\":\"leave\",\"amount\":\"\",\"form\":{\"leave_type\":\"事假\",\"start_date\":\"2026-10-19\",\"end_date\":\"2026-10-22\",\"reason\":\"事假四天\"}}";
        let preview = request(
            port,
            "POST",
            "/api/applet-runtime/leave/work-items/process-preview",
            &shell,
            Some(origin),
            body,
        );
        let (status, _, payload) = split_response(&preview);
        assert_eq!(status, 200, "{preview}");
        assert!(payload.contains("\"version\":3"), "{payload}");
        assert!(payload.contains("直属主管"), "{payload}");
        assert!(!payload.contains(&shell), "{payload}");
        let release = request(
            port,
            "GET",
            "/api/applet-runtime/leave/package/release",
            &shell,
            Some(origin),
            "",
        );
        assert_eq!(split_response(&release).0, 401, "{release}");
        let stranger = request(
            port,
            "POST",
            "/api/applet-runtime/leave/work-items/process-preview",
            "not-the-shell-token",
            Some(origin),
            body,
        );
        assert_eq!(split_response(&stranger).0, 401, "{stranger}");
        let travel = authority
            .mint_applet_ticket("travel", "applet-travel")
            .unwrap();
        let wrong_app = request(
            port,
            "POST",
            "/api/applet-runtime/leave/work-items/process-preview",
            &travel,
            Some(origin),
            body,
        );
        assert_eq!(split_response(&wrong_app).0, 401, "{wrong_app}");
        let forwarded = seen.lock().expect("seen");
        assert!(
            forwarded
                .iter()
                .any(|raw| raw.contains(&format!("Bearer {member}"))),
            "member session was not attached"
        );
        assert!(forwarded.iter().all(|raw| !raw.contains(&shell)));
        drop(forwarded);
        drop(server);
    }

    #[test]
    fn a_fresh_sign_in_knows_the_member_before_the_main_window_reads_preferences() {
        use std::io::{Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let upstream = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            // The startup capabilities read may land here too; only sign-ins end the loop.
            let mut sign_ins = 0;
            while sign_ins < 2 {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut buf = Vec::new();
                let mut tmp = [0u8; 2048];
                while !buf.windows(4).any(|window| window == b"\r\n\r\n") {
                    match stream.read(&mut tmp) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => buf.extend_from_slice(&tmp[..n]),
                    }
                }
                let raw = String::from_utf8_lossy(&buf).into_owned();
                let head_len = raw.find("\r\n\r\n").map_or(buf.len(), |at| at + 4);
                let body_len = raw
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())?
                    })
                    .unwrap_or(0);
                // Closing with the request body unread resets the socket before the reply lands.
                while buf.len() < head_len + body_len {
                    match stream.read(&mut tmp) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => buf.extend_from_slice(&tmp[..n]),
                    }
                }
                let sign_in = raw.contains("POST /v1/api/org/members/code-sessions ");
                sign_ins += usize::from(sign_in);
                let (status, body): (&str, &[u8]) = if sign_in {
                    (
                        "200 OK",
                        br#"{"code":0,"message":"ok","data":{"token":"a3s_ms_fresh.c.token","token_type":"Bearer","expires_at_epoch":0,"member":{"id":"33333333-3333-4333-8333-333333333333","display_name":"C","phone":"13800138000"}}}"#,
                    )
                } else {
                    ("404 Not Found", br#"{"error":"no"}"#)
                };
                let head = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(body);
            }
        });
        let authority =
            StudioAuthority::start_with_org_origin(&format!("http://{upstream}")).unwrap();
        let port = authority.address().port();
        let origin = format!("http://127.0.0.1:{port}");
        let member = authority.preference_member_session();
        let sign_in = || {
            let signed = request(
                port,
                "POST",
                "/v1/api/org/members/code-sessions",
                authority.token(),
                Some(&origin),
                "{\"phone\":\"13800138000\",\"code\":\"246810\"}",
            );
            assert_eq!(split_response(&signed).0, 200, "{signed}");
        };
        let save = |token: &str| {
            let saved = request(
                port,
                "POST",
                "/api/member-session",
                authority.token(),
                Some(&origin),
                &format!(
                    "{{\"token\":\"{token}\",\"display_name\":\"C\",\"phone\":\"13800138000\"}}"
                ),
            );
            assert_eq!(split_response(&saved).0, 200, "{saved}");
        };

        sign_in();
        save("a3s_ms_other.d.token");
        assert!(member.member_id().is_none());
        assert!(member.with_preference_scope(|_| Ok(())).is_err());

        sign_in();
        save("a3s_ms_fresh.c.token");
        assert_eq!(
            member.member_id().as_deref(),
            Some("33333333-3333-4333-8333-333333333333")
        );
        assert!(member.with_preference_scope(|_| Ok(())).is_ok());
        server.join().unwrap();
    }

    #[test]
    fn leave_profile_avatar_is_served_without_the_phone() {
        use std::io::{Read, Write};
        use std::sync::mpsc;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let upstream = listener.local_addr().unwrap();
        let (tx, rx) = mpsc::channel();
        let payload = r#"{"code":0,"message":"ok","data":{"display_name":"林","phone":"13800138000","avatar":"data:image/png;base64,AAAA","token":"a3s_ms_profile.token"}}"#
            .as_bytes();
        let server = std::thread::spawn(move || {
            for _ in 0..6 {
                let Ok((mut stream, _)) = listener.accept() else {
                    break;
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut buf = Vec::new();
                let mut tmp = [0u8; 2048];
                loop {
                    match stream.read(&mut tmp) {
                        Ok(0) => break,
                        Ok(n) => {
                            buf.extend_from_slice(&tmp[..n]);
                            if buf.windows(4).any(|window| window == b"\r\n\r\n") {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                let raw = String::from_utf8_lossy(&buf).into_owned();
                let mine = raw.contains("GET /v1/api/org/members/me ");
                let body: &[u8] = if mine { payload } else { br#"{"error":"no"}"# };
                let status = if mine { "200 OK" } else { "404 Not Found" };
                let head = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(head.as_bytes()).unwrap();
                stream.write_all(body).unwrap();
                if mine {
                    let _ = tx.send(raw);
                    break;
                }
            }
        });
        let authority =
            StudioAuthority::start_with_org_origin(&format!("http://{upstream}")).unwrap();
        let port = authority.address().port();
        let origin = "http://127.0.0.1:4318";
        let shell = authority.token().to_owned();
        let saved = request(
            port,
            "POST",
            "/api/member-session",
            &shell,
            Some(origin),
            "{\"token\":\"a3s_ms_profile.token\",\"display_name\":\"林\",\"phone\":\"13800138000\"}",
        );
        assert_eq!(split_response(&saved).0, 200, "{saved}");
        let ticket = authority
            .mint_applet_ticket("leave", "applet-leave")
            .unwrap();
        let denied = request(
            port,
            "GET",
            "/api/applet-runtime/leave/profile",
            &shell,
            Some(origin),
            "",
        );
        assert_eq!(split_response(&denied).0, 401, "{denied}");
        let loaded = request(
            port,
            "GET",
            "/api/applet-runtime/leave/profile",
            &ticket,
            Some(origin),
            "",
        );
        let (status, _, body) = split_response(&loaded);
        assert_eq!(status, 200, "{loaded}");
        let profile: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(profile["displayName"], "林");
        assert_eq!(profile["avatar"], "data:image/png;base64,AAAA");
        assert!(profile.get("phone").is_none(), "{body}");
        assert!(profile.get("token").is_none(), "{body}");
        assert!(!body.contains("13800138000"), "{body}");
        assert!(!body.contains("a3s_ms_"), "{body}");
        let seen = rx.recv_timeout(Duration::from_secs(2)).expect("members/me");
        assert!(seen.contains("GET /v1/api/org/members/me "));
        assert!(seen.contains("a3s_ms_profile.token"));
        server.join().unwrap();
    }

    #[test]
    fn inbox_ticket_forwards_work_items_and_keeps_the_shell_token_off_the_page() {
        use std::io::Write;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::{Arc, Mutex};
        use std::time::Instant;

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let upstream = listener.local_addr().unwrap();
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let seen_thread = Arc::clone(&seen);
        let stop_thread = Arc::clone(&stop);
        let server = std::thread::spawn(move || {
            while !stop_thread.load(Ordering::SeqCst) {
                let accepted = listener.accept();
                let mut stream = match accepted {
                    Ok((stream, _)) => stream,
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                    Err(_) => break,
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut buf = Vec::new();
                let mut tmp = [0u8; 2048];
                let mut header_end = None;
                loop {
                    match stream.read(&mut tmp) {
                        Ok(0) => break,
                        Ok(n) => {
                            buf.extend_from_slice(&tmp[..n]);
                            if header_end.is_none() {
                                header_end =
                                    buf.windows(4).position(|window| window == b"\r\n\r\n");
                            }
                            if let Some(end) = header_end {
                                let head = String::from_utf8_lossy(&buf[..end]);
                                let length = head
                                    .lines()
                                    .find_map(|line| {
                                        let (name, value) = line.split_once(':')?;
                                        name.eq_ignore_ascii_case("content-length")
                                            .then(|| value.trim().parse::<usize>().ok())
                                            .flatten()
                                    })
                                    .unwrap_or(0);
                                if buf.len() >= end + 4 + length {
                                    break;
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
                let raw = String::from_utf8_lossy(&buf).into_owned();
                let body: &[u8] = if raw.contains("/v1/api/org/members/work-items") {
                    if raw.contains("GET /v1/api/org/members/work-items/summary") {
                        br#"{"code":0,"message":"ok","data":{"todo":2,"done":1,"cc":0,"sent":3}}"#
                    } else if raw.contains("GET /v1/api/org/members/work-items?")
                        || raw.contains("GET /v1/api/org/members/work-items ")
                    {
                        br#"{"code":0,"message":"ok","data":[]}"#
                    } else {
                        br#"{"code":0,"message":"ok","data":{"id":"11111111-1111-4111-8111-111111111111","kind":"approval","title":"purchase","status":"pending"}}"#
                    }
                } else {
                    br#"{"error":"no"}"#
                };
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(body);
                if raw.contains("/v1/api/org/members/work-items") {
                    seen_thread.lock().expect("seen").push(raw);
                }
            }
        });

        let authority =
            StudioAuthority::start_with_org_origin(&format!("http://{upstream}")).unwrap();
        let port = authority.address().port();
        let origin = "http://127.0.0.1:4318";
        let shell = authority.token().to_owned();
        let member = "a3s_ms_inbox.token";
        let saved = request(
            port,
            "POST",
            "/api/member-session",
            &shell,
            Some(origin),
            &format!(
                "{{\"token\":\"{member}\",\"display_name\":\"林\",\"phone\":\"13800138000\"}}"
            ),
        );
        assert_eq!(split_response(&saved).0, 200, "{saved}");
        let inbox = authority
            .mint_applet_ticket("inbox", "applet-inbox")
            .unwrap();
        let leave = authority
            .mint_applet_ticket("leave", "applet-leave")
            .unwrap();
        assert_ne!(inbox, shell);
        assert_ne!(leave, inbox);

        let shell_on_inbox = request(
            port,
            "GET",
            "/api/applet-runtime/inbox/work-items/summary",
            &shell,
            Some(origin),
            "",
        );
        assert_eq!(split_response(&shell_on_inbox).0, 401, "{shell_on_inbox}");
        let leave_on_inbox = request(
            port,
            "GET",
            "/api/applet-runtime/inbox/work-items/summary",
            &leave,
            Some(origin),
            "",
        );
        assert_eq!(split_response(&leave_on_inbox).0, 401, "{leave_on_inbox}");

        let summary = request(
            port,
            "GET",
            "/api/applet-runtime/inbox/work-items/summary",
            &inbox,
            Some(origin),
            "",
        );
        let (status, _, body) = split_response(&summary);
        assert_eq!(status, 200, "{summary}");
        assert!(body.contains("\"todo\":2"), "{body}");
        assert!(!body.contains(&shell), "{body}");
        assert!(!body.contains(member), "{body}");
        assert!(!body.contains("13800138000"), "{body}");

        let list = request(
            port,
            "GET",
            "/api/applet-runtime/inbox/work-items?box=todo&kind=approval&q=hello",
            &inbox,
            Some(origin),
            "",
        );
        assert_eq!(split_response(&list).0, 200, "{list}");
        let created = request(
            port,
            "POST",
            "/api/applet-runtime/inbox/work-items",
            &inbox,
            Some(origin),
            "{\"org_id\":\"org-1\",\"title\":\"采购\",\"summary\":\"说明\"}",
        );
        let (status, _, body) = split_response(&created);
        assert_eq!(status, 200, "{created}");
        assert!(body.contains("\"title\":\"purchase\""), "{body}");
        assert!(!body.contains(&shell) && !body.contains(member), "{body}");
        let decision = request(
            port,
            "POST",
            "/api/applet-runtime/inbox/work-items/11111111-1111-4111-8111-111111111111/decision",
            &inbox,
            Some(origin),
            "{\"decision\":\"approve\"}",
        );
        assert_eq!(split_response(&decision).0, 200, "{decision}");
        let bad_decision = request(
            port,
            "POST",
            "/api/applet-runtime/inbox/work-items/not-a-uuid/decision",
            &inbox,
            Some(origin),
            "{\"decision\":\"approve\"}",
        );
        assert_eq!(split_response(&bad_decision).0, 404, "{bad_decision}");
        let leave_list = request(
            port,
            "GET",
            "/api/applet-runtime/leave/work-items",
            &leave,
            Some(origin),
            "",
        );
        assert_eq!(split_response(&leave_list).0, 404, "{leave_list}");
        let leave_create = request(
            port,
            "POST",
            "/api/applet-runtime/leave/work-items",
            &leave,
            Some(origin),
            "{\"org_id\":\"org-1\",\"title\":\"请假\",\"summary\":\"事假\"}",
        );
        assert_eq!(split_response(&leave_create).0, 200, "{leave_create}");

        let empty_center = request(
            port,
            "GET",
            "/api/applet-runtime/inbox/center",
            &inbox,
            Some(origin),
            "",
        );
        let (status, _, body) = split_response(&empty_center);
        assert_eq!(status, 200, "{empty_center}");
        assert!(body.contains("\"id\":\"personal\""), "{body}");
        let rejected = request(
            port,
            "POST",
            "/api/member-center",
            &shell,
            Some(origin),
            "{\"id\":\"../x\",\"extra\":1}",
        );
        assert_eq!(split_response(&rejected).0, 400, "{rejected}");
        let ticket_post = request(
            port,
            "POST",
            "/api/member-center",
            &inbox,
            Some(origin),
            "{\"id\":\"org-1\"}",
        );
        assert_eq!(split_response(&ticket_post).0, 401, "{ticket_post}");
        let stored = request(
            port,
            "POST",
            "/api/member-center",
            &shell,
            Some(origin),
            "{\"id\":\"org-1\"}",
        );
        assert_eq!(split_response(&stored).0, 200, "{stored}");
        let center = request(
            port,
            "GET",
            "/api/applet-runtime/inbox/center",
            &inbox,
            Some(origin),
            "",
        );
        let (status, _, body) = split_response(&center);
        assert_eq!(status, 200, "{center}");
        assert!(body.contains("\"id\":\"org-1\""), "{body}");
        assert!(!body.contains(&shell) && !body.contains(member), "{body}");
        let shell_center = request(
            port,
            "GET",
            "/api/applet-runtime/inbox/center",
            &shell,
            Some(origin),
            "",
        );
        assert_eq!(split_response(&shell_center).0, 401, "{shell_center}");
        let leave_center = request(
            port,
            "GET",
            "/api/applet-runtime/leave/center",
            &leave,
            Some(origin),
            "",
        );
        assert!(split_response(&leave_center).2.contains("\"id\":\"org-1\""));
        let crossed = request(
            port,
            "GET",
            "/api/applet-runtime/inbox/center",
            &leave,
            Some(origin),
            "",
        );
        assert_eq!(split_response(&crossed).0, 401, "{crossed}");
        let forged = request(
            port,
            "GET",
            "/api/applet-runtime/inbox/work-items?box=todo&org=org-other",
            &inbox,
            Some(origin),
            "",
        );
        assert_eq!(split_response(&forged).0, 200, "{forged}");
        let logged_out = request(
            port,
            "POST",
            "/api/member-session/logout",
            &shell,
            Some(origin),
            "",
        );
        assert_eq!(split_response(&logged_out).0, 200, "{logged_out}");
        let cleared = request(
            port,
            "GET",
            "/api/applet-runtime/inbox/center",
            &inbox,
            Some(origin),
            "",
        );
        assert!(
            split_response(&cleared).2.contains("\"id\":\"personal\""),
            "{cleared}"
        );

        let deadline = Instant::now() + Duration::from_secs(3);
        let rows = loop {
            let rows = seen.lock().expect("seen").clone();
            if rows.iter().any(|row| row.contains("title\":\"请假")) || Instant::now() > deadline
            {
                break rows;
            }
            drop(rows);
            std::thread::sleep(Duration::from_millis(20));
        };
        let joined = rows.join("\n");
        assert!(
            joined.contains("GET /v1/api/org/members/work-items/summary?org=personal "),
            "{joined}"
        );
        assert!(
            joined.contains(
                "GET /v1/api/org/members/work-items?box=todo&kind=approval&q=hello&org=personal "
            ),
            "{joined}"
        );
        assert!(
            joined.contains("POST /v1/api/org/members/work-items "),
            "{joined}"
        );
        assert!(joined.contains("\"title\":\"采购\""), "{joined}");
        assert!(joined.contains("\"title\":\"请假\""), "{joined}");
        assert!(
            joined.contains(
                "POST /v1/api/org/members/work-items/11111111-1111-4111-8111-111111111111/decision "
            ),
            "{joined}"
        );
        assert!(
            joined.contains("GET /v1/api/org/members/work-items?box=todo&org=org-1 "),
            "{joined}"
        );
        assert!(!joined.contains("org-other"), "{joined}");
        assert!(joined.contains("Bearer a3s_ms_inbox.token"), "{joined}");
        assert!(!joined.contains(&shell), "{joined}");
        assert_eq!(rows.len(), 6, "{joined}");
        stop.store(true, Ordering::SeqCst);
        server.join().unwrap();
    }

    #[test]
    fn org_centers_stay_with_their_member() {
        use std::io::Write;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::{Arc, Mutex};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let upstream = listener.local_addr().unwrap();
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let seen_thread = Arc::clone(&seen);
        let stop_thread = Arc::clone(&stop);
        let server = std::thread::spawn(move || {
            while !stop_thread.load(Ordering::SeqCst) {
                let accepted = listener.accept();
                let mut stream = match accepted {
                    Ok((stream, _)) => stream,
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                    Err(_) => break,
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut buf = Vec::new();
                let mut tmp = [0u8; 2048];
                let mut header_end = None;
                loop {
                    match stream.read(&mut tmp) {
                        Ok(0) => break,
                        Ok(n) => {
                            buf.extend_from_slice(&tmp[..n]);
                            if header_end.is_none() {
                                header_end =
                                    buf.windows(4).position(|window| window == b"\r\n\r\n");
                            }
                            if let Some(end) = header_end {
                                let head = String::from_utf8_lossy(&buf[..end]);
                                let length = head
                                    .lines()
                                    .find_map(|line| {
                                        let (name, value) = line.split_once(':')?;
                                        name.eq_ignore_ascii_case("content-length")
                                            .then(|| value.trim().parse::<usize>().ok())
                                            .flatten()
                                    })
                                    .unwrap_or(0);
                                if buf.len() >= end + 4 + length {
                                    break;
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
                let raw = String::from_utf8_lossy(&buf).into_owned();
                let mine = raw.contains("GET /v1/api/org/members/me ");
                let body: &[u8] = if mine && raw.contains("a3s_ms_member.a.token") {
                    br#"{"id":"11111111-1111-4111-8111-111111111111","display_name":"A"}"#
                } else if mine && raw.contains("a3s_ms_member.b.token") {
                    br#"{"id":"22222222-2222-4222-8222-222222222222","display_name":"B"}"#
                } else if raw.contains("/v1/api/org/members/work-items") {
                    seen_thread.lock().expect("seen").push(raw.clone());
                    br#"{"code":0,"message":"ok","data":[]}"#
                } else {
                    br#"{"error":"no"}"#
                };
                let status = if mine || raw.contains("/v1/api/org/members/work-items") {
                    "200 OK"
                } else {
                    "404 Not Found"
                };
                let head = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(body);
            }
        });

        let authority =
            StudioAuthority::start_with_org_origin(&format!("http://{upstream}")).unwrap();
        let port = authority.address().port();
        let origin = "http://127.0.0.1:4318";
        let shell = authority.token().to_owned();
        let inbox = authority
            .mint_applet_ticket("inbox", "applet-inbox")
            .unwrap();
        let member_a = "a3s_ms_member.a.token";
        let member_b = "a3s_ms_member.b.token";

        let save = |token: &str| {
            request(
                port,
                "POST",
                "/api/member-session",
                &shell,
                Some(origin),
                &format!(
                    "{{\"token\":\"{token}\",\"display_name\":\"A\",\"phone\":\"13800138000\"}}"
                ),
            )
        };
        let refresh = || request(port, "GET", "/api/member/me", &shell, Some(origin), "");
        let center = || {
            request(
                port,
                "GET",
                "/api/applet-runtime/inbox/center",
                &inbox,
                Some(origin),
                "",
            )
        };
        let select = |id: &str| {
            request(
                port,
                "POST",
                "/api/member-center",
                &shell,
                Some(origin),
                &format!("{{\"id\":\"{id}\"}}"),
            )
        };

        assert_eq!(split_response(&save(member_a)).0, 200);
        assert_eq!(split_response(&refresh()).0, 200);
        assert_eq!(split_response(&select("org-a")).0, 200);
        assert!(split_response(&center()).2.contains("\"id\":\"org-a\""));

        assert_eq!(split_response(&save(member_b)).0, 200);
        assert!(
            split_response(&center()).2.contains("\"id\":\"personal\""),
            "a token change must not keep the previous team"
        );
        assert_eq!(split_response(&refresh()).0, 200);
        assert!(split_response(&center()).2.contains("\"id\":\"personal\""));
        assert_eq!(split_response(&select("org-b")).0, 200);
        assert!(split_response(&center()).2.contains("\"id\":\"org-b\""));

        let forged = request(
            port,
            "GET",
            "/api/applet-runtime/inbox/work-items?box=todo&org=org-a",
            &inbox,
            Some(origin),
            "",
        );
        assert_eq!(split_response(&forged).0, 200, "{forged}");

        assert_eq!(split_response(&save(member_a)).0, 200);
        assert_eq!(split_response(&refresh()).0, 200);
        assert!(
            split_response(&center()).2.contains("\"id\":\"org-a\""),
            "the first member keeps their own team"
        );

        let joined = seen.lock().expect("seen").join("\n");
        assert!(
            joined.contains("GET /v1/api/org/members/work-items?box=todo&org=org-b "),
            "{joined}"
        );
        assert!(!joined.contains("org=org-a"), "{joined}");
        stop.store(true, Ordering::SeqCst);
        server.join().unwrap();
    }

    #[test]
    fn saves_a_provider_without_putting_the_key_on_the_list() {
        let authority = StudioAuthority::start().unwrap();
        let port = authority.address().port();
        let origin = format!("http://127.0.0.1:{port}");
        let token = authority.token();
        let listed = request(port, "GET", "/api/providers", token, Some(&origin), "");
        let (status, _, body) = split_response(&listed);
        assert_eq!(status, 200, "{listed}");
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        let providers = json["providers"].as_array().unwrap();
        assert!(
            providers
                .iter()
                .any(|item| item["id"] == "openai" && item["name"] == "OpenAI")
        );
        assert!(providers.iter().all(|item| item.get("apiKey").is_none()));

        let secret = "sk-shell-provider-secret";
        let saved = request(
            port,
            "POST",
            "/api/providers/openai",
            token,
            Some(&origin),
            &format!("{{\"enabled\":true,\"apiKey\":\"{secret}\"}}"),
        );
        assert_eq!(split_response(&saved).0, 200, "{saved}");
        let detail: serde_json::Value = serde_json::from_str(&split_response(&saved).2).unwrap();
        assert_eq!(detail["enabled"].as_bool(), Some(true));
        assert_eq!(detail["apiKey"].as_str(), Some(secret));

        let again = request(port, "GET", "/api/providers", token, Some(&origin), "");
        let (_, _, list_body) = split_response(&again);
        assert!(!list_body.contains(secret), "{again}");

        let added = request(
            port,
            "POST",
            "/api/providers/openai/models",
            token,
            Some(&origin),
            "{\"id\":\"gpt-4o\",\"name\":\"gpt-4o\"}",
        );
        assert_eq!(split_response(&added).0, 200, "{added}");
        let after: serde_json::Value = serde_json::from_str(&split_response(&added).2).unwrap();
        assert_eq!(after["models"][0]["id"].as_str(), Some("gpt-4o"));
        assert_eq!(after["models"][0]["chat"].as_bool(), Some(true));

        let counted = request(port, "GET", "/api/providers", token, Some(&origin), "");
        let counted_json: serde_json::Value =
            serde_json::from_str(&split_response(&counted).2).unwrap();
        let openai = counted_json["providers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["id"] == "openai")
            .unwrap();
        assert_eq!(openai["enabledChatModels"].as_u64(), Some(1));
        assert_eq!(openai["hasApiKey"].as_bool(), Some(true));

        let defaults = request(
            port,
            "POST",
            "/api/providers/defaults",
            token,
            Some(&origin),
            "{\"defaultModel\":{\"providerId\":\"openai\",\"modelId\":\"gpt-4o\"},\"quickModel\":{\"providerId\":\"openai\",\"modelId\":\"gpt-4o\"},\"translateModel\":{\"providerId\":\"openai\",\"modelId\":\"gpt-4o\"}}",
        );
        assert_eq!(split_response(&defaults).0, 200, "{defaults}");

        let denied = request(port, "GET", "/api/providers", "nope", Some(&origin), "");
        assert_eq!(split_response(&denied).0, 401, "{denied}");
    }

    #[test]
    fn shipped_client_reads_the_live_session_and_fails_closed() {
        let authority = StudioAuthority::start().unwrap();
        let module = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../src/renderer/shell/studioClient.mjs");
        let module = std::fs::canonicalize(&module).unwrap();
        let url = format!("file://{}", module.display());
        let read = Command::new("node")
            .arg("--input-type=module")
            .arg("-e")
            .arg(format!(
                "import {{ readStudioBootstrap }} from {url:?};\n\
                 const host = {{ __A3S_STUDIO_RUNTIME__: {{ apiBaseUrl: process.env.STUDIO_API, token: process.env.STUDIO_TOKEN }} }};\n\
                 const body = await readStudioBootstrap(host);\n\
                 if (typeof body.sessionId !== 'string' || body.sessionId.length === 0) process.exit(2);\n\
                 process.stdout.write(body.sessionId);\n"
            ))
            .env("STUDIO_API", authority.api_base_url())
            .env("STUDIO_TOKEN", authority.token())
            .output()
            .expect("node");
        assert!(
            read.status.success(),
            "client read failed: {}",
            String::from_utf8_lossy(&read.stderr)
        );
        assert_eq!(
            String::from_utf8(read.stdout).unwrap(),
            authority.session_id()
        );

        let closed = Command::new("node")
            .arg("--input-type=module")
            .arg("-e")
            .arg(format!(
                "import {{ readStudioBootstrap }} from {url:?};\n\
                 let called = false;\n\
                 const fetchImpl = async () => {{ called = true; throw new Error('fetch'); }};\n\
                 try {{ await readStudioBootstrap({{}}, fetchImpl); process.exit(2); }} catch (error) {{\n\
                   if (called || !String(error).includes('absent')) process.exit(3);\n\
                 }}\n\
                 try {{\n\
                   await readStudioBootstrap({{ __A3S_STUDIO_RUNTIME__: {{ apiBaseUrl: 'http://example.com', token: '{}' }} }}, fetchImpl);\n\
                   process.exit(4);\n\
                 }} catch (error) {{\n\
                   if (called || !String(error).includes('invalid')) process.exit(5);\n\
                 }}\n\
                 process.stdout.write('closed');\n",
                "ab".repeat(32)
            ))
            .output()
            .expect("node");
        assert!(
            closed.status.success(),
            "fail closed failed: {}",
            String::from_utf8_lossy(&closed.stderr)
        );
        assert_eq!(String::from_utf8(closed.stdout).unwrap(), "closed");
    }
}
