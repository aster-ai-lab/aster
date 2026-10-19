//! Studio projection of the installed `a3s/review` Use package.
//!
//! The host reads the capability snapshot. It does not scan a plugin
//! directory. A surface whose file digest does not match stays unpublished.

use std::cell::Cell;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use a3s_use_extension::{ExtensionManifest, SkillHostMode};
use a3s_use_reconcile::{
    GenerationLease, PackageFile, PackageSpec, Payload, ReconcileStore, SurfaceKind, SurfaceSpec,
    files_sha256,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tauri::Manager;

const PACKAGE_ID: &str = "use/a3s/review";
const SELECTION_ID: &str = "studio-installed-plugins";
const MANIFEST_NAME: &str = "a3s-use-extension.acl";
const MAX_AGENT_PROMPT: u64 = 32 * 1024;
const SKILL_ID: &str = "review";
const PICKER_ID: &str = "picker";
const REFRESH_AFTER: Duration = Duration::from_secs(2);

#[derive(Clone)]
struct ServedAsset {
    relative: String,
    path: PathBuf,
    sha256: String,
    media_type: String,
}

const MAX_SERVED_ASSETS: usize = 512;
const MAX_SERVED_ASSET_BYTES: u64 = 8 * 1024 * 1024;

struct Publication {
    skill: bool,
    ui: bool,
    title: String,
    picker: Option<PathBuf>,
    picker_sha256: String,
    assets: Vec<ServedAsset>,
    refreshed: Option<Instant>,
    lease: Option<Arc<GenerationLease>>,
    /// Body of the published `review` skill when its host is `docx-review`.
    skill_text: String,
}

static PUBLICATION: Mutex<Publication> = Mutex::new(Publication {
    skill: false,
    ui: false,
    title: String::new(),
    picker: None,
    picker_sha256: String::new(),
    assets: Vec::new(),
    refreshed: None,
    lease: None,
    skill_text: String::new(),
});

static APP: Mutex<Option<tauri::AppHandle>> = Mutex::new(None);

/// Set after the member catalog publishes `use/a3s/review`. While it is set,
/// snapshot reload leaves that generation in place.
static CATALOG_OWNS: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
#[path = "review_plugin_lease_tests.rs"]
mod lease_tests;

thread_local! {
    static SKILL_OVERRIDE: Cell<Option<bool>> = const { Cell::new(None) };
}

pub struct ReconcileReport {
    pub skill: bool,
    pub ui: bool,
    pub title: String,
    pub steps: Vec<&'static str>,
}

pub fn remember_app(app: tauri::AppHandle) {
    let mut slot = match APP.lock() {
        Ok(slot) => slot,
        Err(poisoned) => poisoned.into_inner(),
    };
    *slot = Some(app);
}

/// Child-page facade. The shell bearer is not a parameter and is not written here.
pub fn review_facade_script() -> String {
    r#"(function(){function b64(bytes){var out='';var size=0x8000;for(var i=0;i<bytes.length;i+=size){out+=String.fromCharCode.apply(null,bytes.subarray(i,i+size));}return btoa(out);}function tauri(){return globalThis.__TAURI_INTERNALS__||null;}function windowCommand(name){var internals=tauri();var invoke=internals&&internals.invoke;var label=internals&&internals.metadata&&internals.metadata.currentWindow&&internals.metadata.currentWindow.label;if(typeof invoke!=='function'||typeof label!=='string'||label.indexOf('applet-')!==0){return Promise.resolve(false);}return invoke('plugin:window|'+name,{label:label}).then(function(){return true;},function(){return false;});}function deliver(file){var name=file&&typeof file.name==='string'?file.name:'';var raw=file&&file.bytes;var bytes=raw instanceof Uint8Array?raw:new Uint8Array(0);var internals=tauri();var invoke=internals&&internals.invoke;if(typeof invoke!=='function'){return Promise.reject(new Error('unavailable'));}return invoke('deliver_workbench_review',{fileName:name,bytesBase64:b64(bytes)}).then(function(){return windowCommand('close');});}function deliverText(source){var internals=tauri();var invoke=internals&&internals.invoke;if(typeof invoke!=='function'){return Promise.reject(new Error('unavailable'));}return invoke('deliver_workbench_review',{text:source&&source.text,task:source&&source.task}).then(function(){return windowCommand('close');});}var review=Object.freeze({deliver:deliver,deliverText:deliverText});var windowControls=Object.freeze({minimize:function(){return windowCommand('minimize');},toggleMaximize:function(){return windowCommand('toggle_maximize');},close:function(){return windowCommand('close');},startDragging:function(){return windowCommand('start_dragging');}});var a3s=Object.freeze({review:review,window:windowControls});try{Object.defineProperty(globalThis,'a3s',{value:a3s,configurable:false,enumerable:true,writable:false});}catch(e){}document.addEventListener('DOMContentLoaded',function(){var zoomed=false;function paintZoom(){var button=document.querySelector('[data-a3s-window="toggle-maximize"]');if(!button)return;button.setAttribute('data-zoomed',zoomed?'true':'false');button.setAttribute('aria-label',zoomed?'还原':'最大化');var square=button.querySelector('[data-icon="maximize"]');var restore=button.querySelector('[data-icon="restore"]');if(square)square.hidden=zoomed;if(restore)restore.hidden=!zoomed;}document.querySelectorAll('[data-a3s-window]').forEach(function(button){button.addEventListener('mousedown',function(event){event.stopPropagation();});button.addEventListener('click',function(){var action=button.getAttribute('data-a3s-window');if(action==='toggle-maximize'){zoomed=!zoomed;paintZoom();void windowControls.toggleMaximize();return;}if(action==='minimize')void windowControls.minimize();if(action==='close')void windowControls.close();});});paintZoom();var drag=document.querySelector('[data-a3s-drag]');if(drag){drag.addEventListener('mousedown',function(event){if(event.button!==0||event.target.closest('[data-a3s-window]'))return;void windowControls.startDragging();});drag.addEventListener('dblclick',function(event){if(event.button!==0||event.target.closest('[data-a3s-window]'))return;event.preventDefault();zoomed=!zoomed;paintZoom();void windowControls.toggleMaximize();});}});})();"#
        .to_owned()
}

/// Persona plus skill text for an installed agent that `requires_skill`s exactly
/// one skill whose host is `docx-review`. `agent_id` is that agent surface id
/// or the installed package slug. A slug matches only when the package declares
/// exactly one such agent. A missing skill file returns nothing.
pub fn docx_review_instructions(tenant: Option<&Path>, agent_id: &str) -> Option<String> {
    if !agent_id_ok(agent_id) {
        return None;
    }
    let tenant = tenant?;
    let store = ReconcileStore::open(tenant.join("package-generations")).ok()?;
    let snapshot = store.selection_sync(SELECTION_ID).ok()??;
    if !snapshot.is_published() {
        return None;
    }
    let lease = store.acquire_selection_sync(&snapshot.cursor()).ok()?;
    let slug_package =
        crate::capabilities::installed_package_id(&tenant.join("capabilities"), agent_id);
    let mut from_slug = Vec::new();
    for publication in lease.publications() {
        let slug_hit = slug_package.as_deref() == Some(publication.package_id.as_str());
        let Some(manifest) = publication
            .surfaces
            .iter()
            .find_map(|surface| manifest_in(&surface.directory))
        else {
            continue;
        };
        for agent in &manifest.agents {
            let id_hit = agent.id == agent_id;
            if !id_hit && !slug_hit {
                continue;
            }
            let Some(text) = docx_review_turn(publication, &manifest, agent) else {
                continue;
            };
            if id_hit {
                return Some(text);
            }
            from_slug.push(text);
        }
    }
    if from_slug.len() == 1 {
        from_slug.pop()
    } else {
        None
    }
}

/// Published `/review` skill body. Empty until that skill declares `docx-review`
/// and its `SKILL.md` can be read.
pub fn published_review_skill_text() -> Option<String> {
    let text = publication().skill_text.clone();
    let text = text.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_owned())
    }
}

