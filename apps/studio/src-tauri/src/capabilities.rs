//! Installed connectors and skills on this machine. The OS owns the catalog.
//! Skills unpack into `skills/<slug>/`, then a symlink is published into
//! `~/.agents/skills/<name>/` so a3s-code and the other agents that follow the
//! Agent Skills layout can load them. The symlink is the only thing removed on
//! uninstall; a directory the user already had at that name is left in place.
//! Connectors land in `mcp.json`. a3s-code does not read that file;
//! [`acp_session_servers`] turns it into the `mcpServers` array sent on ACP
//! `session/new`. Plugin tools land in `tools.json` and ride `_meta.a3sTools`
//! on that same body. Plugin flows land in `flows.json` as a `WorkflowSpec`.
//! Publishing one calls [`a3s_flow::read_native_ts_entrypoint`], which opens
//! the unpacked source through `SourceSnapshot::read` and does not start the
//! compiler.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use a3s_use_reconcile::{
    PackageFile, PackageSpec, Payload, ReconcileError, ReconcileStore, SurfaceKind, SurfaceSpec,
    files_sha256,
};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

#[path = "capabilities_aix.rs"]
mod aix_projection;
#[path = "capabilities_selection.rs"]
mod selection;
#[path = "capabilities_ui.rs"]
mod ui_projection;
#[path = "capabilities_workbench.rs"]
mod workbench_projection;

pub(crate) use workbench_projection::ViewAgent;
pub(crate) use workbench_projection::ViewResourceIdentity;

const MAX_ENTRIES: usize = 512;
const MAX_UNPACKED: u64 = 64 * 1024 * 1024;
/// Compressed ceiling for a skill a person uploads. The unpacked ceiling stays `MAX_UNPACKED`.
pub(crate) const MAX_SKILL_ARCHIVE: usize = 32 * 1024 * 1024;
const MAX_SKILL_MD: u64 = 256 * 1024;
const MAX_DESCRIPTION_CHARS: usize = 1024;
const UPLOAD_MARKER: &str = ".a3s-upload";
const DIGEST_MARKER: &str = ".a3s-digest";
const PACKAGE_ID_MARKER: &str = ".a3s-package-id";

/// Package id recorded when this slug was installed. Absent when the slug is not installed.
pub(crate) fn installed_package_id(caps: &Path, slug: &str) -> Option<String> {
    if !crate::org_login::capability_slug_ok(slug) {
        return None;
    }
    let text = fs::read_to_string(caps.join("plugins").join(slug).join(PACKAGE_ID_MARKER)).ok()?;
    let id = text.trim();
    if id.is_empty() || id.len() > 200 || id.contains('\0') {
        return None;
    }
    Some(id.to_owned())
}
const REVIEW_PACKAGE_ID: &str = "use/a3s/review";
const CONFIG_FILE: &str = "connector-config.json";
/// Non-secret specs for connectors the member authored. Reconcile rewrites
/// `mcp.json` from the catalog, then merges this file back for names the
/// catalog does not own. Secrets stay in `connector-config.json`.
const CUSTOM_FILE: &str = "custom-connectors.json";
const CUSTOM_MARK: &str = "_a3s";

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PluginOkfSave {
    path: String,
    markdown: String,
    #[serde(default, rename = "workspaceWiki")]
    workspace_wiki: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PluginOkfSeed {
    directory: String,
}
const MAX_CUSTOM_COMMAND: usize = 1024;
const MAX_CUSTOM_ARG: usize = 1024;
const MAX_CUSTOM_ARGS: usize = 32;
const MAX_CUSTOM_URL: usize = 2048;
const MAX_CONFIG_FIELDS: usize = 32;
const MAX_CONFIG_VALUE: usize = 4096;
const MAX_CONFIG_BODY: usize = 64 * 1024;

/// One installed plugin package. `digest` is the zip. `package_id` is the reconcile id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginPin {
    pub require_aix: bool,
    pub digest: String,
    pub package_id: String,
}

/// The installed set as the OS reports it, reduced to what lands on disk.
#[derive(Debug, Default, PartialEq)]
pub struct Desired {
    /// Server entries for `mcp.json`, keyed by capability slug.
    pub mcp: Map<String, Value>,
    /// Skill slug → pinned package digest (`sha256:<hex>`).
    pub skills: BTreeMap<String, String>,
    /// Non-secret connector usage, keyed by slug. Command, env, and headers stay out.
    pub notes: BTreeMap<String, String>,
    /// Plugin slug → pinned zip and reconcile package id.
    pub plugins: BTreeMap<String, PluginPin>,
}

/// Reads the OS catalog (`/v1/api/org/capabilities` data). Malformed or uninstalled
/// entries are skipped; the OS validated every install spec on write.
pub fn desired(entries: &[Value]) -> Desired {
    let mut out = Desired::default();
    for entry in entries {
        if entry.get("installed").and_then(Value::as_bool) != Some(true) {
            continue;
        }
        let Some(slug) = entry
            .get("slug")
            .and_then(Value::as_str)
            .filter(|slug| crate::org_login::capability_slug_ok(slug))
        else {
            continue;
        };
        let Some(install) = entry.get("install") else {
            continue;
        };
        match install.get("type").and_then(Value::as_str) {
            Some("mcp") => {
                if install.get("packageType").and_then(Value::as_str) == Some("aix") {
                    let digest = install
                        .get("digest")
                        .and_then(Value::as_str)
                        .filter(|digest| digest_ok(digest));
                    let package_id = install
                        .get("packageId")
                        .and_then(Value::as_str)
                        .filter(|package_id| package_id_ok(package_id));
                    if let (Some(digest), Some(package_id)) = (digest, package_id) {
                        out.plugins.insert(
                            slug.to_owned(),
                            PluginPin {
                                require_aix: true,
                                digest: digest.to_owned(),
                                package_id: package_id.to_owned(),
                            },
                        );
                    }
                    continue;
                }
                if let Some(server) = mcp_server(install) {
                    let note = connector_usage(entry);
                    if !note.is_empty() {
                        out.notes.insert(slug.to_owned(), note);
                    }
                    out.mcp.insert(slug.to_owned(), server);
                }
            }
            Some("skill") => {
                if let Some(digest) = install
                    .get("digest")
                    .and_then(Value::as_str)
                    .filter(|digest| digest_ok(digest))
                {
                    out.skills.insert(slug.to_owned(), digest.to_owned());
                }
            }
            Some("plugin") => {
                if install
                    .get("packageType")
                    .is_some_and(|value| !matches!(value.as_str(), Some("native" | "aix")))
                {
                    continue;
                }
                if install
                    .get("available")
                    .is_some_and(|value| value.as_bool() != Some(true))
                {
                    continue;
                }
                let digest = install
                    .get("digest")
                    .and_then(Value::as_str)
                    .filter(|digest| digest_ok(digest));
                let package_id = install
                    .get("packageId")
                    .and_then(Value::as_str)
                    .filter(|package_id| package_id_ok(package_id));
                if let (Some(digest), Some(package_id)) = (digest, package_id) {
                    out.plugins.insert(
                        slug.to_owned(),
                        PluginPin {
                            require_aix: install.get("packageType").and_then(Value::as_str)
                                == Some("aix"),
                            digest: digest.to_owned(),
                            package_id: package_id.to_owned(),
                        },
                    );
                }
            }
            _ => {}
        }
    }
    out
}

/// Catalog `description` or `summary`. Install command, URL, env, and headers are ignored.
fn connector_usage(entry: &Value) -> String {
    let raw = entry
        .get("description")
        .and_then(Value::as_str)
        .or_else(|| entry.get("summary").and_then(Value::as_str))
        .unwrap_or("");
    crate::connector_notes::bounded_usage(raw)
}

/// `mcp.json` server shape shared by Claude Code, Cursor, and most MCP hosts.
fn mcp_server(install: &Value) -> Option<Value> {
    let text = |key: &str| install.get(key).and_then(Value::as_str);
    match text("transport")? {
        "stdio" => {
            let args: Vec<&str> = install
                .get("args")
                .and_then(Value::as_array)
                .map(|args| args.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            Some(json!({ "type": "stdio", "command": text("command")?, "args": args }))
        }
        "streamable_http" => Some(json!({ "type": "http", "url": text("url")? })),
        "sse" => Some(json!({ "type": "sse", "url": text("url")? })),
        _ => None,
    }
}

fn package_id_ok(raw: &str) -> bool {
    let segments: Vec<&str> = raw.split('/').collect();
    if segments.len() < 2 || raw.len() > 200 {
        return false;
    }
    segments.iter().all(|segment| {
        !segment.is_empty()
            && *segment != "."
            && *segment != ".."
            && segment.chars().all(|character| {
                character.is_ascii_alphanumeric()
                    || character == '-'
                    || character == '_'
                    || character == '.'
            })
    })
}

pub(crate) fn digest_ok(digest: &str) -> bool {
    digest.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

pub(crate) fn digest_of(bytes: &[u8]) -> String {
    let hash = Sha256::digest(bytes);
    let hex: String = hash.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("sha256:{hex}")
}

pub(crate) fn validate_plugin_publication(
    publication: &a3s_use_reconcile::Publication,
) -> io::Result<()> {
    aix_projection::validate_publication(publication, false)
}

pub(crate) fn validate_plugin_selection<'a>(
    publications: impl IntoIterator<Item = &'a a3s_use_reconcile::Publication>,
) -> io::Result<()> {
    aix_projection::validate_selection(publications)
}

pub struct LocalCapabilities {
    root: Option<PathBuf>,
    reconciling: Mutex<()>,
    ui_sessions: Mutex<workbench_projection::UiSessions>,
}

impl LocalCapabilities {
    pub fn open(slug: &str) -> Self {
        Self::open_at(root_path(slug))
    }

    pub fn open_at(root: Option<PathBuf>) -> Self {
        Self {
            root,
            reconciling: Mutex::new(()),
            ui_sessions: Mutex::new(workbench_projection::UiSessions::default()),
        }
    }

    /// Where installed skills land; `None` in tests and for invalid tenant slugs.
    pub fn skills_dir(&self) -> Option<PathBuf> {
        self.root.as_ref().map(|root| root.join("skills"))
    }

    pub(crate) fn drafts_dir(&self) -> Option<PathBuf> {
        self.root.as_ref().map(|root| root.join("authoring-drafts"))
    }

    /// Write the member's saved wiki pages beside this capabilities directory.
    pub fn project_wiki(&self, body: &[u8]) -> io::Result<()> {
        let Some(root) = &self.root else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "unavailable"));
        };
        let Some(tenant) = root.parent() else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "unavailable"));
        };
        crate::local_assets::project_wiki(&tenant.join("wiki"), body)
    }

    /// Surfaces this machine can call for one installed plugin package.
    pub fn callable_plugin(&self, slug: &str) -> io::Result<Vec<u8>> {
        let Some(root) = &self.root else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "unavailable"));
        };
        let _guard = self.okf_lock();
        callable_plugin_json(root, slug)
    }

    pub fn plugin_ui(&self, slug: &str, surface: &str) -> io::Result<Vec<u8>> {
        let root = self
            .root
            .as_deref()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        let _guard = self.okf_lock();
        ui_projection::document(root, slug, surface)
    }

    pub fn plugin_views(&self) -> io::Result<Vec<u8>> {
        let root = self
            .root
            .as_deref()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        let _guard = self.okf_lock();
        workbench_projection::catalogue(root)
    }

    pub fn open_plugin_view(&self, owner: &str, body: &[u8]) -> io::Result<Vec<u8>> {
        let root = self
            .root
            .as_deref()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        let _guard = self.okf_lock();
        let mut sessions = self
            .ui_sessions
            .lock()
            .map_err(|_| io::Error::from(io::ErrorKind::Other))?;
        sessions.open(root, owner, body)
    }

    pub fn plugin_view_asset(
        &self,
        owner: &str,
        token: &str,
        path: &str,
    ) -> io::Result<(&'static str, Vec<u8>)> {
        let root = self
            .root
            .as_deref()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        let _guard = self.okf_lock();
        let mut sessions = self
            .ui_sessions
            .lock()
            .map_err(|_| io::Error::from(io::ErrorKind::Other))?;
        sessions.asset(root, owner, token, path)
    }

    pub fn close_plugin_view(&self, owner: &str, token: &str) -> io::Result<()> {
        let mut sessions = self
            .ui_sessions
            .lock()
            .map_err(|_| io::Error::from(io::ErrorKind::Other))?;
        sessions.close(owner, token)
    }

    /// The edge package agent the live view `token` may call. Denied and missing are both NotFound.
    pub fn plugin_view_agent(
        &self,
        owner: &str,
        token: &str,
        agent: &str,
    ) -> io::Result<ViewAgent> {
        let root = self
            .root
            .as_deref()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        let _guard = self.okf_lock();
        let mut sessions = self
            .ui_sessions
            .lock()
            .map_err(|_| io::Error::from(io::ErrorKind::Other))?;
        sessions.agent(root, owner, token, agent)
    }

    /// Serializes resource commits with view closure and package withdrawal.
    pub fn with_plugin_view_resource<T>(
        &self,
        owner: &str,
        token: &str,
        action: impl FnOnce(&ViewResourceIdentity) -> io::Result<T>,
    ) -> io::Result<T> {
        let root = self
            .root
            .as_deref()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        let _guard = self
            .reconciling
            .lock()
            .map_err(|_| io::Error::other("capability reconcile lock failed"))?;
        let mut sessions = self
            .ui_sessions
            .lock()
            .map_err(|_| io::Error::other("view session lock failed"))?;
        sessions.with_resource(root, owner, token, action)
    }

    pub fn cancel_plugin_view_request(
        &self,
        owner: &str,
        request_id: &str,
        body: &[u8],
    ) -> io::Result<Vec<u8>> {
        let mut sessions = self
            .ui_sessions
            .lock()
            .map_err(|_| io::Error::from(io::ErrorKind::Other))?;
        sessions.cancel_request(owner, request_id, body)
    }

    pub fn list_plugin_okf(&self, slug: &str) -> io::Result<Vec<u8>> {
        let tenant = self.tenant_root()?;
        let _guard = self.okf_lock();
        let pages = crate::plugin_okf::list_pages(&tenant, slug)?;
        json_bytes(&serde_json::json!({ "pages": pages }))
    }

    pub fn read_plugin_okf(&self, slug: &str, relative: &str) -> io::Result<Vec<u8>> {
        let tenant = self.tenant_root()?;
        let _guard = self.okf_lock();
        let page = crate::plugin_okf::read_page(&tenant, slug, relative)?;
        json_bytes(&serde_json::json!(page))
    }

    pub fn save_plugin_okf(&self, slug: &str, body: &[u8]) -> io::Result<Vec<u8>> {
        let tenant = self.tenant_root()?;
        let _guard = self.okf_lock();
        let request: PluginOkfSave = serde_json::from_slice(body)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid"))?;
        let workspace = request
            .workspace_wiki
            .as_deref()
            .filter(|path| !path.is_empty())
            .map(PathBuf::from);
        let indexed = crate::plugin_okf::save_page(
            &tenant,
            slug,
            &request.path,
            &request.markdown,
            workspace.as_deref(),
        )?;
        json_bytes(&serde_json::json!({ "ok": true, "indexed": indexed }))
    }

    pub fn seed_plugin_okf(&self, slug: &str, body: &[u8]) -> io::Result<Vec<u8>> {
        let tenant = self.tenant_root()?;
        let _guard = self.okf_lock();
        let request: PluginOkfSeed = serde_json::from_slice(body)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid"))?;
        let seeded = crate::plugin_okf::seed_missing(&tenant, slug, Path::new(&request.directory))?;
        json_bytes(&serde_json::json!({ "seeded": seeded }))
    }

    fn tenant_root(&self) -> io::Result<PathBuf> {
        let Some(root) = &self.root else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "unavailable"));
        };
        root.parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "unavailable"))
    }

    fn okf_lock(&self) -> std::sync::MutexGuard<'_, ()> {
        self.reconciling
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    /// Makes disk match the OS installed set: rewrites `mcp.json`, downloads skills
    /// whose pinned package changed, and deletes skills no longer installed.
    pub fn reconcile(
        &self,
        catalog: &[Value],
        bundle: impl Fn(&str) -> io::Result<Vec<u8>>,
    ) -> io::Result<()> {
        let Some(root) = &self.root else {
            return Ok(());
        };
        for entry in catalog
            .iter()
            .filter(|e| e.get("installed").and_then(Value::as_bool) == Some(true))
        {
            let Some(install) = entry.get("install") else {
                continue;
            };
            if install.get("type").and_then(Value::as_str) == Some("plugin") {
                if install
                    .get("packageType")
                    .is_some_and(|value| !matches!(value.as_str(), Some("native" | "aix")))
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "unsupported plugin package type",
                    ));
                }
            }
        }
        let _guard = self
            .reconciling
            .lock()
            .map_err(|_| io::Error::other("capability reconcile lock failed"))?;
        reconcile_at(root, &desired(catalog), bundle)
    }

    /// Member-supplied connector values on this machine. `{}` when there is no directory.
    pub fn config_json(&self) -> String {
        let Some(root) = &self.root else {
            return "{}".to_owned();
        };
        let _guard = self.reconciling.lock().expect("capability config");
        serde_json::to_string(&Value::Object(sanitize_config(&load_config(root))))
            .unwrap_or_else(|_| "{}".to_owned())
    }

    /// Stores one connector's values and refreshes `mcp.json` when that file exists.
    pub fn save_config(&self, slug: &str, body: &[u8]) -> io::Result<()> {
        let Some(root) = &self.root else {
            return Ok(());
        };
        let _guard = self.reconciling.lock().expect("capability config");
        store_config(root, slug, body)
    }

    /// Custom connectors on this device. `Err(unavailable)` when this process has no directory.
    pub fn custom_json(&self) -> io::Result<String> {
        let Some(root) = &self.root else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "unavailable"));
        };
        let _guard = self.reconciling.lock().expect("capability custom");
        Ok(list_custom_json(root))
    }

    /// Validates and stores one custom connector, then refreshes `mcp.json`.
    pub fn save_custom(&self, body: &[u8]) -> io::Result<()> {
        let Some(root) = &self.root else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "unavailable"));
        };
        let _guard = self.reconciling.lock().expect("capability custom");
        save_custom_connector(root, body)
    }

    /// Removes one custom connector. A catalog server with the same name is left alone.
    pub fn remove_custom(&self, name: &str) -> io::Result<()> {
        let Some(root) = &self.root else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "unavailable"));
        };
        let _guard = self.reconciling.lock().expect("capability custom");
        remove_custom_connector(root, name)
    }
}

