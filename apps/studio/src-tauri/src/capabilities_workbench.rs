//! Native AIX identities, view discovery and read-only, selection-bound asset sessions.

use super::{DIGEST_MARKER, PACKAGE_ID_MARKER, host_skill_error, read_plugin_marker};
use a3s_use_extension::{
    AgentHarness, AgentRuntimePlace, ExtensionManifest, PluginUiPoint, PluginUiSurface,
    STUDIO_CONTRIBUTION_PROTOCOL_VERSION, studio_extension_point_contracts,
};
use a3s_use_reconcile::{
    Publication, PublishedSurface, ReconcileStore, SelectionInspection, SurfaceKind,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Component, Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// An edge package agent a live view may call. It belongs to the view's own
/// package generation and revision, and nothing else.
#[derive(Debug)]
pub(crate) struct ViewAgent {
    pub entry: PathBuf,
    pub declared_entry: String,
    pub release_identity: String,
    pub cpu_millis: u64,
    pub memory_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ViewResourceIdentity {
    pub package_id: String,
    pub generation: u64,
    pub revision: String,
    pub activation_id: String,
    pub instance_id: String,
    pub can_read: bool,
    pub can_write: bool,
    #[serde(skip)]
    pub surface: String,
}

const MAX_ASSET: u64 = 16 * 1024 * 1024;
const SESSION_LIFETIME: Duration = Duration::from_secs(30 * 60);
const MAX_SESSIONS: usize = 64;
const VIEW_PROTOCOL: u32 = 3;
const ADMISSION_LIFETIME_MS: u64 = 30_000;
const MAX_REQUEST_FENCES: usize = 4096;
const MAX_CATALOGUE_DIAGNOSTICS: usize = 128;

fn now_ms() -> io::Result<u64> {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| missing())?
            .as_millis(),
    )
    .map_err(|_| missing())
}

fn request_identity(owner: &str, id: &str, expires_at: u64, now: u64) -> io::Result<()> {
    if owner.is_empty()
        || id.len() != 64
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || expires_at == 0
        || expires_at > now.checked_add(ADMISSION_LIFETIME_MS).ok_or_else(missing)?
    {
        return Err(missing());
    }
    Ok(())
}

fn view_identity(activation_id: &str, instance_id: &str) -> io::Result<()> {
    for identity in [activation_id, instance_id] {
        if identity.len() != 64
            || !identity
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(missing());
        }
    }
    Ok(())
}

fn missing() -> io::Error {
    io::ErrorKind::NotFound.into()
}

fn store(caps: &Path) -> io::Result<ReconcileStore> {
    let root = caps.parent().ok_or_else(missing)?;
    ReconcileStore::open(root.join("package-generations")).map_err(host_skill_error)
}

fn selection(caps: &Path) -> io::Result<Option<SelectionInspection>> {
    let store = store(caps)?;
    store
        .selection_sync(super::selection::INSTALLED_SELECTION)
        .map_err(host_skill_error)?
        .map(|snapshot| {
            let selection = store
                .inspect_selection_sync(&snapshot.cursor())
                .map_err(host_skill_error)?;
            super::validate_plugin_selection(selection.publications())?;
            Ok(selection)
        })
        .transpose()
}

fn installed(caps: &Path, slug: &str, package_id: &str) -> io::Result<()> {
    if !crate::org_login::capability_slug_ok(slug) {
        return Err(missing());
    }
    let dir = caps.join("plugins").join(slug);
    if !fs::symlink_metadata(&dir)?.is_dir()
        || dir.canonicalize()? != caps.canonicalize()?.join("plugins").join(slug)
        || read_plugin_marker(&dir.join(PACKAGE_ID_MARKER))? != package_id
    {
        return Err(missing());
    }
    for entry in fs::read_dir(caps.join("plugins"))? {
        let other = entry?;
        if other.path() != dir
            && other.file_type().is_ok_and(|kind| kind.is_dir())
            && other
                .file_name()
                .into_string()
                .ok()
                .is_some_and(|name| crate::org_login::capability_slug_ok(&name))
            && read_plugin_marker(&other.path().join(PACKAGE_ID_MARKER))
                .ok()
                .as_deref()
                == Some(package_id)
        {
            return Err(missing());
        }
    }
    Ok(())
}