fn agent_id_ok(id: &str) -> bool {
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

fn docx_review_turn(
    publication: &a3s_use_reconcile::Publication,
    manifest: &ExtensionManifest,
    agent: &a3s_use_extension::PluginAgentSurface,
) -> Option<String> {
    let required: Vec<_> = agent
        .requires_skills
        .iter()
        .filter_map(|id| {
            manifest
                .skills
                .iter()
                .find(|skill| skill.id == *id && skill.host == Some(SkillHostMode::DocxReview))
        })
        .collect();
    if required.len() != 1 {
        return None;
    }
    let skill = required[0];
    let agent_dir = publication
        .surfaces
        .iter()
        .find(|surface| surface.kind == SurfaceKind::Agent && surface.id == agent.id)?;
    let skill_dir = publication
        .surfaces
        .iter()
        .find(|surface| surface.kind == SurfaceKind::Skill && surface.id == skill.id)?;
    let persona = bounded_text(&agent_dir.directory, agent.instructions.as_ref()?)?;
    let body = bounded_text(&skill_dir.directory, &skill.path)
        .or_else(|| bounded_text(&skill_dir.directory, Path::new("SKILL.md")))?;
    Some(format!("{persona}\n\n{body}"))
}

fn manifest_in(dir: &Path) -> Option<ExtensionManifest> {
    let bytes = fs::read(dir.join(MANIFEST_NAME)).ok()?;
    if bytes.len() as u64 > 1024 * 1024 {
        return None;
    }
    let text = String::from_utf8(bytes).ok()?;
    ExtensionManifest::parse_acl(&text).ok()
}

fn skill_text_from_dir(dir: &Path) -> Option<String> {
    let manifest = manifest_in(dir)?;
    let skill = manifest
        .skills
        .iter()
        .find(|skill| skill.id == SKILL_ID && skill.host == Some(SkillHostMode::DocxReview))?;
    bounded_text(dir, &skill.path).or_else(|| bounded_text(dir, Path::new("SKILL.md")))
}

fn bounded_text(dir: &Path, relative: &Path) -> Option<String> {
    if relative.as_os_str().is_empty() || relative.is_absolute() {
        return None;
    }
    if relative
        .components()
        .any(|component| component == Component::ParentDir)
    {
        return None;
    }
    let path = dir.join(relative);
    if !path.starts_with(dir) {
        return None;
    }
    let meta = fs::symlink_metadata(&path).ok()?;
    if !meta.file_type().is_file() || meta.len() == 0 || meta.len() > MAX_AGENT_PROMPT {
        return None;
    }
    let text = fs::read_to_string(&path).ok()?;
    let text = text.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_owned())
    }
}

pub fn skill_published() -> bool {
    if let Some(value) = SKILL_OVERRIDE.with(|cell| cell.get()) {
        return value;
    }
    publication().skill
}

#[cfg(test)]
pub fn set_skill_published_for_tests(value: Option<bool>) {
    SKILL_OVERRIDE.with(|cell| cell.set(value));
}

pub fn refresh() {
    reload(false);
}

fn reload(force: bool) {
    if SKILL_OVERRIDE.with(|cell| cell.get()).is_some() {
        return;
    }
    if CATALOG_OWNS.load(Ordering::Acquire) {
        return;
    }
    if !force {
        let slot = publication();
        if slot
            .refreshed
            .is_some_and(|at| at.elapsed() < REFRESH_AFTER)
        {
            return;
        }
    }
    let Some(text) = load_snapshot_text() else {
        let mut slot = publication_mut();
        if slot.refreshed.is_none() {
            slot.skill = false;
            slot.ui = false;
            slot.title.clear();
            slot.picker = None;
            slot.picker_sha256.clear();
            slot.assets.clear();
            slot.skill_text.clear();
            slot.refreshed = Some(Instant::now());
        }
        return;
    };
    let Some(agents) = crate::local_skills::agents_root() else {
        let _ = reconcile(&text, Path::new(""));
        return;
    };
    let _ = reconcile(&text, &agents);
}

pub fn status_json() -> String {
    reload(true);
    let slot = publication();
    json!({
        "skill": slot.skill,
        "ui": slot.ui,
        "title": slot.title,
    })
    .to_string()
}

/// Serve one file of the review SPA. An empty path or `index.html` is the
/// entry document. A missing document route returns that same document. A
/// missing script, style, or other asset extension returns 404.
pub fn picker_http_path(relative: &str) -> (u16, &'static str, &'static str, Vec<u8>) {
    reload(true);
    if !picker_relative_ok(relative) {
        return picker_not_found();
    }
    let slot = publication();
    let picker = slot.picker.clone();
    let picker_sha = slot.picker_sha256.clone();
    let assets = slot.assets.clone();
    let _lease = slot.lease.clone();
    let ui = slot.ui;
    drop(slot);
    if !ui {
        return picker_not_found();
    }
    let Some(picker) = picker else {
        return picker_not_found();
    };
    if relative.is_empty() || relative == "index.html" {
        return serve_checked(&picker, &picker_sha, "text/html");
    }
    if let Some(asset) = assets.iter().find(|asset| asset.relative == relative) {
        return serve_checked(&asset.path, &asset.sha256, &asset.media_type);
    }
    if asset_extension(relative).is_some() {
        return picker_not_found();
    }
    serve_checked(&picker, &picker_sha, "text/html")
}

fn serve_checked(
    path: &Path,
    expected: &str,
    media: &str,
) -> (u16, &'static str, &'static str, Vec<u8>) {
    let Some(content_type) = media_content_type(media) else {
        return picker_not_found();
    };
    match read_regular_file(path) {
        Some((bytes, sha)) if sha == expected && bytes.len() as u64 <= MAX_SERVED_ASSET_BYTES => {
            (200, "OK", content_type, bytes)
        }
        _ => picker_not_found(),
    }
}

fn picker_not_found() -> (u16, &'static str, &'static str, Vec<u8>) {
    (
        404,
        "Not Found",
        "text/plain; charset=utf-8",
        b"not_found".to_vec(),
    )
}

fn picker_relative_ok(relative: &str) -> bool {
    if relative.is_empty() {
        return true;
    }
    if relative.starts_with('/') || relative.contains('\\') || relative.contains('\0') {
        return false;
    }
    relative
        .split('/')
        .all(|part| !part.is_empty() && part != "." && part != "..")
}

fn asset_extension(relative: &str) -> Option<&str> {
    let name = relative.rsplit('/').next().unwrap_or(relative);
    let (stem, extension) = name.rsplit_once('.')?;
    if extension.is_empty() {
        return None;
    }
    if stem.is_empty() {
        return Some(extension);
    }
    Some(extension)
}

fn media_content_type(media: &str) -> Option<&'static str> {
    match media {
        "text/html" => Some("text/html; charset=utf-8"),
        "text/css" => Some("text/css; charset=utf-8"),
        "text/javascript" => Some("text/javascript; charset=utf-8"),
        "application/json" => Some("application/json; charset=utf-8"),
        "image/svg+xml" => Some("image/svg+xml; charset=utf-8"),
        "image/png" => Some("image/png"),
        "image/webp" => Some("image/webp"),
        "image/x-icon" => Some("image/x-icon"),
        "font/woff" => Some("font/woff"),
        "font/woff2" => Some("font/woff2"),
        "application/wasm" => Some("application/wasm"),
        _ => None,
    }
}

fn extension_matches_media(path: &Path, media: &str) -> bool {
    let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
        return false;
    };
    match extension.to_ascii_lowercase().as_str() {
        "html" => media == "text/html",
        "css" => media == "text/css",
        "js" | "mjs" => media == "text/javascript",
        "json" | "map" => media == "application/json",
        "svg" => media == "image/svg+xml",
        "png" => media == "image/png",
        "webp" => media == "image/webp",
        "ico" => media == "image/x-icon",
        "woff" => media == "font/woff",
        "woff2" => media == "font/woff2",
        "wasm" => media == "application/wasm",
        _ => false,
    }
}

