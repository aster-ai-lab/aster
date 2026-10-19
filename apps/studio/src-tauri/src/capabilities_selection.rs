use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::Path;

use a3s_use_reconcile::{ReconcileStore, SelectionCursor};

use super::{
    DIGEST_MARKER, PACKAGE_ID_MARKER, PluginPin, digest_ok, host_skill_error, package_id_ok,
    read_plugin_marker,
};

pub(super) const INSTALLED_SELECTION: &str = "studio-installed-plugins";

pub(super) struct InstalledSelection {
    store: ReconcileStore,
    expected: Option<SelectionCursor>,
}

impl InstalledSelection {
    pub(super) fn prepare(agents: &Path, pins: &BTreeMap<String, PluginPin>) -> io::Result<Self> {
        let mut packages = BTreeSet::new();
        if pins.len() > 128 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "too many selected plugins",
            ));
        }
        for (slug, pin) in pins {
            if !crate::org_login::capability_slug_ok(slug)
                || !package_id_ok(&pin.package_id)
                || !digest_ok(&pin.digest)
                || !packages.insert(&pin.package_id)
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "invalid or duplicate plugin selection",
                ));
            }
        }
        let parent = agents.parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "package generation root is unavailable",
            )
        })?;
        let store =
            ReconcileStore::open(parent.join("package-generations")).map_err(host_skill_error)?;
        let expected = store
            .selection_sync(INSTALLED_SELECTION)
            .map_err(host_skill_error)?
            .map(|snapshot| snapshot.cursor());
        Ok(Self { store, expected })
    }

    pub(super) fn publish(self, root: &Path, pins: &BTreeMap<String, PluginPin>) -> io::Result<()> {
        let plugins = root.canonicalize()?.join("plugins");
        let mut cursors = Vec::with_capacity(pins.len());
        let mut publications = Vec::with_capacity(pins.len());
        for (slug, pin) in pins {
            let target = plugins.join(slug);
            let metadata = fs::symlink_metadata(&target)?;
            if !metadata.is_dir() || target.canonicalize()? != target {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "installed plugin directory is invalid",
                ));
            }
            if read_plugin_marker(&target.join(PACKAGE_ID_MARKER))? != pin.package_id
                || read_plugin_marker(&target.join(DIGEST_MARKER))? != pin.digest
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "installed plugin markers do not match the selection",
                ));
            }
            let publication = self
                .store
                .current_sync(&pin.package_id)
                .map_err(host_skill_error)?
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::NotFound, "selected package is unpublished")
                })?;
            super::validate_plugin_publication(&publication)?;
            cursors.push(publication.cursor());
            publications.push(publication);
        }
        super::validate_plugin_selection(&publications)?;
        self.store
            .publish_selection_sync(INSTALLED_SELECTION, self.expected.as_ref(), &cursors)
            .map_err(host_skill_error)?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "capabilities_selection_tests.rs"]
mod tests;