pub fn reconcile_at(
    root: &Path,
    desired: &Desired,
    bundle: impl Fn(&str) -> io::Result<Vec<u8>>,
) -> io::Result<()> {
    fs::create_dir_all(root)?;
    let mut servers = desired.mcp.clone();
    // Catalog entries win on a shared name. Custom servers are merged back so
    // this rewrite does not delete a connector the member authored.
    let mut custom_names = BTreeSet::new();
    for (name, spec) in load_custom(root) {
        if servers.contains_key(&name) {
            continue;
        }
        custom_names.insert(name.clone());
        servers.insert(name, spec);
    }
    publish_mcp(root, &mut servers, &custom_names)?;
    write_connector_notes(root, &servers, &desired.mcp, &desired.notes)?;

    let skills = root.join("skills");
    fs::create_dir_all(&skills)?;
    for entry in fs::read_dir(&skills)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !desired.skills.contains_key(&name) {
            remove_any(&entry.path())?;
            if let Some(store) = package_store(skills.parent()) {
                let _ = store.withdraw_sync(&format!("catalog/{name}"));
            }
        }
    }
    let mut failures = Vec::new();
    for (slug, digest) in &desired.skills {
        let target = skills.join(slug);
        if fs::read_to_string(target.join(DIGEST_MARKER))
            .ok()
            .as_deref()
            == Some(digest)
        {
            continue;
        }
        if let Err(err) = install_skill(&skills, slug, digest, &bundle) {
            failures.push(format!("{slug}: {err}"));
        }
    }
    if let Some(agents) = user_agents_skills_dir() {
        if let Err(err) = publish_skills(&skills, &agents, &desired.skills) {
            failures.push(format!("agents: {err}"));
        }
    }
    // Package generations, queries, and scope leases all belong to this tenant.
    // The optional global Agent Skills projection must not choose the package store.
    let tenant = root
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "tenant root is missing"))?;
    let agents = tenant.join("agents");
    fs::create_dir_all(&agents)?;
    if let Err(err) = sync_plugins(root, &agents, &desired.plugins, &bundle) {
        failures.push(format!("plugins: {err}"));
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(io::Error::other(failures.join("; ")))
    }
}

/// `~/.agents/skills`, the user-level directory from the Agent Skills layout.
/// Hidden under test so a library run cannot relink the developer's home.
pub(crate) fn user_agents_skills_dir() -> Option<PathBuf> {
    if cfg!(test) {
        return None;
    }
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".agents").join("skills"))
}

/// Point `~/.agents/skills/<name>` at each installed package. A name already
/// occupied by a directory this install does not own is left alone, and the
/// package is linked under its slug when that path is free.
pub(crate) fn publish_skills(
    skills_root: &Path,
    agents_root: &Path,
    installed: &BTreeMap<String, String>,
) -> io::Result<()> {
    use std::collections::BTreeSet;

    fs::create_dir_all(agents_root)?;
    let skills_root = fs::canonicalize(skills_root).unwrap_or_else(|_| skills_root.to_path_buf());
    let mut wanted: Vec<(String, PathBuf)> = Vec::new();
    let mut taken = BTreeSet::new();
    for slug in installed.keys() {
        let source = skills_root.join(slug);
        if !source.join("SKILL.md").is_file() {
            continue;
        }
        let preferred = skill_link_name(&source, slug);
        let name = [preferred, slug.clone()].into_iter().find(|candidate| {
            !candidate.is_empty()
                && skill_name_ok(candidate)
                && !taken.contains(candidate)
                && !crate::skill_links::held_by_review(agents_root, candidate)
                && slot_claimable(&agents_root.join(candidate), &skills_root)
        });
        let Some(name) = name else {
            continue;
        };
        taken.insert(name.clone());
        wanted.push((name, source));
    }
    if let Ok(entries) = fs::read_dir(agents_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if managed_link_target(&path, &skills_root).is_none() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if crate::skill_links::held_by_review(agents_root, &name) {
                continue;
            }
            let current = wanted.iter().find(|(wanted_name, _)| wanted_name == &name);
            if current.is_some_and(|(_, source)| links_to(&path, source)) {
                continue;
            }
            fs::remove_file(&path)?;
            match fs::remove_file(crate::skill_links::owner_file(agents_root, &name)) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
    }
    for (name, source) in &wanted {
        let link = agents_root.join(name);
        if links_to(&link, source) {
            crate::skill_links::write_owner(agents_root, crate::skill_links::OWNER_CATALOG, name)?;
            continue;
        }
        if link.symlink_metadata().is_ok() {
            continue;
        }
        claim_catalog_link(agents_root, name, source)?;
    }
    Ok(())
}

fn claim_catalog_link(agents: &Path, name: &str, source: &Path) -> io::Result<()> {
    crate::skill_links::claim_skill_link(agents, crate::skill_links::OWNER_CATALOG, name, source)
        .map_err(|error| match error {
            crate::skill_links::ClaimError::Io(error) => error,
            crate::skill_links::ClaimError::HeldByOther
            | crate::skill_links::ClaimError::UserOwned => io::Error::new(
                io::ErrorKind::AlreadyExists,
                "skill link is owned by someone else",
            ),
        })
}

/// Folder name agents expect: a valid frontmatter `name`, otherwise `id`,
/// otherwise the catalog slug. Display names such as `Agent Shield` are not
/// directory names.
fn skill_link_name(skill_dir: &Path, slug: &str) -> String {
    let text = fs::read_to_string(skill_dir.join("SKILL.md")).unwrap_or_default();
    let matter = front_matter(&text);
    for key in ["name", "id"] {
        if let Some(value) = matter_scalar(matter, key) {
            if skill_name_ok(&value) {
                return value;
            }
        }
    }
    if skill_name_ok(slug) {
        slug.to_owned()
    } else {
        String::new()
    }
}

/// A missing path, or a symlink this install already owns, can take the name.
/// A real directory or someone else's symlink cannot.
fn place_link(source: &Path, link: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(source, link)
    }
    #[cfg(not(unix))]
    {
        let _ = (source, link);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "skill links are published on unix",
        ))
    }
}

fn slot_claimable(path: &Path, skills_root: &Path) -> bool {
    match path.symlink_metadata() {
        Err(err) if err.kind() == io::ErrorKind::NotFound => true,
        Ok(meta) if meta.file_type().is_symlink() => {
            managed_link_target(path, skills_root).is_some()
        }
        _ => false,
    }
}

fn links_to(link: &Path, source: &Path) -> bool {
    let Ok(meta) = link.symlink_metadata() else {
        return false;
    };
    if !meta.file_type().is_symlink() {
        return false;
    }
    let Ok(raw) = fs::read_link(link) else {
        return false;
    };
    let absolute = if raw.is_absolute() {
        raw
    } else {
        let Some(parent) = link.parent() else {
            return false;
        };
        parent.join(raw)
    };
    fs::canonicalize(&absolute).ok().as_ref() == fs::canonicalize(source).ok().as_ref()
        && fs::canonicalize(source).is_ok()
}

fn managed_link_target(path: &Path, skills_root: &Path) -> Option<PathBuf> {
    let meta = fs::symlink_metadata(path).ok()?;
    if !meta.file_type().is_symlink() {
        return None;
    }
    let raw = fs::read_link(path).ok()?;
    let absolute = if raw.is_absolute() {
        raw
    } else {
        path.parent()?.join(raw)
    };
    let target = fs::canonicalize(&absolute).unwrap_or(absolute);
    target.starts_with(skills_root).then_some(target)
}

fn skill_name_ok(name: &str) -> bool {
    let bytes = name.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 64
        && bytes[0].is_ascii_lowercase()
        && !name.ends_with('-')
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

fn front_matter(text: &str) -> &str {
    let text = text.trim_start();
    let Some(rest) = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))
    else {
        return "";
    };
    rest.split_once("\n---")
        .map(|(block, _)| block)
        .unwrap_or("")
}

fn matter_scalar(block: &str, wanted: &str) -> Option<String> {
    if block.is_empty() {
        return None;
    }
    for line in block.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('-') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        if key.trim() == wanted {
            let value = value.trim().trim_matches(['"', '\'']).trim();
            if !value.is_empty() {
                return Some(value.to_owned());
            }
        }
    }
    None
}

/// Install and remove plugin packages under `plugins/<slug>/`.
/// The generation store is `agents/../package-generations`, the same root the
/// review window reads. A digest match still adopts the generation so a
/// restarted process fills the window. The digest marker is written only
/// after that adopt succeeds.
pub(crate) fn sync_plugins(
    root: &Path,
    agents: &Path,
    pins: &BTreeMap<String, PluginPin>,
    bundle: &impl Fn(&str) -> io::Result<Vec<u8>>,
) -> io::Result<()> {
    aix_projection::preflight_dependencies(root, agents, pins, bundle)?;
    let selection = selection::InstalledSelection::prepare(agents, pins)?;
    let plugins = root.join("plugins");
    fs::create_dir_all(&plugins)?;
    let mut failures = Vec::new();
    for entry in fs::read_dir(&plugins)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            let _ = remove_any(&entry.path());
            continue;
        }
        if pins.contains_key(&name) {
            continue;
        }
        if let Err(error) = remove_plugin(&plugins, agents, &name) {
            failures.push(format!("{name}: {error}"));
        }
    }
    for (slug, pin) in pins {
        if let Err(error) = install_plugin(&plugins, agents, slug, pin, bundle) {
            failures.push(format!("{slug}: {error}"));
        }
    }
    if let Err(error) = publish_plugin_surfaces(root) {
        failures.push(format!("mcp: {error}"));
    }
    if failures.is_empty() {
        selection.publish(root, pins)
    } else {
        Err(io::Error::other(failures.join("; ")))
    }
}

fn install_plugin(
    plugins: &Path,
    agents: &Path,
    slug: &str,
    pin: &PluginPin,
    bundle: &impl Fn(&str) -> io::Result<Vec<u8>>,
) -> io::Result<()> {
    let target = plugins.join(slug);
    if plugin_marker_matches(&target, pin) {
        match adopt_installed_plugin(agents, &target, pin) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    let bytes = bundle(slug)?;
    if digest_of(&bytes) != pin.digest {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "bundle does not match its pinned digest",
        ));
    }
    let staging = plugins.join(format!(".staging-{slug}"));
    remove_any(&staging)?;
    if let Err(error) = unpack(&bytes, &staging) {
        let _ = remove_any(&staging);
        return Err(error);
    }
    if pin.require_aix && !staging.join("a3s-use-extension.acl").is_file() {
        let _ = remove_any(&staging);
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "AIX installation requires its native manifest",
        ));
    }
    let spec = match package_spec(&staging, &pin.package_id) {
        Ok(spec) => spec,
        Err(error) => {
            let _ = remove_any(&staging);
            return Err(error);
        }
    };
    let store = plugin_store(agents).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "package generation root is unavailable",
        )
    })?;
    if let Err(error) = store.apply_sync(&spec) {
        let _ = remove_any(&staging);
        return Err(host_skill_error(error));
    }
    remove_any(&target)?;
    if let Err(error) = fs::rename(&staging, &target) {
        let _ = remove_any(&staging);
        return Err(error);
    }
    fs::write(target.join(PACKAGE_ID_MARKER), &pin.package_id)?;
    if let Err(error) = adopt_installed_plugin(agents, &target, pin) {
        return Err(error);
    }
    fs::write(target.join(DIGEST_MARKER), &pin.digest)?;
    Ok(())
}

fn plugin_marker_matches(target: &Path, pin: &PluginPin) -> bool {
    (!pin.require_aix || target.join("a3s-use-extension.acl").is_file())
        && fs::read_to_string(target.join(DIGEST_MARKER))
            .ok()
            .as_deref()
            == Some(pin.digest.as_str())
        && fs::read_to_string(target.join(PACKAGE_ID_MARKER))
            .ok()
            .as_deref()
            == Some(pin.package_id.as_str())
}

fn adopt_installed_plugin(agents: &Path, target: &Path, pin: &PluginPin) -> io::Result<()> {
    let store = plugin_store(agents)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "package generation is missing"))?;
    let publication = store
        .current_sync(&pin.package_id)
        .map_err(host_skill_error)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "package generation is missing"))?;
    if (pin.require_aix || target.join("a3s-use-extension.acl").is_file())
        && !publication
            .surfaces
            .iter()
            .any(|surface| surface.directory.join("a3s-use-extension.acl").is_file())
    {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "AIX generation requires verified manifest reinstall",
        ));
    }
    aix_projection::validate_publication(&publication, pin.require_aix)?;
    if pin.package_id == REVIEW_PACKAGE_ID {
        return crate::review_plugin::adopt_catalog_review(agents, &target.join("skills/review"));
    }
    link_package_skills(agents, target, &pin.package_id)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for surface in &publication.surfaces {
            if surface.kind == SurfaceKind::Mcp && surface.entry.is_none() {
                if let Some(command) = surface
                    .mcp
                    .as_ref()
                    .filter(|server| server["type"] == "stdio")
                    .and_then(|server| server["command"].as_str())
                {
                    if surface.directory.join(command).is_file() {
                        return Err(io::Error::new(
                            io::ErrorKind::NotFound,
                            "package MCP requires a verified managed-entry reinstall",
                        ));
                    }
                }
            }
            if matches!(surface.kind, SurfaceKind::Tool | SurfaceKind::Mcp) {
                if let Some(entry) = &surface.entry {
                    if fs::symlink_metadata(entry)?.permissions().mode() & 0o100 == 0 {
                        return Err(io::Error::new(
                            io::ErrorKind::NotFound,
                            "package Tool requires verified reinstall",
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

fn remove_plugin(plugins: &Path, agents: &Path, slug: &str) -> io::Result<()> {
    let target = plugins.join(slug);
    let package_id = fs::read_to_string(target.join(PACKAGE_ID_MARKER)).unwrap_or_default();
    let package_id = package_id.trim();
    if package_id == REVIEW_PACKAGE_ID {
        crate::review_plugin::release_catalog_review(agents)?;
    }
    if package_id_ok(package_id) {
        let store = plugin_store(agents).ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "package generation is unavailable")
        })?;
        store.withdraw_sync(package_id).map_err(host_skill_error)?;
        release_package_skills(agents, &target, package_id)?;
        release_owned_skill_links(agents, package_id)?;
    }
    release_links_into(agents, &target, package_id)?;
    if package_id == crate::agent_plane::presenton_mcp::PACKAGE_ID {
        crate::agent_plane::presenton_mcp::release_installed();
        crate::aix_applet_window::release_home_slug(crate::agent_plane::presenton_mcp::SLUG);
    }
    if crate::org_login::capability_slug_ok(slug) {
        if let Some(tenant) = plugins.parent().and_then(|root| root.parent()) {
            crate::plugin_okf::remove_local_state(tenant, slug)?;
        }
    }
    remove_any(&target)
}

fn plugin_store(agents: &Path) -> Option<ReconcileStore> {
    let parent = agents.parent()?;
    ReconcileStore::open(parent.join("package-generations")).ok()
}

const PLUGIN_MCP_MARK: &str = "_a3sPlugin";

fn package_spec(root: &Path, package_id: &str) -> io::Result<PackageSpec> {
    let mut files = Vec::new();
    collect_files(root, root, &mut files)?;
    if files
        .iter()
        .any(|(path, _)| path == "a3s-use-extension.acl")
    {
        return aix_projection::package_spec(&files, package_id);
    }
    let mut skills: BTreeMap<String, Vec<PackageFile>> = BTreeMap::new();
    let mut interfaces: BTreeMap<String, Vec<PackageFile>> = BTreeMap::new();
    let mut servers: BTreeMap<String, Vec<PackageFile>> = BTreeMap::new();
    let mut tools: BTreeMap<String, Vec<PackageFile>> = BTreeMap::new();
    let mut flows: BTreeMap<String, Vec<PackageFile>> = BTreeMap::new();
    for (relative, bytes) in files {
        let mut parts = relative.split('/');
        let head = parts.next().unwrap_or("");
        if !matches!(
            head,
            "skills" | "ui" | "mcp" | "tool" | "tools" | "flow" | "flows"
        ) {
            continue;
        }
        let name = parts.next().unwrap_or("");
        let rest = parts.collect::<Vec<_>>().join("/");
        let (bucket, id, path) = if rest.is_empty() {
            // Use packages ship `mcp/context`, `tools/echo`, and `flows/reason.ts`
            // as files. An empty remainder is that file, not a broken directory.
            let Some(flat) = flat_surface(head, name) else {
                continue;
            };
            flat
        } else {
            if !surface_id_ok(name) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "bundle surface id is invalid",
                ));
            }
            let path = if head == "ui" {
                format!("ui/{name}/{rest}")
            } else {
                rest
            };
            (head, name.to_owned(), path)
        };
        let bucket = match bucket {
            "skills" => &mut skills,
            "ui" => &mut interfaces,
            "mcp" => &mut servers,
            "tool" | "tools" => &mut tools,
            "flow" | "flows" => &mut flows,
            _ => continue,
        };
        bucket
            .entry(id)
            .or_default()
            .push(PackageFile { path, bytes });
    }
    if skills.is_empty()
        && interfaces.is_empty()
        && servers.is_empty()
        && tools.is_empty()
        && flows.is_empty()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "bundle has no surface",
        ));
    }
    let mut surfaces = Vec::new();
    for (id, payload) in skills {
        if !payload
            .iter()
            .any(|file| file.path == "SKILL.md" || file.path.ends_with("/SKILL.md"))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "bundle has no SKILL.md",
            ));
        }
        surfaces.push(surface_files(SurfaceKind::Skill, id, None, payload)?);
    }
    for (id, payload) in interfaces {
        let entry = format!("ui/{id}/index.html");
        if !payload.iter().any(|file| file.path == entry) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "bundle has no index.html",
            ));
        }
        surfaces.push(surface_files(SurfaceKind::Ui, id, Some(entry), payload)?);
    }
    for (id, mut payload) in servers {
        if !payload.iter().any(|file| file.path == "server.json") {
            if !payload.iter().any(|file| file.path == id) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "bundle has no server.json",
                ));
            }
            let spec = serde_json::to_vec(&json!({
                "type": "stdio",
                "command": id,
                "args": ["--stdio"]
            }))
            .map_err(|_| io::Error::other("bundle mcp server is invalid"))?;
            payload.push(PackageFile {
                path: "server.json".to_owned(),
                bytes: spec,
            });
        }
        let spec = payload
            .iter()
            .find(|file| file.path == "server.json")
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "bundle has no server.json")
            })?;
        if mcp_server_value(&spec.bytes).is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "bundle mcp server is invalid",
            ));
        }
        surfaces.push(surface_files(SurfaceKind::Mcp, id, None, payload)?);
    }
    for (id, payload) in tools {
        let entry = tool_entry(&id, &payload)?;
        surfaces.push(surface_files(SurfaceKind::Tool, id, Some(entry), payload)?);
    }
    for (id, payload) in flows {
        let entry = flow_entry(&payload)?;
        surfaces.push(surface_files(SurfaceKind::Flow, id, Some(entry), payload)?);
    }
    Ok(PackageSpec {
        package_id: package_id.to_owned(),
        surfaces,
    })
}

fn surface_files(
    kind: SurfaceKind,
    id: String,
    entry: Option<String>,
    payload: Vec<PackageFile>,
) -> io::Result<SurfaceSpec> {
    Ok(SurfaceSpec {
        kind,
        id,
        sha256: files_sha256(&payload).map_err(host_skill_error)?,
        entry,
        payload: Payload::Files(payload),
    })
}

