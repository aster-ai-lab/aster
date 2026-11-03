//! Declaration origins and native tool selections outside the managed store.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::env_acl::{
    merge_into, resolve_tool_identity, validate_version_list, visit_declaration_layers,
    visit_env_layers, EnvLayerRoot,
};
use crate::{
    EnvConfig, EnvDeclarations, ResolvedTool, ToolAcquireError, ToolStore, VersionRequest,
    VersionRequestKind,
};

mod paths;
#[cfg(test)]
mod tests;

/// Merged declarations retaining the logical root of each selected tool.
#[derive(Debug, Clone)]
pub struct LoadedEnvConfig<C = EnvConfig> {
    config: C,
    roots: BTreeMap<String, DeclarationRoot>,
    sources: BTreeSet<PathBuf>,
}

/// Ordered declarations retaining each tool's logical source root.
pub type LoadedEnvDeclarations = LoadedEnvConfig<EnvDeclarations>;

#[derive(Debug, Clone)]
enum DeclarationRoot {
    User,
    Project(PathBuf),
}

/// A tool request with its admitted origin and native external layout.
#[derive(Debug, Clone)]
pub struct ResolvedEnvTool {
    tool: ResolvedTool,
    origin: EnvToolOrigin,
}

/// Installation authority selected by a declaration, independent of version text.
#[derive(Debug, Clone)]
pub enum EnvToolOrigin {
    Managed,
    System,
    Path(ExternalToolPaths),
}

/// An existing external directory and its backend's candidate executable paths.
#[derive(Debug, Clone)]
pub struct ExternalToolPaths {
    root: PathBuf,
    bin_dirs: Vec<PathBuf>,
}

impl ExternalToolPaths {
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn bin_dirs(&self) -> &[PathBuf] {
        &self.bin_dirs
    }
}

impl<C> LoadedEnvConfig<C> {
    pub fn config(&self) -> &C {
        &self.config
    }

    /// Explicitly remember admitted declaration files for unused-version cleanup.
    /// This writes bounded native-path metadata under the store's install lock.
    /// Loading or inspecting declarations alone never changes the tracked set.
    pub fn track_sources(&self, store: &ToolStore) -> Result<(), ToolAcquireError> {
        store.track_env_sources(&self.sources)
    }

    fn source_root<'a>(
        &'a self,
        name: &str,
        user_home: Option<&'a Path>,
    ) -> Result<Option<&'a Path>, ToolAcquireError> {
        match self.roots.get(name) {
            Some(DeclarationRoot::User) => Ok(user_home),
            Some(DeclarationRoot::Project(path)) => Ok(Some(path.as_path())),
            None => Err(ToolAcquireError::InvalidEnvAcl(
                "tool declaration has no source root".into(),
            )),
        }
    }
}

impl LoadedEnvConfig {
    /// Resolve all declarations, preserving parent and user configuration roots.
    /// Filesystem inspection is blocking; run it off an async executor thread.
    pub fn resolve_tools(
        &self,
        user_home: Option<&Path>,
    ) -> Result<Vec<ResolvedEnvTool>, ToolAcquireError> {
        self.config
            .tools
            .keys()
            .map(|name| self.resolve_declared(name, user_home))
            .collect()
    }