fn manifest(publication: &Publication) -> io::Result<ExtensionManifest> {
    super::aix_projection::publication_manifest(publication, true)
        .map_err(|_| missing())?
        .ok_or_else(missing)
}

fn declared<'a>(
    manifest: &'a ExtensionManifest,
    surface: &PublishedSurface,
) -> io::Result<&'a PluginUiSurface> {
    let view = manifest
        .ui
        .iter()
        .find(|view| view.id == surface.id)
        .ok_or_else(missing)?;
    if surface.entry.as_ref() != Some(&surface.directory.join(&view.entry))
        || view.entry.extension().and_then(|s| s.to_str()) != Some("html")
    {
        return Err(missing());
    }
    Ok(view)
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
enum CatalogueRefusal {
    PackageUnavailable,
    PackageContentRefused,
}

#[derive(Serialize)]
struct CatalogueDiagnostic {
    slug: Option<String>,
    reason: CatalogueRefusal,
}

fn catalogue_package(
    caps: &Path,
    slug: &str,
    lease: &SelectionInspection,
) -> io::Result<Option<serde_json::Value>> {
    let package_id = read_plugin_marker(&caps.join("plugins").join(slug).join(PACKAGE_ID_MARKER))?;
    installed(caps, slug, &package_id)?;
    let package_digest = read_plugin_marker(&caps.join("plugins").join(slug).join(DIGEST_MARKER))?;
    if !super::digest_ok(&package_digest) {
        return Err(missing());
    }
    let publication = lease
        .publications()
        .find(|p| p.package_id == package_id)
        .ok_or_else(missing)?;
    let Some(first) =
        super::aix_projection::publication_manifest(publication, false).map_err(|_| missing())?
    else {
        return Ok(None);
    };
    let surfaces: Vec<_> = publication
        .surfaces
        .iter()
        .filter(|s| s.kind == SurfaceKind::Ui)
        .collect();
    // Agent-only packages still need an active archive identity for update checks.
    let mut views = Vec::new();
    for surface in surfaces {
        let view = declared(&first, surface)?;
        let point = if let Some(container) = &view.container {
            match first
                .view_containers
                .iter()
                .find(|c| &c.id == container)
                .ok_or_else(missing)?
                .location
            {
                a3s_use_extension::PluginViewLocation::ActivityBar => "studio.sidebar",
                a3s_use_extension::PluginViewLocation::Panel => "studio.panel",
            }
        } else {
            view.point.unwrap_or(PluginUiPoint::Editor).as_str()
        };
        let mut facts = json!({"id":view.id, "title":view.title, "description":view.description, "icon":view.icon,
            "order":view.order, "container":view.container, "point":point});
        if let Some(version) = view.contract_version {
            facts["contractVersion"] = json!(version);
        }
        if let Some(mode) = view.mode {
            facts["mode"] = json!(mode.as_str());
        }
        if let Some(presentation) = view.presentation {
            facts["presentation"] = json!(presentation.as_str());
        }
        if let Some(scope) = view.scope {
            facts["scope"] = json!(scope.as_str());
        }
        if let Some(condition) = &view.when {
            facts["when"] = json!(condition);
        }
        if !view.commands.is_empty() {
            facts["commands"] = json!(view.commands);
        }
        if !view.menus.is_empty() {
            facts["menus"] = json!(view.menus);
        }
        if let Some(selector) = &view.selector {
            facts["selector"] = json!(selector);
        }
        if let Some(priority) = view.priority {
            facts["priority"] = json!(priority.as_str());
        }
        views.push(facts);
    }
    views.sort_by_key(|view| {
        (
            view["order"].as_i64().unwrap_or(100),
            view["id"].as_str().unwrap_or("").to_owned(),
        )
    });
    let mut package = json!({"slug":slug, "packageId":package_id, "packageDigest":package_digest, "generation":publication.generation,
        "revision":publication.cursor().revision, "containers":first.view_containers, "views":views});
    if !first.commands.is_empty() {
        package["commands"] = json!(first.commands);
    }
    if !first.menus.is_empty() {
        package["menus"] = json!(first.menus);
    }
    if !first.keybindings.is_empty() {
        package["keybindings"] = json!(first.keybindings);
    }
    if !first.harness.is_empty() {
        package["harness"] = json!(first.harness);
    }
    if !first.home.is_empty() {
        package["home"] = json!(first.home);
    }
    if !first.composer_modes.is_empty() {
        package["composerModes"] = json!(first.composer_modes);
    }
    Ok(Some(package))
}