fn tool_entry(id: &str, files: &[PackageFile]) -> io::Result<String> {
    if let Some(manifest) = files.iter().find(|file| file.path == "tool.json") {
        let value: Value = serde_json::from_slice(&manifest.bytes).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "bundle tool manifest is invalid",
            )
        })?;
        let entry = value.get("entry").and_then(Value::as_str).unwrap_or("");
        if !entry.is_empty() && files.iter().any(|file| file.path == entry) {
            return Ok(entry.to_owned());
        }
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "bundle tool has no entry",
        ));
    }
    let fallback = format!("bin/{id}");
    if files.iter().any(|file| file.path == fallback) {
        return Ok(fallback);
    }
    if files.iter().any(|file| file.path == id) {
        return Ok(id.to_owned());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "bundle tool has no entry",
    ))
}

fn flow_entry(files: &[PackageFile]) -> io::Result<String> {
    if let Some(manifest) = files.iter().find(|file| file.path == "flow.json") {
        let value: Value = serde_json::from_slice(&manifest.bytes).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "bundle flow manifest is invalid",
            )
        })?;
        let source = value.get("source").and_then(Value::as_str).unwrap_or("");
        if !source.is_empty() && files.iter().any(|file| file.path == source) {
            return Ok(source.to_owned());
        }
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "bundle flow has no source",
        ));
    }
    if files.iter().any(|file| file.path == "index.ts") {
        return Ok("index.ts".to_owned());
    }
    if let Some(source) = files.iter().find(|file| {
        let path = file.path.as_str();
        path.ends_with(".ts") && !path.contains('/')
    }) {
        return Ok(source.path.clone());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "bundle flow has no source",
    ))
}

/// `mcp/context`, `tools/echo`, and `flows/reason.ts` are files, not directories.
fn flat_surface(head: &str, name: &str) -> Option<(&'static str, String, String)> {
    if name.is_empty() || name == "." || name == ".." {
        return None;
    }
    let id = match head {
        "mcp" | "tool" | "tools" => name.to_owned(),
        "flow" | "flows" => file_stem(name).to_owned(),
        _ => return None,
    };
    if !surface_id_ok(&id) {
        return None;
    }
    let bucket = match head {
        "mcp" => "mcp",
        "tool" | "tools" => "tool",
        _ => "flow",
    };
    Some((bucket, id, name.to_owned()))
}

fn file_stem(name: &str) -> &str {
    name.rsplit_once('.')
        .map(|(stem, _extension)| stem)
        .filter(|stem| !stem.is_empty())
        .unwrap_or(name)
}

fn mcp_server_value(bytes: &[u8]) -> Option<Value> {
    let value: Value = serde_json::from_slice(bytes).ok()?;
    let object = value.as_object()?;
    match object.get("type").and_then(Value::as_str) {
        Some("stdio") => {
            let command = object.get("command").and_then(Value::as_str)?;
            if command.is_empty() || command.contains('\0') {
                return None;
            }
            Some(value)
        }
        Some("http") | Some("sse") => {
            let url = object.get("url").and_then(Value::as_str)?;
            if url.starts_with("https://") || url.starts_with("http://") {
                Some(value)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// One installed plugin surface the shell can call.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PluginCall {
    kind: &'static str,
    id: String,
    detail: Value,
}

const PLUGIN_TOOLS_FILE: &str = "tools.json";
const PLUGIN_FLOWS_FILE: &str = "flows.json";

/// Rewrites plugin MCP, tool, and flow records from the unpacked plugins.
/// Catalog MCP servers stay. Tool and flow files contain only installed plugins,
/// so an uninstall clears the previous entries.
fn publish_plugin_surfaces(caps: &Path) -> io::Result<()> {
    let mut plugin_servers: Map<String, Value> = Map::new();
    let mut tools = Vec::new();
    let mut flows = Vec::new();
    let mut seen_tools = BTreeSet::new();
    let mut seen_flows = BTreeSet::new();
    let plugins = caps.join("plugins");
    if let Ok(read) = fs::read_dir(&plugins) {
        for entry in read.flatten() {
            let slug = entry.file_name().to_string_lossy().into_owned();
            if slug.starts_with('.') || !crate::org_login::capability_slug_ok(&slug) {
                continue;
            }
            for call in published_plugin_calls(caps, &slug)? {
                match call.kind {
                    "mcp" => {
                        if plugin_servers.contains_key(&call.id) {
                            continue;
                        }
                        let mut spec = call.detail;
                        if let Some(object) = spec.as_object_mut() {
                            object.insert(PLUGIN_MCP_MARK.to_owned(), json!(slug));
                        }
                        plugin_servers.insert(call.id, spec);
                    }
                    "tool" => {
                        let Some(entry_path) = call.detail.get("entry").and_then(Value::as_str)
                        else {
                            continue;
                        };
                        if !seen_tools.insert(call.id.clone()) {
                            continue;
                        }
                        tools.push(json!({ "id": call.id, "entry": entry_path }));
                    }
                    "flow" => {
                        let Some(source) = call.detail.get("source").and_then(Value::as_str) else {
                            continue;
                        };
                        if !seen_flows.insert(call.id.clone()) {
                            continue;
                        }
                        let mut row = json!({
                            "name": call.id,
                            "version": "1",
                            "runtime": {
                                "kind": "native_ts",
                                "entrypoint": source,
                                "export_name": "run"
                            }
                        });
                        let read = read_flow_source(&row)?;
                        if read.logical_path != source || read.path != Path::new(source) {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "flow source was not read",
                            ));
                        }
                        row["sourceHash"] = json!(read.source_hash);
                        flows.push(row);
                    }
                    _ => {}
                }
            }
        }
    }
    tools.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    flows.sort_by(|left, right| left["name"].as_str().cmp(&right["name"].as_str()));
    write_atomic(
        &caps.join(PLUGIN_TOOLS_FILE),
        &serde_json::to_vec_pretty(&json!({ "tools": tools }))?,
    )?;
    write_atomic(
        &caps.join(PLUGIN_FLOWS_FILE),
        &serde_json::to_vec_pretty(&json!({ "flows": flows }))?,
    )?;
    let path = caps.join("mcp.json");
    let mut servers = Map::new();
    if let Ok(bytes) = fs::read(&path) {
        if let Ok(doc) = serde_json::from_slice::<Value>(&bytes) {
            if let Some(existing) = doc.get("mcpServers").and_then(Value::as_object) {
                for (name, spec) in existing {
                    if spec.get(PLUGIN_MCP_MARK).and_then(Value::as_str).is_some() {
                        continue;
                    }
                    servers.insert(name.clone(), spec.clone());
                }
            }
        }
    }
    for (name, spec) in plugin_servers {
        // The verified package replaces a same-named server left by an older install.
        servers.insert(name, spec);
    }
    let mcp = serde_json::to_vec_pretty(&json!({ "mcpServers": servers }))?;
    write_atomic(&path, &mcp)
}

/// Installed agent file plus the generation surface digest.
pub(crate) struct InstalledAgent {
    pub entry: PathBuf,
    pub release_identity: String,
}

/// Absolute entry of the installed package's agent surface.
pub(crate) fn installed_agent_entry(caps: &Path, slug: &str) -> io::Result<PathBuf> {
    Ok(installed_agent(caps, slug)?.entry)
}

/// Absolute entry and `sha256:` release identity of the current agent generation.
pub(crate) fn installed_agent(caps: &Path, slug: &str) -> io::Result<InstalledAgent> {
    if !crate::org_login::capability_slug_ok(slug) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid"));
    }
    let dir = caps.join("plugins").join(slug);
    if !fs::symlink_metadata(&dir)?.is_dir()
        || dir.canonicalize()? != caps.canonicalize()?.join("plugins").join(slug)
    {
        return Err(io::Error::new(io::ErrorKind::NotFound, "not_installed"));
    }
    let package_id = read_plugin_marker(&dir.join(PACKAGE_ID_MARKER))?;
    if !package_id_ok(&package_id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid installation marker",
        ));
    }
    let root = caps
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "not_installed"))?;
    let lease = ReconcileStore::open(root.join("package-generations"))
        .and_then(|store| store.acquire_current_sync(&package_id))
        .map_err(host_skill_error)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "not_published"))?;
    let publication = lease.publication();
    let surface = publication
        .surfaces
        .iter()
        .find(|surface| surface.kind == SurfaceKind::Agent)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "agent entry is missing"))?;
    let entry = surface
        .entry
        .clone()
        .filter(|entry| entry.is_file())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "agent entry is missing"))?;
    if !digest_ok(&surface.sha256) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid release identity",
        ));
    }
    Ok(InstalledAgent {
        entry,
        release_identity: surface.sha256.clone(),
    })
}

fn callable_plugin_json(caps: &Path, slug: &str) -> io::Result<Vec<u8>> {
    if !crate::org_login::capability_slug_ok(slug) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid"));
    }
    let dir = caps.join("plugins").join(slug);
    if !fs::symlink_metadata(&dir)?.is_dir()
        || dir.canonicalize()? != caps.canonicalize()?.join("plugins").join(slug)
    {
        return Err(io::Error::new(io::ErrorKind::NotFound, "not_installed"));
    }
    let package_id = read_plugin_marker(&dir.join(PACKAGE_ID_MARKER))?;
    let digest = read_plugin_marker(&dir.join(DIGEST_MARKER))?;
    if !package_id_ok(&package_id) || !digest_ok(&digest) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid installation marker",
        ));
    }
    let root = caps
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "not_installed"))?;
    let lease = ReconcileStore::open(root.join("package-generations"))
        .and_then(|store| store.acquire_current_sync(&package_id))
        .map_err(host_skill_error)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "not_published"))?;
    let publication = lease.publication();
    validate_plugin_publication(publication)?;
    let mut surfaces = Vec::new();
    for surface in &publication.surfaces {
        if !matches!(
            surface.kind,
            SurfaceKind::Skill | SurfaceKind::Mcp | SurfaceKind::Tool | SurfaceKind::Flow
        ) {
            continue;
        }
        let mut row = Map::new();
        row.insert("kind".to_owned(), json!(surface.kind.as_str()));
        row.insert("id".to_owned(), json!(surface.id));
        if let Some(entry) = &surface.entry {
            let key = if surface.kind == SurfaceKind::Flow {
                "source"
            } else {
                "entry"
            };
            row.insert(key.to_owned(), json!(entry));
        }
        if let Some(command) = surface
            .mcp
            .as_ref()
            .and_then(|mcp| mcp.get("command"))
            .and_then(Value::as_str)
        {
            row.insert("command".to_owned(), json!(command));
        }
        surfaces.push(Value::Object(row));
    }
    json_bytes(&json!({
        "packageId": publication.package_id,
        "packageDigest": digest,
        "generation": publication.generation,
        "revision": publication.cursor().revision,
        "surfaces": surfaces
    }))
}

/// Runtime registrations come from verified contributions, never from extra archive files.
fn published_plugin_calls(caps: &Path, slug: &str) -> io::Result<Vec<PluginCall>> {
    let dir = caps.join("plugins").join(slug);
    let id = read_plugin_marker(&dir.join(PACKAGE_ID_MARKER))?;
    let root = caps
        .parent()
        .ok_or_else(|| io::Error::other("package root is missing"))?;
    let lease = ReconcileStore::open(root.join("package-generations"))
        .and_then(|store| store.acquire_current_sync(&id))
        .map_err(host_skill_error)?
        .ok_or_else(|| io::Error::other("package is not published"))?;
    let mut calls = Vec::new();
    validate_plugin_publication(lease.publication())?;
    for surface in &lease.publication().surfaces {
        let (kind, detail) = match surface.kind {
            SurfaceKind::Skill => ("skill", json!({})),
            SurfaceKind::Mcp => {
                let mut server = surface
                    .mcp
                    .clone()
                    .ok_or_else(|| io::Error::other("package MCP declaration is missing"))?;
                if server["type"] == "stdio" {
                    if let Some(entry) = &surface.entry {
                        server["command"] = json!(entry);
                    }
                }
                ("mcp", server)
            }
            SurfaceKind::Tool => (
                "tool",
                json!({ "entry": surface.entry.as_ref().ok_or_else(|| io::Error::other("package Tool entry is missing"))? }),
            ),
            SurfaceKind::Flow => (
                "flow",
                json!({ "source": surface.entry.as_ref().ok_or_else(|| io::Error::other("package Flow entry is missing"))? }),
            ),
            SurfaceKind::Ui | SurfaceKind::Okf | SurfaceKind::Agent | SurfaceKind::Plugin => {
                continue;
            }
        };
        calls.push(PluginCall {
            kind,
            id: surface.id.clone(),
            detail,
        });
    }
    Ok(calls)
}

fn read_plugin_marker(path: &Path) -> io::Result<String> {
    if !fs::symlink_metadata(path)?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid installation marker",
        ));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?.take(201).read_to_end(&mut bytes)?;
    if bytes.len() > 200 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid installation marker",
        ));
    }
    String::from_utf8(bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid installation marker"))
}

#[cfg(test)]
#[path = "capabilities_plugin_tests.rs"]
mod plugin_publication_tests;

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "bundle entry is a link",
            ));
        }
        if metadata.is_dir() {
            collect_files(root, &path, out)?;
            continue;
        }
        if !metadata.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "bundle entry is not a file",
            ));
        }
        let relative = path.strip_prefix(root).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "bundle entry escapes its directory",
            )
        })?;
        let relative = relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        out.push((relative, fs::read(path)?));
    }
    Ok(())
}

fn surface_id_ok(id: &str) -> bool {
    let mut chars = id.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    id.len() <= 64
        && first.is_ascii_alphanumeric()
        && chars.all(|character| {
            character.is_ascii_alphanumeric()
                || character == '-'
                || character == '_'
                || character == '.'
        })
}

fn link_package_skills(agents: &Path, target: &Path, owner: &str) -> io::Result<()> {
    let store = plugin_store(agents).ok_or_else(|| io::Error::other("package store is missing"))?;
    let lease = store
        .acquire_current_sync(owner)
        .map_err(host_skill_error)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "package is not published"))?;
    if lease
        .publication()
        .surfaces
        .iter()
        .any(|surface| surface.directory.join("a3s-use-extension.acl").is_file())
    {
        for surface in &lease.publication().surfaces {
            if surface.kind == SurfaceKind::Skill {
                crate::skill_links::claim_skill_link(
                    agents,
                    owner,
                    &surface.id,
                    &surface.directory,
                )
                .map_err(claim_to_io)?;
            }
        }
        return Ok(());
    }
    let skills = target.join("skills");
    let read = match fs::read_dir(&skills) {
        Ok(read) => read,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in read {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !surface_id_ok(&name) {
            continue;
        }
        crate::skill_links::claim_skill_link(agents, owner, &name, &entry.path())
            .map_err(claim_to_io)?;
    }
    Ok(())
}

/// Drop every agent link this package recorded, including a skill directory
/// that is no longer inside the unpacked package.
fn release_owned_skill_links(agents: &Path, owner: &str) -> io::Result<()> {
    let entries = match fs::read_dir(agents) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let mut names = Vec::new();
    for entry in entries {
        let name = entry?.file_name().to_string_lossy().into_owned();
        let Some(skill) = name
            .strip_prefix('.')
            .and_then(|rest| rest.strip_suffix(".a3s-owner"))
        else {
            continue;
        };
        if skill.is_empty() || skill.contains('/') || skill.contains('\0') {
            continue;
        }
        if crate::skill_links::recorded_owner(agents, skill).as_deref() == Some(owner) {
            names.push(skill.to_string());
        }
    }
    for name in names {
        crate::skill_links::release_skill_link(agents, owner, &name).map_err(claim_to_io)?;
    }
    Ok(())
}

/// Remove a symlink whose target lives inside this plugin. A real directory stays.
fn release_links_into(agents: &Path, plugin_dir: &Path, owner: &str) -> io::Result<()> {
    let Some(root) = fs::canonicalize(plugin_dir).ok() else {
        return Ok(());
    };
    let entries = match fs::read_dir(agents) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if !metadata.file_type().is_symlink() {
            continue;
        }
        let Ok(raw) = fs::read_link(&path) else {
            continue;
        };
        let absolute = if raw.is_absolute() {
            raw
        } else {
            let Some(parent) = path.parent() else {
                continue;
            };
            parent.join(raw)
        };
        let inside = fs::canonicalize(&absolute)
            .ok()
            .is_some_and(|target| target.starts_with(&root));
        if !inside {
            continue;
        }
        fs::remove_file(&path)?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let recorded = crate::skill_links::recorded_owner(agents, &name);
        if recorded.is_none() || recorded.as_deref() == Some(owner) {
            match fs::remove_file(crate::skill_links::owner_file(agents, &name)) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
    }
    Ok(())
}

fn release_package_skills(agents: &Path, target: &Path, owner: &str) -> io::Result<()> {
    let skills = target.join("skills");
    let read = match fs::read_dir(&skills) {
        Ok(read) => read,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in read {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !surface_id_ok(&name) {
            continue;
        }
        crate::skill_links::release_skill_link(agents, owner, &name).map_err(claim_to_io)?;
    }
    Ok(())
}

fn claim_to_io(error: crate::skill_links::ClaimError) -> io::Error {
    match error {
        crate::skill_links::ClaimError::Io(error) => error,
        crate::skill_links::ClaimError::HeldByOther | crate::skill_links::ClaimError::UserOwned => {
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                "skill link is owned by someone else",
            )
        }
    }
}

fn package_store(root: Option<&Path>) -> Option<ReconcileStore> {
    let root = root?;
    ReconcileStore::open(root.join("package-generations")).ok()
}

fn host_skill_error(error: ReconcileError) -> io::Error {
    let message = match error {
        ReconcileError::DigestMismatch => "bundle does not match its pinned digest".to_string(),
        ReconcileError::SkillMissing => "bundle has no SKILL.md".to_string(),
        other => other.to_string(),
    };
    io::Error::new(io::ErrorKind::InvalidData, message)
}

/// Verify one catalog skill as its own package generation, then copy the
/// verified files to `skills/<slug>/`. A digest failure returns before that
/// copy, so an older directory stays where it is.
fn install_skill(
    skills: &Path,
    slug: &str,
    digest: &str,
    bundle: &impl Fn(&str) -> io::Result<Vec<u8>>,
) -> io::Result<()> {
    let bytes = bundle(slug)?;
    if digest_of(&bytes) != digest {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "bundle does not match its pinned digest",
        ));
    }
    let store = package_store(skills.parent()).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "package generation root is unavailable",
        )
    })?;
    let publication = store
        .apply_sync(&PackageSpec {
            package_id: format!("catalog/{slug}"),
            surfaces: vec![SurfaceSpec {
                kind: SurfaceKind::Skill,
                id: slug.to_string(),
                sha256: digest.to_string(),
                entry: None,
                payload: Payload::Zip(bytes),
            }],
        })
        .map_err(host_skill_error)?;
    let surface = publication
        .surfaces
        .iter()
        .find(|surface| surface.kind == SurfaceKind::Skill)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "bundle has no SKILL.md"))?;
    materialize_skill(&surface.directory, &skills.join(slug), digest)
}