    /// Resolve temporary selections at `root`, retaining other declaration roots.
    /// Overrides replace declarations before external paths are inspected and do
    /// not write configuration. The same blocking I/O/errors as resolution apply.
    pub fn resolve_tools_with_overrides(
        &self,
        overrides: &BTreeMap<String, String>,
        root: &Path,
        user_home: Option<&Path>,
    ) -> Result<Vec<ResolvedEnvTool>, ToolAcquireError> {
        self.config
            .tools
            .keys()
            .chain(overrides.keys())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|name| match overrides.get(name) {
                Some(request) => resolve_env_tool(name, request, Some(root), user_home),
                None => self.resolve_declared(name, user_home),
            })
            .collect()
    }

    /// Resolve one declared tool without inspecting unrelated external layouts.
    pub fn resolve_tool(
        &self,
        name: &str,
        user_home: Option<&Path>,
    ) -> Result<Option<ResolvedEnvTool>, ToolAcquireError> {
        if !self.config.tools.contains_key(name) {
            return Ok(None);
        }
        self.resolve_declared(name, user_home).map(Some)
    }

    fn resolve_declared(
        &self,
        name: &str,
        user_home: Option<&Path>,
    ) -> Result<ResolvedEnvTool, ToolAcquireError> {
        let root = self.source_root(name, user_home)?;
        let request = self.config.tools.get(name).ok_or_else(|| {
            ToolAcquireError::InvalidEnvAcl("tool declaration is missing its request".into())
        })?;
        resolve_env_tool(name, request, root, user_home)
    }
}

impl LoadedEnvDeclarations {
    /// Resolve every declared version in order without network access or writes.
    /// Native path lookup is blocking and returns its original I/O errors.
    pub fn resolve_tools(
        &self,
        user_home: Option<&Path>,
    ) -> Result<Vec<ResolvedEnvTool>, ToolAcquireError> {
        let mut tools = Vec::new();
        for (name, requests) in &self.config.tools {
            tools.extend(self.resolve_versions(name, requests, user_home)?);
        }
        Ok(tools)
    }

    /// Resolve one tool's complete version list, leaving unrelated paths uninspected.
    pub fn resolve_tool_versions(
        &self,
        name: &str,
        user_home: Option<&Path>,
    ) -> Result<Option<Vec<ResolvedEnvTool>>, ToolAcquireError> {
        self.config
            .tools
            .get(name)
            .map(|requests| self.resolve_versions(name, requests, user_home))
            .transpose()
    }

    /// Apply temporary single-tool overrides before inspecting declaration paths.
    /// Other tools retain all their ordered versions and original logical roots.
    pub fn resolve_tools_with_overrides(
        &self,
        overrides: &BTreeMap<String, String>,
        root: &Path,
        user_home: Option<&Path>,
    ) -> Result<Vec<ResolvedEnvTool>, ToolAcquireError> {
        self.resolve_tools_with_version_overrides(
            &overrides
                .iter()
                .map(|(name, request)| (name.clone(), vec![request.clone()]))
                .collect(),
            root,
            user_home,
        )
    }

    /// Replace overridden tools with their ordered requests rooted at `root`.
    /// Exact duplicates keep their first occurrence. Empty or oversized lists
    /// refuse before path lookup; other tools retain their declaration roots.
    /// Resolution performs blocking path inspection but no network or writes.
    pub fn resolve_tools_with_version_overrides(
        &self,
        overrides: &BTreeMap<String, Vec<String>>,
        root: &Path,
        user_home: Option<&Path>,
    ) -> Result<Vec<ResolvedEnvTool>, ToolAcquireError> {
        for requests in overrides.values() {
            validate_version_list(requests)?;
        }
        let mut tools = Vec::new();
        for name in self
            .config
            .tools
            .keys()
            .chain(overrides.keys())
            .collect::<BTreeSet<_>>()
        {
            match overrides.get(name) {
                Some(requests) => {
                    let mut seen = BTreeSet::new();
                    for request in requests {
                        if seen.insert(request) {
                            tools.push(resolve_env_tool(name, request, Some(root), user_home)?);
                        }
                    }
                }
                None => {
                    if let Some(requests) = self.config.tools.get(name) {
                        tools.extend(self.resolve_versions(name, requests, user_home)?);
                    }
                }
            }
        }
        Ok(tools)
    }

    fn resolve_versions(
        &self,
        name: &str,
        requests: &[String],
        user_home: Option<&Path>,
    ) -> Result<Vec<ResolvedEnvTool>, ToolAcquireError> {
        let root = self.source_root(name, user_home)?;
        requests
            .iter()
            .map(|request| resolve_env_tool(name, request, root, user_home))
            .collect()
    }
}

