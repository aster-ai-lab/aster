//! Read self-contained UI documents from the installed AIX selection.

use super::{
    DIGEST_MARKER, PACKAGE_ID_MARKER, digest_ok, host_skill_error, package_id_ok,
    read_plugin_marker,
};
use a3s_use_reconcile::{ReconcileStore, SurfaceKind};
use std::{fs, io, io::Read, path::Path};

const MAX_HTML_BYTES: u64 = 4 * 1024 * 1024;

pub(super) fn document(caps: &Path, slug: &str, id: &str) -> io::Result<Vec<u8>> {
    let missing = || io::Error::from(io::ErrorKind::NotFound);
    if !crate::org_login::capability_slug_ok(slug) || !crate::org_login::capability_slug_ok(id) {
        return Err(missing());
    }
    let dir = caps.join("plugins").join(slug);
    if !fs::symlink_metadata(&dir)?.is_dir()
        || dir.canonicalize()? != caps.canonicalize()?.join("plugins").join(slug)
    {
        return Err(missing());
    }
    let package_id = read_plugin_marker(&dir.join(PACKAGE_ID_MARKER))?;
    if !package_id_ok(&package_id) || !digest_ok(&read_plugin_marker(&dir.join(DIGEST_MARKER))?) {
        return Err(missing());
    }
    let root = caps.parent().ok_or_else(missing)?;
    let store = ReconcileStore::open(root.join("package-generations")).map_err(host_skill_error)?;
    let selected = store
        .selection_sync(super::selection::INSTALLED_SELECTION)
        .map_err(host_skill_error)?
        .ok_or_else(missing)?;
    let lease = store
        .acquire_selection_sync(&selected.cursor())
        .map_err(host_skill_error)?;
    super::validate_plugin_selection(lease.publications()).map_err(|_| missing())?;
    let publication = lease
        .publications()
        .find(|p| p.package_id == package_id)
        .ok_or_else(missing)?;
    super::validate_plugin_publication(publication).map_err(|_| missing())?;
    let surface = publication
        .surfaces
        .iter()
        .find(|s| s.kind == SurfaceKind::Ui && s.id == id)
        .ok_or_else(missing)?;
    let entry = surface.entry.as_ref().ok_or_else(missing)?;
    if entry.extension().and_then(|ext| ext.to_str()) != Some("html") {
        return Err(missing());
    }
    let path = surface.directory.join(entry);
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.is_file() || metadata.len() > MAX_HTML_BYTES {
        return Err(missing());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(MAX_HTML_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_HTML_BYTES {
        return Err(missing());
    }
    surface.verify_content().map_err(host_skill_error)?;
    let html = String::from_utf8(bytes).map_err(|_| missing())?;
    super::json_bytes(&serde_json::json!({
        "packageId": package_id, "surface": id,
        "revision": publication.cursor().revision, "html": html
    }))
}

#[cfg(test)]
#[path = "capabilities_ui_tests.rs"]
mod tests;