fn materialize_skill(from: &Path, target: &Path, digest: &str) -> io::Result<()> {
    let parent = target
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "skill target has no parent"))?;
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "skill target name"))?;
    let staging = parent.join(format!(".staging-{name}"));
    remove_any(&staging)?;
    let copied = copy_tree(from, &staging).and_then(|()| {
        if staging.join("SKILL.md").is_file() {
            if staging.join("a3s-use-extension.acl").is_file() {
                let text = fs::read_to_string(staging.join("a3s-use-extension.acl"))?;
                let manifest = a3s_use_extension::ExtensionManifest::parse_acl(&text)
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.message))?;
                if manifest.skills.is_empty() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "AIX skill package declares no skill",
                    ));
                }
            }
            fs::write(staging.join(DIGEST_MARKER), digest)
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "bundle has no SKILL.md",
            ))
        }
    });
    if let Err(error) = copied {
        let _ = remove_any(&staging);
        return Err(error);
    }
    remove_any(target)?;
    fs::rename(&staging, target)
}

fn copy_tree(from: &Path, into: &Path) -> io::Result<()> {
    fs::create_dir_all(into)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "bundle entry is a link",
            ));
        }
        let dest = into.join(entry.file_name());
        if metadata.is_dir() {
            copy_tree(&entry.path(), &dest)?;
        } else if metadata.is_file() {
            fs::copy(entry.path(), &dest)?;
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "bundle entry is not a file",
            ));
        }
    }
    Ok(())
}

/// Extracts regular files only; rejects links, escaping paths, and oversize archives.
fn unpack(bytes: &[u8], into: &Path) -> io::Result<()> {
    let invalid = |message: &str| io::Error::new(io::ErrorKind::InvalidData, message.to_owned());
    let mut archive =
        zip::ZipArchive::new(io::Cursor::new(bytes)).map_err(|err| invalid(&err.to_string()))?;
    if archive.len() > MAX_ENTRIES {
        return Err(invalid("bundle has too many entries"));
    }
    fs::create_dir_all(into)?;
    let mut total = 0u64;
    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|err| invalid(&err.to_string()))?;
        let relative = file
            .enclosed_name()
            .ok_or_else(|| invalid("bundle entry escapes its directory"))?;
        if file.is_symlink() {
            return Err(invalid("bundle entry is a link"));
        }
        let path = into.join(relative);
        if file.is_dir() {
            fs::create_dir_all(&path)?;
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut contents = Vec::new();
        let limit = MAX_UNPACKED - total + 1;
        (&mut file).take(limit).read_to_end(&mut contents)?;
        total += contents.len() as u64;
        if total > MAX_UNPACKED {
            return Err(invalid("bundle unpacks too large"));
        }
        fs::write(&path, contents)?;
    }
    Ok(())
}

fn remove_any(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.to_owned())
}

fn json_bytes(value: &Value) -> io::Result<Vec<u8>> {
    serde_json::to_vec(value).map_err(|_| io::Error::other("failed"))
}

fn config_key_ok(key: &str) -> bool {
    let mut bytes = key.bytes();
    let Some(head) = bytes.next() else {
        return false;
    };
    key.len() <= 64
        && (head.is_ascii_alphabetic() || head == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

fn load_config(root: &Path) -> Map<String, Value> {
    match fs::read(root.join(CONFIG_FILE)) {
        Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
            Ok(Value::Object(map)) => map,
            _ => Map::new(),
        },
        Err(_) => Map::new(),
    }
}

fn sanitize_config(raw: &Map<String, Value>) -> Map<String, Value> {
    let mut out = Map::new();
    for (slug, value) in raw {
        if !crate::org_login::capability_slug_ok(slug) {
            continue;
        }
        let Some(fields) = value.as_object() else {
            continue;
        };
        let mut clean = Map::new();
        for (key, raw_value) in fields {
            if !config_key_ok(key) {
                continue;
            }
            let Some(text) = raw_value.as_str().filter(|text| {
                !text.is_empty() && text.len() <= MAX_CONFIG_VALUE && !text.contains('\0')
            }) else {
                continue;
            };
            clean.insert(key.clone(), Value::String(text.to_owned()));
            if clean.len() == MAX_CONFIG_FIELDS {
                break;
            }
        }
        if !clean.is_empty() {
            out.insert(slug.clone(), Value::Object(clean));
        }
    }
    out
}

/// `stdio` servers take `env`; hosted servers take `headers`. Empty sets are omitted.
fn apply_config(servers: &mut Map<String, Value>, config: &Map<String, Value>) {
    for (slug, server) in servers.iter_mut() {
        let Some(fields) = config.get(slug).and_then(Value::as_object) else {
            continue;
        };
        let Some(server) = server.as_object_mut() else {
            continue;
        };
        if fields.is_empty() {
            continue;
        }
        let slot = if server.get("type").and_then(Value::as_str) == Some("stdio") {
            "env"
        } else {
            "headers"
        };
        server.insert(slot.to_owned(), Value::Object(fields.clone()));
    }
}

fn write_config_file(root: &Path, config: &Map<String, Value>) -> io::Result<()> {
    let path = root.join(CONFIG_FILE);
    if config.is_empty() {
        return match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err),
        };
    }
    let bytes = serde_json::to_vec_pretty(&Value::Object(config.clone()))?;
    write_atomic(&path, &bytes)
}

fn parse_config_body(body: &[u8]) -> io::Result<Map<String, Value>> {
    if body.len() > MAX_CONFIG_BODY {
        return Err(invalid("config is too large"));
    }
    let value: Value = serde_json::from_slice(body).map_err(|_| invalid("config must be json"))?;
    let values = value
        .get("values")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("config needs values"))?;
    if values.len() > MAX_CONFIG_FIELDS {
        return Err(invalid("too many config fields"));
    }
    let mut clean = Map::new();
    for (key, raw) in values {
        if !config_key_ok(key) {
            return Err(invalid("config key is invalid"));
        }
        let text = raw
            .as_str()
            .ok_or_else(|| invalid("config values must be strings"))?;
        if text.len() > MAX_CONFIG_VALUE || text.contains('\0') {
            return Err(invalid("config value is invalid"));
        }
        if !text.is_empty() {
            clean.insert(key.clone(), Value::String(text.to_owned()));
        }
    }
    Ok(clean)
}

fn store_config(root: &Path, slug: &str, body: &[u8]) -> io::Result<()> {
    if !crate::org_login::capability_slug_ok(slug) {
        return Err(invalid("config slug is invalid"));
    }
    let values = parse_config_body(body)?;
    fs::create_dir_all(root)?;
    let mut config = sanitize_config(&load_config(root));
    if values.is_empty() {
        config.remove(slug);
    } else {
        config.insert(slug.to_owned(), Value::Object(values));
    }
    write_config_file(root, &config)?;
    refresh_mcp(root, &config)
}

/// Rewrites env and headers on an existing `mcp.json` from the saved values.
fn refresh_mcp(root: &Path, config: &Map<String, Value>) -> io::Result<()> {
    let path = root.join("mcp.json");
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };
    let mut doc: Value = match serde_json::from_slice(&bytes) {
        Ok(value) => value,
        Err(_) => return Ok(()),
    };
    let Some(servers) = doc.get_mut("mcpServers").and_then(Value::as_object_mut) else {
        return Ok(());
    };
    for server in servers.values_mut() {
        if let Some(server) = server.as_object_mut() {
            server.remove("env");
            server.remove("headers");
        }
    }
    apply_config(servers, config);
    let next = serde_json::to_vec_pretty(&doc)?;
    write_atomic(&path, &next)
}

pub(crate) fn custom_connector_code(err: &io::Error) -> &'static str {
    match err.to_string().as_str() {
        "invalid_name" => "invalid_name",
        "missing_command" => "missing_command",
        "missing_url" => "missing_url",
        "unknown_transport" => "unknown_transport",
        "connector_exists" => "connector_exists",
        "invalid_url" => "invalid_url",
        "invalid_config" => "invalid_config",
        "unavailable" => "unavailable",
        _ => "failed",
    }
}

struct CustomDraft {
    name: String,
    spec: Value,
    values: Map<String, Value>,
}

/// Accepts one custom connector. Refuses before creating a directory.
pub fn save_custom_connector(root: &Path, body: &[u8]) -> io::Result<()> {
    let draft = parse_custom_body(body)?;
    if catalog_blocks(root, &draft.name) {
        return Err(invalid("connector_exists"));
    }
    let mut ledger = load_custom(root);
    if ledger.len() >= MAX_ENTRIES && !ledger.contains_key(&draft.name) {
        return Err(invalid("invalid_config"));
    }
    fs::create_dir_all(root)?;
    ledger.insert(draft.name.clone(), draft.spec);
    write_custom(root, &ledger)?;
    let mut config = sanitize_config(&load_config(root));
    if draft.values.is_empty() {
        config.remove(&draft.name);
    } else {
        config.insert(draft.name, Value::Object(draft.values));
    }
    write_config_file(root, &config)?;
    publish_from_disk(root)
}

/// Drops one authored connector. A missing ledger entry does not rewrite `mcp.json`.
pub fn remove_custom_connector(root: &Path, name: &str) -> io::Result<()> {
    if !crate::org_login::capability_slug_ok(name) {
        return Err(invalid("invalid_name"));
    }
    if !root.exists() {
        return Ok(());
    }
    let mut ledger = load_custom(root);
    if ledger.remove(name).is_none() {
        return Ok(());
    }
    write_custom(root, &ledger)?;
    let mut config = sanitize_config(&load_config(root));
    if config.remove(name).is_some() {
        write_config_file(root, &config)?;
    }
    publish_from_disk(root)
}

pub fn list_custom_json(root: &Path) -> String {
    let ledger = load_custom(root);
    let config = sanitize_config(&load_config(root));
    let mut connectors = Vec::with_capacity(ledger.len());
    for (name, spec) in ledger {
        let Some(obj) = spec.as_object() else {
            continue;
        };
        let values = config
            .get(&name)
            .cloned()
            .unwrap_or_else(|| Value::Object(Map::new()));
        let row = match obj.get("type").and_then(Value::as_str) {
            Some("stdio") => json!({
                "name": name,
                "transport": "stdio",
                "command": obj.get("command").cloned().unwrap_or(Value::String(String::new())),
                "args": obj.get("args").cloned().unwrap_or_else(|| json!([])),
                "values": values,
            }),
            Some("sse") => json!({
                "name": name,
                "transport": "sse",
                "url": obj.get("url").cloned().unwrap_or(Value::String(String::new())),
                "values": values,
            }),
            Some("http") => json!({
                "name": name,
                "transport": "streamable_http",
                "url": obj.get("url").cloned().unwrap_or(Value::String(String::new())),
                "values": values,
            }),
            _ => continue,
        };
        connectors.push(row);
    }
    serde_json::to_string(&json!({ "connectors": connectors }))
        .unwrap_or_else(|_| "{\"connectors\":[]}".to_owned())
}

fn parse_custom_body(body: &[u8]) -> io::Result<CustomDraft> {
    if body.len() > MAX_CONFIG_BODY {
        return Err(invalid("invalid_config"));
    }
    let value: Value = serde_json::from_slice(body).map_err(|_| invalid("invalid_config"))?;
    let obj = value.as_object().ok_or_else(|| invalid("invalid_config"))?;
    let name = obj.get("name").and_then(Value::as_str).unwrap_or("").trim();
    if !crate::org_login::capability_slug_ok(name) {
        return Err(invalid("invalid_name"));
    }
    let transport = obj.get("transport").and_then(Value::as_str).unwrap_or("");
    let values = parse_custom_values(obj.get("values"))?;
    let spec = match transport {
        "stdio" => {
            let command = obj
                .get("command")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            if command.is_empty() {
                return Err(invalid("missing_command"));
            }
            if !bounded_text(command, MAX_CUSTOM_COMMAND) {
                return Err(invalid("invalid_config"));
            }
            let args = parse_custom_args(obj.get("args"))?;
            json!({ "type": "stdio", "command": command, "args": args })
        }
        "sse" | "streamable_http" => {
            let url = obj.get("url").and_then(Value::as_str).unwrap_or("").trim();
            if url.is_empty() {
                return Err(invalid("missing_url"));
            }
            if !custom_url_ok(url) {
                return Err(invalid("invalid_url"));
            }
            let wire = if transport == "sse" { "sse" } else { "http" };
            json!({ "type": wire, "url": url })
        }
        _ => return Err(invalid("unknown_transport")),
    };
    Ok(CustomDraft {
        name: name.to_owned(),
        spec,
        values,
    })
}

fn parse_custom_args(value: Option<&Value>) -> io::Result<Vec<String>> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    if value.is_null() {
        return Ok(Vec::new());
    }
    let items = value.as_array().ok_or_else(|| invalid("invalid_config"))?;
    if items.len() > MAX_CUSTOM_ARGS {
        return Err(invalid("invalid_config"));
    }
    let mut args = Vec::new();
    for item in items {
        let text = item
            .as_str()
            .ok_or_else(|| invalid("invalid_config"))?
            .trim();
        if text.is_empty() {
            continue;
        }
        if !bounded_text(text, MAX_CUSTOM_ARG) {
            return Err(invalid("invalid_config"));
        }
        args.push(text.to_owned());
    }
    Ok(args)
}

fn parse_custom_values(value: Option<&Value>) -> io::Result<Map<String, Value>> {
    let Some(value) = value else {
        return Ok(Map::new());
    };
    if value.is_null() {
        return Ok(Map::new());
    }
    let fields = value.as_object().ok_or_else(|| invalid("invalid_config"))?;
    if fields.len() > MAX_CONFIG_FIELDS {
        return Err(invalid("invalid_config"));
    }
    let mut clean = Map::new();
    for (key, raw) in fields {
        if !config_key_ok(key) {
            return Err(invalid("invalid_config"));
        }
        let text = raw.as_str().ok_or_else(|| invalid("invalid_config"))?;
        if text.len() > MAX_CONFIG_VALUE || text.contains('\0') {
            return Err(invalid("invalid_config"));
        }
        if !text.is_empty() {
            clean.insert(key.clone(), Value::String(text.to_owned()));
        }
    }
    Ok(clean)
}

fn bounded_text(text: &str, max: usize) -> bool {
    !text.is_empty() && text.len() <= max && !text.contains('\0')
}

fn custom_url_ok(url: &str) -> bool {
    if url.len() > MAX_CUSTOM_URL || url.contains('\0') || url.chars().any(char::is_whitespace) {
        return false;
    }
    let Some((scheme, rest)) = url.split_once("://") else {
        return false;
    };
    (scheme == "http" || scheme == "https") && !rest.is_empty() && !rest.starts_with('/')
}

/// An unmarked `mcp.json` entry is a catalog server. Saving that name would replace it.
fn catalog_blocks(root: &Path, name: &str) -> bool {
    let Some(server) = read_mcp_servers(root).get(name).cloned() else {
        return false;
    };
    server.get(CUSTOM_MARK).and_then(Value::as_str) != Some("custom")
}

fn read_mcp_servers(root: &Path) -> Map<String, Value> {
    let Ok(bytes) = fs::read(root.join("mcp.json")) else {
        return Map::new();
    };
    let Ok(doc) = serde_json::from_slice::<Value>(&bytes) else {
        return Map::new();
    };
    doc.get("mcpServers")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn load_custom(root: &Path) -> BTreeMap<String, Value> {
    let Ok(bytes) = fs::read(root.join(CUSTOM_FILE)) else {
        return BTreeMap::new();
    };
    let Ok(Value::Object(map)) = serde_json::from_slice::<Value>(&bytes) else {
        return BTreeMap::new();
    };
    let mut out = BTreeMap::new();
    for (name, spec) in map {
        if out.len() == MAX_ENTRIES {
            break;
        }
        if !crate::org_login::capability_slug_ok(&name) {
            continue;
        }
        if let Some(clean) = clean_custom_spec(&spec) {
            out.insert(name, clean);
        }
    }
    out
}

fn clean_custom_spec(spec: &Value) -> Option<Value> {
    let obj = spec.as_object()?;
    match obj.get("type").and_then(Value::as_str) {
        Some("stdio") => {
            let command = obj.get("command").and_then(Value::as_str)?;
            if !bounded_text(command, MAX_CUSTOM_COMMAND) {
                return None;
            }
            let args = clean_custom_args(obj.get("args"))?;
            Some(json!({ "type": "stdio", "command": command, "args": args }))
        }
        Some(kind @ ("sse" | "http")) => {
            let url = obj.get("url").and_then(Value::as_str)?;
            if !custom_url_ok(url) {
                return None;
            }
            Some(json!({ "type": kind, "url": url }))
        }
        _ => None,
    }
}

fn clean_custom_args(value: Option<&Value>) -> Option<Vec<String>> {
    let Some(value) = value else {
        return Some(Vec::new());
    };
    let items = value.as_array()?;
    if items.len() > MAX_CUSTOM_ARGS {
        return None;
    }
    let mut args = Vec::new();
    for item in items {
        let text = item.as_str()?;
        if text.is_empty() {
            continue;
        }
        if !bounded_text(text, MAX_CUSTOM_ARG) {
            return None;
        }
        args.push(text.to_owned());
    }
    Some(args)
}

fn write_custom(root: &Path, ledger: &BTreeMap<String, Value>) -> io::Result<()> {
    let path = root.join(CUSTOM_FILE);
    if ledger.is_empty() {
        return match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err),
        };
    }
    let bytes = serde_json::to_vec_pretty(ledger)?;
    write_atomic(&path, &bytes)
}

fn publish_from_disk(root: &Path) -> io::Result<()> {
    let ledger = load_custom(root);
    let existing = read_mcp_servers(root);
    let mut servers = Map::new();
    let mut custom_names = BTreeSet::new();
    for (name, server) in existing {
        let marked = server.get(CUSTOM_MARK).and_then(Value::as_str) == Some("custom");
        if marked {
            continue;
        }
        if let Some(clean) = catalog_server(&server) {
            servers.insert(name, clean);
        }
    }
    for (name, spec) in &ledger {
        if servers.contains_key(name) {
            continue;
        }
        custom_names.insert(name.clone());
        servers.insert(name.clone(), spec.clone());
    }
    publish_mcp(root, &mut servers, &custom_names)?;
    write_connector_notes(root, &servers, &Map::new(), &BTreeMap::new())
}

/// Writes `<root>/<slug>.md` for every installed server.
///
/// Catalog servers take the catalog description, or a transport label when that
/// description is empty. Custom servers keep an existing note. The note never
/// receives the server command, URL, env, or headers.
fn write_connector_notes(
    root: &Path,
    servers: &Map<String, Value>,
    catalog_servers: &Map<String, Value>,
    catalog_notes: &BTreeMap<String, String>,
) -> io::Result<()> {
    let mut owned = Vec::with_capacity(servers.len());
    for (name, server) in servers {
        let kind = server.get("type").and_then(Value::as_str);
        let usage = if catalog_servers.contains_key(name) {
            catalog_notes
                .get(name)
                .filter(|text| !text.trim().is_empty())
                .cloned()
                .unwrap_or_else(|| crate::connector_notes::transport_label(kind))
        } else {
            catalog_notes
                .get(name)
                .filter(|text| !text.trim().is_empty())
                .cloned()
                .or_else(|| existing_usage(root, name))
                .unwrap_or_else(|| crate::connector_notes::transport_label(kind))
        };
        owned.push((name.clone(), usage));
    }
    owned.sort_by(|left, right| left.0.cmp(&right.0));
    let view: Vec<(&str, &str)> = owned
        .iter()
        .map(|(name, usage)| (name.as_str(), usage.as_str()))
        .collect();
    crate::connector_notes::sync_connector_notes(root, &view)
}