impl ResolvedEnvTool {
    pub fn tool(&self) -> &ResolvedTool {
        &self.tool
    }

    pub fn origin(&self) -> &EnvToolOrigin {
        &self.origin
    }

    /// Locate the selected executable without fetching or changing store pointers.
    /// Native lookup and filesystem errors retain their existing error sources.
    pub fn which(
        &self,
        store: &ToolStore,
        inherited_path: Option<&OsStr>,
        cwd: &Path,
    ) -> Result<Option<PathBuf>, ToolAcquireError> {
        let directories = match &self.origin {
            EnvToolOrigin::Managed => {
                return store
                    .installed_for(&self.tool)?
                    .map(|receipt| {
                        store
                            .installation_paths(&receipt)
                            .map(|paths| paths.executable().to_path_buf())
                    })
                    .transpose();
            }
            EnvToolOrigin::System => store.unmanaged_path_dirs(inherited_path),
            EnvToolOrigin::Path(paths) => paths.bin_dirs.clone(),
        };
        if directories.is_empty() {
            return Ok(None);
        }
        let search = std::env::join_paths(directories).map_err(|error| {
            ToolAcquireError::io("join tool search path", cwd, std::io::Error::other(error))
        })?;
        find_env_executable(OsStr::new(&self.tool.executable), Some(&search), cwd)
    }
}

/// Find a native executable using explicit PATH and cwd, including platform
/// executable permissions and Windows PATHEXT. No shell or subprocess runs.
/// This performs blocking filesystem lookups; other lookup errors retain causes.
pub fn find_env_executable(
    program: &OsStr,
    path: Option<&OsStr>,
    cwd: &Path,
) -> Result<Option<PathBuf>, ToolAcquireError> {
    match which::which_in(program, path, cwd) {
        Ok(path) => Ok(Some(path)),
        Err(which::Error::CannotFindBinaryPath) => Ok(None),
        Err(error) => Err(ToolAcquireError::io(
            "find selected executable",
            cwd,
            std::io::Error::other(error),
        )),
    }
}

/// Load bounded ACL layers while retaining tool origins. Environment values stay
/// redacted in Debug. User-relative paths use the supplied home during resolution;
/// project-relative paths use the declaring workspace, including ancestor layers.
/// This performs blocking reads, creates no files, and returns parse/I/O failures.
pub fn load_env_config_with_sources(
    project: impl AsRef<Path>,
    aster_home: impl AsRef<Path>,
) -> Result<LoadedEnvConfig, ToolAcquireError> {
    let aster_home = std::path::absolute(aster_home.as_ref()).map_err(|error| {
        ToolAcquireError::io("resolve environment home", aster_home.as_ref(), error)
    })?;
    let mut loaded = LoadedEnvConfig {
        config: EnvConfig::default(),
        roots: BTreeMap::new(),
        sources: BTreeSet::new(),
    };
    visit_env_layers(project.as_ref(), &aster_home, |config, root| {
        loaded.sources.insert(match root {
            EnvLayerRoot::User => crate::user_env_path(&aster_home),
            EnvLayerRoot::Project(path) => crate::project_env_path(path),
        });
        for name in config.tools.keys() {
            let root = match root {
                EnvLayerRoot::User => DeclarationRoot::User,
                EnvLayerRoot::Project(path) => DeclarationRoot::Project(path.to_path_buf()),
            };
            loaded.roots.insert(name.clone(), root);
        }
        merge_into(&mut loaded.config, config);
    })?;
    Ok(loaded)
}