fn cached_picker_http() -> (u16, &'static str, &'static str, Vec<u8>) {
    let slot = publication();
    let _lease = slot.lease.clone();
    let Some(path) = slot.picker.clone() else {
        return (
            404,
            "Not Found",
            "text/plain; charset=utf-8",
            b"not_found".to_vec(),
        );
    };
    let expected = slot.picker_sha256.clone();
    drop(slot);
    match read_regular_file(&path) {
        Some((bytes, sha)) if sha == expected => (200, "OK", "text/html; charset=utf-8", bytes),
        _ => (
            404,
            "Not Found",
            "text/plain; charset=utf-8",
            b"not_found".to_vec(),
        ),
    }
}

pub fn verified_picker() -> Option<(PathBuf, String)> {
    reload(true);
    let slot = publication();
    let _lease = slot.lease.clone();
    let path = slot.picker.clone()?;
    let expected = slot.picker_sha256.clone();
    let title = slot.title.clone();
    drop(slot);
    let (_bytes, sha) = read_regular_file(&path)?;
    if sha != expected {
        return None;
    }
    Some((path, title))
}

/// Publish or remove the review surfaces. `agents` is `~/.agents/skills`.
pub fn reconcile(snapshot: &str, agents: &Path) -> ReconcileReport {
    let Some(store) = open_review_store(agents) else {
        return reconcile_from_projection(snapshot, agents);
    };
    match review_verdict(snapshot) {
        ReviewVerdict::Absent => unpublish(agents, Some(&store)),
        ReviewVerdict::Mismatch => match store.acquire_current_sync(PACKAGE_ID) {
            Ok(Some(lease)) => restore_publication(lease),
            _ => unpublish(agents, None),
        },
        ReviewVerdict::Ready(ready) => publish_ready(agents, &store, ready),
    }
}

fn reconcile_from_projection(snapshot: &str, agents: &Path) -> ReconcileReport {
    let projected = project(snapshot);
    let mut steps = Vec::new();
    if !projected.ui {
        close_review_window();
        steps.push("close-window");
        steps.push("drop-entry");
    }
    let skill = if projected.skill {
        let linked = link_skill(agents, &session_skill_dir(&projected.skill_dir));
        if linked {
            steps.push("link-skill");
        }
        linked
    } else {
        unlink_skill(agents);
        steps.push("unlink-skill");
        false
    };
    let title = if projected.ui {
        projected.title.clone()
    } else {
        String::new()
    };
    let picker = if projected.ui {
        Some(projected.picker.clone())
    } else {
        None
    };
    let sha = if projected.ui {
        Some(projected.picker_sha256.clone())
    } else {
        None
    };
    let assets = if projected.ui {
        projected.assets.clone()
    } else {
        Vec::new()
    };
    write_slot(
        skill,
        projected.ui,
        title.clone(),
        picker,
        sha,
        assets,
        None,
    );
    ReconcileReport {
        skill,
        ui: projected.ui,
        title,
        steps,
    }
}

enum ReviewVerdict {
    Absent,
    Mismatch,
    Ready(ReadyReview),
}

struct ReadyReview {
    skill_dir: PathBuf,
    skill: Vec<u8>,
    assets: Vec<ServedAsset>,
    title: String,
    html_sha: String,
    picker: PathBuf,
}

/// Fill the review window from the generation the member catalog just applied.
/// The skill link points at the verified generation used by Session capabilities.
pub(crate) fn adopt_catalog_review(agents: &Path, _skill_dir: &Path) -> io::Result<()> {
    let store = open_review_store(agents)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "review generation is missing"))?;
    let saved = store
        .acquire_current_sync(PACKAGE_ID)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "review generation is missing"))?;
    let publication = saved.publication();
    let agent = publication
        .surfaces
        .iter()
        .any(|surface| surface.kind == SurfaceKind::Agent);
    let skill_dir = publication
        .surfaces
        .iter()
        .find(|surface| surface.kind == SurfaceKind::Skill && surface.id == SKILL_ID)
        .map(|surface| surface.directory.as_path());
    if !agent && skill_dir.is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "review agent publication is missing",
        ));
    }
    if let Some(skill_dir) = skill_dir {
        if !link_skill(agents, skill_dir) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "skill link is owned by someone else",
            ));
        }
        // ACP discovers ~/.agents/skills. The tenant link alone still leaves
        // that directory on the unpacked plugin tree.
        if let Some(home) = crate::capabilities::user_agents_skills_dir() {
            if home != agents {
                let _ = link_skill(&home, skill_dir);
            }
        }
    }
    let _ = restore_publication(saved);
    CATALOG_OWNS.store(true, Ordering::Release);
    Ok(())
}

/// Close the review window, drop its skill link, and delete its generation.
pub(crate) fn release_catalog_review(agents: &Path) -> io::Result<()> {
    let store = open_review_store(agents);
    let withdrawn = store
        .as_ref()
        .map(|store| store.withdraw_sync(PACKAGE_ID))
        .transpose();
    let _ = unpublish(agents, None);
    CATALOG_OWNS.store(false, Ordering::Release);
    withdrawn
        .map(|_| ())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))
}

#[cfg(test)]
pub(crate) fn clear_catalog_owns_for_tests() {
    CATALOG_OWNS.store(false, Ordering::Release);
}

#[cfg(test)]
pub(crate) fn lock_review_tests() -> std::sync::MutexGuard<'static, ()> {
    tests::lock_gate()
}

fn open_review_store(agents: &Path) -> Option<ReconcileStore> {
    if agents.as_os_str().is_empty() {
        return None;
    }
    let parent = agents.parent()?;
    ReconcileStore::open(parent.join("package-generations")).ok()
}

fn review_verdict(snapshot: &str) -> ReviewVerdict {
    let Ok(value) = serde_json::from_str::<Value>(snapshot) else {
        return ReviewVerdict::Absent;
    };
    let Some(capabilities) = value
        .pointer("/data/registry/capabilities")
        .or_else(|| value.pointer("/registry/capabilities"))
        .or_else(|| value.get("capabilities"))
        .and_then(Value::as_array)
    else {
        return ReviewVerdict::Absent;
    };
    for capability in capabilities {
        if !is_review(capability) {
            continue;
        }
        let Some(skill_dir) = published_skill(capability) else {
            return ReviewVerdict::Mismatch;
        };
        let Some((title, picker, html_sha, assets)) = published_picker(capability) else {
            return ReviewVerdict::Mismatch;
        };
        let Some((skill, _)) = read_regular_file(&skill_dir.join("SKILL.md")) else {
            return ReviewVerdict::Mismatch;
        };
        return ReviewVerdict::Ready(ReadyReview {
            skill_dir,
            skill,
            assets,
            title,
            html_sha,
            picker,
        });
    }
    ReviewVerdict::Absent
}

fn review_spec(ready: &ReadyReview) -> Result<PackageSpec, a3s_use_reconcile::ReconcileError> {
    let skill_files = vec![PackageFile {
        path: "SKILL.md".to_string(),
        bytes: ready.skill.clone(),
    }];
    let mut ui_files = Vec::with_capacity(ready.assets.len());
    for asset in &ready.assets {
        let (bytes, sha) = read_regular_file(&asset.path)
            .ok_or(a3s_use_reconcile::ReconcileError::DigestMismatch)?;
        if sha != asset.sha256 {
            return Err(a3s_use_reconcile::ReconcileError::DigestMismatch);
        }
        ui_files.push(PackageFile {
            path: format!("ui/picker/{}", asset.relative),
            bytes,
        });
    }
    Ok(PackageSpec {
        package_id: PACKAGE_ID.to_string(),
        surfaces: vec![
            SurfaceSpec {
                kind: SurfaceKind::Skill,
                id: SKILL_ID.to_string(),
                sha256: files_sha256(&skill_files)?,
                entry: None,
                payload: Payload::Files(skill_files),
            },
            SurfaceSpec {
                kind: SurfaceKind::Ui,
                id: PICKER_ID.to_string(),
                sha256: files_sha256(&ui_files)?,
                entry: Some("ui/picker/index.html".to_string()),
                payload: Payload::Files(ui_files),
            },
        ],
    })
}

