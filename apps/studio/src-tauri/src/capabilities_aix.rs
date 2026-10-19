//! Project declared AIX surfaces into owned, verified package generations.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Cursor, Read};
use std::path::{Path, PathBuf};

use super::{PackageFile, PackageSpec, SurfaceKind, surface_files};
use a3s_use_extension::ExtensionManifest;
use a3s_use_reconcile::{Publication, PublishedSurface, ReconcileStore};
use zip::ZipArchive;

const MANIFEST: &str = "a3s-use-extension.acl";
const MANIFEST_MAX: u64 = 1024 * 1024;

fn validate_manifest(manifest: &ExtensionManifest, package_id: &str) -> io::Result<()> {
    if manifest.schema_version != 3 || package_id != format!("use/{}", manifest.package_id) {
        return Err(invalid(
            "AIX manifest does not match its installation identity",
        ));
    }
    manifest
        .require_current_use()
        .map_err(|error| invalid(error.message))?;
    if !manifest.tools.is_empty() || !manifest.flows.is_empty() {
        return Err(invalid(
            "this AIX host adapter currently supports Skill, UI, MCP, Agent, and OKF surfaces",
        ));
    }
    Ok(())
}

pub(super) fn read_manifest(
    surface: &PublishedSurface,
    package_id: &str,
) -> io::Result<Option<ExtensionManifest>> {
    let Some(bytes) = surface
        .read_verified_file(MANIFEST)
        .map_err(super::host_skill_error)?
    else {
        return Ok(None);
    };
    if bytes.len() as u64 > MANIFEST_MAX {
        return Err(invalid("AIX manifest exceeds 1 MiB"));
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| invalid("AIX manifest is not UTF-8"))?;
    let manifest = ExtensionManifest::parse_acl(text).map_err(|error| invalid(error.message))?;
    validate_manifest(&manifest, package_id)?;
    Ok(Some(manifest))
}

pub(super) fn validate_publication(publication: &Publication, require_aix: bool) -> io::Result<()> {
    publication_manifest(publication, require_aix).map(|_| ())
}

/// Validate the exact selected package set against every declared AIX dependency.
/// Call this both before publishing a new selection and when reading one back.
pub(super) fn validate_selection<'a>(
    publications: impl IntoIterator<Item = &'a Publication>,
) -> io::Result<()> {
    let publications: Vec<_> = publications.into_iter().collect();
    let mut manifests = BTreeMap::new();
    for publication in &publications {
        // A stale or damaged package must not withdraw an unrelated healthy
        // sibling. Its own contribution is refused by the owning consumer;
        // a selected package that depends on it will fail resolution below.
        if let Ok(Some(manifest)) = publication_manifest(publication, false) {
            manifests.insert(publication.package_id.clone(), manifest);
        }
    }

    validate_dependency_manifests(&manifests)
}

pub(super) fn preflight_dependencies(
    root: &Path,
    agents: &Path,
    pins: &BTreeMap<String, super::PluginPin>,
    bundle: &impl Fn(&str) -> io::Result<Vec<u8>>,
) -> io::Result<()> {
    let mut manifests = BTreeMap::new();
    let plugins = root.join("plugins");
    let generation_root = agents.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "package generation root is unavailable",
        )
    })?;
    let store = ReconcileStore::open(generation_root.join("package-generations"))
        .map_err(super::host_skill_error)?;
    for (slug, pin) in pins {
        let target = plugins.join(slug);
        if super::plugin_marker_matches(&target, pin) {
            if let Some(publication) = store
                .current_sync(&pin.package_id)
                .map_err(super::host_skill_error)?
            {
                if let Ok(Some(manifest)) = publication_manifest(&publication, pin.require_aix) {
                    manifests.insert(pin.package_id.clone(), manifest);
                }
                continue;
            }
        }
        let bytes = bundle(slug)?;
        if super::digest_of(&bytes) != pin.digest {
            return Err(invalid("AIX dependency preflight bundle digest mismatch"));
        }
        let mut archive = ZipArchive::new(Cursor::new(bytes))
            .map_err(|_| invalid("plugin archive is invalid"))?;
        let manifest_entries = archive
            .file_names()
            .filter(|path| *path == MANIFEST)
            .count();
        if manifest_entries == 0 {
            if pin.require_aix {
                return Err(invalid("AIX installation requires its native manifest"));
            }
            continue;
        }
        if manifest_entries != 1 {
            return Err(invalid("AIX archive contains duplicate manifests"));
        }
        let file = archive
            .by_name(MANIFEST)
            .map_err(|_| invalid("AIX manifest is unreadable"))?;
        if file.size() > MANIFEST_MAX {
            return Err(invalid("AIX manifest exceeds 1 MiB"));
        }
        let mut manifest_bytes = Vec::with_capacity(file.size() as usize);
        file.take(MANIFEST_MAX + 1)
            .read_to_end(&mut manifest_bytes)
            .map_err(|_| invalid("AIX manifest is unreadable"))?;
        let text = std::str::from_utf8(&manifest_bytes)
            .map_err(|_| invalid("AIX manifest is not UTF-8"))?;
        let manifest =
            ExtensionManifest::parse_acl(text).map_err(|error| invalid(error.message))?;
        validate_manifest(&manifest, &pin.package_id)?;
        manifests.insert(pin.package_id.clone(), manifest);
    }
    validate_dependency_manifests(&manifests)
}

