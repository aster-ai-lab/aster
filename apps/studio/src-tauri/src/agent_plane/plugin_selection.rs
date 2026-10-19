use std::path::PathBuf;

use a3s_use_reconcile::{ReconcileStore, SelectionCursor};
use serde_json::{Value, json};

use super::PlaneConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Binding {
    pub store: PathBuf,
    pub cursor: SelectionCursor,
    pub mcp_names: Vec<String>,
}

pub(super) async fn current(
    config: &PlaneConfig,
    legacy_tools: &[Value],
) -> Result<Option<Binding>, String> {
    let Some(tenant) = config.root.as_ref().and_then(|root| root.parent()) else {
        if legacy_tools.is_empty() {
            return Ok(None);
        }
        return Err("Plugin selection has no tenant-owned store.".into());
    };
    let root = tenant.join("package-generations");
    let opened = tokio::task::spawn_blocking(move || ReconcileStore::open(root))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    let snapshot = opened
        .selection("studio-installed-plugins")
        .await
        .map_err(|error| error.to_string())?;
    match snapshot {
        Some(snapshot) if snapshot.is_published() => {
            let lease = opened.acquire_selection(&snapshot.cursor()).await.map_err(|error| error.to_string())?;
            let publications: Vec<_> = lease.publications().collect();
            for publication in &publications {
                crate::capabilities::validate_plugin_publication(publication).map_err(|error| error.to_string())?;
            }
            crate::capabilities::validate_plugin_selection(publications.iter().copied()).map_err(|error| error.to_string())?;
            let mcp_names = publications.iter().flat_map(|publication| &publication.surfaces).filter(|surface| surface.kind == a3s_use_reconcile::SurfaceKind::Mcp).map(|surface| surface.id.clone()).collect();
            Ok(Some(Binding { store: tenant.join("package-generations"), cursor: snapshot.cursor(), mcp_names }))
        },
        None if legacy_tools.is_empty() => Ok(None),
        _ => Err("Plugin selection is missing or withdrawn; synchronize installed plugins before execution.".into()),
    }
}

impl Binding {
    pub fn append_launch_args(&self, args: &mut Vec<String>) {
        args.extend([
            "--plugin-store".into(),
            self.store.to_string_lossy().into_owned(),
        ]);
    }

    pub fn bind_session(&self, params: &mut Value) {
        if params.get("_meta").is_none() {
            params["_meta"] = json!({});
        }
        params["_meta"]["a3sPluginSelection"] = json!(self.cursor);
    }
}

#[cfg(test)]
#[path = "plugin_selection_tests.rs"]
mod tests;
