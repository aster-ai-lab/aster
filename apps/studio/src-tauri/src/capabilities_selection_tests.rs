#![cfg(test)]

use std::io::{Cursor, Write};
use std::path::PathBuf;

use a3s_use_reconcile::SurfaceKind;
use zip::{ZipWriter, write::SimpleFileOptions};

use super::*;
use crate::capabilities::{digest_of, sync_plugins};

struct Installation {
    root: PathBuf,
    pins: BTreeMap<String, PluginPin>,
    bundles: BTreeMap<String, Vec<u8>>,
}

impl Installation {
    fn new() -> Self {
        let mut fixture = Self::empty();
        fixture.add("one", "acme/one", "old one");
        fixture.add("two", "acme/two", "old two");
        fixture
    }

    fn empty() -> Self {
        let root =
            std::env::temp_dir().join(format!("studio-plugin-selection-{}", uuid::Uuid::now_v7()));
        fs::create_dir_all(root.join("agents")).unwrap();
        Self {
            root,
            pins: BTreeMap::new(),
            bundles: BTreeMap::new(),
        }
    }

    fn add(&mut self, slug: &str, package_id: &str, body: &str) {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file("tools/echo", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(body.as_bytes()).unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        self.pins.insert(
            slug.into(),
            PluginPin {
                require_aix: false,
                package_id: package_id.into(),
                digest: digest_of(&bytes),
            },
        );
        self.bundles.insert(slug.into(), bytes);
    }

    fn add_aix(
        &mut self,
        slug: &str,
        package_id: &str,
        version: &str,
        dependency: Option<(&str, &str)>,
    ) {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        let id = package_id.strip_prefix("use/").unwrap();
        let route = id.rsplit('/').next().unwrap();
        let dependency = dependency
            .map(|(id, requirement)| {
                format!("dependency \"{id}\" {{ version = \"{requirement}\" }}")
            })
            .unwrap_or_default();
        let manifest = format!(
            "extension \"{id}\" {{ schema_version = 3 version = \"{version}\" requires_use = \">=0.3.0, <0.4.0\" route = \"{route}\" actions = [\"read\"] repository {{ url = \"https://example.org/{route}\" }} {dependency} ui \"home\" {{ entry = \"ui/index.html\" point = \"studio.editor\" }} }}"
        );
        for (path, body) in [
            ("a3s-use-extension.acl", manifest.as_bytes()),
            ("README.md", b"# AIX fixture".as_slice()),
            ("ui/index.html", b"<html/>".as_slice()),
        ] {
            zip.start_file(path, SimpleFileOptions::default()).unwrap();
            zip.write_all(body).unwrap();
        }
        let bytes = zip.finish().unwrap().into_inner();
        self.pins.insert(
            slug.into(),
            PluginPin {
                require_aix: true,
                package_id: package_id.into(),
                digest: digest_of(&bytes),
            },
        );
        self.bundles.insert(slug.into(), bytes);
    }

    fn store(&self) -> ReconcileStore {
        ReconcileStore::open(self.root.join("package-generations")).unwrap()
    }

    fn sync(&self) -> io::Result<()> {
        sync_plugins(
            &self.root.join("capabilities"),
            &self.root.join("agents"),
            &self.pins,
            &|slug| {
                self.bundles
                    .get(slug)
                    .cloned()
                    .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "bundle missing"))
            },
        )
    }
}