fn validate_dependency_manifests(
    manifests: &BTreeMap<String, ExtensionManifest>,
) -> io::Result<()> {
    let mut graph: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (package_id, manifest) in manifests {
        let mut dependencies = Vec::with_capacity(manifest.dependencies.len());
        for dependency in &manifest.dependencies {
            let dependency_id = format!("use/{}", dependency.package_id);
            let resolved = manifests.get(&dependency_id).ok_or_else(|| {
                invalid("AIX dependency is not present in the selected package pins")
            })?;
            if !dependency
                .matches(&resolved.version)
                .map_err(|error| invalid(error.message))?
            {
                return Err(invalid(
                    "selected AIX dependency does not satisfy its version requirement",
                ));
            }
            dependencies.push(dependency_id);
        }
        graph.insert(package_id.clone(), dependencies);
    }

    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for package_id in graph.keys() {
        visit_dependency(package_id, &graph, &mut visiting, &mut visited)?;
    }
    Ok(())
}

fn visit_dependency(
    package_id: &str,
    graph: &BTreeMap<String, Vec<String>>,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
) -> io::Result<()> {
    if visited.contains(package_id) {
        return Ok(());
    }
    if !visiting.insert(package_id.to_owned()) {
        return Err(invalid("AIX package dependency cycle is not allowed"));
    }
    if let Some(dependencies) = graph.get(package_id) {
        for dependency in dependencies {
            visit_dependency(dependency, graph, visiting, visited)?;
        }
    }
    visiting.remove(package_id);
    visited.insert(package_id.to_owned());
    Ok(())
}

pub(super) fn publication_manifest(
    publication: &Publication,
    require_aix: bool,
) -> io::Result<Option<ExtensionManifest>> {
    let mut canonical = None;
    let mut missing = false;
    for surface in &publication.surfaces {
        match read_manifest(surface, &publication.package_id)? {
            Some(manifest) => {
                if canonical.as_ref().is_some_and(|first| first != &manifest) {
                    return Err(invalid("AIX surface manifests disagree"));
                }
                canonical = Some(manifest);
            }
            None => missing = true,
        }
    }
    if (require_aix && canonical.is_none()) || (canonical.is_some() && missing) {
        return Err(invalid(
            "AIX publication is missing its canonical surface manifest",
        ));
    }
    if let Some(manifest) = &canonical {
        let declared: BTreeSet<_> = manifest
            .skills
            .iter()
            .map(|skill| (SurfaceKind::Skill, skill.id.as_str()))
            .chain(
                manifest
                    .ui
                    .iter()
                    .map(|ui| (SurfaceKind::Ui, ui.id.as_str())),
            )
            .chain(
                manifest
                    .plugins
                    .iter()
                    .map(|plugin| (SurfaceKind::Plugin, plugin.id.as_str())),
            )
            .chain(
                manifest
                    .mcp_servers
                    .iter()
                    .map(|server| (SurfaceKind::Mcp, server.id.as_str())),
            )
            .chain(
                manifest
                    .agents
                    .iter()
                    .map(|agent| (SurfaceKind::Agent, agent.id.as_str())),
            )
            .chain(
                manifest
                    .okf
                    .iter()
                    .map(|okf| (SurfaceKind::Okf, okf.id.as_str())),
            )
            .collect();
        let published: BTreeSet<_> = publication
            .surfaces
            .iter()
            .map(|surface| (surface.kind, surface.id.as_str()))
            .collect();
        if declared != published {
            return Err(invalid(
                "AIX publication surfaces differ from its declaration",
            ));
        }
    }
    Ok(canonical)
}