fn publish_ready(agents: &Path, store: &ReconcileStore, ready: ReadyReview) -> ReconcileReport {
    let spec = match review_spec(&ready) {
        Ok(spec) => spec,
        Err(_) => return unpublish(agents, None),
    };
    if store.apply_sync(&spec).is_err() {
        return match store.acquire_current_sync(PACKAGE_ID) {
            Ok(Some(lease)) => restore_publication(lease),
            _ => unpublish(agents, None),
        };
    }
    let mut steps = Vec::new();
    let linked = link_skill(agents, &session_skill_dir(&ready.skill_dir));
    if linked {
        steps.push("link-skill");
    }
    let skill_text = skill_text_from_dir(&ready.skill_dir).unwrap_or_default();
    write_slot(
        linked,
        true,
        ready.title.clone(),
        Some(ready.picker),
        Some(ready.html_sha),
        ready.assets,
        None,
    );
    publication_mut().skill_text = skill_text;
    ReconcileReport {
        skill: linked,
        ui: true,
        title: ready.title,
        steps,
    }
}

fn restore_publication(lease: GenerationLease) -> ReconcileReport {
    let saved = lease.publication();
    let skill_text = saved
        .surfaces
        .iter()
        .find(|surface| surface.kind == SurfaceKind::Skill && surface.id == SKILL_ID)
        .and_then(|surface| skill_text_from_dir(&surface.directory))
        .unwrap_or_default();
    let title = {
        let slot = publication();
        if slot.title.is_empty() {
            "审查".to_string()
        } else {
            slot.title.clone()
        }
    };
    let skill = saved.surfaces.iter().any(|surface| {
        surface.kind == SurfaceKind::Skill && surface.directory.join("SKILL.md").is_file()
    });
    let entry = saved
        .surfaces
        .iter()
        .find(|surface| surface.kind == SurfaceKind::Ui)
        .and_then(|surface| surface.entry.clone());
    let loaded = entry.as_ref().and_then(|path| assets_beside(path));
    let (ui, picker, sha, assets) = match (entry, loaded) {
        (Some(path), Some(assets)) => {
            let sha = assets
                .iter()
                .find(|asset| asset.path == path)
                .map(|asset| asset.sha256.clone());
            match sha {
                Some(sha) => (true, Some(path), Some(sha), assets),
                None => (false, None, None, Vec::new()),
            }
        }
        _ => (false, None, None, Vec::new()),
    };
    write_slot(
        skill,
        ui,
        title.clone(),
        picker,
        sha,
        assets,
        Some(Arc::new(lease)),
    );
    publication_mut().skill_text = skill_text;
    ReconcileReport {
        skill,
        ui,
        title,
        steps: Vec::new(),
    }
}

fn unpublish(agents: &Path, store: Option<&ReconcileStore>) -> ReconcileReport {
    close_review_window();
    unlink_skill(agents);
    if let Some(store) = store {
        let _ = store.withdraw_sync(PACKAGE_ID);
    }
    write_slot(false, false, String::new(), None, None, Vec::new(), None);
    ReconcileReport {
        skill: false,
        ui: false,
        title: String::new(),
        steps: vec!["close-window", "drop-entry", "unlink-skill"],
    }
}

fn write_slot(
    skill: bool,
    ui: bool,
    title: String,
    picker: Option<PathBuf>,
    sha: Option<String>,
    assets: Vec<ServedAsset>,
    lease: Option<Arc<GenerationLease>>,
) {
    let mut slot = publication_mut();
    slot.skill = skill;
    slot.ui = ui;
    slot.title = if ui { title } else { String::new() };
    slot.picker = if ui { picker } else { None };
    slot.picker_sha256 = if ui {
        sha.unwrap_or_default()
    } else {
        String::new()
    };
    slot.assets = if ui { assets } else { Vec::new() };
    slot.skill_text.clear();
    slot.refreshed = Some(Instant::now());
    let prior = std::mem::replace(&mut slot.lease, if skill || ui { lease } else { None });
    drop(slot);
    drop(prior);
}

struct Projected {
    skill: bool,
    skill_dir: PathBuf,
    ui: bool,
    title: String,
    picker: PathBuf,
    picker_sha256: String,
    assets: Vec<ServedAsset>,
}

fn project(snapshot: &str) -> Projected {
    let empty = Projected {
        skill: false,
        skill_dir: PathBuf::new(),
        ui: false,
        title: String::new(),
        picker: PathBuf::new(),
        picker_sha256: String::new(),
        assets: Vec::new(),
    };
    let Ok(value) = serde_json::from_str::<Value>(snapshot) else {
        return empty;
    };
    let Some(capabilities) = value
        .pointer("/data/registry/capabilities")
        .or_else(|| value.pointer("/registry/capabilities"))
        .or_else(|| value.get("capabilities"))
        .and_then(Value::as_array)
    else {
        return empty;
    };
    for capability in capabilities {
        if !is_review(capability) {
            continue;
        }
        let skill = published_skill(capability);
        let ui = published_picker(capability);
        return Projected {
            skill: skill.is_some(),
            skill_dir: skill.unwrap_or_default(),
            ui: ui.is_some(),
            title: ui.as_ref().map(|item| item.0.clone()).unwrap_or_default(),
            picker: ui.as_ref().map(|item| item.1.clone()).unwrap_or_default(),
            picker_sha256: ui.as_ref().map(|item| item.2.clone()).unwrap_or_default(),
            assets: ui.map(|item| item.3).unwrap_or_default(),
        };
    }
    empty
}

fn is_review(capability: &Value) -> bool {
    if capability.get("enabled") != Some(&Value::Bool(true)) {
        return false;
    }
    if let Some(readiness) = capability.get("readiness").and_then(Value::as_str) {
        if readiness != "ready" {
            return false;
        }
    }
    capability.get("id").and_then(Value::as_str) == Some(PACKAGE_ID)
}

fn published_skill(capability: &Value) -> Option<PathBuf> {
    let skills = capability.get("skills")?.as_array()?;
    for skill in skills {
        if skill.get("id").and_then(Value::as_str) != Some(SKILL_ID) {
            continue;
        }
        let path = PathBuf::from(skill.get("path")?.as_str()?);
        if path.file_name().and_then(|name| name.to_str()) != Some("SKILL.md") {
            return None;
        }
        let expected = skill.get("sha256")?.as_str()?.to_ascii_lowercase();
        let (bytes, sha) = read_regular_file(&path)?;
        if sha != expected || bytes.is_empty() {
            return None;
        }
        return path.parent().map(Path::to_path_buf);
    }
    None
}

fn published_picker(capability: &Value) -> Option<(String, PathBuf, String, Vec<ServedAsset>)> {
    let entries = capability
        .get("activityBar")
        .or_else(|| capability.get("activity_bar"))?
        .as_array()?;
    for entry in entries {
        if entry.get("id").and_then(Value::as_str) != Some(PICKER_ID) {
            continue;
        }
        let asset = entry.get("entry")?;
        let path = PathBuf::from(asset.get("path")?.as_str()?);
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_none_or(|ext| !ext.eq_ignore_ascii_case("html"))
        {
            return None;
        }
        let expected = asset.get("sha256")?.as_str()?.to_ascii_lowercase();
        let (_bytes, sha) = read_regular_file(&path)?;
        if sha != expected {
            return None;
        }
        let title = entry
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or(PICKER_ID)
            .to_owned();
        let assets = load_served_assets(entry, &path, &expected)?;
        return Some((title, path, expected, assets));
    }
    None
}