fn existing_usage(root: &Path, name: &str) -> Option<String> {
    let text = fs::read_to_string(root.join(format!("{name}.md"))).ok()?;
    // A missing closer is `None`. Do not treat the frontmatter as the usage.
    let body = crate::connector_notes::note_body(&text)?.trim();
    if body.is_empty() {
        None
    } else {
        Some(body.to_owned())
    }
}

fn catalog_server(server: &Value) -> Option<Value> {
    let obj = server.as_object()?;
    let mut clean = obj.clone();
    clean.remove(CUSTOM_MARK);
    clean.remove("env");
    clean.remove("headers");
    Some(Value::Object(clean))
}

fn publish_mcp(
    root: &Path,
    servers: &mut Map<String, Value>,
    custom_names: &BTreeSet<String>,
) -> io::Result<()> {
    let mut config = sanitize_config(&load_config(root));
    config.retain(|slug, _| servers.contains_key(slug));
    apply_config(servers, &config);
    for (name, server) in servers.iter_mut() {
        let Some(obj) = server.as_object_mut() else {
            continue;
        };
        if custom_names.contains(name) {
            obj.insert(CUSTOM_MARK.to_owned(), json!("custom"));
        } else {
            obj.remove(CUSTOM_MARK);
        }
    }
    write_config_file(root, &config)?;
    let mcp = serde_json::to_vec_pretty(&json!({ "mcpServers": servers }))?;
    write_atomic(&root.join("mcp.json"), &mcp)
}

/// ACP `session/new` `mcpServers` for the connectors recorded in `root/mcp.json`.
///
/// Stdio entries are untagged (`name`, `command`, `args`, `env`). HTTP and SSE
/// entries carry `type` plus `headers`. `env` and `headers` are `{name,value}`
/// lists, the shape agent-client-protocol 0.10.4 deserializes. A missing file
/// is an empty list, so a session still starts when nothing is installed.
pub(crate) fn acp_session_servers(root: &Path) -> Vec<Value> {
    acp_session_servers_excluding_native(root, &[])
}

pub(crate) fn acp_session_servers_excluding_native(root: &Path, native: &[String]) -> Vec<Value> {
    let Ok(bytes) = fs::read(root.join("mcp.json")) else {
        return Vec::new();
    };
    let Ok(doc) = serde_json::from_slice::<Value>(&bytes) else {
        return Vec::new();
    };
    let Some(servers) = doc.get("mcpServers").and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut names: Vec<&str> = servers
        .keys()
        .map(String::as_str)
        .filter(|slug| crate::org_login::capability_slug_ok(slug))
        .collect();
    names.sort_unstable();
    names.truncate(MAX_ENTRIES);
    let mut out = Vec::with_capacity(names.len());
    for name in names {
        let Some(server) = servers.get(name).and_then(Value::as_object) else {
            continue;
        };
        if server
            .get(PLUGIN_MCP_MARK)
            .and_then(Value::as_str)
            .is_some()
            && native.iter().any(|selected| selected == name)
        {
            continue;
        }
        if let Some(entry) = acp_server(name, server) {
            out.push(entry);
        }
    }
    out
}

/// Installed plugin tools for ACP `session/new`. Each `entry` is the absolute
/// unpacked executable (`tools/echo`). A missing file is an empty list.
pub(crate) fn acp_session_tools(root: &Path) -> Vec<Value> {
    published_rows(root, PLUGIN_TOOLS_FILE, "tools", |row| {
        let id = row.get("id").and_then(Value::as_str)?;
        let entry = row.get("entry").and_then(Value::as_str)?;
        if !surface_id_ok(id) || !absolute_file(entry) {
            return None;
        }
        Some(json!({ "id": id, "entry": entry }))
    })
}

/// Installed plugin flows as `WorkflowSpec` values. `runtime.entrypoint` is the
/// absolute unpacked source (`flows/reason.ts`). `sourceHash` is present only
/// after `SourceSnapshot::read` opened that file during publish.
pub(crate) fn installed_flow_specs(root: &Path) -> Vec<Value> {
    published_rows(root, PLUGIN_FLOWS_FILE, "flows", |row| {
        let name = row.get("name").and_then(Value::as_str)?;
        let runtime = row.get("runtime")?;
        let entrypoint = runtime.get("entrypoint").and_then(Value::as_str)?;
        let export_name = runtime.get("export_name").and_then(Value::as_str)?;
        let source_hash = row.get("sourceHash").and_then(Value::as_str)?;
        if !surface_id_ok(name)
            || runtime.get("kind").and_then(Value::as_str) != Some("native_ts")
            || export_name.is_empty()
            || export_name.contains('\0')
            || !absolute_file(entrypoint)
            || !source_hash_ok(source_hash)
        {
            return None;
        }
        Some(row.clone())
    })
}

fn source_hash_ok(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Open `spec.runtime.entrypoint` through the flow runtime's source snapshot.
/// A running Tokio worker cannot host a second runtime, so that case reads
/// on a helper thread. The compiler is not started.
fn read_flow_source(spec: &Value) -> io::Result<a3s_flow::NativeTsEntrypointRead> {
    let parsed: a3s_flow::WorkflowSpec = serde_json::from_value(spec.clone())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    let read = |spec: &a3s_flow::WorkflowSpec| {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| io::Error::other(error.to_string()))?;
        runtime
            .block_on(a3s_flow::read_native_ts_entrypoint(spec))
            .map_err(|error| io::Error::other(error.to_string()))
    };
    if tokio::runtime::Handle::try_current().is_ok() {
        match std::thread::scope(|scope| scope.spawn(|| read(&parsed)).join()) {
            Ok(result) => result,
            Err(_) => Err(io::Error::other("flow source read panicked")),
        }
    } else {
        read(&parsed)
    }
}

fn published_rows(
    root: &Path,
    file: &str,
    key: &str,
    keep: impl Fn(&Value) -> Option<Value>,
) -> Vec<Value> {
    let Ok(bytes) = fs::read(root.join(file)) else {
        return Vec::new();
    };
    let Ok(doc) = serde_json::from_slice::<Value>(&bytes) else {
        return Vec::new();
    };
    let Some(rows) = doc.get(key).and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for row in rows {
        if let Some(kept) = keep(row) {
            out.push(kept);
        }
        if out.len() == MAX_ENTRIES {
            break;
        }
    }
    out
}

fn absolute_file(path: &str) -> bool {
    !path.is_empty()
        && !path.contains('\0')
        && Path::new(path).is_absolute()
        && Path::new(path).is_file()
}

fn acp_server(name: &str, server: &Map<String, Value>) -> Option<Value> {
    let text = |key: &str| {
        server
            .get(key)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty() && !value.contains('\0'))
    };
    match server.get("type").and_then(Value::as_str) {
        Some("stdio") => Some(json!({
            "name": name,
            "command": text("command")?,
            "args": string_list(server.get("args")),
            "env": named_pairs(server.get("env")),
        })),
        Some("http") => Some(json!({
            "type": "http",
            "name": name,
            "url": text("url")?,
            "headers": named_pairs(server.get("headers")),
        })),
        Some("sse") => Some(json!({
            "type": "sse",
            "name": name,
            "url": text("url")?,
            "headers": named_pairs(server.get("headers")),
        })),
        _ => None,
    }
}

fn string_list(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .filter(|item| !item.is_empty() && !item.contains('\0'))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn named_pairs(value: Option<&Value>) -> Vec<Value> {
    let Some(map) = value.and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut pairs: Vec<(&str, &str)> = map
        .iter()
        .filter_map(|(key, raw)| {
            let text = raw.as_str()?;
            if !config_key_ok(key) || text.is_empty() || text.contains('\0') {
                return None;
            }
            Some((key.as_str(), text))
        })
        .collect();
    pairs.sort_unstable();
    pairs
        .into_iter()
        .map(|(name, value)| json!({ "name": name, "value": value }))
        .collect()
}

/// Install one uploaded `.zip` or `.skill` package into `~/.agents/skills/<name>/`.
/// The directory is real, so a later catalog publish will not replace it.
pub(crate) fn import_uploaded_skill(bytes: &[u8]) -> io::Result<String> {
    let Some(agents) = user_agents_skills_dir() else {
        return Err(import_err("unavailable"));
    };
    import_skill_archive(bytes, &agents, MAX_SKILL_ARCHIVE)
}

/// Link a bundled skill into `~/.agents/skills` when that name is still free.
pub(crate) fn prepare_builtin_skill_at_home(name: &str) -> io::Result<()> {
    let Some(agents) = user_agents_skills_dir() else {
        return Err(import_err("unavailable"));
    };
    prepare_builtin_skill(name, &agents)
}

/// Remove a symlink that still points at the retired find-skills bundle.
/// A real directory, or a link aimed somewhere else, stays.
pub(crate) fn retire_removed_builtin_skills() {
    let Some(agents) = user_agents_skills_dir() else {
        return;
    };
    retire_removed_builtin_skills_at(&agents);
}

fn retire_removed_builtin_skills_at(agents: &Path) {
    let link = agents.join("find-skills");
    let Ok(meta) = fs::symlink_metadata(&link) else {
        return;
    };
    if !meta.file_type().is_symlink() {
        return;
    }
    let Ok(target) = fs::read_link(&link) else {
        return;
    };
    if target.ends_with(Path::new("resources").join("skills").join("find-skills")) {
        let _ = fs::remove_file(&link);
    }
}

pub(crate) fn skill_import_code(err: &io::Error) -> &'static str {
    match err.to_string().as_str() {
        "unavailable" => "unavailable",
        "too_large" => "too_large",
        "bad_format" => "bad_format",
        "no_skill_md" => "no_skill_md",
        "ambiguous" => "ambiguous",
        "nested" => "nested",
        "invalid_frontmatter" => "invalid_frontmatter",
        "invalid_name" => "invalid_name",
        "invalid_description" => "invalid_description",
        "too_many_entries" => "too_many_entries",
        "unpacks_too_large" => "unpacks_too_large",
        "escapes" => "escapes",
        "symlink" => "symlink",
        "skill_exists" => "skill_exists",
        "skill_md_too_large" => "skill_md_too_large",
        "unknown_builtin" => "unknown_builtin",
        "missing_builtin" => "missing_builtin",
        _ => "failed",
    }
}

fn import_err(code: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, code)
}

fn import_skill_archive(bytes: &[u8], agents: &Path, max_archive: usize) -> io::Result<String> {
    if bytes.len() > max_archive {
        return Err(import_err("too_large"));
    }
    if !bytes.starts_with(b"PK\x03\x04") {
        return Err(import_err("bad_format"));
    }
    let staging = upload_staging_dir();
    let unpacked = unpack(bytes, &staging).map_err(map_unpack_error);
    if let Err(err) = unpacked {
        let _ = remove_any(&staging);
        return Err(err);
    }
    let installed = (|| {
        let root = skill_package_root(&staging)?;
        let (name, _description) = validate_skill_md(&root.join("SKILL.md"))?;
        place_uploaded_skill(&root, agents, &name)?;
        Ok(name)
    })();
    let _ = remove_any(&staging);
    installed
}

fn upload_staging_dir() -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(1);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("a3s-skill-upload-{}-{n}", std::process::id()))
}

fn map_unpack_error(err: io::Error) -> io::Error {
    let message = err.to_string();
    let code = if message.contains("escapes") {
        "escapes"
    } else if message.contains("link") {
        "symlink"
    } else if message.contains("too many") {
        "too_many_entries"
    } else if message.contains("too large") {
        "unpacks_too_large"
    } else {
        "bad_format"
    };
    import_err(code)
}

fn skill_package_root(staging: &Path) -> io::Result<PathBuf> {
    let mut found = Vec::new();
    collect_skill_files(staging, &mut found, 0)?;
    if found.is_empty() {
        return Err(import_err("no_skill_md"));
    }
    if found.len() != 1 {
        return Err(import_err("ambiguous"));
    }
    let parent = found[0].parent().unwrap_or(staging);
    let depth = parent
        .strip_prefix(staging)
        .map(|path| path.components().count())
        .unwrap_or(0);
    if depth > 2 {
        return Err(import_err("nested"));
    }
    Ok(parent.to_path_buf())
}

fn collect_skill_files(dir: &Path, found: &mut Vec<PathBuf>, depth: usize) -> io::Result<()> {
    if depth > 4 {
        return Ok(());
    }
    let mut entries: Vec<_> = fs::read_dir(dir)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let meta = fs::symlink_metadata(&path)?;
        if meta.file_type().is_symlink() {
            return Err(import_err("symlink"));
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == "__MACOSX" || name.starts_with('.') {
            continue;
        }
        if meta.is_dir() {
            collect_skill_files(&path, found, depth + 1)?;
        } else if name == "SKILL.md" && meta.is_file() {
            found.push(path);
        }
    }
    Ok(())
}

fn validate_skill_md(path: &Path) -> io::Result<(String, String)> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() {
        return Err(import_err("no_skill_md"));
    }
    if meta.len() > MAX_SKILL_MD {
        return Err(import_err("skill_md_too_large"));
    }
    let text = fs::read_to_string(path).map_err(|err| {
        if err.kind() == io::ErrorKind::InvalidData {
            import_err("bad_format")
        } else {
            err
        }
    })?;
    if text.contains('\0') {
        return Err(import_err("bad_format"));
    }
    let matter = front_matter(&text);
    if matter.is_empty() {
        return Err(import_err("invalid_frontmatter"));
    }
    let name = front_matter_value(matter, "name").unwrap_or_default();
    if !upload_name_ok(&name) {
        return Err(import_err("invalid_name"));
    }
    let description = front_matter_value(matter, "description").unwrap_or_default();
    if !description_ok(&description) {
        return Err(import_err("invalid_description"));
    }
    Ok((name, description))
}

fn upload_name_ok(name: &str) -> bool {
    skill_name_ok(name) && !name.contains("--")
}

fn description_ok(text: &str) -> bool {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    !flat.is_empty()
        && flat.chars().count() <= MAX_DESCRIPTION_CHARS
        && !flat.contains('<')
        && !flat.contains('>')
        && !text.contains('\0')
}

/// `name` and `description`, including a single folded (`>`) or literal (`|`) block.
fn front_matter_value(block: &str, wanted: &str) -> Option<String> {
    let lines: Vec<&str> = block.lines().collect();
    let mut index = 0;
    while index < lines.len() {
        let trimmed = lines[index].trim();
        index += 1;
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('-') {
            continue;
        }
        let Some((key, raw)) = trimmed.split_once(':') else {
            continue;
        };
        if key.trim() != wanted {
            continue;
        }
        let raw = raw.trim();
        if matches!(raw, ">" | "|" | ">-" | "|-" | ">+" | "|+") {
            let mut parts = Vec::new();
            while index < lines.len() {
                let next = lines[index];
                if next.starts_with(' ') || next.starts_with('\t') {
                    parts.push(next.trim());
                    index += 1;
                } else if next.trim().is_empty() {
                    index += 1;
                } else {
                    break;
                }
            }
            let joined = if raw.starts_with('|') {
                parts.join("\n")
            } else {
                parts.join(" ")
            };
            let joined = joined.trim();
            if joined.is_empty() {
                return None;
            }
            return Some(joined.to_owned());
        }
        let value = unquote_scalar(raw);
        if value.is_empty() {
            return None;
        }
        return Some(value);
    }
    None
}

fn unquote_scalar(value: &str) -> String {
    let value = value.trim();
    let bytes = value.as_bytes();
    if bytes.len() >= 2 {
        let quoted = (bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\'');
        if quoted {
            return value[1..value.len() - 1].trim().to_owned();
        }
    }
    value.to_owned()
}

fn place_uploaded_skill(source: &Path, agents: &Path, name: &str) -> io::Result<()> {
    fs::create_dir_all(agents)?;
    let dest = agents.join(name);
    match dest.symlink_metadata() {
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Ok(meta) if meta.is_dir() && owned_upload(&dest, name) => {}
        _ => return Err(import_err("skill_exists")),
    }
    let staging = agents.join(format!(".staging-{name}"));
    remove_any(&staging)?;
    fs::create_dir_all(&staging)?;
    let copied = (|| {
        let mut budget = 0u64;
        copy_skill_files(source, &staging, &mut budget)?;
        fs::write(staging.join(UPLOAD_MARKER), name)?;
        Ok(())
    })();
    if let Err(err) = copied {
        let _ = remove_any(&staging);
        return Err(err);
    }
    if dest.symlink_metadata().is_ok() {
        let backup = agents.join(format!(".backup-{name}"));
        remove_any(&backup)?;
        fs::rename(&dest, &backup)?;
        if let Err(err) = fs::rename(&staging, &dest) {
            let _ = fs::rename(&backup, &dest);
            let _ = remove_any(&staging);
            return Err(err);
        }
        remove_any(&backup)?;
    } else {
        fs::rename(&staging, &dest)?;
    }
    Ok(())
}

fn owned_upload(dest: &Path, name: &str) -> bool {
    let marker = dest.join(UPLOAD_MARKER);
    let Ok(meta) = fs::symlink_metadata(&marker) else {
        return false;
    };
    if !meta.is_file() || meta.len() > 128 {
        return false;
    }
    fs::read_to_string(&marker)
        .ok()
        .as_deref()
        .is_some_and(|text| text.trim() == name)
}

fn copy_skill_files(from: &Path, to: &Path, budget: &mut u64) -> io::Result<()> {
    let mut entries: Vec<_> = fs::read_dir(from)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let meta = fs::symlink_metadata(&path)?;
        if meta.file_type().is_symlink() {
            return Err(import_err("symlink"));
        }
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if matches!(
            name_str.as_ref(),
            "__MACOSX" | ".DS_Store" | ".a3s-digest" | ".a3s-upload"
        ) {
            continue;
        }
        let dest = to.join(&name);
        if meta.is_dir() {
            fs::create_dir_all(&dest)?;
            copy_skill_files(&path, &dest, budget)?;
        } else if meta.is_file() {
            *budget += meta.len();
            if *budget > MAX_UNPACKED {
                return Err(import_err("unpacks_too_large"));
            }
            if name_str == "SKILL.md" && meta.len() > MAX_SKILL_MD {
                return Err(import_err("skill_md_too_large"));
            }
            fs::write(&dest, fs::read(&path)?)?;
        } else {
            return Err(import_err("bad_format"));
        }
    }
    Ok(())
}