/// Load ordered ACL layers and retain their roots for relative and user paths.
/// Reads are bounded and blocking; no directories, receipts or pointers are written.
pub fn load_env_declarations_with_sources(
    project: impl AsRef<Path>,
    aster_home: impl AsRef<Path>,
) -> Result<LoadedEnvDeclarations, ToolAcquireError> {
    let aster_home = std::path::absolute(aster_home.as_ref()).map_err(|error| {
        ToolAcquireError::io("resolve environment home", aster_home.as_ref(), error)
    })?;
    let mut loaded = LoadedEnvDeclarations {
        config: EnvDeclarations::default(),
        roots: BTreeMap::new(),
        sources: BTreeSet::new(),
    };
    visit_declaration_layers(project.as_ref(), &aster_home, |config, root| {
        loaded.sources.insert(match root {
            EnvLayerRoot::User => crate::user_env_path(&aster_home),
            EnvLayerRoot::Project(path) => crate::project_env_path(path),
        });
        for name in config.tools.keys() {
            let root = match root {
                EnvLayerRoot::User => DeclarationRoot::User,
                EnvLayerRoot::Project(path) => DeclarationRoot::Project(path.to_path_buf()),
            };
            loaded.roots.insert(name.clone(), root);
        }
        loaded.config.tools.extend(config.tools);
        loaded.config.vars.extend(config.vars);
    })?;
    Ok(loaded)
}

/// Resolve an argument or declaration into a managed, system, or external tool.
/// `root` is the logical declaration root; `user_home` supplies `~/` expansion.
/// No network request, installation, execution, receipt or shim write occurs.
/// External roots must exist. Layout inspection is bounded and blocking.
pub fn resolve_env_tool(
    name: &str,
    raw_request: &str,
    root: Option<&Path>,
    user_home: Option<&Path>,
) -> Result<ResolvedEnvTool, ToolAcquireError> {
    let mut tool = resolve_tool_identity(name)?;
    let request = VersionRequest::parse(raw_request)?;
    tool.version = match request.kind() {
        VersionRequestKind::Latest => None,
        _ => Some(request.canonical()),
    };
    let origin = match request.kind() {
        VersionRequestKind::System => EnvToolOrigin::System,
        VersionRequestKind::Path(path) => {
            let path = if let Ok(relative) = path.strip_prefix("~") {
                user_home
                    .ok_or_else(|| {
                        ToolAcquireError::InvalidEnvAcl(
                            "cannot expand ~/ without a home directory".into(),
                        )
                    })?
                    .join(relative)
            } else if path.is_absolute() {
                path.clone()
            } else {
                root.ok_or_else(|| {
                    ToolAcquireError::InvalidEnvAcl(
                        "relative tool path has no declaration root".into(),
                    )
                })?
                .join(path)
            };
            let path = std::fs::canonicalize(&path).map_err(|error| {
                ToolAcquireError::io("resolve external tool root", &path, error)
            })?;
            let metadata = std::fs::metadata(&path).map_err(|error| {
                ToolAcquireError::io("inspect external tool root", &path, error)
            })?;
            if !metadata.is_dir() {
                return Err(ToolAcquireError::io(
                    "inspect external tool root",
                    &path,
                    std::io::Error::new(
                        std::io::ErrorKind::NotADirectory,
                        "external tool root must be a directory",
                    ),
                ));
            }
            let bin_dirs = paths::external_bin_dirs(&tool.spec, &path, &request)?;
            EnvToolOrigin::Path(ExternalToolPaths {
                root: path,
                bin_dirs,
            })
        }
        _ => EnvToolOrigin::Managed,
    };
    Ok(ResolvedEnvTool { tool, origin })
}

impl ToolStore {
    /// Remove the native install/shim roots Aster itself puts on PATH, preserving
    /// every other entry verbatim, including empty and non-UTF-8 entries.
    pub fn unmanaged_path_dirs(&self, inherited: Option<&OsStr>) -> Vec<PathBuf> {
        let shims = self.root_path().join("shims");
        let installs = self.root_path().join("installs");
        inherited
            .into_iter()
            .flat_map(std::env::split_paths)
            .filter(|path| !path.starts_with(&shims) && !path.starts_with(&installs))
            .collect()
    }
}