fn load_served_assets(
    item: &Value,
    entry_path: &Path,
    entry_sha: &str,
) -> Option<Vec<ServedAsset>> {
    let parent = entry_path.parent()?;
    let Some(listed) = item.get("assets") else {
        return Some(vec![ServedAsset {
            relative: "index.html".to_string(),
            path: entry_path.to_path_buf(),
            sha256: entry_sha.to_string(),
            media_type: "text/html".to_string(),
        }]);
    };
    let listed = listed.as_array()?;
    if listed.is_empty() || listed.len() > MAX_SERVED_ASSETS {
        return None;
    }
    let mut assets: Vec<ServedAsset> = Vec::with_capacity(listed.len());
    let mut saw_entry = false;
    for asset in listed {
        let path = PathBuf::from(asset.get("path")?.as_str()?);
        let media = asset
            .get("mediaType")
            .or_else(|| asset.get("media_type"))?
            .as_str()?;
        if media_content_type(media).is_none() || !extension_matches_media(&path, media) {
            return None;
        }
        let relative = relative_under(parent, &path)?;
        if assets.iter().any(|existing| existing.relative == relative) {
            return None;
        }
        let expected = asset.get("sha256")?.as_str()?.to_ascii_lowercase();
        if expected.len() != 64 || !expected.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        if !served_file_len_ok(&path) {
            return None;
        }
        let (_bytes, sha) = read_regular_file(&path)?;
        if sha != expected {
            return None;
        }
        if path == entry_path {
            if expected != entry_sha {
                return None;
            }
            saw_entry = true;
        }
        assets.push(ServedAsset {
            relative,
            path,
            sha256: expected,
            media_type: media.to_string(),
        });
    }
    if !saw_entry {
        return None;
    }
    Some(assets)
}

fn assets_beside(entry: &Path) -> Option<Vec<ServedAsset>> {
    let parent = entry.parent()?;
    let metadata = fs::symlink_metadata(parent).ok()?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return None;
    }
    let mut assets = Vec::new();
    let mut pending = vec![parent.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let read = fs::read_dir(&directory).ok()?;
        for item in read {
            let item = item.ok()?;
            let path = item.path();
            let metadata = fs::symlink_metadata(&path).ok()?;
            if metadata.file_type().is_symlink() {
                return None;
            }
            if metadata.is_dir() {
                pending.push(path);
                continue;
            }
            if !metadata.is_file() {
                return None;
            }
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("");
            let Some(media) = extension_media(extension) else {
                continue;
            };
            if metadata.len() == 0 || metadata.len() > MAX_SERVED_ASSET_BYTES {
                return None;
            }
            let relative = relative_under(parent, &path)?;
            let (_bytes, sha) = read_regular_file(&path)?;
            assets.push(ServedAsset {
                relative,
                path,
                sha256: sha,
                media_type: media.to_string(),
            });
            if assets.len() > MAX_SERVED_ASSETS {
                return None;
            }
        }
    }
    if !assets.iter().any(|asset| asset.path == entry) {
        return None;
    }
    Some(assets)
}

fn extension_media(extension: &str) -> Option<&'static str> {
    match extension.to_ascii_lowercase().as_str() {
        "html" => Some("text/html"),
        "css" => Some("text/css"),
        "js" | "mjs" => Some("text/javascript"),
        "json" | "map" => Some("application/json"),
        "svg" => Some("image/svg+xml"),
        "png" => Some("image/png"),
        "webp" => Some("image/webp"),
        "ico" => Some("image/x-icon"),
        "woff" => Some("font/woff"),
        "woff2" => Some("font/woff2"),
        "wasm" => Some("application/wasm"),
        _ => None,
    }
}

fn relative_under(parent: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(parent).ok()?;
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_str()?),
            _ => return None,
        }
    }
    if parts.is_empty() {
        return None;
    }
    let relative = parts.join("/");
    if picker_relative_ok(&relative) {
        Some(relative)
    } else {
        None
    }
}

fn served_file_len_ok(path: &Path) -> bool {
    fs::symlink_metadata(path).ok().is_some_and(|metadata| {
        !metadata.file_type().is_symlink()
            && metadata.is_file()
            && metadata.len() > 0
            && metadata.len() <= MAX_SERVED_ASSET_BYTES
    })
}

fn read_regular_file(path: &Path) -> Option<(Vec<u8>, String)> {
    let meta = fs::symlink_metadata(path).ok()?;
    if meta.file_type().is_symlink() || !meta.is_file() {
        return None;
    }
    let bytes = fs::read(path).ok()?;
    let sha = format!("{:x}", Sha256::digest(&bytes));
    Some((bytes, sha))
}

/// Snapshot paths name the unpacked plugin tree. Session admission names the
/// verified generation. Discovery and the projection must be that same file.
fn session_skill_dir(snapshot_dir: &Path) -> PathBuf {
    leased_skill_dir(snapshot_dir).unwrap_or_else(|| snapshot_dir.to_path_buf())
}

fn leased_skill_dir(snapshot_dir: &Path) -> Option<PathBuf> {
    let mut cursor = snapshot_dir;
    let plugins = loop {
        cursor = cursor.parent()?;
        if cursor.file_name()? == "plugins" {
            break cursor;
        }
    };
    let capabilities = plugins.parent()?;
    if capabilities.file_name()? != "capabilities" {
        return None;
    }
    let root = capabilities.parent()?.join("package-generations");
    if !root.is_dir() {
        return None;
    }
    let publication = ReconcileStore::open(root)
        .ok()?
        .current_sync(PACKAGE_ID)
        .ok()??;
    let directory = publication
        .surfaces
        .iter()
        .find(|surface| surface.kind == SurfaceKind::Skill && surface.id == SKILL_ID)?
        .directory
        .clone();
    directory.join("SKILL.md").is_file().then_some(directory)
}

fn link_skill(agents: &Path, source: &Path) -> bool {
    if source.as_os_str().is_empty() || agents.as_os_str().is_empty() {
        return false;
    }
    crate::skill_links::claim_skill_link(agents, crate::skill_links::OWNER_REVIEW, SKILL_ID, source)
        .is_ok()
}

fn unlink_skill(agents: &Path) {
    let _ =
        crate::skill_links::release_skill_link(agents, crate::skill_links::OWNER_REVIEW, SKILL_ID);
}

fn close_review_window() {
    let slot = match APP.lock() {
        Ok(slot) => slot,
        Err(poisoned) => poisoned.into_inner(),
    };
    let Some(app) = slot.as_ref() else {
        return;
    };
    if let Some(window) = app.get_webview_window("applet-review") {
        let _ = window.close();
    }
}

fn load_snapshot_text() -> Option<String> {
    if let Some(path) = std::env::var_os("A3S_STUDIO_CAPABILITY_SNAPSHOT") {
        return fs::read_to_string(path).ok();
    }
    if cfg!(test) {
        return None;
    }
    let plain = run_snapshot(&["capability", "snapshot", "--json"]);
    if snapshot_ok(plain.as_deref()) {
        return plain;
    }
    if plain
        .as_deref()
        .is_some_and(|text| text.contains("scope-kind") || text.contains("scope-id"))
    {
        return run_snapshot(&[
            "capability",
            "snapshot",
            "--scope-kind",
            "user",
            "--scope-id",
            "user/current",
            "--json",
        ]);
    }
    None
}

fn run_snapshot(args: &[&str]) -> Option<String> {
    let output = std::process::Command::new("a3s-use")
        .args(args)
        .output()
        .ok()?;
    String::from_utf8(output.stdout).ok()
}

fn snapshot_ok(text: Option<&str>) -> bool {
    let Some(text) = text else {
        return false;
    };
    serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|value| value.get("ok").and_then(Value::as_bool))
        .unwrap_or(false)
        && text.contains("capabilities")
}