pub(super) fn catalogue(caps: &Path) -> io::Result<Vec<u8>> {
    let Some(lease) = selection(caps)? else {
        return super::json_bytes(
            &json!({"protocolVersion":STUDIO_CONTRIBUTION_PROTOCOL_VERSION,"extensionPoints":studio_extension_point_contracts(),"revision":"", "packages":[], "diagnostics":[]}),
        );
    };
    let mut packages = Vec::new();
    let mut diagnostics = Vec::new();
    let dirs = match fs::read_dir(caps.join("plugins")) {
        Ok(dirs) => Some(dirs),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let mut dirs = dirs.into_iter().flatten().collect::<io::Result<Vec<_>>>()?;
    dirs.sort_by_key(|dir| dir.file_name());
    for dir in dirs {
        let slug = dir
            .file_name()
            .into_string()
            .ok()
            .filter(|slug| crate::org_login::capability_slug_ok(slug));
        let result = slug
            .as_deref()
            .ok_or_else(missing)
            .and_then(|slug| catalogue_package(caps, slug, &lease));
        match result {
            Ok(Some(package)) => packages.push(package),
            Ok(None) => {}
            Err(_) if diagnostics.len() < MAX_CATALOGUE_DIAGNOSTICS => {
                let marker = read_plugin_marker(&dir.path().join(PACKAGE_ID_MARKER)).ok();
                let reason = if lease
                    .refusals()
                    .any(|(cursor, _)| Some(&cursor.package_id) == marker.as_ref())
                {
                    CatalogueRefusal::PackageContentRefused
                } else {
                    CatalogueRefusal::PackageUnavailable
                };
                diagnostics.push(CatalogueDiagnostic { slug, reason });
            }
            Err(_) => {}
        }
    }
    packages.sort_by_key(|package| package["slug"].as_str().unwrap_or("").to_owned());
    let mut value = json!({"protocolVersion":STUDIO_CONTRIBUTION_PROTOCOL_VERSION,
        "extensionPoints":studio_extension_point_contracts(),
        "revision":lease.snapshot().cursor().revision,"packages":packages, "diagnostics":diagnostics});
    value["revision"] = json!(super::digest_of(&super::json_bytes(&value)?));
    super::json_bytes(&value)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ViewRequest {
    protocol_version: u32,
    request_id: String,
    activation_id: String,
    instance_id: String,
    expires_at: u64,
    slug: String,
    surface: String,
    revision: String,
}

struct UiSession {
    request_id: String,
    activation_id: String,
    instance_id: String,
    admission_deadline: u64,
    owner: String,
    slug: String,
    surface: String,
    package_id: String,
    package_generation: u64,
    publication_revision: String,
    expires: Instant,
}

struct RequestFence {
    owner: String,
    expires_at: u64,
    activation_id: String,
    instance_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CancelRequest {
    protocol_version: u32,
    activation_id: String,
    instance_id: String,
    expires_at: u64,
}

/// One publication's view, identified exactly as its sessions were admitted.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ViewKey {
    package_id: String,
    generation: u64,
    revision: String,
    surface: String,
}

impl ViewKey {
    fn of(session: &UiSession) -> Self {
        Self {
            package_id: session.package_id.clone(),
            generation: session.package_generation,
            revision: session.publication_revision.clone(),
            surface: session.surface.clone(),
        }
    }
}

/// The servable files of a view, verified once against the content digest.
/// Every read still rechecks the selection and the requested file on disk.
struct VerifiedView {
    directory: PathBuf,
    files: BTreeMap<String, (&'static str, Vec<u8>)>,
    can_read: bool,
    can_write: bool,
}

#[derive(Default)]
pub(super) struct UiSessions {
    sessions: BTreeMap<String, UiSession>,
    requests: BTreeMap<String, RequestFence>,
    views: BTreeMap<ViewKey, VerifiedView>,
    observed_time: u64,
}

fn servable(view: &PluginUiSurface, relative: &str) -> io::Result<&'static str> {
    let path = Path::new(relative);
    if relative.is_empty()
        || relative.contains(['\\', ':'])
        || relative.chars().any(char::is_control)
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(missing());
    }
    let allowed = path == view.entry
        || view.styles.iter().any(|p| p == path)
        || view.scripts.iter().any(|p| p == path)
        || view
            .root
            .as_ref()
            .is_some_and(|root| path.starts_with(root));
    if !allowed {
        return Err(missing());
    }
    mime(path)
}

/// Admit a session's view through the full selection, manifest and digest checks.
fn verify_view(caps: &Path, session: &UiSession) -> io::Result<VerifiedView> {
    let lease = selection(caps)?.ok_or_else(missing)?;
    installed(caps, &session.slug, &session.package_id)?;
    let publication = lease
        .publications()
        .find(|p| {
            p.package_id == session.package_id
                && p.generation == session.package_generation
                && p.cursor().revision == session.publication_revision
        })
        .ok_or_else(missing)?;
    let surface = publication
        .surfaces
        .iter()
        .find(|s| s.kind == SurfaceKind::Ui && s.id == session.surface)
        .ok_or_else(missing)?;
    let manifest = manifest(publication)?;
    let view = declared(&manifest, surface)?;
    let files = surface
        .read_verified_files()
        .map_err(host_skill_error)?
        .into_iter()
        .filter(|file| file.bytes.len() as u64 <= MAX_ASSET)
        .filter_map(|file| {
            let content_type = servable(view, &file.path).ok()?;
            Some((file.path, (content_type, file.bytes)))
        })
        .collect();
    Ok(VerifiedView {
        directory: surface.directory.clone(),
        files,
        can_read: manifest.actions.contains(&a3s_use_core::RiskClass::Read),
        can_write: manifest.actions.contains(&a3s_use_core::RiskClass::Mutate),
    })
}

/// The installed selection still publishes exactly the session's generation.
fn still_selected(caps: &Path, session: &UiSession) -> io::Result<()> {
    installed(caps, &session.slug, &session.package_id)?;
    let snapshot = store(caps)?
        .selection_sync(super::selection::INSTALLED_SELECTION)
        .map_err(host_skill_error)?
        .ok_or_else(missing)?;
    let current = snapshot.is_published()
        && snapshot.packages().iter().any(|cursor| {
            cursor.package_id == session.package_id
                && cursor.generation == session.package_generation
                && cursor.revision == session.publication_revision
        });
    if current { Ok(()) } else { Err(missing()) }
}

impl UiSessions {
    fn retire_unused_views(&mut self) {
        let sessions = &self.sessions;
        self.views.retain(|key, _| {
            sessions
                .values()
                .any(|session| ViewKey::of(session) == *key)
        });
    }

    fn now(&mut self) -> io::Result<u64> {
        self.observed_time = self.observed_time.max(now_ms()?);
        Ok(self.observed_time)
    }

    pub(super) fn open(&mut self, caps: &Path, owner: &str, bytes: &[u8]) -> io::Result<Vec<u8>> {
        let request: ViewRequest = serde_json::from_slice(bytes).map_err(|_| missing())?;
        let now = self.now()?;
        request_identity(owner, &request.request_id, request.expires_at, now)?;
        view_identity(&request.activation_id, &request.instance_id)?;
        if request.protocol_version != VIEW_PROTOCOL || request.expires_at <= now {
            return Err(missing());
        }
        let lease = selection(caps)?.ok_or_else(missing)?;
        if !crate::org_login::capability_slug_ok(&request.slug) {
            return Err(missing());
        }
        let package_id = read_plugin_marker(
            &caps
                .join("plugins")
                .join(&request.slug)
                .join(PACKAGE_ID_MARKER),
        )?;
        installed(caps, &request.slug, &package_id)?;
        let publication = lease
            .publications()
            .find(|p| p.package_id == package_id)
            .ok_or_else(missing)?;
        if publication.cursor().revision != request.revision {
            return Err(missing());
        }
        let surface = publication
            .surfaces
            .iter()
            .find(|s| s.kind == SurfaceKind::Ui && s.id == request.surface)
            .ok_or_else(missing)?;
        let manifest = manifest(publication)?;
        let view = declared(&manifest, surface)?;
        let now = self.now()?;
        self.requests.retain(|_, fence| fence.expires_at > now);
        if request.expires_at <= now
            || self.requests.contains_key(&request.request_id)
            || self.sessions.values().any(|s| {
                s.request_id == request.request_id
                    || (s.owner == owner
                        && s.activation_id == request.activation_id
                        && s.instance_id == request.instance_id)
            })
            || self.requests.values().any(|fence| {
                fence.owner == owner
                    && fence.activation_id == request.activation_id
                    && fence.instance_id == request.instance_id
            })
        {
            return Err(missing());
        }
        if self.requests.len() >= MAX_REQUEST_FENCES {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        self.sessions.retain(|_, session| {
            session.expires > Instant::now()
                && session.owner == owner
                && installed(caps, &session.slug, &session.package_id).is_ok()
                && lease.publications().any(|publication| {
                    publication.package_id == session.package_id
                        && publication.generation == session.package_generation
                        && publication.cursor().revision == session.publication_revision
                })
        });
        self.retire_unused_views();
        if self.sessions.len() >= MAX_SESSIONS {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let mut random = [0u8; 32];
        getrandom::getrandom(&mut random).map_err(|_| io::Error::from(io::ErrorKind::Other))?;
        let token: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
        let entry = view.entry.to_str().ok_or_else(missing)?;
        let segments: Vec<String> = entry
            .split('/')
            .map(|part| {
                url::form_urlencoded::byte_serialize(part.as_bytes())
                    .collect::<String>()
                    .replace('+', "%20")
            })
            .collect();
        let path = format!("/aix-view/{token}/{}", segments.join("/"));
        self.requests.insert(
            request.request_id.clone(),
            RequestFence {
                owner: owner.to_owned(),
                expires_at: request.expires_at,
                activation_id: request.activation_id.clone(),
                instance_id: request.instance_id.clone(),
            },
        );
        self.sessions.insert(
            token.clone(),
            UiSession {
                request_id: request.request_id.clone(),
                activation_id: request.activation_id.clone(),
                instance_id: request.instance_id.clone(),
                admission_deadline: request.expires_at,
                owner: owner.to_owned(),
                slug: request.slug,
                surface: request.surface,
                package_id,
                package_generation: publication.generation,
                publication_revision: request.revision,
                expires: Instant::now() + SESSION_LIFETIME,
            },
        );
        super::json_bytes(
            &json!({"protocolVersion":VIEW_PROTOCOL,"requestId":request.request_id,"activationId":request.activation_id,"instanceId":request.instance_id,"expiresAt":request.expires_at,"token":token,"path":path}),
        )
    }

    pub(super) fn cancel_request(
        &mut self,
        owner: &str,
        id: &str,
        bytes: &[u8],
    ) -> io::Result<Vec<u8>> {
        let request: CancelRequest = serde_json::from_slice(bytes).map_err(|_| missing())?;
        let now = self.now()?;
        request_identity(owner, id, request.expires_at, now)?;
        view_identity(&request.activation_id, &request.instance_id)?;
        if request.protocol_version != VIEW_PROTOCOL {
            return Err(missing());
        }
        self.requests.retain(|_, fence| fence.expires_at > now);
        if self.requests.get(id).is_some_and(|f| {
            f.owner != owner
                || f.expires_at != request.expires_at
                || f.activation_id != request.activation_id
                || f.instance_id != request.instance_id
        }) || self.sessions.values().any(|s| {
            s.request_id == id
                && (s.owner != owner
                    || s.admission_deadline != request.expires_at
                    || s.activation_id != request.activation_id
                    || s.instance_id != request.instance_id)
        }) {
            return Err(missing());
        }
        if !self.requests.contains_key(id)
            && self.requests.values().any(|fence| {
                fence.owner == owner
                    && fence.activation_id == request.activation_id
                    && fence.instance_id == request.instance_id
            })
        {
            return Err(missing());
        }
        if request.expires_at > now && !self.requests.contains_key(id) {
            if self.requests.len() >= MAX_REQUEST_FENCES {
                return Err(io::ErrorKind::WouldBlock.into());
            }
            self.requests.insert(
                id.to_owned(),
                RequestFence {
                    owner: owner.to_owned(),
                    expires_at: request.expires_at,
                    activation_id: request.activation_id.clone(),
                    instance_id: request.instance_id.clone(),
                },
            );
        }
        self.sessions.retain(|_, session| session.request_id != id);
        self.retire_unused_views();
        super::json_bytes(
            &json!({"protocolVersion":VIEW_PROTOCOL,"requestId":id,"activationId":request.activation_id,"instanceId":request.instance_id,"expiresAt":request.expires_at,"closed":true}),
        )
    }

    pub(super) fn close(&mut self, owner: &str, token: &str) -> io::Result<()> {
        if !self.sessions.get(token).is_some_and(|s| s.owner == owner) {
            return Err(missing());
        }
        self.sessions.remove(token);
        self.retire_unused_views();
        Ok(())
    }

    pub(super) fn asset(
        &mut self,
        caps: &Path,
        owner: &str,
        token: &str,
        relative: &str,
    ) -> io::Result<(&'static str, Vec<u8>)> {
        self.sessions
            .retain(|_, session| session.expires > Instant::now() && session.owner == owner);
        self.retire_unused_views();
        let session = self.sessions.get(token).ok_or_else(missing)?;
        let key = ViewKey::of(session);
        if let Err(error) = if self.views.contains_key(&key) {
            still_selected(caps, session)
        } else {
            verify_view(caps, session).map(|view| {
                self.views.insert(key.clone(), view);
            })
        } {
            self.views.remove(&key);
            return Err(error);
        }
        let view = self.views.get(&key).ok_or_else(missing)?;
        let (content_type, bytes) = view.files.get(relative).ok_or_else(missing)?;
        if fs::read(view.directory.join(relative)).ok().as_ref() != Some(bytes) {
            self.views.remove(&key);
            return Err(missing());
        }
        Ok((content_type, bytes.clone()))
    }

    pub(super) fn with_resource<T>(
        &mut self,
        caps: &Path,
        owner: &str,
        token: &str,
        action: impl FnOnce(&ViewResourceIdentity) -> io::Result<T>,
    ) -> io::Result<T> {
        self.sessions.retain(|_, session| {
            session.expires > Instant::now() + Duration::from_secs(60) && session.owner == owner
        });
        let session = self.sessions.get(token).ok_or_else(missing)?;
        let view = verify_view(caps, session).map_err(|_| missing())?;
        action(&ViewResourceIdentity {
            package_id: session.package_id.clone(),
            generation: session.package_generation,
            revision: session.publication_revision.clone(),
            activation_id: session.activation_id.clone(),
            instance_id: session.instance_id.clone(),
            can_read: view.can_read,
            can_write: view.can_write,
            surface: session.surface.clone(),
        })
    }

    pub(super) fn agent(
        &mut self,
        caps: &Path,
        owner: &str,
        token: &str,
        agent: &str,
    ) -> io::Result<ViewAgent> {
        self.sessions
            .retain(|_, session| session.expires > Instant::now() && session.owner == owner);
        let session = self.sessions.get(token).ok_or_else(missing)?;
        let lease = selection(caps)?.ok_or_else(missing)?;
        installed(caps, &session.slug, &session.package_id)?;
        let publication = lease
            .publications()
            .find(|p| {
                p.package_id == session.package_id
                    && p.generation == session.package_generation
                    && p.cursor().revision == session.publication_revision
            })
            .ok_or_else(missing)?;
        let manifest = manifest(publication)?;
        let declared = manifest
            .agents
            .iter()
            .find(|declared| declared.id == agent)
            .ok_or_else(missing)?;
        let (Some(AgentHarness::Package), Some(resources), Some(entry)) =
            (&declared.harness, &declared.resources, &declared.entry)
        else {
            return Err(missing());
        };
        if !matches!(declared.runtime, AgentRuntimePlace::Edge) {
            return Err(missing());
        }
        let surface = publication
            .surfaces
            .iter()
            .find(|s| s.kind == SurfaceKind::Agent && s.id == agent)
            .ok_or_else(missing)?;
        let installed_entry = surface
            .entry
            .clone()
            .filter(|path| *path == surface.directory.join(entry) && path.is_file())
            .ok_or_else(missing)?;
        let declared_entry = entry
            .to_str()
            .filter(|entry| !entry.is_empty())
            .ok_or_else(missing)?;
        if !super::digest_ok(&surface.sha256) {
            return Err(missing());
        }
        Ok(ViewAgent {
            entry: installed_entry,
            declared_entry: declared_entry.to_owned(),
            release_identity: surface.sha256.clone(),
            cpu_millis: resources.cpu_millis,
            memory_bytes: resources.memory_bytes,
        })
    }
}

fn mime(path: &Path) -> io::Result<&'static str> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => Ok("text/html; charset=utf-8"),
        Some("js" | "mjs") => Ok("text/javascript; charset=utf-8"),
        Some("css") => Ok("text/css; charset=utf-8"),
        Some("json") => Ok("application/json"),
        Some("svg") => Ok("image/svg+xml"),
        Some("png") => Ok("image/png"),
        Some("jpg" | "jpeg") => Ok("image/jpeg"),
        Some("webp") => Ok("image/webp"),
        Some("woff") => Ok("font/woff"),
        Some("woff2") => Ok("font/woff2"),
        Some("wasm") => Ok("application/wasm"),
        _ => Err(missing()),
    }
}

#[cfg(test)]
#[path = "capabilities_workbench_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "capabilities_composer_mode_tests.rs"]
mod composer_mode_tests;

#[cfg(test)]
#[path = "capabilities_digital_twin_tests.rs"]
mod digital_twin_tests;

#[cfg(test)]
#[path = "capabilities_aix_host_tests.rs"]
mod host_tests;

#[cfg(test)]
#[path = "capabilities_view_agent_tests.rs"]
mod view_agent_tests;

#[cfg(test)]
#[path = "capabilities_view_e2e_tests.rs"]
mod view_e2e_tests;