/// `ask-form`, `skill-creator`, `schedule-task`, and `stc-answer` ship with the app.
/// `ask-form` lets a task emit one OpenUI Lang form. `stc-answer` is the bundled knowledge skill. Its Studio label is 知识理解.
/// Review is an installed Use package, linked by `review_plugin` when its skill surface is published.
/// A name that is already a directory or a symlink is left in place.
pub(crate) fn prepare_builtin_skill(name: &str, agents: &Path) -> io::Result<()> {
    if name != "ask-form"
        && name != "skill-creator"
        && name != "schedule-task"
        && name != "stc-answer"
    {
        return Err(import_err("unknown_builtin"));
    }
    let source =
        fs::canonicalize(bundled_skill_dir(name)).map_err(|_| import_err("missing_builtin"))?;
    if !source.join("SKILL.md").is_file() {
        return Err(import_err("missing_builtin"));
    }
    fs::create_dir_all(agents)?;
    let link = agents.join(name);
    if link.symlink_metadata().is_ok() {
        return Ok(());
    }
    place_link(&source, &link)
}

fn bundled_skill_dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../resources/skills")
        .join(name)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let staging = path.with_extension("json.tmp");
    fs::write(&staging, bytes)?;
    fs::rename(&staging, path)
}

fn root_path(slug: &str) -> Option<PathBuf> {
    // Library tests drive the live listener; they must not touch Application Support.
    if cfg!(test) || !crate::org_login::capability_slug_ok(slug) {
        return None;
    }
    Some(crate::tenant::profile_dir(slug)?.join("capabilities"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_root(label: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("a3s-capabilities-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        root
    }

    #[test]
    fn existing_usage_keeps_a_crlf_body_and_rejects_a_missing_closer() {
        let root = temp_root("note-body");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("docs.md"),
            "---\r\nname: docs\r\ndescription: hello\r\n---\r\nCatalog body\r\n",
        )
        .unwrap();
        let usage = existing_usage(&root, "docs").expect("crlf note body");
        assert_eq!(usage, "Catalog body");
        assert!(!usage.contains('\r'));

        fs::write(
            root.join("broken.md"),
            "---\r\nname: broken\r\ndescription: hello\r\nsecret command",
        )
        .unwrap();
        assert!(
            existing_usage(&root, "broken").is_none(),
            "a missing closer must not become usage"
        );

        let mut servers = Map::new();
        servers.insert(
            "broken".to_string(),
            json!({ "type": "stdio", "command": "do-not-copy" }),
        );
        write_connector_notes(&root, &servers, &Map::new(), &BTreeMap::new())
            .expect("rewrite notes");
        let written = fs::read_to_string(root.join("broken.md")).expect("rewritten note");
        assert!(written.contains("Local stdio connector"));
        assert!(!written.contains("secret command"));
        assert!(!written.contains("do-not-copy"));
        let _ = fs::remove_dir_all(&root);
    }

    fn usage_past_the_catalog_line(head: &str, body: &str) -> String {
        let mut prefix = String::from(head);
        while prefix.chars().count() < 160 {
            prefix.push_str(" notes");
        }
        let prefix: String = prefix.chars().take(160).collect();
        format!("{prefix}\n{body}")
    }

    fn bundle_zip(files: &[(&str, &str)]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
        for (name, body) in files {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(body.as_bytes()).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn pdf_v1() -> Vec<u8> {
        bundle_zip(&[("SKILL.md", "# PDF"), ("scripts/run.py", "print(1)")])
    }

    fn pdf_v2() -> Vec<u8> {
        bundle_zip(&[("SKILL.md", "# PDF v2")])
    }

    fn catalog(pdf_digest: &str, pdf_installed: bool) -> Vec<Value> {
        vec![
            json!({ "slug": "github", "installed": true, "install": {
                "type": "mcp", "transport": "stdio", "command": "npx",
                "args": ["-y", "@modelcontextprotocol/server-github"] } }),
            json!({ "slug": "linear", "installed": true, "install": {
                "type": "mcp", "transport": "streamable_http", "url": "https://mcp.linear.app/mcp" } }),
            json!({ "slug": "slack", "installed": false, "install": {
                "type": "mcp", "transport": "sse", "url": "https://slack.example/sse" } }),
            json!({ "slug": "pdf", "installed": pdf_installed, "install": {
                "type": "skill", "digest": pdf_digest, "size": 1 } }),
            json!({ "slug": "legacy", "installed": true, "install": {
                "type": "skill", "releaseId": "00000000-0000-0000-0000-00000000000a" } }),
            json!({ "slug": "../evil", "installed": true, "install": {
                "type": "mcp", "transport": "sse", "url": "https://x" } }),
        ]
    }

    #[test]
    fn desired_keeps_only_installed_well_formed_entries() {
        let v1 = digest_of(&pdf_v1());
        let desired = desired(&catalog(&v1, true));
        assert_eq!(
            Value::Object(desired.mcp),
            json!({
                "github": { "type": "stdio", "command": "npx",
                            "args": ["-y", "@modelcontextprotocol/server-github"] },
                "linear": { "type": "http", "url": "https://mcp.linear.app/mcp" }
            })
        );
        assert_eq!(desired.skills, BTreeMap::from([("pdf".to_owned(), v1)]));
        assert!(
            desired.notes.is_empty(),
            "a catalog entry without a description does not invent usage from the command"
        );
        assert!(desired.plugins.is_empty());
    }

    #[test]
    fn an_aix_connector_installs_as_a_package_instead_of_a_raw_server() {
        let digest = format!("sha256:{}", "ab".repeat(32));
        let desired = desired(&[json!({
            "slug": "github",
            "installed": true,
            "install": {
                "type": "mcp",
                "transport": "stdio",
                "command": "npx",
                "args": ["server"],
                "packageType": "aix",
                "digest": digest,
                "packageId": "use/a3s/github"
            }
        })]);
        assert!(desired.mcp.is_empty());
        let pin = desired.plugins.get("github").expect("package");
        assert!(pin.require_aix);
        assert_eq!(pin.package_id, "use/a3s/github");
    }

    #[test]
    fn desired_keeps_a_plugin_package_pin() {
        let digest = format!("sha256:{}", "ab".repeat(32));
        let entries = vec![
            json!({
                "slug": "a3s-review",
                "installed": true,
                "install": {
                    "type": "plugin",
                    "digest": digest,
                    "size": 12,
                    "packageId": "use/a3s/review"
                }
            }),
            json!({
                "slug": "broken",
                "installed": true,
                "install": { "type": "plugin", "digest": "nope", "size": 1, "packageId": "review" }
            }),
            json!({
                "slug": "aside",
                "installed": false,
                "install": {
                    "type": "plugin",
                    "digest": format!("sha256:{}", "cd".repeat(32)),
                    "size": 4,
                    "packageId": "use/a3s/other"
                }
            }),
        ];
        let desired = desired(&entries);
        assert_eq!(desired.plugins.len(), 1);
        assert_eq!(desired.plugins["a3s-review"].package_id, "use/a3s/review");
        assert!(desired.skills.is_empty());
    }

    struct ClearCatalog;

    impl Drop for ClearCatalog {
        fn drop(&mut self) {
            crate::review_plugin::clear_catalog_owns_for_tests();
        }
    }

    #[test]
    fn uninstalling_presenton_drops_the_deck_snapshot_and_the_homepage_seed() {
        let _snapshot = crate::agent_plane::presenton_mcp::snapshot_test_lock();
        let _home = crate::aix_applet_window::HOME_TEST_LOCK
            .lock()
            .expect("home test lock");
        let tenant = temp_root("presenton-uninstall");
        let plugins = tenant.join("capabilities/plugins");
        let plugin = plugins.join(crate::agent_plane::presenton_mcp::SLUG);
        fs::create_dir_all(&plugin).unwrap();
        fs::write(
            plugin.join(PACKAGE_ID_MARKER),
            crate::agent_plane::presenton_mcp::PACKAGE_ID,
        )
        .unwrap();
        fs::write(
            plugin.join("a3s-use-extension.acl"),
            "extension \"a3s/presenton\" {}\n",
        )
        .unwrap();
        fs::create_dir_all(tenant.join("agents")).unwrap();
        crate::agent_plane::presenton_mcp::set_snapshot_for_tests("旧文稿");
        // The home store is process-global. This test holds HOME_TEST_LOCK, which the
        // applet tests also take before they reset that store.
        let seeded = crate::aix_applet_window::stage_home_seed_for_tests(
            crate::agent_plane::presenton_mcp::SLUG,
            "workbench",
            r#"{"content":"topic"}"#,
        );
        assert!(seeded.is_ok(), "{seeded:?}");
        remove_plugin(
            &plugins,
            &tenant.join("agents"),
            crate::agent_plane::presenton_mcp::SLUG,
        )
        .unwrap();
        assert!(!plugin.exists());
        assert_eq!(crate::agent_plane::presenton_mcp::snapshot_for_tests(), "");
        assert!(!crate::aix_applet_window::home_has_slug_for_tests(
            crate::agent_plane::presenton_mcp::SLUG
        ));
        println!(
            "uninstall_cleared snapshot={} homepage_seed=false",
            crate::agent_plane::presenton_mcp::snapshot_for_tests()
        );
        let _ = fs::remove_dir_all(&tenant);
    }

    #[test]
    fn a_failed_presenton_withdraw_keeps_the_deck_snapshot_and_the_homepage_seed() {
        let _snapshot = crate::agent_plane::presenton_mcp::snapshot_test_lock();
        let _home = crate::aix_applet_window::HOME_TEST_LOCK
            .lock()
            .expect("home test lock");
        let tenant = temp_root("presenton-withdraw-fail");
        let plugins = tenant.join("capabilities/plugins");
        let plugin = plugins.join(crate::agent_plane::presenton_mcp::SLUG);
        fs::create_dir_all(&plugin).unwrap();
        fs::write(
            plugin.join(PACKAGE_ID_MARKER),
            crate::agent_plane::presenton_mcp::PACKAGE_ID,
        )
        .unwrap();
        fs::write(
            plugin.join("a3s-use-extension.acl"),
            "extension \"a3s/presenton\" {}\n",
        )
        .unwrap();
        fs::create_dir_all(tenant.join("agents")).unwrap();
        let outside =
            std::env::temp_dir().join(format!("presenton-withdraw-link-{}", std::process::id()));
        fs::write(&outside, b"not-a-package-root").unwrap();
        let generations = tenant.join("package-generations");
        fs::create_dir_all(&generations).unwrap();
        std::os::unix::fs::symlink(&outside, generations.join("packages")).unwrap();
        crate::agent_plane::presenton_mcp::set_snapshot_for_tests("旧文稿");
        let seeded = crate::aix_applet_window::stage_home_seed_for_tests(
            crate::agent_plane::presenton_mcp::SLUG,
            "workbench",
            r#"{"content":"topic"}"#,
        );
        assert!(seeded.is_ok(), "{seeded:?}");
        let failed = remove_plugin(
            &plugins,
            &tenant.join("agents"),
            crate::agent_plane::presenton_mcp::SLUG,
        );
        assert!(
            failed.is_err(),
            "withdraw must fail before the deck is cleared"
        );
        assert!(plugin.exists());
        assert_eq!(
            crate::agent_plane::presenton_mcp::snapshot_for_tests(),
            "旧文稿"
        );
        assert!(crate::aix_applet_window::home_has_slug_for_tests(
            crate::agent_plane::presenton_mcp::SLUG
        ));
        println!("failed_withdraw snapshot=旧文稿 homepage_seed=true");
        let _ = fs::remove_file(&outside);
        let _ = fs::remove_dir_all(&tenant);
    }

    #[test]
    fn a_review_plugin_installs_from_the_member_catalog_and_uninstalls() {
        let _gate = crate::review_plugin::lock_review_tests();
        let _clear = ClearCatalog;
        let root = temp_root("review-plugin");
        let agents = root.join("agents");
        fs::create_dir_all(&agents).unwrap();
        let zip = bundle_zip(&[
            (
                "a3s-use-extension.acl",
                include_str!("../../../use-registry/packages/review/a3s-use-extension.acl"),
            ),
            (
                "agents/legal-expert/INSTRUCTIONS.md",
                "Review the supplied contract without inventing facts.\n",
            ),
            ("README.md", "# 审查\n"),
            (
                "skills/review/SKILL.md",
                "---\nname: review\n---\nReview the document.\n",
            ),
            ("ui/picker/index.html", "<!doctype html><title>审查</title>"),
            ("ui/picker/app.js", "/* review */\n"),
            ("ui/picker/app.css", "body{margin:0}\n"),
        ]);
        let digest = digest_of(&zip);
        let mut pins = BTreeMap::new();
        pins.insert(
            "a3s-review".to_owned(),
            PluginPin {
                require_aix: true,
                digest: digest.clone(),
                package_id: "use/a3s/review".to_owned(),
            },
        );
        let downloads = std::cell::Cell::new(0);
        let fetch = |slug: &str| {
            assert_eq!(slug, "a3s-review");
            downloads.set(downloads.get() + 1);
            Ok(zip.clone())
        };
        let caps = root.join("capabilities");
        sync_plugins(&caps, &agents, &pins, &fetch).unwrap();
        let initial_downloads = downloads.get();
        let installed = caps.join("plugins/a3s-review");
        assert_eq!(
            fs::read_to_string(installed.join("skills/review/SKILL.md")).unwrap(),
            "---\nname: review\n---\nReview the document.\n"
        );
        assert_eq!(
            fs::read_to_string(installed.join(DIGEST_MARKER)).unwrap(),
            digest
        );
        assert_eq!(
            fs::read_to_string(installed.join(PACKAGE_ID_MARKER)).unwrap(),
            "use/a3s/review"
        );
        assert!(
            agents
                .join("review")
                .symlink_metadata()
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(crate::review_plugin::skill_published());
        let store = ReconcileStore::open(root.join("package-generations")).unwrap();
        let current = store
            .current_sync("use/a3s/review")
            .unwrap()
            .expect("generation");
        assert!(
            current
                .surfaces
                .iter()
                .any(|surface| surface.kind == SurfaceKind::Skill)
        );
        assert!(
            current
                .surfaces
                .iter()
                .any(|surface| surface.kind == SurfaceKind::Agent)
        );
        assert!(
            !current
                .surfaces
                .iter()
                .any(|surface| surface.kind == SurfaceKind::Ui)
        );

        sync_plugins(&caps, &agents, &pins, &fetch).unwrap();
        assert_eq!(
            downloads.get(),
            initial_downloads,
            "same package is not downloaded again"
        );

        sync_plugins(&caps, &agents, &BTreeMap::new(), &fetch).unwrap();
        assert!(!installed.exists());
        assert!(!agents.join("review").exists());
        assert!(store.current_sync("use/a3s/review").unwrap().is_none());
        assert!(!crate::review_plugin::skill_published());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn installed_skill_mcp_tool_and_flow_plugins_are_callable() {
        let root = temp_root("plugin-calls");
        let caps = root.join("capabilities");
        let agents = root.join("agents");
        fs::create_dir_all(&agents).unwrap();
        let skill = bundle_zip(&[("skills/notes/SKILL.md", "---\nname: notes\n---\nNotes\n")]);
        let mcp = bundle_zip(&[(
            "mcp/notes-mcp/server.json",
            r#"{"type":"stdio","command":"notes-mcp","args":[]}"#,
        )]);
        let tool = bundle_zip(&[
            ("tool/quiet/tool.json", r#"{"entry":"bin/quiet"}"#),
            ("tool/quiet/bin/quiet", "#!/bin/sh\necho quiet\n"),
        ]);
        let flow = bundle_zip(&[
            ("flow/payroll/flow.json", r#"{"source":"index.ts"}"#),
            ("flow/payroll/index.ts", "export function run(){return 1}\n"),
        ]);
        let mut pins = BTreeMap::new();
        for (slug, bytes, package_id) in [
            ("notes-skill", &skill, "use/a3s/notes-skill"),
            ("notes-mcp", &mcp, "use/a3s/notes-mcp"),
            ("quiet-tool", &tool, "use/a3s/quiet-tool"),
            ("payroll-flow", &flow, "use/a3s/payroll-flow"),
        ] {
            pins.insert(
                slug.to_owned(),
                PluginPin {
                    require_aix: false,
                    digest: digest_of(bytes),
                    package_id: package_id.to_owned(),
                },
            );
        }
        let fetch = |slug: &str| match slug {
            "notes-skill" => Ok(skill.clone()),
            "notes-mcp" => Ok(mcp.clone()),
            "quiet-tool" => Ok(tool.clone()),
            "payroll-flow" => Ok(flow.clone()),
            _ => Err(io::Error::other("missing bundle")),
        };
        sync_plugins(&caps, &agents, &pins, &fetch).unwrap();

        assert!(
            agents
                .join("notes")
                .symlink_metadata()
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let servers = acp_session_servers(&caps);
        let notes = servers
            .iter()
            .find(|server| server["name"] == "notes-mcp")
            .expect("mcp server is on the session list");
        assert_eq!(notes["command"], "notes-mcp");

        let store = ReconcileStore::open(root.join("package-generations")).unwrap();
        let published = store.current_sync("use/a3s/quiet-tool").unwrap().unwrap();
        let tool_surface = published
            .surfaces
            .iter()
            .find(|surface| surface.kind == SurfaceKind::Tool)
            .unwrap();
        assert!(tool_surface.entry.as_ref().unwrap().ends_with("bin/quiet"));
        assert!(tool_surface.entry.as_ref().unwrap().is_file());
        let flow_surface = store
            .current_sync("use/a3s/payroll-flow")
            .unwrap()
            .unwrap()
            .surfaces;
        assert!(
            flow_surface
                .iter()
                .any(|surface| surface.kind == SurfaceKind::Flow && surface.entry.is_some())
        );
        assert!(
            store
                .current_sync("use/a3s/notes-mcp")
                .unwrap()
                .unwrap()
                .surfaces
                .iter()
                .any(|surface| surface.kind == SurfaceKind::Mcp
                    && surface.mcp.as_ref().unwrap()["command"] == "notes-mcp")
        );

        for (slug, kind) in [
            ("notes-skill", "skill"),
            ("notes-mcp", "mcp"),
            ("quiet-tool", "tool"),
            ("payroll-flow", "flow"),
        ] {
            let body = callable_plugin_json(&caps, slug).unwrap();
            let doc: Value = serde_json::from_slice(&body).unwrap();
            assert!(
                doc["surfaces"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|row| row["kind"] == kind),
                "{slug} did not report {kind}: {doc}"
            );
        }

        pins.remove("notes-mcp");
        sync_plugins(&caps, &agents, &pins, &fetch).unwrap();
        assert!(
            acp_session_servers(&caps)
                .iter()
                .all(|server| server["name"] != "notes-mcp")
        );
        assert!(callable_plugin_json(&caps, "notes-mcp").is_err());
        fs::remove_dir_all(&root).unwrap();
    }

    /// A legacy native zip ships `mcp/context`, `tools/echo`, and `flows/reason.ts` as files
    /// next to `skills/reason/SKILL.md`. The skill link survives, the agent session
    /// spawns the unpacked MCP binary, and the tool and flow paths are the files
    /// those runners open.
    #[test]
    fn legacy_package_layout_without_aix_manifest_is_callable() {
        let root = temp_root("use-layout");
        let caps = root.join("capabilities");
        let agents = root.join("agents");
        fs::create_dir_all(&agents).unwrap();
        let zip = bundle_zip(&[
            (
                "skills/reason/SKILL.md",
                "---\nname: reason\ndescription: Reason over the package.\n---\n# Reason\n",
            ),
            (
                "mcp/context",
                "#!/bin/sh\nprintf '%s\\n' 'stdio MCP fixture'\n",
            ),
            (
                "tools/echo",
                "#!/bin/sh\nprintf '%s\\n' '{\"message\":\"cognitive package ready\"}'\n",
            ),
            (
                "flows/reason.ts",
                "export async function run() {\n  return { type: \"complete\", output: null };\n}\n",
            ),
        ]);
        let digest = digest_of(&zip);
        let mut pins = BTreeMap::new();
        pins.insert(
            "cognitive".to_owned(),
            PluginPin {
                require_aix: false,
                digest,
                package_id: "acme/cognitive".to_owned(),
            },
        );
        let fetch = |slug: &str| {
            assert_eq!(slug, "cognitive");
            Ok(zip.clone())
        };
        sync_plugins(&caps, &agents, &pins, &fetch).unwrap();

        let skill_link = fs::read_link(agents.join("reason")).unwrap();
        assert!(
            agents
                .join("reason")
                .symlink_metadata()
                .unwrap()
                .file_type()
                .is_symlink(),
            "the skill in the same zip was dropped"
        );
        assert!(
            skill_link.ends_with("skills/reason"),
            "skill link: {}",
            skill_link.display()
        );

        let published = ReconcileStore::open(root.join("package-generations"))
            .unwrap()
            .current_sync("acme/cognitive")
            .unwrap()
            .unwrap();
        let managed = |kind| {
            published
                .surfaces
                .iter()
                .find(|surface| surface.kind == kind)
                .unwrap()
                .entry
                .as_ref()
                .unwrap()
                .clone()
        };
        let servers = acp_session_servers(&caps);
        let context = servers
            .iter()
            .find(|server| server["name"] == "context")
            .expect("mcp/context is on the agent session list");
        let command = context["command"].as_str().expect("session command");
        let unpacked_mcp = managed(SurfaceKind::Mcp);
        assert_eq!(
            command,
            unpacked_mcp.to_string_lossy(),
            "session/new must spawn the verified managed MCP binary"
        );
        let args: Vec<&str> = context["args"]
            .as_array()
            .expect("session args")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert!(
            args.iter().any(|arg| *arg == "--stdio"),
            "session args: {args:?}"
        );

        let unpacked_tool = managed(SurfaceKind::Tool);
        let tool_path = unpacked_tool.to_string_lossy().into_owned();
        let echo = acp_session_tools(&caps)
            .into_iter()
            .find(|tool| tool["id"] == "echo")
            .expect("tools/echo is published for session/new");
        assert_eq!(echo["entry"].as_str(), Some(tool_path.as_str()));

        let unpacked_flow = managed(SurfaceKind::Flow);
        let flow_path = unpacked_flow.to_string_lossy().into_owned();
        let reason = installed_flow_specs(&caps)
            .into_iter()
            .find(|spec| spec["name"] == "reason")
            .expect("flows/reason.ts is stored as a workflow spec");
        assert_eq!(reason["runtime"]["kind"], "native_ts");
        assert_eq!(reason["runtime"]["export_name"], "run");
        assert_eq!(
            reason["runtime"]["entrypoint"].as_str(),
            Some(flow_path.as_str())
        );
        let opened =
            super::read_flow_source(&reason).expect("publish already opened this entrypoint");
        assert_eq!(opened.path, unpacked_flow);
        assert_eq!(opened.logical_path, flow_path);
        assert_eq!(
            reason["sourceHash"].as_str(),
            Some(opened.source_hash.as_str()),
            "publish stored a hash that SourceSnapshot::read did not produce"
        );

        let published = ReconcileStore::open(root.join("package-generations"))
            .unwrap()
            .current_sync("acme/cognitive")
            .unwrap()
            .expect("generation");
        for kind in [
            SurfaceKind::Skill,
            SurfaceKind::Mcp,
            SurfaceKind::Tool,
            SurfaceKind::Flow,
        ] {
            assert!(
                published.surfaces.iter().any(|row| row.kind == kind),
                "generation dropped {kind:?}"
            );
        }

        sync_plugins(&caps, &agents, &BTreeMap::new(), &fetch).unwrap();
        assert!(acp_session_tools(&caps).is_empty());
        assert!(installed_flow_specs(&caps).is_empty());
        assert!(
            acp_session_servers(&caps)
                .iter()
                .all(|server| server["name"] != "context")
        );
        assert!(!agents.join("reason").exists());

        fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn uninstalling_a_plugin_removes_its_skill_links_and_okf_state() {
        let root = temp_root("plugin-deps");
        let caps = root.join("capabilities");
        let agents = root.join("agents");
        fs::create_dir_all(&agents).unwrap();
        let zip = bundle_zip(&[
            ("skills/alpha/SKILL.md", "---\nname: alpha\n---\nAlpha\n"),
            ("skills/beta/SKILL.md", "---\nname: beta\n---\nBeta\n"),
            ("ui/home/index.html", "<!doctype html>"),
        ]);
        let digest = digest_of(&zip);
        let mut pins = BTreeMap::new();
        pins.insert(
            "office-demo".to_owned(),
            PluginPin {
                require_aix: false,
                digest,
                package_id: "use/a3s/office-demo".to_owned(),
            },
        );
        let fetch = |slug: &str| {
            assert_eq!(slug, "office-demo");
            Ok(zip.clone())
        };
        sync_plugins(&caps, &agents, &pins, &fetch).unwrap();
        assert!(
            agents
                .join("alpha")
                .symlink_metadata()
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(
            agents
                .join("beta")
                .symlink_metadata()
                .unwrap()
                .file_type()
                .is_symlink()
        );

        let retired = agents.join("retired");
        std::os::unix::fs::symlink(caps.join("plugins/office-demo/skills/alpha"), &retired)
            .unwrap();
        fs::write(agents.join(".retired.a3s-owner"), "use/a3s/office-demo").unwrap();
        let alias = agents.join("alias");
        std::os::unix::fs::symlink(caps.join("plugins/office-demo/skills/beta"), &alias).unwrap();

        fs::create_dir_all(caps.join("skills/kept")).unwrap();
        fs::write(caps.join("skills/kept/SKILL.md"), "# kept\n").unwrap();
        std::os::unix::fs::symlink(caps.join("skills/kept"), agents.join("kept")).unwrap();
        fs::write(agents.join(".kept.a3s-owner"), "catalog").unwrap();
        fs::create_dir_all(agents.join("mine")).unwrap();
        fs::write(agents.join("mine/SKILL.md"), "# mine\n").unwrap();

        let overlay = root.join("wiki/plugins/office-demo/okf/domain");
        fs::create_dir_all(&overlay).unwrap();
        fs::write(overlay.join("index.md"), "甲方\n").unwrap();
        let index = root.join("vec/plugin-okf/office-demo");
        fs::create_dir_all(&index).unwrap();
        fs::write(index.join("marker"), "idx").unwrap();

        sync_plugins(&caps, &agents, &BTreeMap::new(), &fetch).unwrap();

        assert!(!caps.join("plugins/office-demo").exists());
        assert!(!agents.join("alpha").exists());
        assert!(!agents.join("beta").exists());
        assert!(!agents.join("retired").exists());
        assert!(!agents.join(".retired.a3s-owner").exists());
        assert!(!agents.join("alias").exists());
        assert!(
            agents
                .join("kept")
                .symlink_metadata()
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            fs::read_to_string(agents.join(".kept.a3s-owner")).unwrap(),
            "catalog"
        );
        assert_eq!(
            fs::read_to_string(agents.join("mine/SKILL.md")).unwrap(),
            "# mine\n"
        );
        assert!(!root.join("wiki/plugins/office-demo").exists());
        assert!(!root.join("vec/plugin-okf/office-demo").exists());
        let store = ReconcileStore::open(root.join("package-generations")).unwrap();
        assert!(store.current_sync("use/a3s/office-demo").unwrap().is_none());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn reconcile_publishes_connector_usage_without_the_command() {
        let root = temp_root("usage");
        let secret = "do-not-leak-connector-cmd-9f3a";
        let token = "do-not-leak-token-9f3a";
        let usage =
            usage_past_the_catalog_line("Installed session notes", "BODY-connector-9f3a-unique");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("connector-config.json"),
            format!(r#"{{"session-notes":{{"TOKEN":"{token}"}}}}"#),
        )
        .unwrap();
        let catalog = vec![json!({
            "slug": "session-notes",
            "installed": true,
            "description": usage,
            "install": {
                "type": "mcp",
                "transport": "stdio",
                "command": secret,
                "args": ["--secret", "do-not-leak-env-9f3a"]
            }
        })];
        let desired = desired(&catalog);
        assert_eq!(desired.mcp["session-notes"]["command"], secret);
        assert!(desired.mcp["session-notes"].get("description").is_none());
        assert!(desired.notes["session-notes"].contains("Installed session notes"));
        assert!(!desired.notes["session-notes"].contains(secret));
        assert!(!desired.notes["session-notes"].contains(token));
        reconcile_at(&root, &desired, |_| Err(io::Error::other("no bundle"))).unwrap();
        let mcp = fs::read_to_string(root.join("mcp.json")).unwrap();
        let note = fs::read_to_string(root.join("session-notes.md")).unwrap();
        assert!(mcp.contains(secret));
        assert!(mcp.contains(token));
        assert!(!mcp.contains("BODY-connector-9f3a-unique"));
        assert!(note.contains("Installed session notes"));
        assert!(note.contains("BODY-connector-9f3a-unique"));
        assert!(!note.contains(secret));
        assert!(!note.contains(token));
        assert!(!note.contains("do-not-leak-env-9f3a"));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn reconcile_writes_mcp_installs_skills_and_tracks_digests() {
        let root = temp_root("reconcile");
        let (v1, v2) = (digest_of(&pdf_v1()), digest_of(&pdf_v2()));
        let served = std::cell::RefCell::new(pdf_v1());
        let downloads = std::cell::Cell::new(0);
        let fetch = |slug: &str| {
            assert_eq!(slug, "pdf");
            downloads.set(downloads.get() + 1);
            Ok(served.borrow().clone())
        };

        reconcile_at(&root, &desired(&catalog(&v1, true)), fetch).unwrap();
        let mcp: Value = serde_json::from_slice(&fs::read(root.join("mcp.json")).unwrap()).unwrap();
        assert_eq!(
            mcp["mcpServers"]["linear"]["url"],
            "https://mcp.linear.app/mcp"
        );
        assert_eq!(mcp["mcpServers"]["github"]["type"], "stdio");
        assert_eq!(mcp["mcpServers"]["github"]["command"], "npx");
        assert!(mcp["mcpServers"].get("slack").is_none());
        let skill_md = fs::read_to_string(root.join("skills/pdf/SKILL.md")).unwrap();
        let pinned = fs::read_to_string(root.join("skills/pdf").join(DIGEST_MARKER)).unwrap();
        assert_eq!(skill_md, "# PDF");
        assert_eq!(pinned, v1);
        eprintln!("digest-checked unpack: SKILL.md={skill_md:?} pinned={pinned}");
        assert_eq!(
            fs::read_to_string(root.join("skills/pdf/scripts/run.py")).unwrap(),
            "print(1)"
        );
        assert_eq!(downloads.get(), 1);

        reconcile_at(&root, &desired(&catalog(&v1, true)), fetch).unwrap();
        assert_eq!(downloads.get(), 1, "same package is not downloaded again");
        *served.borrow_mut() = pdf_v2();
        reconcile_at(&root, &desired(&catalog(&v2, true)), fetch).unwrap();
        assert_eq!(
            downloads.get(),
            2,
            "a new pinned package replaces the skill"
        );
        assert_eq!(
            fs::read_to_string(root.join("skills/pdf").join(DIGEST_MARKER)).unwrap(),
            v2
        );
        assert!(!root.join("skills/pdf/scripts").exists());

        fs::create_dir_all(root.join("skills/stale")).unwrap();
        reconcile_at(&root, &desired(&catalog(&v2, false)), fetch).unwrap();
        assert!(!root.join("skills/pdf").exists());
        assert!(!root.join("skills/stale").exists());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_bad_bundle_leaves_nothing_behind_and_reports_the_skill() {
        let root = temp_root("bad-bundle");
        for bytes in [
            bundle_zip(&[("README.md", "no skill")]),
            bundle_zip(&[("../escape/SKILL.md", "# x")]),
            b"not a zip".to_vec(),
        ] {
            let desired = desired(&catalog(&digest_of(&bytes), true));
            let err = reconcile_at(&root, &desired, |_| Ok(bytes.clone())).unwrap_err();
            assert!(err.to_string().starts_with("pdf: "), "{err}");
            assert_eq!(fs::read_dir(root.join("skills")).unwrap().count(), 0);
            assert!(!root.join("escape").exists());
        }
        let pinned = desired(&catalog(&digest_of(&pdf_v1()), true));
        let err = reconcile_at(&root, &pinned, |_| Ok(pdf_v2())).unwrap_err();
        assert!(err.to_string().contains("pinned digest"), "{err}");
        eprintln!("digest-checked unpack refused: {err}");
        assert_eq!(fs::read_dir(root.join("skills")).unwrap().count(), 0);
        assert!(root.join("mcp.json").is_file(), "connectors still land");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn skill_failure_keeps_the_connector_file_bytes() {
        let root = temp_root("skill-keeps-mcp");
        let installed = digest_of(&pdf_v1());
        let requested = digest_of(&pdf_v2());
        reconcile_at(
            &root,
            &desired(&catalog(&installed, true)),
            |_| Ok(pdf_v1()),
        )
        .unwrap();
        let before = fs::read(root.join("mcp.json")).unwrap();
        let err = reconcile_at(
            &root,
            &desired(&catalog(&requested, true)),
            |_| Ok(pdf_v1()),
        )
        .unwrap_err();
        assert!(err.to_string().contains("pinned digest"), "{err}");
        assert_eq!(fs::read(root.join("mcp.json")).unwrap(), before);
        assert_eq!(
            fs::read_to_string(root.join("skills/pdf").join(DIGEST_MARKER)).unwrap(),
            installed
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn installed_skills_link_into_the_agents_directory_without_clobbering_user_skills() {
        let root = temp_root("publish");
        let agents = temp_root("agents-skills");
        let digest = digest_of(&pdf_v1());
        reconcile_at(&root, &desired(&catalog(&digest, true)), |_| Ok(pdf_v1())).unwrap();
        fs::write(
            root.join("skills/pdf/SKILL.md"),
            "---\nname: Agent Shield\nid: agent-shield\n---\n# PDF\n",
        )
        .unwrap();
        fs::create_dir_all(agents.join("agent-shield")).unwrap();
        fs::write(agents.join("agent-shield/SKILL.md"), "# mine\n").unwrap();
        let elsewhere = temp_root("foreign-skill");
        fs::create_dir_all(&elsewhere).unwrap();
        std::os::unix::fs::symlink(&elsewhere, agents.join("notes")).unwrap();

        let installed = desired(&catalog(&digest, true)).skills;
        publish_skills(&root.join("skills"), &agents, &installed).unwrap();
        assert!(
            agents.join("agent-shield/SKILL.md").is_file()
                && fs::read_to_string(agents.join("agent-shield/SKILL.md")).unwrap() == "# mine\n",
            "a directory the user owns stays"
        );
        assert!(
            fs::symlink_metadata(agents.join("pdf"))
                .unwrap()
                .file_type()
                .is_symlink(),
            "the catalog slug is the fallback link"
        );
        assert_eq!(
            fs::read_to_string(agents.join("pdf/SKILL.md")).unwrap(),
            "---\nname: Agent Shield\nid: agent-shield\n---\n# PDF\n"
        );
        assert!(
            fs::symlink_metadata(agents.join("notes"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read_link(agents.join("notes")).unwrap(), elsewhere);

        publish_skills(&root.join("skills"), &agents, &BTreeMap::new()).unwrap();
        assert!(!agents.join("pdf").exists());
        assert!(agents.join("agent-shield").is_dir());
        assert_eq!(fs::read_link(agents.join("notes")).unwrap(), elsewhere);
        fs::remove_dir_all(&root).unwrap();
        fs::remove_dir_all(&agents).unwrap();
        fs::remove_dir_all(&elsewhere).unwrap();
    }

    #[test]
    fn member_config_is_applied_to_mcp_json_and_removed_on_uninstall() {
        let root = temp_root("config");
        let saved = store_config(
            &root,
            "github",
            br#"{"values":{"GITHUB_PERSONAL_ACCESS_TOKEN":"ghp_x","1NO":"no"}}"#,
        );
        assert!(saved.is_err(), "a bad key rejects the whole save");

        store_config(
            &root,
            "github",
            br#"{"values":{"GITHUB_PERSONAL_ACCESS_TOKEN":"ghp_x"}}"#,
        )
        .unwrap();
        store_config(&root, "linear", br#"{"values":{"LINEAR_API_KEY":"lin_x"}}"#).unwrap();
        let v1 = digest_of(&pdf_v1());
        reconcile_at(&root, &desired(&catalog(&v1, true)), |_| Ok(pdf_v1())).unwrap();
        let mcp: Value = serde_json::from_slice(&fs::read(root.join("mcp.json")).unwrap()).unwrap();
        assert_eq!(
            mcp["mcpServers"]["github"]["env"]["GITHUB_PERSONAL_ACCESS_TOKEN"],
            "ghp_x"
        );
        assert_eq!(
            mcp["mcpServers"]["linear"]["headers"]["LINEAR_API_KEY"],
            "lin_x"
        );
        assert!(mcp["mcpServers"]["github"].get("headers").is_none());

        let mut entries = catalog(&v1, true);
        entries[0]["installed"] = json!(false);
        reconcile_at(&root, &desired(&entries), |_| Ok(pdf_v1())).unwrap();
        let mcp: Value = serde_json::from_slice(&fs::read(root.join("mcp.json")).unwrap()).unwrap();
        assert!(mcp["mcpServers"].get("github").is_none());
        let config: Value =
            serde_json::from_slice(&fs::read(root.join("connector-config.json")).unwrap()).unwrap();
        assert!(config.get("github").is_none());
        assert_eq!(config["linear"]["LINEAR_API_KEY"], "lin_x");

        store_config(&root, "linear", br#"{"values":{}}"#).unwrap();
        let mcp: Value = serde_json::from_slice(&fs::read(root.join("mcp.json")).unwrap()).unwrap();
        assert!(mcp["mcpServers"]["linear"].get("headers").is_none());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn installed_connectors_become_acp_session_servers() {
        let root = temp_root("acp-servers");
        assert!(acp_session_servers(&root).is_empty());
        store_config(
            &root,
            "github",
            br#"{"values":{"GITHUB_PERSONAL_ACCESS_TOKEN":"ghp_x"}}"#,
        )
        .unwrap();
        store_config(&root, "linear", br#"{"values":{"LINEAR_API_KEY":"lin_x"}}"#).unwrap();
        let v1 = digest_of(&pdf_v1());
        reconcile_at(&root, &desired(&catalog(&v1, true)), |_| Ok(pdf_v1())).unwrap();

        let servers = acp_session_servers(&root);
        let github = servers
            .iter()
            .find(|server| server["name"] == "github")
            .unwrap();
        assert!(github.get("type").is_none(), "stdio is untagged in ACP");
        assert_eq!(github["command"], "npx");
        assert_eq!(
            github["args"],
            json!(["-y", "@modelcontextprotocol/server-github"])
        );
        assert_eq!(
            github["env"],
            json!([{ "name": "GITHUB_PERSONAL_ACCESS_TOKEN", "value": "ghp_x" }])
        );
        let linear = servers
            .iter()
            .find(|server| server["name"] == "linear")
            .unwrap();
        assert_eq!(linear["type"], "http");
        assert_eq!(linear["url"], "https://mcp.linear.app/mcp");
        assert_eq!(
            linear["headers"],
            json!([{ "name": "LINEAR_API_KEY", "value": "lin_x" }])
        );
        assert!(servers.iter().all(|server| server["name"] != "slack"));
        assert!(servers.iter().all(|server| server["name"] != "pdf"));

        let mut doc: Value =
            serde_json::from_slice(&fs::read(root.join("mcp.json")).unwrap()).unwrap();
        doc["mcpServers"]["broken"] = json!({ "type": "stdio" });
        doc["mcpServers"]["notes"] = json!({
            "type": "sse",
            "url": "https://notes.example/sse",
            "headers": { "X_TOKEN": "n" }
        });
        fs::write(root.join("mcp.json"), serde_json::to_vec(&doc).unwrap()).unwrap();
        let servers = acp_session_servers(&root);
        assert!(servers.iter().all(|server| server["name"] != "broken"));
        let notes = servers
            .iter()
            .find(|server| server["name"] == "notes")
            .unwrap();
        assert_eq!(notes["type"], "sse");
        assert_eq!(notes["url"], "https://notes.example/sse");
        assert_eq!(
            notes["headers"],
            json!([{ "name": "X_TOKEN", "value": "n" }])
        );
        fs::remove_dir_all(&root).unwrap();
    }

    fn skill_md(name: &str, description: &str) -> String {
        format!("---\nname: {name}\ndescription: {description}\n---\n# {name}\n")
    }

    #[test]
    fn an_uploaded_skill_lands_as_a_real_directory_and_survives_publish() {
        let agents = temp_root("upload-ok");
        let skills = temp_root("upload-catalog");
        fs::create_dir_all(&skills).unwrap();
        let bytes = bundle_zip(&[(
            "SKILL.md",
            &skill_md("demo-skill", "Writes a short demo without any markup."),
        )]);
        assert_eq!(
            import_skill_archive(&bytes, &agents, MAX_SKILL_ARCHIVE).unwrap(),
            "demo-skill"
        );
        let dir = agents.join("demo-skill");
        assert!(dir.join("SKILL.md").is_file());
        assert!(!dir.symlink_metadata().unwrap().file_type().is_symlink());
        assert_eq!(
            fs::read_to_string(dir.join(".a3s-upload")).unwrap(),
            "demo-skill"
        );
        publish_skills(&skills, &agents, &BTreeMap::new()).unwrap();
        assert!(dir.join("SKILL.md").is_file());

        let wrapped = bundle_zip(&[(
            "repo/demo-skill/SKILL.md",
            &skill_md(
                "demo-skill",
                "A folded update that replaces the previous upload.",
            ),
        )]);
        import_skill_archive(&wrapped, &agents, MAX_SKILL_ARCHIVE).unwrap();
        let body = fs::read_to_string(dir.join("SKILL.md")).unwrap();
        assert!(body.contains("folded update"), "{body}");
        fs::remove_dir_all(&agents).unwrap();
        fs::remove_dir_all(&skills).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn an_upload_refuses_a_package_that_could_escape_or_clobber() {
        let agents = temp_root("upload-refuse");
        fs::create_dir_all(agents.join("kept-skill")).unwrap();
        fs::write(agents.join("kept-skill/SKILL.md"), "# mine\n").unwrap();
        let foreign = temp_root("upload-foreign");
        fs::create_dir_all(&foreign).unwrap();
        std::os::unix::fs::symlink(&foreign, agents.join("linked-skill")).unwrap();

        let kept = bundle_zip(&[(
            "SKILL.md",
            &skill_md("kept-skill", "Should not replace a directory."),
        )]);
        let err = import_skill_archive(&kept, &agents, MAX_SKILL_ARCHIVE).unwrap_err();
        assert_eq!(skill_import_code(&err), "skill_exists");
        assert_eq!(
            fs::read_to_string(agents.join("kept-skill/SKILL.md")).unwrap(),
            "# mine\n"
        );

        let linked = bundle_zip(&[(
            "SKILL.md",
            &skill_md("linked-skill", "Should not replace a symlink."),
        )]);
        assert_eq!(
            skill_import_code(
                &import_skill_archive(&linked, &agents, MAX_SKILL_ARCHIVE).unwrap_err()
            ),
            "skill_exists"
        );
        assert!(
            agents
                .join("linked-skill")
                .symlink_metadata()
                .unwrap()
                .file_type()
                .is_symlink()
        );

        let escape = bundle_zip(&[("../escape/SKILL.md", &skill_md("escape-skill", "Nope."))]);
        assert_eq!(
            skill_import_code(
                &import_skill_archive(&escape, &agents, MAX_SKILL_ARCHIVE).unwrap_err()
            ),
            "escapes"
        );
        assert!(!agents.join("escape").exists());
        assert!(!std::env::temp_dir().join("escape").exists());

        let mut writer = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
        writer
            .add_symlink(
                "SKILL.md",
                "/tmp/outside",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        let symlink_zip = writer.finish().unwrap().into_inner();
        assert_eq!(
            skill_import_code(
                &import_skill_archive(&symlink_zip, &agents, MAX_SKILL_ARCHIVE).unwrap_err()
            ),
            "symlink"
        );
        assert_eq!(fs::read_dir(&agents).unwrap().count(), 2);

        let bad_name = bundle_zip(&[(
            "SKILL.md",
            &skill_md("Bad Name", "A display name is not a folder."),
        )]);
        assert_eq!(
            skill_import_code(
                &import_skill_archive(&bad_name, &agents, MAX_SKILL_ARCHIVE).unwrap_err()
            ),
            "invalid_name"
        );
        let brackets = bundle_zip(&[(
            "SKILL.md",
            &skill_md("bracket-skill", "Uses <b>markup</b>."),
        )]);
        assert_eq!(
            skill_import_code(
                &import_skill_archive(&brackets, &agents, MAX_SKILL_ARCHIVE).unwrap_err()
            ),
            "invalid_description"
        );
        let huge = vec![0u8; 64];
        assert_eq!(
            skill_import_code(&import_skill_archive(&huge, &agents, 32).unwrap_err()),
            "too_large"
        );
        assert_eq!(
            skill_import_code(
                &import_skill_archive(b"not a zip", &agents, MAX_SKILL_ARCHIVE).unwrap_err()
            ),
            "bad_format"
        );
        fs::remove_dir_all(&agents).unwrap();
        fs::remove_dir_all(&foreign).unwrap();
    }

    #[test]
    fn builtin_skills_link_without_replacing_a_directory() {
        let agents = temp_root("builtin");
        fs::create_dir_all(agents.join("schedule-task")).unwrap();
        fs::write(agents.join("schedule-task/SKILL.md"), "# mine\n").unwrap();
        prepare_builtin_skill("schedule-task", &agents).unwrap();
        assert_eq!(
            fs::read_to_string(agents.join("schedule-task/SKILL.md")).unwrap(),
            "# mine\n"
        );
        assert!(
            !agents
                .join("schedule-task")
                .symlink_metadata()
                .unwrap()
                .file_type()
                .is_symlink()
        );

        prepare_builtin_skill("skill-creator", &agents).unwrap();
        prepare_builtin_skill("stc-answer", &agents).unwrap();
        prepare_builtin_skill("ask-form", &agents).unwrap();
        let form = fs::read_to_string(agents.join("ask-form/SKILL.md")).unwrap();
        assert!(form.contains("name: ask-form"));
        assert!(form.contains("```openui"));
        assert!(agents.join("schedule-task/SKILL.md").is_file());
        let knowledge = fs::read_to_string(agents.join("stc-answer/SKILL.md")).unwrap();
        assert!(knowledge.contains("name: stc-answer"));
        assert!(
            agents
                .join("stc-answer")
                .symlink_metadata()
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let err = prepare_builtin_skill("review", &agents).unwrap_err();
        assert_eq!(skill_import_code(&err), "unknown_builtin");
        assert!(!agents.join("review").exists());
        assert!(
            agents
                .join("skill-creator")
                .symlink_metadata()
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(agents.join("skill-creator/SKILL.md").is_file());
        let err = prepare_builtin_skill("pdf", &agents).unwrap_err();
        assert_eq!(skill_import_code(&err), "unknown_builtin");
        let err = prepare_builtin_skill("find-skills", &agents).unwrap_err();
        assert_eq!(skill_import_code(&err), "unknown_builtin");
        assert!(!agents.join("find-skills").exists());
        fs::remove_dir_all(&agents).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn drops_a_retired_find_skills_bundle_link_and_keeps_a_directory() {
        let root = temp_root("retire-find-skills");
        let agents = root.join("agents");
        fs::create_dir_all(&agents).unwrap();
        let bundled = root.join("resources/skills/find-skills");
        fs::create_dir_all(&bundled).unwrap();
        std::os::unix::fs::symlink(&bundled, agents.join("find-skills")).unwrap();
        retire_removed_builtin_skills_at(&agents);
        assert!(agents.join("find-skills").symlink_metadata().is_err());

        fs::create_dir(agents.join("find-skills")).unwrap();
        fs::write(agents.join("find-skills/SKILL.md"), "mine").unwrap();
        retire_removed_builtin_skills_at(&agents);
        assert_eq!(
            fs::read_to_string(agents.join("find-skills/SKILL.md")).unwrap(),
            "mine"
        );

        let elsewhere = root.join("elsewhere");
        fs::create_dir_all(&elsewhere).unwrap();
        fs::remove_dir_all(agents.join("find-skills")).unwrap();
        std::os::unix::fs::symlink(&elsewhere, agents.join("find-skills")).unwrap();
        retire_removed_builtin_skills_at(&agents);
        assert_eq!(
            fs::read_link(agents.join("find-skills")).unwrap(),
            elsewhere
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn custom_connectors_land_in_mcp_json_and_survive_reconcile() {
        let parent = temp_root("custom-parent");
        fs::create_dir_all(&parent).unwrap();
        let root = parent.join("capabilities");
        let refused = [
            &br#"{"name":"","transport":"stdio","command":"npx"}"#[..],
            &br#"{"name":"local-fs","transport":"stdio","command":"   "}"#[..],
            &br#"{"name":"docs","transport":"sse"}"#[..],
            &br#"{"name":"search","transport":"streamable_http","url":""}"#[..],
            &br#"{"name":"docs","transport":"sse","url":"javascript:alert(1)"}"#[..],
            &br#"{"name":"mem","transport":"in-memory","command":"node"}"#[..],
        ];
        let codes = [
            "invalid_name",
            "missing_command",
            "missing_url",
            "missing_url",
            "invalid_url",
            "unknown_transport",
        ];
        for (body, code) in refused.iter().zip(codes) {
            let err = save_custom_connector(&root, body).unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "{code}");
            assert_eq!(custom_connector_code(&err), code);
            assert!(!root.exists(), "invalid input creates nothing");
        }
        assert!(!parent.join(".cursor").join("mcp.json").exists());
        assert!(!parent.join(".mcp.json").exists());
        assert!(!parent.join("mcp.json").exists());

        save_custom_connector(
            &root,
            br#"{"name":"local-fs","transport":"stdio","command":"npx","args":["-y","pkg"],"values":{"TOKEN":"tok_test"}}"#,
        )
        .unwrap();
        save_custom_connector(
            &root,
            br#"{"name":"docs","transport":"sse","url":"https://docs.example/sse","values":{"Authorization":"Bearer tok_test"}}"#,
        )
        .unwrap();
        save_custom_connector(
            &root,
            br#"{"name":"search","transport":"streamable_http","url":"https://search.example/mcp","values":{"X_API_KEY":"tok_test"}}"#,
        )
        .unwrap();

        let mcp: Value = serde_json::from_slice(&fs::read(root.join("mcp.json")).unwrap()).unwrap();
        assert_eq!(mcp["mcpServers"]["local-fs"]["type"], "stdio");
        assert_eq!(mcp["mcpServers"]["local-fs"]["command"], "npx");
        assert_eq!(mcp["mcpServers"]["local-fs"]["args"], json!(["-y", "pkg"]));
        assert_eq!(mcp["mcpServers"]["local-fs"]["env"]["TOKEN"], "tok_test");
        assert_eq!(mcp["mcpServers"]["local-fs"]["_a3s"], "custom");
        assert_eq!(mcp["mcpServers"]["docs"]["type"], "sse");
        assert_eq!(mcp["mcpServers"]["docs"]["url"], "https://docs.example/sse");
        assert_eq!(
            mcp["mcpServers"]["docs"]["headers"]["Authorization"],
            "Bearer tok_test"
        );
        assert_eq!(mcp["mcpServers"]["search"]["type"], "http");
        assert_eq!(
            mcp["mcpServers"]["search"]["url"],
            "https://search.example/mcp"
        );
        assert_eq!(
            mcp["mcpServers"]["search"]["headers"]["X_API_KEY"],
            "tok_test"
        );

        let ledger: Value =
            serde_json::from_slice(&fs::read(root.join("custom-connectors.json")).unwrap())
                .unwrap();
        assert!(ledger["local-fs"].get("env").is_none());
        assert!(ledger["docs"].get("headers").is_none());
        assert!(
            !ledger.to_string().contains("tok_test"),
            "secrets stay out of the ledger"
        );
        let config: Value =
            serde_json::from_slice(&fs::read(root.join("connector-config.json")).unwrap()).unwrap();
        assert_eq!(config["local-fs"]["TOKEN"], "tok_test");

        let listed: Value = serde_json::from_str(&list_custom_json(&root)).unwrap();
        let search = listed["connectors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["name"] == "search")
            .unwrap();
        assert_eq!(search["transport"], "streamable_http");
        assert_eq!(search["url"], "https://search.example/mcp");
        assert_eq!(search["values"]["X_API_KEY"], "tok_test");

        let servers = acp_session_servers(&root);
        let local = servers
            .iter()
            .find(|server| server["name"] == "local-fs")
            .unwrap();
        assert!(local.get("type").is_none(), "stdio is untagged in ACP");
        assert_eq!(local["command"], "npx");
        assert_eq!(local["args"], json!(["-y", "pkg"]));
        assert_eq!(
            local["env"],
            json!([{ "name": "TOKEN", "value": "tok_test" }])
        );
        assert!(local.get("_a3s").is_none());
        let docs = servers
            .iter()
            .find(|server| server["name"] == "docs")
            .unwrap();
        assert_eq!(docs["type"], "sse");
        assert_eq!(docs["url"], "https://docs.example/sse");
        let remote = servers
            .iter()
            .find(|server| server["name"] == "search")
            .unwrap();
        assert_eq!(remote["type"], "http");
        assert_eq!(remote["url"], "https://search.example/mcp");

        save_custom_connector(
            &root,
            br#"{"name":"local-fs","transport":"stdio","command":"node","args":["server.js"],"values":{"TOKEN":"tok_next"}}"#,
        )
        .unwrap();
        let mcp: Value = serde_json::from_slice(&fs::read(root.join("mcp.json")).unwrap()).unwrap();
        assert_eq!(mcp["mcpServers"]["local-fs"]["command"], "node");
        assert_eq!(mcp["mcpServers"]["local-fs"]["env"]["TOKEN"], "tok_next");
        assert_eq!(mcp["mcpServers"]["docs"]["type"], "sse");

        let v1 = digest_of(&pdf_v1());
        reconcile_at(&root, &desired(&catalog(&v1, false)), |_| Ok(pdf_v1())).unwrap();
        let mcp: Value = serde_json::from_slice(&fs::read(root.join("mcp.json")).unwrap()).unwrap();
        assert_eq!(mcp["mcpServers"]["local-fs"]["command"], "node");
        assert_eq!(mcp["mcpServers"]["local-fs"]["env"]["TOKEN"], "tok_next");
        assert_eq!(mcp["mcpServers"]["local-fs"]["_a3s"], "custom");
        assert_eq!(mcp["mcpServers"]["docs"]["type"], "sse");
        assert_eq!(mcp["mcpServers"]["search"]["type"], "http");
        assert_eq!(mcp["mcpServers"]["github"]["command"], "npx");
        assert!(mcp["mcpServers"]["github"].get("_a3s").is_none());
        assert_eq!(
            mcp["mcpServers"]["linear"]["url"],
            "https://mcp.linear.app/mcp"
        );
        assert!(mcp["mcpServers"].get("slack").is_none());
        let servers = acp_session_servers(&root);
        assert!(servers.iter().any(|server| server["name"] == "local-fs"));
        assert!(servers.iter().any(|server| server["name"] == "github"));
        assert!(servers.iter().any(|server| server["name"] == "linear"));

        let before = fs::read(root.join("mcp.json")).unwrap();
        let ledger_before = fs::read(root.join("custom-connectors.json")).unwrap();
        let err = save_custom_connector(
            &root,
            br#"{"name":"github","transport":"stdio","command":"evil"}"#,
        )
        .unwrap_err();
        assert_eq!(err.to_string(), "connector_exists");
        assert_eq!(fs::read(root.join("mcp.json")).unwrap(), before);
        let err =
            save_custom_connector(&root, br#"{"name":"next","transport":"nope"}"#).unwrap_err();
        assert_eq!(custom_connector_code(&err), "unknown_transport");
        assert_eq!(fs::read(root.join("mcp.json")).unwrap(), before);
        assert_eq!(
            fs::read(root.join("custom-connectors.json")).unwrap(),
            ledger_before
        );

        remove_custom_connector(&root, "docs").unwrap();
        let mcp: Value = serde_json::from_slice(&fs::read(root.join("mcp.json")).unwrap()).unwrap();
        assert!(mcp["mcpServers"].get("docs").is_none());
        assert_eq!(mcp["mcpServers"]["search"]["type"], "http");
        assert_eq!(mcp["mcpServers"]["github"]["type"], "stdio");
        assert_eq!(mcp["mcpServers"]["local-fs"]["command"], "node");
        let ledger: Value =
            serde_json::from_slice(&fs::read(root.join("custom-connectors.json")).unwrap())
                .unwrap();
        assert!(ledger.get("docs").is_none());
        let config: Value =
            serde_json::from_slice(&fs::read(root.join("connector-config.json")).unwrap()).unwrap();
        assert!(config.get("docs").is_none());
        assert_eq!(config["local-fs"]["TOKEN"], "tok_next");
        assert!(
            acp_session_servers(&root)
                .iter()
                .all(|server| server["name"] != "docs")
        );

        assert!(!parent.join(".cursor").exists());
        assert!(!parent.join(".mcp.json").exists());
        assert!(!parent.join("mcp.json").exists());
        fs::remove_dir_all(&parent).unwrap();
    }
}