fn publication() -> std::sync::MutexGuard<'static, Publication> {
    PUBLICATION
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn publication_mut() -> std::sync::MutexGuard<'static, Publication> {
    publication()
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_GATE: Mutex<()> = Mutex::new(());

    pub(super) fn lock_gate() -> std::sync::MutexGuard<'static, ()> {
        TEST_GATE
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn lock_tests() -> std::sync::MutexGuard<'static, ()> {
        super::lock_review_tests()
    }

    fn sha(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    fn fixture(root: &Path) -> (String, PathBuf) {
        let skill_dir = root.join("pkg/skills/review");
        let picker_dir = root.join("pkg/ui/picker");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::create_dir_all(&picker_dir).unwrap();
        let skill = b"---\nname: review\n---\nReview the document.\nfixture-docx-review-skill\n";
        fs::write(
            skill_dir.join(MANIFEST_NAME),
            r#"extension "a3s/review" {
  schema_version = 3
  version        = "1.0.0"
  route          = "review"
  requires_use   = ">=0.3.0, <0.4.0"
  actions        = ["read"]

  repository {
    url      = "https://github.com/A3S-Lab/Use-Registry"
    revision = "c1dcedcd84794ce4bb0010c8d17a7eff4c62d212"
  }

  skill "review" {
    path = "SKILL.md"
    host = "docx-review"
  }
}
"#,
        )
        .unwrap();
        let html = b"<!doctype html><html><head><link rel=\"stylesheet\" href=\"./app.css\"></head><body><header data-a3s-window-bar></header><main id=\"app\"></main><script src=\"./app.js\"></script></body></html>";
        let css = b"body{margin:0}\n";
        let js = b"/* review */\n";
        fs::write(skill_dir.join("SKILL.md"), skill).unwrap();
        let picker = picker_dir.join("index.html");
        let css_path = picker_dir.join("app.css");
        let js_path = picker_dir.join("app.js");
        fs::write(&picker, html).unwrap();
        fs::write(&css_path, css).unwrap();
        fs::write(&js_path, js).unwrap();
        let snapshot = json!({
            "data": {
                "registry": {
                    "capabilities": [{
                        "id": "use/a3s/review",
                        "route": "review",
                        "enabled": true,
                        "readiness": "ready",
                        "skills": [{
                            "id": "review",
                            "path": skill_dir.join("SKILL.md"),
                            "sha256": sha(skill)
                        }],
                        "activityBar": [{
                            "id": "picker",
                            "title": "审查",
                            "entry": {
                                "path": &picker,
                                "sha256": sha(html),
                                "mediaType": "text/html"
                            },
                            "assets": [
                                { "path": &picker, "sha256": sha(html), "mediaType": "text/html" },
                                { "path": &css_path, "sha256": sha(css), "mediaType": "text/css" },
                                { "path": &js_path, "sha256": sha(js), "mediaType": "text/javascript" }
                            ]
                        }]
                    }]
                }
            }
        });
        (snapshot.to_string(), picker)
    }

    #[test]
    fn unpublished_snapshot_has_no_entry_and_no_skill() {
        let _gate = lock_tests();
        let root = std::env::temp_dir().join(format!("a3s-review-closed-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let agents = root.join("agents");
        fs::create_dir_all(&agents).unwrap();
        let report = reconcile("{}", &agents);
        assert!(!report.skill);
        assert!(!report.ui);
        assert_eq!(report.steps, ["close-window", "drop-entry", "unlink-skill"]);
        assert!(!agents.join("review").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn install_links_the_skill_and_serves_the_picker_digest() {
        let _gate = lock_tests();
        let root = std::env::temp_dir().join(format!("a3s-review-install-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let (snapshot, picker) = fixture(&root);
        let agents = root.join("agents");
        let report = reconcile(&snapshot, &agents);
        assert!(report.skill);
        assert!(report.ui);
        assert_eq!(report.title, "审查");
        assert_eq!(report.steps, ["link-skill"]);
        let linked = fs::read_to_string(agents.join("review/SKILL.md")).unwrap();
        assert!(linked.contains("name: review"));
        assert!(agents.join(".a3s-review-managed").is_file());
        let body = cached_picker_http();
        assert_eq!(body.0, 200);
        assert_eq!(body.3, fs::read(&picker).unwrap());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn home_link_follows_the_verified_generation() {
        let _gate = lock_tests();
        let root =
            std::env::temp_dir().join(format!("a3s-review-generation-link-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let mirror = root.join("capabilities/plugins/a3s-review/skills/review");
        let picker_dir = root.join("picker");
        fs::create_dir_all(&mirror).unwrap();
        fs::create_dir_all(&picker_dir).unwrap();
        let skill = b"---\nname: review\ndescription: Check a document\n---\n\nReview.\n";
        let html = b"<p>picker</p>";
        fs::write(mirror.join("SKILL.md"), skill).unwrap();
        let picker = picker_dir.join("index.html");
        fs::write(&picker, html).unwrap();
        let skill_files = vec![PackageFile {
            path: "SKILL.md".into(),
            bytes: skill.to_vec(),
        }];
        let ui_files = vec![PackageFile {
            path: "ui/picker/index.html".into(),
            bytes: html.to_vec(),
        }];
        let spec = PackageSpec {
            package_id: PACKAGE_ID.into(),
            surfaces: vec![
                SurfaceSpec {
                    kind: SurfaceKind::Skill,
                    id: SKILL_ID.into(),
                    sha256: files_sha256(&skill_files).unwrap(),
                    entry: None,
                    payload: Payload::Files(skill_files),
                },
                SurfaceSpec {
                    kind: SurfaceKind::Ui,
                    id: PICKER_ID.into(),
                    sha256: files_sha256(&ui_files).unwrap(),
                    entry: Some("ui/picker/index.html".into()),
                    payload: Payload::Files(ui_files),
                },
            ],
        };
        let published = ReconcileStore::open(root.join("package-generations"))
            .unwrap()
            .apply_sync(&spec)
            .unwrap();
        let generation = published
            .surfaces
            .iter()
            .find(|surface| surface.kind == SurfaceKind::Skill)
            .unwrap()
            .directory
            .clone();
        let snapshot = json!({
            "data": { "registry": { "capabilities": [{
                "id": "use/a3s/review",
                "enabled": true,
                "readiness": "ready",
                "skills": [{
                    "id": "review",
                    "path": mirror.join("SKILL.md"),
                    "sha256": format!("{:x}", Sha256::digest(skill))
                }],
                "activityBar": [{
                    "id": "picker",
                    "title": "审查",
                    "entry": { "path": &picker, "sha256": format!("{:x}", Sha256::digest(html)), "mediaType": "text/html" },
                    "assets": [{ "path": &picker, "sha256": format!("{:x}", Sha256::digest(html)), "mediaType": "text/html" }]
                }]
            }]}}
        });
        let agents = root.join("agents");
        fs::create_dir_all(&agents).unwrap();
        assert!(reconcile(&snapshot.to_string(), &agents).skill);
        assert_eq!(
            fs::canonicalize(agents.join("review")).unwrap(),
            fs::canonicalize(&generation).unwrap()
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn digest_mismatch_stays_unpublished() {
        let _gate = lock_tests();
        let root = std::env::temp_dir().join(format!("a3s-review-digest-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let (mut snapshot, _) = fixture(&root);
        snapshot = snapshot.replace("审查", "审查");
        let value = serde_json::from_str::<Value>(&snapshot).unwrap();
        let mut broken = value.clone();
        broken["data"]["registry"]["capabilities"][0]["skills"][0]["sha256"] =
            Value::String("ab".repeat(32));
        broken["data"]["registry"]["capabilities"][0]["activityBar"][0]["entry"]["sha256"] =
            Value::String("cd".repeat(32));
        let agents = root.join("agents");
        let report = reconcile(&broken.to_string(), &agents);
        assert!(!report.skill);
        assert!(!report.ui);
        assert!(!agents.join("review").exists());
        assert_eq!(cached_picker_http().0, 404);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn uninstall_closes_then_drops_the_entry_then_unlinks() {
        let _gate = lock_tests();
        let root = std::env::temp_dir().join(format!("a3s-review-remove-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let (snapshot, _) = fixture(&root);
        let agents = root.join("agents");
        assert!(reconcile(&snapshot, &agents).skill);
        let report = reconcile("{}", &agents);
        assert_eq!(report.steps, ["close-window", "drop-entry", "unlink-skill"]);
        assert!(!report.ui);
        assert!(!report.skill);
        assert!(!agents.join("review").exists());
        assert!(!agents.join(".a3s-review-managed").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_user_review_directory_is_left_in_place() {
        let _gate = lock_tests();
        let root = std::env::temp_dir().join(format!("a3s-review-user-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let (snapshot, _) = fixture(&root);
        let agents = root.join("agents");
        fs::create_dir_all(agents.join("review")).unwrap();
        fs::write(agents.join("review/SKILL.md"), "# mine\n").unwrap();
        let report = reconcile(&snapshot, &agents);
        assert!(!report.skill);
        assert_eq!(
            fs::read_to_string(agents.join("review/SKILL.md")).unwrap(),
            "# mine\n"
        );
        assert!(
            !agents
                .join("review")
                .symlink_metadata()
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let _ = fs::remove_dir_all(&root);
    }

    struct ChainHome(PathBuf);

    impl Drop for ChainHome {
        fn drop(&mut self) {
            unsafe {
                std::env::remove_var("A3S_STUDIO_CAPABILITY_SNAPSHOT");
            }
            crate::local_skills::set_test_agents_root(None);
            set_skill_published_for_tests(None);
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn one_process_snapshot_chain_covers_absence_handoff_and_uninstall() {
        let _gate = lock_tests();
        let root = std::env::temp_dir().join(format!("a3s-review-chain-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let _home = ChainHome(root.clone());
        set_skill_published_for_tests(None);
        let home = root.join("home");
        let agents = home.join(".agents/skills");
        fs::create_dir_all(&agents).unwrap();
        crate::local_skills::set_test_agents_root(Some(agents.clone()));
        let snapshot_path = root.join("snapshot.json");
        fs::write(&snapshot_path, "{}").unwrap();
        unsafe {
            std::env::set_var("A3S_STUDIO_CAPABILITY_SNAPSHOT", &snapshot_path);
        }

        let closed = serde_json::from_str::<Value>(&status_json()).unwrap();
        assert_eq!(closed["skill"], Value::Bool(false));
        assert_eq!(closed["ui"], Value::Bool(false));
        assert!(!agents.join("review").exists());
        assert!(!agents.join(".a3s-review-managed").exists());

        let (snapshot, picker) = fixture(&root);
        fs::write(&snapshot_path, &snapshot).unwrap();
        let open = serde_json::from_str::<Value>(&status_json()).unwrap();
        assert_eq!(open["skill"], Value::Bool(true));
        assert_eq!(open["ui"], Value::Bool(true));
        assert_eq!(open["title"], "审查");
        assert!(agents.join(".a3s-review-managed").is_file());
        assert!(
            agents
                .join("review")
                .symlink_metadata()
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let body = cached_picker_http();
        assert_eq!(body.0, 200);
        assert_eq!(body.3, fs::read(&picker).unwrap());
        let script = review_facade_script();
        assert!(!script.to_ascii_lowercase().contains("bearer"));
        assert!(!script.contains("token"));

        let docx = root.join("contract.docx");
        write_docx(
            &docx,
            r#"<?xml version="1.0"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:r><w:t>甲方应在三日内付款。</w:t></w:r></w:p></w:body>
</w:document>"#,
        );
        let prompt = crate::agent_plane::review_model_prompt_for_tests(&format!(
            "/review 审查这个文件\n{}",
            docx.display()
        ));
        assert!(prompt.contains("甲方应在三日内付款。"));
        assert!(prompt.contains("fixture-docx-review-skill"));
        assert!(!prompt.contains("not a formal opinion from a licensed lawyer"));

        fs::write(&snapshot_path, "{}").unwrap();
        let report = reconcile("{}", &agents);
        assert_eq!(report.steps, ["close-window", "drop-entry", "unlink-skill"]);
        let closed_again = serde_json::from_str::<Value>(&status_json()).unwrap();
        assert_eq!(closed_again["ui"], Value::Bool(false));
        assert_eq!(closed_again["skill"], Value::Bool(false));
        assert!(!agents.join("review").exists());
        assert!(!agents.join(".a3s-review-managed").exists());

        fs::create_dir_all(agents.join("review")).unwrap();
        fs::write(agents.join("review/SKILL.md"), "# mine\n").unwrap();
        fs::write(&snapshot_path, &snapshot).unwrap();
        let kept = reconcile(&snapshot, &agents);
        assert!(!kept.skill);
        assert_eq!(
            fs::read_to_string(agents.join("review/SKILL.md")).unwrap(),
            "# mine\n"
        );
        assert!(
            !agents
                .join("review")
                .symlink_metadata()
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }

    fn write_docx(path: &Path, xml: &str) {
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut cursor);
            writer
                .start_file(
                    "word/document.xml",
                    zip::write::SimpleFileOptions::default(),
                )
                .unwrap();
            std::io::Write::write_all(&mut writer, xml.as_bytes()).unwrap();
            writer.finish().unwrap();
        }
        fs::write(path, cursor.into_inner()).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn published_skill_replaces_only_the_bundled_review_symlink() {
        let _gate = lock_tests();
        let root =
            std::env::temp_dir().join(format!("a3s-review-bundled-link-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let _home = ChainHome(root.clone());
        let agents = root.join(".agents/skills");
        fs::create_dir_all(&agents).unwrap();
        let bundled = fs::canonicalize(crate::skill_links::bundled_review_dir()).unwrap();
        let bundled_bytes = fs::read(bundled.join("SKILL.md")).unwrap();
        std::os::unix::fs::symlink(&bundled, agents.join("review")).unwrap();
        let elsewhere = root.join("user-review");
        fs::create_dir_all(&elsewhere).unwrap();
        fs::write(elsewhere.join("SKILL.md"), "# user link\n").unwrap();

        let (snapshot, _) = fixture(&root);
        let claimed = reconcile(&snapshot, &agents);
        assert!(claimed.skill);
        assert!(agents.join(".a3s-review-managed").is_file());
        let linked = fs::canonicalize(agents.join("review")).unwrap();
        assert!(linked.ends_with("pkg/skills/review"));
        assert_eq!(fs::read(bundled.join("SKILL.md")).unwrap(), bundled_bytes);
        assert_eq!(
            fs::read_to_string(elsewhere.join("SKILL.md")).unwrap(),
            "# user link\n"
        );

        fs::remove_file(agents.join("review")).unwrap();
        fs::remove_file(agents.join(".a3s-review-managed")).unwrap();
        std::os::unix::fs::symlink(&elsewhere, agents.join("review")).unwrap();
        let kept = reconcile(&snapshot, &agents);
        assert!(!kept.skill);
        assert_eq!(fs::read_link(agents.join("review")).unwrap(), elsewhere);
        assert!(!agents.join(".a3s-review-managed").exists());
    }

    #[test]
    #[ignore = "reads the operator a3s-use install"]
    fn live_use_snapshot_projects_review_without_an_override_file() {
        assert!(std::env::var_os("A3S_STUDIO_CAPABILITY_SNAPSHOT").is_none());
        let text = run_snapshot(&[
            "capability",
            "snapshot",
            "--scope-kind",
            "user",
            "--scope-id",
            "user/current",
            "--json",
        ])
        .expect("a3s-use snapshot");
        assert!(snapshot_ok(Some(&text)));
        let projected = project(&text);
        assert!(projected.skill);
        assert!(projected.ui);
        assert_eq!(projected.title, "审查");
        let skill = fs::read(projected.skill_dir.join("SKILL.md")).unwrap();
        assert!(!skill.is_empty());
        let picker = fs::read(&projected.picker).unwrap();
        assert!(
            projected
                .picker
                .extension()
                .is_some_and(|ext| ext == "html")
        );
        assert!(!picker.is_empty());
    }

    #[test]
    fn facade_script_exposes_deliver_without_a_bearer() {
        let script = review_facade_script();
        assert!(script.contains("deliver_workbench_review"));
        assert!(script.contains("globalThis,'a3s'"));
        assert!(!script.to_ascii_lowercase().contains("bearer"));
        assert!(!script.contains("token"));
        assert!(!script.contains("appletTicket"));
        assert!(script.contains("plugin:window|"));
        assert!(script.contains("toggle_maximize"));
        assert!(script.contains("start_dragging"));
        assert!(script.contains("data-a3s-window"));
        assert!(script.contains("windowCommand('close')"));
        assert!(!script.contains("data-a3s-composed"));
    }

    #[test]
    fn served_picker_returns_package_bytes_and_falls_back_for_client_routes() {
        let _gate = lock_tests();
        let root = std::env::temp_dir().join(format!("a3s-review-spa-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let (snapshot, picker) = fixture(&root);
        let agents = root.join("agents");
        let report = reconcile(&snapshot, &agents);
        assert!(report.ui);
        assert_eq!(report.steps, ["link-skill"]);
        let html = fs::read(&picker).unwrap();
        let css = fs::read(picker.parent().unwrap().join("app.css")).unwrap();
        let js = fs::read(picker.parent().unwrap().join("app.js")).unwrap();
        let entry = picker_http_path("");
        assert_eq!(entry.0, 200);
        assert_eq!(entry.2, "text/html; charset=utf-8");
        assert_eq!(entry.3, html);
        assert_eq!(picker_http_path("index.html").3, html);
        let style = picker_http_path("app.css");
        assert_eq!(style.0, 200);
        assert_eq!(style.2, "text/css; charset=utf-8");
        assert_eq!(style.3, css);
        let script = picker_http_path("app.js");
        assert_eq!(script.0, 200);
        assert_eq!(script.2, "text/javascript; charset=utf-8");
        assert_eq!(script.3, js);
        assert_eq!(picker_http_path("queued").3, html);
        assert_eq!(picker_http_path("inbox").3, html);
        assert_eq!(picker_http_path("missing.js").0, 404);
        assert_eq!(picker_http_path("../secret").0, 404);
        assert_eq!(picker_http_path("notes.txt").0, 404);
        assert_eq!(fs::read(&picker).unwrap(), html);
        assert!(
            agents
                .join("review")
                .symlink_metadata()
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(
            fs::read_link(agents.join("review"))
                .unwrap()
                .ends_with("pkg/skills/review")
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn asset_digest_mismatch_stays_unpublished() {
        let _gate = lock_tests();
        let root = std::env::temp_dir().join(format!("a3s-review-asset-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let (snapshot, _) = fixture(&root);
        let mut broken = serde_json::from_str::<Value>(&snapshot).unwrap();
        let assets = broken["data"]["registry"]["capabilities"][0]["activityBar"][0]["assets"]
            .as_array_mut()
            .unwrap();
        for asset in assets {
            if asset["path"].as_str().unwrap().ends_with("app.js") {
                asset["sha256"] = Value::String("ab".repeat(32));
            }
        }
        let agents = root.join("agents");
        let report = reconcile(&broken.to_string(), &agents);
        assert!(!report.skill);
        assert!(!report.ui);
        assert!(!agents.join("review").exists());
        assert_eq!(picker_http_path("app.js").0, 404);
        assert_eq!(cached_picker_http().0, 404);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn digest_mismatch_keeps_the_published_generation() {
        let _gate = lock_tests();
        let root = std::env::temp_dir().join(format!("a3s-review-keep-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let (snapshot, picker) = fixture(&root);
        let agents = root.join("agents");
        assert!(reconcile(&snapshot, &agents).ui);
        let html = fs::read(&picker).unwrap();
        let js = fs::read(picker.parent().unwrap().join("app.js")).unwrap();
        let mut broken = serde_json::from_str::<Value>(&snapshot).unwrap();
        let assets = broken["data"]["registry"]["capabilities"][0]["activityBar"][0]["assets"]
            .as_array_mut()
            .unwrap();
        for asset in assets {
            if asset["path"].as_str().unwrap().ends_with("app.js") {
                asset["sha256"] = Value::String("cd".repeat(32));
            }
        }
        let report = reconcile(&broken.to_string(), &agents);
        assert!(report.ui);
        assert!(report.skill);
        assert!(report.steps.is_empty());
        assert!(agents.join(".a3s-review-managed").is_file());
        assert_eq!(picker_http_path("app.js").3, js);
        assert_eq!(picker_http_path("").3, html);
        assert_eq!(picker_http_path("queued").3, html);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_required_docx_review_skill_supplies_the_prompt_and_the_skill() {
        use a3s_use_reconcile::{PackageFile, PackageSpec, Payload, SurfaceSpec, files_sha256};

        let root =
            std::env::temp_dir().join(format!("a3s-docx-review-skill-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let acl = r#"extension "a3s/review" {
  schema_version = 3
  version        = "1.0.0"
  route          = "review"
  requires_use   = ">=0.3.0, <0.4.0"
  actions        = ["read"]
  repository {
    url      = "https://github.com/A3S-Lab/Use-Registry"
    revision = "c1dcedcd84794ce4bb0010c8d17a7eff4c62d212"
  }
  skill "review" {
    path = "skills/review/SKILL.md"
    host = "docx-review"
  }
  agent "legal-expert" {
    harness        = "builtin"
    instructions   = "agents/legal-expert/INSTRUCTIONS.md"
    runtime        = "edge"
    requires_skill = ["review"]
    optional       = false
    resources {
      cpu_millis   = 500
      memory_bytes = 268435456
    }
  }
}
"#;
        let clerk = r#"extension "a3s/clerk" {
  schema_version = 3
  version        = "1.0.0"
  route          = "clerk"
  requires_use   = ">=0.3.0, <0.4.0"
  actions        = ["read"]
  repository {
    url      = "https://github.com/A3S-Lab/Use-Registry"
    revision = "c1dcedcd84794ce4bb0010c8d17a7eff4c62d212"
  }
  agent "clerk" {
    harness      = "builtin"
    instructions = "agents/clerk/INSTRUCTIONS.md"
    runtime  = "edge"
    optional = false
    resources {
      cpu_millis   = 500
      memory_bytes = 268435456
    }
  }
}
"#;
        let files = [
            ("a3s-use-extension.acl", acl),
            ("agents/legal-expert/INSTRUCTIONS.md", "你是合同审查员。"),
            ("skills/review/SKILL.md", "按条款写 a3s-review-finding。"),
        ];
        let clerk_files = [
            ("a3s-use-extension.acl", clerk),
            ("agents/clerk/INSTRUCTIONS.md", "你是文员。"),
        ];
        let store = ReconcileStore::open(root.join("package-generations")).unwrap();
        let spec = PackageSpec {
            package_id: PACKAGE_ID.to_owned(),
            surfaces: vec![
                surface_files(
                    SurfaceKind::Skill,
                    "review",
                    &files,
                    &["a3s-use-extension.acl", "skills/review/SKILL.md"],
                ),
                surface_files(
                    SurfaceKind::Agent,
                    "legal-expert",
                    &files,
                    &[
                        "a3s-use-extension.acl",
                        "agents/legal-expert/INSTRUCTIONS.md",
                    ],
                ),
            ],
        };
        let clerk_spec = PackageSpec {
            package_id: "use/a3s/clerk".to_owned(),
            surfaces: vec![surface_files(
                SurfaceKind::Agent,
                "clerk",
                &clerk_files,
                &["a3s-use-extension.acl", "agents/clerk/INSTRUCTIONS.md"],
            )],
        };
        let published = store.apply_sync(&spec).unwrap();
        let clerk_published = store.apply_sync(&clerk_spec).unwrap();
        store
            .publish_selection_sync(
                SELECTION_ID,
                None,
                &[published.cursor(), clerk_published.cursor()],
            )
            .unwrap();
        let slug = root.join("capabilities/plugins/a3s-review");
        fs::create_dir_all(&slug).unwrap();
        fs::write(slug.join(".a3s-package-id"), PACKAGE_ID).unwrap();

        let text = docx_review_instructions(Some(&root), "legal-expert").unwrap();
        assert!(text.starts_with("你是合同审查员。"));
        assert!(text.contains("按条款写 a3s-review-finding。"));
        assert_eq!(
            docx_review_instructions(Some(&root), "a3s-review").as_deref(),
            Some(text.as_str())
        );
        assert!(docx_review_instructions(Some(&root), "clerk").is_none());
        let _ = fs::remove_dir_all(&root);

        fn surface_files(
            kind: SurfaceKind,
            id: &str,
            files: &[(&str, &str)],
            keep: &[&str],
        ) -> SurfaceSpec {
            let payload = files
                .iter()
                .filter(|(path, _)| keep.contains(path))
                .map(|(path, text)| PackageFile {
                    path: (*path).to_owned(),
                    bytes: text.as_bytes().to_vec(),
                })
                .collect::<Vec<_>>();
            SurfaceSpec {
                kind,
                id: id.to_owned(),
                sha256: files_sha256(&payload).unwrap(),
                entry: None,
                payload: Payload::Files(payload),
            }
        }
    }
}
