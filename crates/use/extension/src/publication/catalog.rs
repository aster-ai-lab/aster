use aster_use_core::{
    CatalogArchive, CatalogAvailability, CatalogMcpTransport, CatalogPackage,
    CatalogPlanningTarget, CatalogSurface, ExecutablePlanningSurface, PackageSurfaceKind,
    PlanningSurfaceActivation, PluginCatalogRecord, PluginPermissionCeiling, PluginPlanningBundle,
    PluginReleaseChannel, PLUGIN_CATALOG_SCHEMA_V3, PLUGIN_PERMISSION_SCHEMA,
    PLUGIN_PLANNING_BUNDLE_SCHEMA,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::{invalid, PreparedAixPublication};
use crate::{ExtensionManifest, PluginMcpLaunch, SurfaceActivation};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CatalogMetadata {
    schema_version: u32,
    display_name: String,
    description: String,
    keywords: Vec<String>,
    categories: Vec<String>,
    target: String,
    channel: PluginReleaseChannel,
    license: String,
    #[serde(default)]
    permission_ceiling: Option<PluginPermissionCeiling>,
}

pub(super) fn parse(input: &[u8]) -> aster_use_core::UseResult<CatalogMetadata> {
    if input.is_empty() || input.len() > 64 * 1024 {
        return Err(invalid(
            "Catalog metadata must contain between 1 byte and 64 KiB.",
        ));
    }
    let metadata: CatalogMetadata = serde_json::from_slice(input)
        .map_err(|error| invalid(format!("Invalid catalog metadata: {error}")))?;
    if metadata.schema_version != 1 {
        return Err(invalid("Catalog metadata requires schemaVersion 1."));
    }
    Ok(metadata)
}

pub(super) async fn prepare(
    publication: &PreparedAixPublication,
    manifest: &ExtensionManifest,
    manifest_bytes: &[u8],
    metadata: CatalogMetadata,
) -> aster_use_core::UseResult<(PluginCatalogRecord, Vec<u8>)> {
    if !metadata.categories.contains(&publication.domain) {
        return Err(invalid(
            "Catalog metadata must include the admitted domain category.",
        ));
    }
    let mut surfaces = Vec::new();
    for surface in manifest.plugin_surfaces()? {
        if !matches!(
            surface.surface.kind,
            PackageSurfaceKind::Plugin | PackageSurfaceKind::Skill | PackageSurfaceKind::Mcp
        ) {
            return Err(invalid(
                "This native publisher supports runtime data, Skills and stdio MCP surfaces.",
            ));
        }
        surfaces.push(CatalogSurface {
            kind: surface.surface.kind,
            id: surface.surface.id,
            optional: surface.optional,
            workload: None,
            mcp_transport: (surface.surface.kind == PackageSurfaceKind::Mcp)
                .then_some(CatalogMcpTransport::Stdio),
            mcp_tool_count: None,
            okf_bundle: None,
            requires: surface.dependencies,
        });
    }
    let permission_ceiling = metadata
        .permission_ceiling
        .unwrap_or(PluginPermissionCeiling {
            schema: PLUGIN_PERMISSION_SCHEMA.into(),
            surfaces: Vec::new(),
        });
    let manifest_sha256 = format!("sha256:{:x}", Sha256::digest(manifest_bytes));
    let name = manifest
        .package_id
        .rsplit('/')
        .next()
        .ok_or_else(|| invalid("Missing package name."))?;
    let prefix = format!(
        "extensions/{}/{}/{}/{}/",
        manifest.package_id,
        manifest.version,
        metadata.channel.as_str(),
        metadata.target
    );
    let mut record = PluginCatalogRecord {
        schema: PLUGIN_CATALOG_SCHEMA_V3.into(),
        package_id: manifest.package_id.clone(),
        display_name: metadata.display_name,
        description: metadata.description,
        publisher: manifest
            .package_id
            .split('/')
            .next()
            .ok_or_else(|| invalid("Missing publisher."))?
            .into(),
        keywords: metadata.keywords,
        categories: metadata.categories,
        version: manifest.version.clone(),
        channel: metadata.channel,
        requires_use: manifest
            .requires_use
            .clone()
            .ok_or_else(|| invalid("Published plugins must declare requires_use."))?,
        dependencies: manifest.dependencies.clone(),
        target: metadata.target,
        surfaces,
        permission_ceiling_digest: permission_ceiling.descriptor_digest()?,
        permission_ceiling,
        planning: None,
        archive: CatalogArchive {
            target_name: format!(
                "{prefix}oci-ghcr-v1/{}/{name}.aix",
                publication.manifest_sha256.trim_start_matches("sha256:")
            ),
            length: publication.archive_length,
            sha256: publication.archive_sha256.clone(),
        },
        package: CatalogPackage {
            expanded_bytes: publication.package.byte_count,
            file_count: publication.package.file_count,
            sha256: Some(format!("sha256:{}", publication.package.package_sha256)),
            manifest_sha256: Some(manifest_sha256.clone()),
        },
        license: metadata.license,
        repository: crate::remote::oci::REPOSITORY.into(),
        availability: CatalogAvailability::Available,
    };
    let mut executable = Vec::new();
    let mut mcp_servers = manifest.mcp_servers.iter().collect::<Vec<_>>();
    mcp_servers.sort_by(|left, right| left.id.cmp(&right.id));
    for mcp in mcp_servers {
        let PluginMcpLaunch::Stdio {
            executable: path,
            args,
        } = &mcp.launch
        else {
            return Err(invalid(
                "This native publisher requires a package-local stdio MCP executable.",
            ));
        };
        executable.push(ExecutablePlanningSurface::McpStdio {
            id: mcp.id.clone(),
            activation: match mcp.activation {
                SurfaceActivation::Eager => PlanningSurfaceActivation::Eager,
                SurfaceActivation::Lazy => PlanningSurfaceActivation::Lazy,
            },
            executable: path
                .to_str()
                .ok_or_else(|| invalid("Planning executable paths must be UTF-8."))?
                .into(),
            args: args.clone(),
        });
    }
    let planning_bytes = if executable.is_empty() {
        Vec::new()
    } else {
        let bundle = PluginPlanningBundle {
            schema: PLUGIN_PLANNING_BUNDLE_SCHEMA.into(),
            package_id: record.package_id.clone(),
            version: record.version.clone(),
            channel: record.channel,
            target: record.target.clone(),
            archive_sha256: record.archive.sha256.clone(),
            package_sha256: format!("sha256:{}", publication.package.package_sha256),
            manifest_sha256,
            permission_ceiling_digest: record.permission_ceiling_digest.clone(),
            surfaces: executable,
        };
        crate::surface_files::validate_planning_bundle_package_binding(
            &bundle,
            manifest,
            &publication.source_path,
        )
        .await?;
        let bytes = bundle.canonical_bytes()?;
        record.planning = Some(CatalogPlanningTarget {
            target_name: format!("{prefix}planning-v1.json"),
            length: bytes.len() as u64,
            sha256: format!("sha256:{:x}", Sha256::digest(&bytes)),
        });
        bytes
    };
    crate::validate_catalog_manifest_binding(&record, manifest)?;
    Ok((record, planning_bytes))
}