pub(super) fn package_spec(
    files: &[(String, Vec<u8>)],
    package_id: &str,
) -> io::Result<PackageSpec> {
    let inventory: BTreeSet<PathBuf> = files.iter().map(|(path, _)| PathBuf::from(path)).collect();
    let files: BTreeMap<_, _> = files
        .iter()
        .map(|(path, bytes)| (path.as_str(), bytes))
        .collect();
    let manifest_bytes = files
        .get(MANIFEST)
        .ok_or_else(|| invalid("AIX manifest is missing"))?;
    if manifest_bytes.len() as u64 > MANIFEST_MAX {
        return Err(invalid("AIX manifest exceeds 1 MiB"));
    }
    let text =
        std::str::from_utf8(manifest_bytes).map_err(|_| invalid("AIX manifest is not UTF-8"))?;
    let manifest = ExtensionManifest::parse_acl(text).map_err(|e| invalid(e.message))?;
    validate_manifest(&manifest, package_id)?;
    manifest
        .validate_file_inventory(&inventory)
        .map_err(|e| invalid(e.message))?;
    let mut surfaces = Vec::new();
    for skill in &manifest.skills {
        let parent = skill
            .path
            .parent()
            .ok_or_else(|| invalid("AIX Skill has no directory"))?;
        let mut payload: Vec<PackageFile> = files
            .iter()
            .filter_map(|(path, bytes)| {
                Path::new(path)
                    .strip_prefix(parent)
                    .ok()
                    .map(|relative| PackageFile {
                        path: relative.to_string_lossy().into_owned(),
                        bytes: (*bytes).clone(),
                    })
            })
            .collect();
        for path in [MANIFEST, "README.md"] {
            let bytes = files
                .get(path)
                .ok_or_else(|| invalid("AIX package metadata is missing"))?;
            if !payload.iter().any(|file| file.path == path) {
                payload.push(PackageFile {
                    path: path.to_owned(),
                    bytes: (*bytes).clone(),
                });
            }
        }
        surfaces.push(surface_files(
            SurfaceKind::Skill,
            skill.id.clone(),
            None,
            payload,
        )?);
    }
    for ui in &manifest.ui {
        let mut selected: BTreeSet<PathBuf> = [ui.entry.clone()].into_iter().collect();
        selected.extend(ui.styles.iter().cloned());
        selected.extend(ui.scripts.iter().cloned());
        if let Some(root) = &ui.root {
            selected.extend(
                inventory
                    .iter()
                    .filter(|path| path.starts_with(root))
                    .cloned(),
            );
        }
        // Keep the native declaration under the same digest and lease as its UI assets.
        // Later workbench queries must read these facts from the verified generation.
        selected.insert(PathBuf::from(MANIFEST));
        selected.insert(PathBuf::from("README.md"));
        let payload = selected
            .into_iter()
            .map(|path| {
                let path = path.to_string_lossy().into_owned();
                let bytes = files
                    .get(path.as_str())
                    .ok_or_else(|| invalid("AIX UI asset is missing"))?;
                Ok(PackageFile {
                    path,
                    bytes: (*bytes).clone(),
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        surfaces.push(surface_files(
            SurfaceKind::Ui,
            ui.id.clone(),
            Some(ui.entry.to_string_lossy().into_owned()),
            payload,
        )?);
    }
    for server in &manifest.mcp_servers {
        let a3s_use_extension::PluginMcpLaunch::Catalog { server: path } = &server.launch else {
            return Err(invalid("this host installs catalog MCP packages"));
        };
        let relative = path.to_string_lossy().into_owned();
        let bytes = files
            .get(relative.as_str())
            .ok_or_else(|| invalid("AIX MCP server.json is missing"))?;
        let mut payload = vec![PackageFile {
            path: relative,
            bytes: (*bytes).clone(),
        }];
        payload = with_package_metadata(&files, payload)?;
        surfaces.push(surface_files(
            SurfaceKind::Mcp,
            server.id.clone(),
            None,
            payload,
        )?);
    }
    for knowledge in &manifest.okf {
        let root = Path::new(&knowledge.bundle.root);
        let mut payload: Vec<PackageFile> = files
            .iter()
            .filter_map(|(path, bytes)| {
                if Path::new(path).starts_with(root) {
                    Some(PackageFile {
                        path: (*path).to_owned(),
                        bytes: (*bytes).clone(),
                    })
                } else {
                    None
                }
            })
            .collect();
        if payload.is_empty() {
            return Err(invalid("AIX OKF bundle is missing"));
        }
        payload = with_package_metadata(&files, payload)?;
        surfaces.push(surface_files(
            SurfaceKind::Okf,
            knowledge.id.clone(),
            None,
            payload,
        )?);
    }
    for agent in &manifest.agents {
        // The entry's directory is the program. A sibling the entry imports
        // is part of this surface, same as a skill directory.
        let mut selected: BTreeSet<PathBuf> = BTreeSet::new();
        for path in agent.instructions.iter().chain(agent.entry.iter()) {
            selected.insert(path.clone());
        }
        if let Some(parent) = agent
            .entry
            .as_ref()
            .and_then(|entry| entry.parent())
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            selected.extend(
                inventory
                    .iter()
                    .filter(|path| path.starts_with(parent))
                    .cloned(),
            );
        }
        if let Some(path) = &agent.parameters {
            let relative = path.to_string_lossy().into_owned();
            let schema = files
                .get(relative.as_str())
                .ok_or_else(|| invalid("AIX agent parameters are missing"))?;
            a3s_use_extension::parse_agent_parameter_schema(schema)
                .map_err(|_| invalid("AIX agent parameters are invalid"))?;
            selected.insert(path.clone());
        }
        if selected.is_empty() {
            return Err(invalid("AIX agent files are missing"));
        }
        let payload = selected
            .into_iter()
            .map(|path| {
                let path = path.to_string_lossy().into_owned();
                let bytes = files
                    .get(path.as_str())
                    .ok_or_else(|| invalid("AIX agent file is missing"))?;
                Ok(PackageFile {
                    path,
                    bytes: (*bytes).clone(),
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        let payload = with_package_metadata(&files, payload)?;
        let entry = agent
            .entry
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned());
        surfaces.push(surface_files(
            SurfaceKind::Agent,
            agent.id.clone(),
            entry,
            payload,
        )?);
    }
    for plugin in &manifest.plugins {
        let mut selected = BTreeSet::new();
        selected.insert(plugin.entry.clone());
        selected.insert(PathBuf::from(MANIFEST));
        selected.insert(PathBuf::from("README.md"));
        let payload = selected
            .into_iter()
            .map(|path| {
                let path = path.to_string_lossy().into_owned();
                let bytes = files
                    .get(path.as_str())
                    .ok_or_else(|| invalid("AIX plugin entry is missing"))?;
                Ok(PackageFile {
                    path,
                    bytes: (*bytes).clone(),
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        surfaces.push(surface_files(
            SurfaceKind::Plugin,
            plugin.id.clone(),
            Some(plugin.entry.to_string_lossy().into_owned()),
            payload,
        )?);
    }
    if surfaces.is_empty() {
        return Err(invalid("AIX package has no supported surfaces"));
    }
    Ok(PackageSpec {
        package_id: package_id.to_owned(),
        surfaces,
    })
}

fn with_package_metadata(
    files: &BTreeMap<&str, &Vec<u8>>,
    mut payload: Vec<PackageFile>,
) -> io::Result<Vec<PackageFile>> {
    for path in [MANIFEST, "README.md"] {
        let bytes = files
            .get(path)
            .ok_or_else(|| invalid("AIX package metadata is missing"))?;
        if !payload.iter().any(|file| file.path == path) {
            payload.push(PackageFile {
                path: path.to_owned(),
                bytes: (*bytes).clone(),
            });
        }
    }
    Ok(payload)
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

#[cfg(test)]
#[path = "capabilities_aix_tests.rs"]
mod tests;
