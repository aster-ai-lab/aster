use std::fs::{File as StdFile, OpenOptions as StdOpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use aster_use_core::{
    PlanScope, PlanScopeKind, PluginOperationAction, PluginPackageId, PluginPackageLock, UseError,
    UseResult, MAX_PLUGIN_PLAN_ITEMS,
};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::io::AsyncWriteExt;

use super::package_manager_error;

mod inventory;
mod pending;

pub(super) use pending::{
    PackageGraphOperationPhase, PendingPackageGraphOperation, PendingPackageGraphStore,
};

const INSTALLED_GRAPH_SCHEMA: &str = "aster.use.installed-package-graph.v1";
const INSTALLATION_SNAPSHOT_SCHEMA: &str = "aster.use.installation-snapshot.v1";
const MAX_GRAPH_RECORD_BYTES: u64 = 2 * 1024 * 1024;
const MAX_INSTALLATION_SNAPSHOT_BYTES: u64 = MAX_GRAPH_RECORD_BYTES * MAX_PLUGIN_PLAN_ITEMS as u64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct InstalledPackageGraph {
    pub schema: String,
    pub package_lock_digest: String,
    pub package_lock: PluginPackageLock,
    pub installed_at_ms: u64,
}

impl InstalledPackageGraph {
    fn new(package_lock: PluginPackageLock, installed_at_ms: u64) -> UseResult<Self> {
        let graph = Self {
            schema: INSTALLED_GRAPH_SCHEMA.to_string(),
            package_lock_digest: package_lock.descriptor_digest()?,
            package_lock,
            installed_at_ms,
        };
        graph.validate()?;
        Ok(graph)
    }

    fn validate(&self) -> UseResult<()> {
        self.package_lock.validate()?;
        if self.schema != INSTALLED_GRAPH_SCHEMA
            || self.package_lock_digest != self.package_lock.descriptor_digest()?
            || self.installed_at_ms == 0
        {
            return Err(store_error("An installed package graph record is invalid."));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InstallationSnapshot {
    schema: String,
    scope_kind: PlanScopeKind,
    scope_id: String,
    generation: u64,
    graphs: Vec<InstalledPackageGraph>,
}

impl InstallationSnapshot {
    fn fresh(
        scope: &PlanScope,
        generation: u64,
        mut graphs: Vec<InstalledPackageGraph>,
    ) -> UseResult<Self> {
        graphs.sort_by(|left, right| {
            left.package_lock
                .root_package_id
                .cmp(&right.package_lock.root_package_id)
        });
        let snapshot = Self {
            schema: INSTALLATION_SNAPSHOT_SCHEMA.to_string(),
            scope_kind: scope.kind,
            scope_id: scope.id.clone(),
            generation,
            graphs,
        };
        snapshot.validate(scope)?;
        Ok(snapshot)
    }

    fn validate(&self, scope: &PlanScope) -> UseResult<()> {
        if self.schema != INSTALLATION_SNAPSHOT_SCHEMA
            || self.scope_kind != scope.kind
            || self.scope_id != scope.id
            || self.generation == 0
            || self.graphs.len() > MAX_PLUGIN_PLAN_ITEMS
        {
            return Err(store_error("An installation snapshot record is invalid."));
        }
        let mut previous = None;
        for graph in &self.graphs {
            graph.validate()?;
            let package_id = graph.package_lock.root_package_id.as_str();
            if previous.is_some_and(|prior| prior >= package_id) {
                return Err(store_error("An installation snapshot record is invalid."));
            }
            previous = Some(package_id);
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub(super) struct InstalledPackageGraphStore {
    state_root: PathBuf,
    scope: PlanScope,
    snapshot: PathBuf,
}

impl InstalledPackageGraphStore {
    pub fn new(state_root: impl Into<PathBuf>, scope: PlanScope) -> UseResult<Self> {
        let state_root = state_root.into();
        validate_scope(&scope)?;
        refuse_legacy_package_graphs(&state_root)?;
        let snapshot = installation_snapshot_path(&state_root, &scope)?;
        Ok(Self {
            state_root,
            scope,
            snapshot,
        })
    }

    #[cfg(test)]
    pub(super) async fn generation(&self) -> UseResult<u64> {
        Ok(self
            .load_snapshot()
            .await?
            .map(|snapshot| snapshot.generation)
            .unwrap_or(0))
    }

    pub async fn put(&self, lock: &PluginPackageLock, installed_at_ms: u64) -> UseResult<bool> {
        let record = InstalledPackageGraph::new(lock.clone(), installed_at_ms)?;
        let _guard = acquire_lock(&self.state_root).await?;
        let snapshot = self.load_snapshot().await?;
        let unchanged = snapshot.as_ref().and_then(|current| {
            current
                .graphs
                .iter()
                .find(|graph| graph.package_lock.root_package_id == lock.root_package_id)
                .map(|graph| graph.package_lock == record.package_lock)
        });
        match unchanged {
            Some(true) => return Ok(false),
            Some(false) => {
                return Err(package_manager_error(
                    "use.plugin.package_graph_reconcile_required",
                    format!(
                        "package '{}' already owns a different installed dependency lock.",
                        lock.root_package_id
                    ),
                ))
            }
            None => {}
        }
        let generation = next_generation(snapshot.as_ref().map(|current| current.generation))?;
        let mut graphs = snapshot.map(|current| current.graphs).unwrap_or_default();
        graphs.push(record);
        self.commit(&InstallationSnapshot::fresh(
            &self.scope,
            generation,
            graphs,
        )?)
        .await?;
        Ok(true)
    }

    pub async fn get(&self, root_package_id: &str) -> UseResult<Option<InstalledPackageGraph>> {
        Ok(self.load_snapshot().await?.and_then(|snapshot| {
            snapshot
                .graphs
                .into_iter()
                .find(|graph| graph.package_lock.root_package_id == root_package_id)
        }))
    }

    pub async fn replace(
        &self,
        root_package_id: &str,
        expected_digest: &str,
        replacement: &PluginPackageLock,
        installed_at_ms: u64,
    ) -> UseResult<bool> {
        if replacement.root_package_id != root_package_id {
            return Err(store_error(
                "A replacement graph does not own the requested root package.",
            ));
        }
        let record = InstalledPackageGraph::new(replacement.clone(), installed_at_ms)?;
        let _guard = acquire_lock(&self.state_root).await?;
        let snapshot = self.load_snapshot().await?.ok_or_else(|| {
            store_error("The installed package graph disappeared before replacement.")
        })?;
        let Some(index) = snapshot
            .graphs
            .iter()
            .position(|graph| graph.package_lock.root_package_id == root_package_id)
        else {
            return Err(store_error(
                "The installed package graph disappeared before replacement.",
            ));
        };
        if snapshot.graphs[index].package_lock == record.package_lock {
            return Ok(false);
        }
        if snapshot.graphs[index].package_lock_digest != expected_digest {
            return Err(store_error(
                "The installed package graph changed before replacement.",
            ));
        }
        let generation = next_generation(Some(snapshot.generation))?;
        let mut graphs = snapshot.graphs;
        graphs[index] = record;
        self.commit(&InstallationSnapshot::fresh(
            &self.scope,
            generation,
            graphs,
        )?)
        .await?;
        Ok(true)
    }

    pub async fn list(&self) -> UseResult<Vec<InstalledPackageGraph>> {
        Ok(self
            .load_snapshot()
            .await?
            .map(|snapshot| snapshot.graphs)
            .unwrap_or_default())
    }

    pub async fn remove(&self, root_package_id: &str, expected_digest: &str) -> UseResult<bool> {
        let _guard = acquire_lock(&self.state_root).await?;
        let Some(snapshot) = self.load_snapshot().await? else {
            return Ok(false);
        };
        let Some(index) = snapshot
            .graphs
            .iter()
            .position(|graph| graph.package_lock.root_package_id == root_package_id)
        else {
            return Ok(false);
        };
        if snapshot.graphs[index].package_lock_digest != expected_digest {
            return Err(store_error(
                "The installed package graph changed before removal.",
            ));
        }
        let generation = next_generation(Some(snapshot.generation))?;
        let mut graphs = snapshot.graphs;
        graphs.remove(index);
        self.commit(&InstallationSnapshot::fresh(
            &self.scope,
            generation,
            graphs,
        )?)
        .await?;
        Ok(true)
    }

    async fn load_snapshot(&self) -> UseResult<Option<InstallationSnapshot>> {
        refuse_legacy_package_graphs(&self.state_root)?;
        let parent = self.snapshot.parent().ok_or_else(path_identity_error)?;
        if !validate_existing_directory_chain(&self.state_root, parent).await? {
            return Ok(None);
        }
        let snapshot: Option<InstallationSnapshot> =
            read_optional_bounded(&self.snapshot, MAX_INSTALLATION_SNAPSHOT_BYTES).await?;
        if let Some(snapshot) = &snapshot {
            snapshot.validate(&self.scope)?;
        }
        Ok(snapshot)
    }

    async fn commit(&self, snapshot: &InstallationSnapshot) -> UseResult<()> {
        write_bounded(
            &self.state_root,
            &self.snapshot,
            snapshot,
            MAX_INSTALLATION_SNAPSHOT_BYTES,
        )
        .await
    }
}

fn next_generation(current: Option<u64>) -> UseResult<u64> {
    current
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| store_error("The installation snapshot generation overflowed."))
}

fn validate_scope(scope: &PlanScope) -> UseResult<()> {
    let scope_id = scope.id.as_str();
    if scope_id.is_empty()
        || scope_id.len() > 256
        || !scope_id
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        || !scope_id.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b':' | b'/' | b'@')
        })
    {
        return Err(package_manager_error(
            "use.plugin.package_scope_invalid",
            "The package manager scope identity is invalid.",
        ));
    }
    Ok(())
}

fn refuse_legacy_package_graphs(state_root: &Path) -> UseResult<()> {
    let legacy = state_root.join("package-graphs");
    match std::fs::symlink_metadata(&legacy) {
        Ok(_) => Err(store_error(
            "This state root still has package-graphs. That layout is superseded. Delete the state root and install the packages again.",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(path_error(
            "inspect superseded package graph directory",
            &legacy,
            error,
        )),
    }
}

fn installation_snapshot_path(state_root: &Path, scope: &PlanScope) -> UseResult<PathBuf> {
    let encoded = scope.id.replace('/', "%2F");
    if encoded.is_empty()
        || encoded.contains('/')
        || encoded.contains('\\')
        || matches!(encoded.as_str(), "." | "..")
    {
        return Err(path_identity_error());
    }
    Ok(state_root
        .join("installations")
        .join(scope.kind.as_str())
        .join(encoded)
        .join("snapshot.json"))
}

fn package_record_path(root: &Path, package_id: &str) -> UseResult<PathBuf> {
    PluginPackageId::parse(package_id.to_string())
        .map_err(|_| store_error("A package graph path contains an invalid package identity."))?;
    let (publisher, package) = package_id
        .split_once('/')
        .ok_or_else(|| store_error("A package graph path is incomplete."))?;
    Ok(root.join(publisher).join(format!("{package}.json")))
}

fn pending_record_path(
    root: &Path,
    action: PluginOperationAction,
    package_id: &str,
) -> UseResult<PathBuf> {
    Ok(root
        .join(action_name(action))
        .join(package_record_path(Path::new(""), package_id)?))
}

fn action_name(action: PluginOperationAction) -> &'static str {
    match action {
        PluginOperationAction::Install => "install",
        PluginOperationAction::Uninstall => "uninstall",
        PluginOperationAction::Upgrade => "upgrade",
        PluginOperationAction::Enable => "enable",
        PluginOperationAction::Disable => "disable",
    }
}

async fn read_optional<T>(path: &Path) -> UseResult<Option<T>>
where
    T: for<'de> Deserialize<'de>,
{
    read_optional_bounded(path, MAX_GRAPH_RECORD_BYTES).await
}

async fn read_optional_bounded<T>(path: &Path, max_bytes: u64) -> UseResult<Option<T>>
where
    T: for<'de> Deserialize<'de>,
{
    match fs::symlink_metadata(path).await {
        Ok(metadata)
            if !aster_use_core::metadata_is_link_or_reparse_point(&metadata)
                && metadata.is_file()
                && metadata.len() <= max_bytes => {}
        Ok(_) => return Err(store_error("A package graph record path is invalid.")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(path_error("inspect package graph record", path, error)),
    }
    read_required_bounded(path, max_bytes).await.map(Some)
}

async fn read_required_bounded<T>(path: &Path, max_bytes: u64) -> UseResult<T>
where
    T: for<'de> Deserialize<'de>,
{
    let metadata = fs::symlink_metadata(path)
        .await
        .map_err(|error| path_error("inspect package graph record", path, error))?;
    if aster_use_core::metadata_is_link_or_reparse_point(&metadata)
        || !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > max_bytes
    {
        return Err(store_error("A package graph record path is invalid."));
    }
    let bytes = fs::read(path)
        .await
        .map_err(|error| path_error("read package graph record", path, error))?;
    if bytes.is_empty() || bytes.len() as u64 > max_bytes {
        return Err(store_error(
            "A package graph record exceeds its size bound.",
        ));
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| store_error("A package graph record contains invalid JSON."))
}

async fn write_new<T: Serialize>(state_root: &Path, path: &Path, value: &T) -> UseResult<()> {
    write_bounded(state_root, path, value, MAX_GRAPH_RECORD_BYTES).await
}

async fn write_bounded<T: Serialize>(
    state_root: &Path,
    path: &Path,
    value: &T,
    max_bytes: u64,
) -> UseResult<()> {
    if !path.starts_with(state_root) || path == state_root {
        return Err(path_identity_error());
    }
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|_| store_error("Failed to encode a package graph record."))?;
    if bytes.is_empty() || bytes.len() as u64 > max_bytes {
        return Err(store_error(
            "A package graph record exceeds its size bound.",
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| store_error("A package graph record has no owned parent."))?;
    ensure_owned_directory(state_root, parent).await?;
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = path.with_extension(format!("tmp-{}-{suffix}", std::process::id()));
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .await
        .map_err(|error| path_error("create temporary package graph record", &temporary, error))?;
    if let Err(error) = async {
        file.write_all(&bytes).await?;
        file.write_all(b"\n").await?;
        file.sync_all().await?;
        Ok::<_, std::io::Error>(())
    }
    .await
    {
        let _ = fs::remove_file(&temporary).await;
        return Err(path_error("commit package graph record", path, error));
    }
    drop(file);
    if let Err(error) = activate_temporary_file(temporary.clone(), path.to_path_buf()).await {
        let _ = fs::remove_file(&temporary).await;
        return Err(error);
    }
    sync_parent(parent).await
}

async fn activate_temporary_file(temporary: PathBuf, target: PathBuf) -> UseResult<()> {
    let error_target = target.clone();
    tokio::task::spawn_blocking(move || {
        aster_use_extension::persist_temporary_replace_blocking(temporary, &target)
    })
    .await
    .map_err(|error| {
        package_manager_error(
            "use.plugin.package_graph_io",
            format!(
                "Failed to commit package graph record '{}': atomic replacement task failed: {error}",
                error_target.display()
            ),
        )
    })?
    .map_err(|error| path_error("commit package graph record", &error_target, error))
}

async fn acquire_lock(state_root: &Path) -> UseResult<StdFile> {
    ensure_owned_directory(state_root, state_root).await?;
    let path = state_root.join(".package-graph.lock");
    match fs::symlink_metadata(&path).await {
        Ok(metadata)
            if aster_use_core::metadata_is_link_or_reparse_point(&metadata)
                || !metadata.is_file() =>
        {
            return Err(path_identity_error())
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(path_error("inspect package graph lock", &path, error)),
    }
    tokio::task::spawn_blocking(move || {
        let file = StdOpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|error| path_error("open package graph lock", &path, error))?;
        file.lock_exclusive()
            .map_err(|error| path_error("lock package graph store", &path, error))?;
        Ok(file)
    })
    .await
    .map_err(|error| {
        store_error(format!(
            "Failed to join the package graph lock task: {error}"
        ))
    })?
}

async fn ensure_owned_directory(state_root: &Path, directory: &Path) -> UseResult<()> {
    if !directory.starts_with(state_root) {
        return Err(path_identity_error());
    }
    fs::create_dir_all(state_root)
        .await
        .map_err(|error| path_error("create package graph state root", state_root, error))?;
    validate_directory(state_root).await?;
    let relative = directory
        .strip_prefix(state_root)
        .map_err(|_| path_identity_error())?;
    let mut current = state_root.to_path_buf();
    for segment in relative.components() {
        current.push(segment.as_os_str());
        match fs::create_dir(&current).await {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(path_error(
                    "create package graph directory",
                    &current,
                    error,
                ))
            }
        }
        validate_directory(&current).await?;
    }
    Ok(())
}

async fn validate_existing_directory_chain(state_root: &Path, directory: &Path) -> UseResult<bool> {
    if !directory.starts_with(state_root) {
        return Err(path_identity_error());
    }
    let relative = directory
        .strip_prefix(state_root)
        .map_err(|_| path_identity_error())?;
    let mut current = state_root.to_path_buf();
    for segment in std::iter::once(None).chain(relative.components().map(Some)) {
        if let Some(segment) = segment {
            current.push(segment.as_os_str());
        }
        match fs::symlink_metadata(&current).await {
            Ok(metadata)
                if !aster_use_core::metadata_is_link_or_reparse_point(&metadata)
                    && metadata.is_dir() => {}
            Ok(_) => return Err(path_identity_error()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => {
                return Err(path_error(
                    "inspect package graph directory",
                    &current,
                    error,
                ))
            }
        }
    }
    Ok(true)
}

async fn validate_directory(path: &Path) -> UseResult<()> {
    let metadata = fs::symlink_metadata(path)
        .await
        .map_err(|error| path_error("inspect package graph directory", path, error))?;
    if aster_use_core::metadata_is_link_or_reparse_point(&metadata) || !metadata.is_dir() {
        return Err(path_identity_error());
    }
    Ok(())
}

#[cfg(unix)]
async fn sync_parent(parent: &Path) -> UseResult<()> {
    fs::File::open(parent)
        .await
        .map_err(|error| path_error("open package graph directory", parent, error))?
        .sync_all()
        .await
        .map_err(|error| path_error("sync package graph directory", parent, error))
}

#[cfg(not(unix))]
async fn sync_parent(_parent: &Path) -> UseResult<()> {
    Ok(())
}

fn path_identity_error() -> UseError {
    store_error("A package graph record escaped or traversed its configured state root.")
}

fn path_error(operation: &str, path: &Path, error: std::io::Error) -> UseError {
    package_manager_error(
        "use.plugin.package_graph_io",
        format!("Failed to {operation} '{}': {error}", path.display()),
    )
}

fn store_error(message: impl Into<String>) -> UseError {
    package_manager_error("use.plugin.package_graph_store_invalid", message)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::package::{
        grant::PackageGraphAuthorization, InstallDisposition, UninstallDisposition,
        UpgradeDisposition,
    };
    use aster_use_core::{
        CatalogAvailability, PlanScope, PlanScopeKind, PluginCatalogRecord, PluginPackageLockHost,
        PluginPackageResolver, PluginWorkspaceGrantSnapshot, SurfaceChangeKind,
        VerifiedCatalogProvenance, VerifiedPluginCatalogRecord, PLUGIN_CATALOG_SCHEMA_V3,
        PLUGIN_WORKSPACE_GRANT_SNAPSHOT_SCHEMA,
    };
    use aster_use_extension::ExtensionManifest;

    const CATALOG: &[u8] =
        include_bytes!("../../core//fixtures/plugins/catalog-record-okf-v3.json");
    const MANIFEST: &str = include_str!("../../extension//fixtures/manifests/plugin-v3-okf.acl");

    fn digest(seed: char) -> String {
        format!("sha256:{}", seed.to_string().repeat(64))
    }

    fn manifest(version: &str) -> ExtensionManifest {
        manifest_for("acme/knowledge", version)
    }

    fn manifest_for(package_id: &str, version: &str) -> ExtensionManifest {
        let mut manifest = ExtensionManifest::parse_acl(MANIFEST).unwrap();
        let (_, name) = package_id.split_once('/').unwrap();
        manifest.package_id = package_id.to_string();
        manifest.version = version.to_string();
        manifest.route = name.to_string();
        if let Some(repository) = &mut manifest.repository {
            repository.url = format!("https://github.com/acme/{name}");
        }
        manifest
    }

    fn package_lock(version: &str, seed: char) -> PluginPackageLock {
        package_lock_for("acme/knowledge", version, seed)
    }

    fn package_lock_for(package_id: &str, version: &str, seed: char) -> PluginPackageLock {
        let mut record = PluginCatalogRecord::from_json(CATALOG).unwrap();
        let (publisher, name) = package_id.split_once('/').unwrap();
        record.schema = PLUGIN_CATALOG_SCHEMA_V3.to_string();
        record.package_id = package_id.to_string();
        record.publisher = publisher.to_string();
        record.display_name = format!("{name} fixture");
        record.description = format!("Package graph fixture for {package_id}.");
        record.version = version.to_string();
        record.archive.target_name = format!(
            "extensions/{package_id}/{version}/stable/linux-x86_64/{publisher}-{name}-{version}.tar.gz"
        );
        record.archive.sha256 = digest(seed);
        record.package.sha256 = Some(digest(seed));
        record.package.manifest_sha256 = Some(digest(seed));
        record.repository = format!("https://github.com/{publisher}/{name}");
        record.availability = CatalogAvailability::Available;
        record.validate().unwrap();
        let provenance = VerifiedCatalogProvenance {
            registry_name: "packages".to_string(),
            registry_url: "https://packages.example.test/aster/".to_string(),
            root_sha256: digest('f'),
            root_version: 1,
            timestamp_version: 1,
            snapshot_version: 1,
            targets_version: 1,
            catalog_record_digest: record.descriptor_digest().unwrap(),
        };
        let verified = VerifiedPluginCatalogRecord::new(record, provenance).unwrap();
        PluginPackageResolver::new(
            PluginPackageLockHost::new("linux-x86_64", env!("CARGO_PKG_VERSION")).unwrap(),
        )
        .resolve(verified, Vec::new())
        .unwrap()
    }

    fn grant_snapshot(state_revision: u64) -> PluginWorkspaceGrantSnapshot {
        PluginWorkspaceGrantSnapshot {
            schema: PLUGIN_WORKSPACE_GRANT_SNAPSHOT_SCHEMA.to_string(),
            scope_id: "current".to_string(),
            state_revision,
            grants: Vec::new(),
        }
    }

    fn scope() -> PlanScope {
        PlanScope {
            kind: PlanScopeKind::User,
            id: "current".to_string(),
        }
    }

    fn install_pending(lock: &PluginPackageLock) -> PendingPackageGraphOperation {
        let package_id = lock.root_package_id.clone();
        let manifests = BTreeMap::from([(
            package_id.clone(),
            manifest_for(&package_id, lock.packages[0].version()),
        )]);
        let dispositions = BTreeMap::from([(package_id, InstallDisposition::Add)]);
        let generated = crate::package::plan::install_operation(
            lock,
            &dispositions,
            &crate::package::plan::all_surface_selections(lock),
            &manifests,
            1,
            &scope(),
            10,
            &grant_snapshot(2),
            &crate::package::StandalonePackageAuthorizationProvider,
        )
        .unwrap();
        PendingPackageGraphOperation::planned(
            generated.envelope,
            10,
            generated.generations,
            manifests,
        )
        .unwrap()
        .admit(10, PackageGraphAuthorization::default())
        .unwrap()
    }

    fn uninstall_pending(lock: &PluginPackageLock) -> PendingPackageGraphOperation {
        let package_id = lock.root_package_id.clone();
        let manifests =
            BTreeMap::from([(package_id.clone(), manifest(lock.packages[0].version()))]);
        let dispositions = BTreeMap::from([(package_id.clone(), UninstallDisposition::Remove)]);
        let generations = BTreeMap::from([(package_id, 7)]);
        let generated = crate::package::plan::uninstall_operation(
            lock,
            &dispositions,
            &crate::package::plan::all_surface_selections(lock),
            generations,
            digest('9'),
            1,
            &scope(),
            10,
            &grant_snapshot(2),
            &crate::package::StandalonePackageAuthorizationProvider,
        )
        .unwrap();
        PendingPackageGraphOperation::planned(
            generated.envelope,
            10,
            generated.generations,
            manifests,
        )
        .unwrap()
        .admit(10, PackageGraphAuthorization::default())
        .unwrap()
    }

    fn upgrade_pending(
        prior: &PluginPackageLock,
        candidate: &PluginPackageLock,
    ) -> PendingPackageGraphOperation {
        let package_id = candidate.root_package_id.clone();
        let manifests = BTreeMap::from([(
            package_id.clone(),
            manifest(candidate.packages[0].version()),
        )]);
        let prior_manifests =
            BTreeMap::from([(package_id.clone(), manifest(prior.packages[0].version()))]);
        let dispositions = BTreeMap::from([(package_id.clone(), UpgradeDisposition::Replace)]);
        let prior_generations = BTreeMap::from([(package_id, 7)]);
        let generated = crate::package::plan::upgrade_operation(
            candidate,
            prior,
            &dispositions,
            &crate::package::plan::all_surface_selections(prior),
            &crate::package::plan::all_surface_selections(candidate),
            &manifests,
            &prior_generations,
            digest('9'),
            8,
            &scope(),
            10,
            &grant_snapshot(9),
            &crate::package::StandalonePackageAuthorizationProvider,
        )
        .unwrap();
        PendingPackageGraphOperation::planned_upgrade(
            generated.envelope,
            10,
            generated.generations,
            manifests,
            prior.clone(),
            prior_generations,
            prior_manifests,
        )
        .unwrap()
        .admit(10, PackageGraphAuthorization::default())
        .unwrap()
    }

    mod records;
    #[cfg(any(unix, windows))]
    mod symlinks;
}