#[test]
fn aix_selection_withholds_publication_until_dependency_pin_is_installed() {
    let mut fixture = Installation::empty();
    fixture.add_aix(
        "app",
        "use/proof/app",
        "1.0.0",
        Some(("proof/base", "^1.0.0")),
    );
    assert_eq!(
        fixture.sync().unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
    assert!(
        fixture
            .store()
            .selection_sync(INSTALLED_SELECTION)
            .unwrap()
            .is_none()
    );
    assert!(!fixture.root.join("capabilities/plugins/app").exists());

    fixture.add_aix("base", "use/proof/base", "1.4.2", None);
    fixture.sync().unwrap();
    let selection = fixture
        .store()
        .selection_sync(INSTALLED_SELECTION)
        .unwrap()
        .unwrap();
    assert!(selection.is_published());
    assert_eq!(selection.packages().len(), 2);
    assert!(
        fixture
            .store()
            .acquire_selection_sync(&selection.cursor())
            .is_ok()
    );
}

#[test]
#[ignore = "requires explicit AIX archive with an absent dependency pin and catalogue output"]
fn aix_selection_rejects_built_artifact_without_exact_dependency_pin() {
    let archive = std::fs::read(
        std::env::var("A3S_CLOUD_TEST_AIX_NEGATIVE_PACKAGE").expect("missing-dependency archive"),
    )
    .unwrap();
    let output = std::env::var("A3S_CLOUD_TEST_AIX_NEGATIVE_CATALOGUE_OUTPUT")
        .expect("empty native catalogue output");
    let mut fixture = Installation::empty();
    fixture.pins.insert(
        "proof-view-commands".into(),
        PluginPin {
            require_aix: true,
            package_id: "use/proof/view-commands".into(),
            digest: digest_of(&archive),
        },
    );
    fixture
        .bundles
        .insert("proof-view-commands".into(), archive);
    assert_eq!(
        fixture.sync().unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
    assert!(
        fixture
            .store()
            .selection_sync(INSTALLED_SELECTION)
            .unwrap()
            .is_none()
    );
    assert!(
        !fixture
            .root
            .join("capabilities/plugins/proof-view-commands")
            .exists()
    );
    let local = super::super::LocalCapabilities::open_at(Some(fixture.root.join("capabilities")));
    let catalogue = local.plugin_views().unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&catalogue).unwrap();
    assert_eq!(parsed["packages"].as_array().unwrap().len(), 0);
    std::fs::write(output, catalogue).unwrap();
}

impl Drop for Installation {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

// Catch an oversized candidate mutating installed bytes or withdrawing its accepted predecessor.
#[test]
fn oversized_selection_is_refused_before_installation_changes() {
    let mut fixture = Installation::new();
    fixture.sync().unwrap();
    let before = fixture
        .store()
        .selection_sync(INSTALLED_SELECTION)
        .unwrap()
        .unwrap();
    for index in 2..128 {
        fixture.add(
            &format!("proof-{index}"),
            &format!("proof/item-{index}"),
            "candidate",
        );
    }
    assert!(InstalledSelection::prepare(&fixture.root.join("agents"), &fixture.pins).is_ok());
    fixture.add("overflow", "proof/overflow", "candidate");
    assert_eq!(
        fixture.sync().unwrap_err().kind(),
        io::ErrorKind::InvalidInput
    );
    let after = fixture
        .store()
        .selection_sync(INSTALLED_SELECTION)
        .unwrap()
        .unwrap();
    assert_eq!(after.cursor(), before.cursor());
    assert_eq!(
        fs::read(fixture.root.join("capabilities/plugins/one/tools/echo")).unwrap(),
        b"old one"
    );
    assert!(
        fixture
            .store()
            .acquire_selection_sync(&before.cursor())
            .is_ok()
    );
}

#[test]
fn actual_install_sync_publishes_a_complete_verified_selection() {
    let fixture = Installation::new();
    fixture.sync().unwrap();
    let store = fixture.store();
    let snapshot = store.selection_sync(INSTALLED_SELECTION).unwrap().unwrap();
    assert!(snapshot.is_published());
    assert_eq!(snapshot.packages().len(), 2);
    let lease = store.acquire_selection_sync(&snapshot.cursor()).unwrap();
    let bodies: Vec<_> = lease
        .publications()
        .map(|p| fs::read(p.surfaces[0].entry.as_ref().unwrap()).unwrap())
        .collect();
    assert_eq!(bodies, [b"old one".to_vec(), b"old two".to_vec()]);
    fixture.sync().unwrap();
    assert_eq!(
        store
            .selection_sync(INSTALLED_SELECTION)
            .unwrap()
            .unwrap()
            .cursor(),
        snapshot.cursor()
    );
}

#[test]
fn bundle_preflight_failure_preserves_the_previous_selection_and_packages() {
    let mut fixture = Installation::new();
    fixture.sync().unwrap();
    let store = fixture.store();
    let before = store.selection_sync(INSTALLED_SELECTION).unwrap().unwrap();
    let old = store.acquire_selection_sync(&before.cursor()).unwrap();
    fixture.add("one", "acme/one", "new one");
    fixture.add("two", "acme/two", "new two");
    fixture.bundles.remove("two");
    assert!(fixture.sync().is_err());
    assert_eq!(
        store
            .selection_sync(INSTALLED_SELECTION)
            .unwrap()
            .unwrap()
            .cursor(),
        before.cursor()
    );
    assert!(store.acquire_selection_sync(&before.cursor()).is_ok());
    let bodies: Vec<_> = old
        .publications()
        .map(|p| fs::read(p.surfaces[0].entry.as_ref().unwrap()).unwrap())
        .collect();
    assert_eq!(bodies, [b"old one".to_vec(), b"old two".to_vec()]);
    fixture.add("two", "acme/two", "new two");
    fixture.sync().unwrap();
    let next = store.selection_sync(INSTALLED_SELECTION).unwrap().unwrap();
    assert!(next.generation() > before.generation());
    assert_eq!(
        store
            .acquire_selection_sync(&next.cursor())
            .unwrap()
            .publications()
            .len(),
        2
    );
}

#[test]
fn uninstall_publishes_empty_selection_and_defers_accepted_bytes_until_release() {
    let mut fixture = Installation::new();
    fixture.sync().unwrap();
    let store = fixture.store();
    let before = store.selection_sync(INSTALLED_SELECTION).unwrap().unwrap();
    let lease = store.acquire_selection_sync(&before.cursor()).unwrap();
    let entries: Vec<_> = lease
        .publications()
        .map(|p| p.surfaces[0].entry.clone().unwrap())
        .collect();
    fixture.pins.clear();
    fixture.sync().unwrap();
    let empty = store.selection_sync(INSTALLED_SELECTION).unwrap().unwrap();
    assert!(empty.is_published());
    assert!(empty.packages().is_empty());
    assert!(store.acquire_selection_sync(&before.cursor()).is_err());
    assert_eq!(
        store
            .acquire_selection_sync(&empty.cursor())
            .unwrap()
            .publications()
            .len(),
        0
    );
    assert!(entries.iter().all(|p| p.exists()));
    assert!(lease.release_sync().into_iter().all(|r| r.result.is_ok()));
    assert!(entries.iter().all(|p| !p.exists()));
}

#[test]
fn concurrent_completed_sync_cannot_be_overwritten_by_an_older_candidate() {
    let mut fixture = Installation::new();
    fixture.sync().unwrap();
    let staged = InstalledSelection::prepare(&fixture.root.join("agents"), &fixture.pins).unwrap();
    fixture.add("one", "acme/one", "new one");
    fixture.sync().unwrap();
    let before = fixture
        .store()
        .selection_sync(INSTALLED_SELECTION)
        .unwrap()
        .unwrap();
    assert!(
        staged
            .publish(&fixture.root.join("capabilities"), &fixture.pins)
            .is_err()
    );
    assert_eq!(
        fixture
            .store()
            .selection_sync(INSTALLED_SELECTION)
            .unwrap()
            .unwrap()
            .cursor(),
        before.cursor()
    );
}

#[test]
fn duplicate_package_identity_is_rejected_before_installation_effects() {
    let mut fixture = Installation::new();
    fixture.pins.get_mut("two").unwrap().package_id = "acme/one".into();
    assert_eq!(
        fixture.sync().unwrap_err().kind(),
        io::ErrorKind::InvalidInput
    );
    assert!(!fixture.root.join("capabilities/plugins").exists());
    assert!(
        fixture
            .store()
            .selection_sync(INSTALLED_SELECTION)
            .unwrap()
            .is_none()
    );
}

#[test]
fn separate_tenant_roots_do_not_share_selection_updates() {
    let mut first = Installation::new();
    let second = Installation::new();
    first.sync().unwrap();
    second.sync().unwrap();
    let second_before = second
        .store()
        .selection_sync(INSTALLED_SELECTION)
        .unwrap()
        .unwrap();
    first.pins.clear();
    first.sync().unwrap();
    assert_eq!(
        second
            .store()
            .selection_sync(INSTALLED_SELECTION)
            .unwrap()
            .unwrap()
            .cursor(),
        second_before.cursor()
    );
    assert_eq!(
        second
            .store()
            .acquire_selection_sync(&second_before.cursor())
            .unwrap()
            .publications()
            .len(),
        2
    );
}

#[cfg(unix)]
#[test]
fn adoption_repairs_non_executable_tools_from_pinned_bundle_instead_of_unverified_raw_copy() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Installation::new();
    fixture.sync().unwrap();
    let store = fixture.store();
    let old = store.acquire_current_sync("acme/one").unwrap().unwrap();
    let entry = old.publication().surfaces[0].entry.as_ref().unwrap();
    fs::set_permissions(entry, fs::Permissions::from_mode(0o600)).unwrap();
    fs::write(
        fixture.root.join("capabilities/plugins/one/tools/echo"),
        b"unverified raw copy",
    )
    .unwrap();
    fixture.sync().unwrap();
    let next = store.current_sync("acme/one").unwrap().unwrap();
    assert!(next.generation > old.publication().generation);
    let next_entry = next.surfaces[0].entry.as_ref().unwrap();
    assert_eq!(fs::read(next_entry).unwrap(), b"old one");
    assert_eq!(
        fs::metadata(next_entry).unwrap().permissions().mode() & 0o7777,
        0o700
    );
    assert_eq!(
        fs::metadata(entry).unwrap().permissions().mode() & 0o7777,
        0o600
    );
}

#[cfg(unix)]
#[test]
fn adoption_repairs_mcp_entry_from_pinned_bundle_and_keeps_the_old_lease_immutable() {
    use std::os::unix::fs::PermissionsExt;
    let mut fixture = Installation::new();
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("mcp/server", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"verified server").unwrap();
    let bundle = zip.finish().unwrap().into_inner();
    fixture.pins.insert(
        "one".into(),
        PluginPin {
            require_aix: false,
            package_id: "acme/one".into(),
            digest: digest_of(&bundle),
        },
    );
    fixture.bundles.insert("one".into(), bundle);
    fixture.sync().unwrap();
    let store = fixture.store();
    let old = store.acquire_current_sync("acme/one").unwrap().unwrap();
    let entry = old
        .publication()
        .surfaces
        .iter()
        .find(|surface| surface.kind == SurfaceKind::Mcp)
        .unwrap()
        .entry
        .as_ref()
        .unwrap();
    fs::set_permissions(entry, fs::Permissions::from_mode(0o600)).unwrap();
    fs::write(
        fixture.root.join("capabilities/plugins/one/mcp/server"),
        b"unverified raw server",
    )
    .unwrap();
    fixture.sync().unwrap();
    let next = store.current_sync("acme/one").unwrap().unwrap();
    assert!(next.generation > old.publication().generation);
    let next_entry = next
        .surfaces
        .iter()
        .find(|surface| surface.kind == SurfaceKind::Mcp)
        .unwrap()
        .entry
        .as_ref()
        .unwrap();
    assert_eq!(fs::read(next_entry).unwrap(), b"verified server");
    assert_eq!(
        fs::metadata(next_entry).unwrap().permissions().mode() & 0o7777,
        0o700
    );
    assert_eq!(fs::read(entry).unwrap(), b"verified server");
    assert_eq!(
        fs::metadata(entry).unwrap().permissions().mode() & 0o7777,
        0o600
    );
    assert!(
        store
            .acquire_selection_sync(
                &store
                    .selection_sync(INSTALLED_SELECTION)
                    .unwrap()
                    .unwrap()
                    .cursor()
            )
            .is_ok()
    );
}
