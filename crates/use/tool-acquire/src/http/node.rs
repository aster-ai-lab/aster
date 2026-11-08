//! Official Node release metadata, signed checksums and complete native layouts.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::version::{compare_versions, semver_prerelease};
use crate::{
    ArchiveFormat, ArchivePayload, InstallPayload, ToolAcquireError, ToolQuery, VersionListing,
    VersionRequest,
};

use super::HttpSource;

mod verify;
pub use verify::NodeSignatureError;
pub(super) const DEFAULT_DIST: &str = "https://nodejs.org/dist";

impl HttpSource {
    pub(super) fn node_versions(&self) -> Result<VersionListing, ToolAcquireError> {
        let metadata = self.get_json(&format!(
            "{}/index.json",
            self.node_dist.trim_end_matches('/')
        ))?;
        let releases = metadata
            .as_array()
            .ok_or(ToolAcquireError::DownloadRejected)?;
        let mut versions = Vec::with_capacity(releases.len().min(crate::version::MAX_VERSIONS));
        let mut aliases = BTreeMap::new();
        let mut latest_lts: Option<String> = None;
        if releases.len() > crate::version::MAX_VERSIONS {
            return Err(ToolAcquireError::VersionListLimit);
        }
        for release in releases {
            let raw = release
                .get("version")
                .and_then(|value| value.as_str())
                .ok_or(ToolAcquireError::DownloadRejected)?;
            let version = node_version(raw.strip_prefix('v').unwrap_or(raw))?;
            versions.push((version.to_string(), semver_prerelease(version)));
            match release.get("lts") {
                None | Some(serde_json::Value::Bool(false)) => {}
                Some(serde_json::Value::String(name)) if !name.is_empty() => {
                    let major = version
                        .split('.')
                        .next()
                        .ok_or(ToolAcquireError::DownloadRejected)?;
                    for alias in [
                        format!("lts/{}", name.to_ascii_lowercase()),
                        format!("lts-{}", name.to_ascii_lowercase()),
                    ] {
                        if aliases
                            .insert(alias, major.to_string())
                            .is_some_and(|old| old != major)
                        {
                            return Err(ToolAcquireError::DownloadRejected);
                        }
                    }
                    if latest_lts
                        .as_ref()
                        .is_none_or(|old| compare_versions(old, version).is_lt())
                    {
                        latest_lts = Some(version.into());
                    }
                }
                _ => return Err(ToolAcquireError::DownloadRejected),
            }
        }
        if let Some(lts) = latest_lts {
            aliases.insert(
                "lts".into(),
                lts.split('.')
                    .next()
                    .ok_or(ToolAcquireError::DownloadRejected)?
                    .to_string(),
            );
            aliases.insert("lts/*".into(), "lts".into());
        }
        VersionListing::new(versions, aliases)
    }

    pub(super) fn fetch_node(&self, query: &ToolQuery) -> Result<InstallPayload, ToolAcquireError> {
        if query.executable != "node" {
            return Err(ToolAcquireError::InvalidName(query.executable.clone()));
        }
        let wanted = query.version.as_deref().map(node_version).transpose()?;
        let target = NodeTarget::current()?;
        let version = match wanted {
            Some(version) => version.to_string(),
            None => self
                .node_versions()?
                .resolve(&VersionRequest::parse("latest")?)?,
        };
        let stem = format!("node-v{version}-{}-{}", target.os, target.arch);
        let filename = format!("{stem}.{}", if target.windows { "zip" } else { "tar.gz" });
        let base = format!("{}/v{version}", self.node_dist.trim_end_matches('/'));
        let checksums = self.download(&format!("{base}/SHASUMS256.txt"))?;
        let signature = self.download(&format!("{base}/SHASUMS256.txt.sig"))?;
        verify::verify_node(&checksums, &signature)?;
        let checksum = checksum_for(&checksums, &filename)?;
        let bytes = self.download(&format!("{base}/{filename}"))?;
        let bin = if target.windows {
            PathBuf::new()
        } else {
            PathBuf::from("bin")
        };
        let command = |name: &str| {
            bin.join(if target.windows {
                if name == "node" {
                    "node.exe".to_string()
                } else {
                    format!("{name}.cmd")
                }
            } else {
                name.to_string()
            })
        };
        ArchivePayload::new(
            version,
            &checksum,
            bytes,
            if target.windows {
                ArchiveFormat::Zip
            } else {
                ArchiveFormat::TarGz
            },
            stem.into(),
            BTreeMap::from([
                ("node".into(), command("node")),
                ("npm".into(), command("npm")),
            ]),
            vec![bin.clone()],
        )?
        .with_optional_entrypoints(BTreeMap::from([
            ("npx".into(), command("npx")),
            ("corepack".into(), command("corepack")),
        ]))
        .map(InstallPayload::Archive)
    }
}

struct NodeTarget {
    os: &'static str,
    arch: &'static str,
    windows: bool,
}

impl NodeTarget {
    fn current() -> Result<Self, ToolAcquireError> {
        let unsupported = || ToolAcquireError::UnsupportedRuntimeTarget {
            runtime: "node",
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            abi: if cfg!(target_env = "musl") {
                "musl"
            } else if cfg!(target_env = "msvc") {
                "msvc"
            } else {
                ""
            },
        };
        let windows = cfg!(windows);
        let os = match std::env::consts::OS {
            "macos" => "darwin",
            "windows" => "win",
            "linux" if cfg!(target_env = "gnu") => "linux",
            _ => return Err(unsupported()),
        };
        let arch = match std::env::consts::ARCH {
            "x86_64" => "x64",
            "aarch64" => "arm64",
            "x86" if windows => "x86",
            _ => return Err(unsupported()),
        };
        Ok(Self { os, arch, windows })
    }
}

fn node_version(version: &str) -> Result<&str, ToolAcquireError> {
    match versions::Versioning::new(version) {
        Some(versions::Versioning::Ideal(_)) => Ok(version),
        _ => Err(ToolAcquireError::InvalidVersion(version.into())),
    }
}

fn checksum_for(data: &[u8], wanted: &str) -> Result<String, ToolAcquireError> {
    let mut checksum = None;
    for line in data.split(|byte| *byte == b'\n') {
        let mut fields = line
            .split(u8::is_ascii_whitespace)
            .filter(|field| !field.is_empty());
        let hash = fields.next();
        if fields.next() != Some(wanted.as_bytes()) {
            continue;
        }
        let hash = hash
            .filter(|hash| hash.len() == 64 && hash.iter().all(u8::is_ascii_hexdigit))
            .ok_or(ToolAcquireError::ChecksumMismatch)?;
        if fields.next().is_some() || checksum.is_some() {
            return Err(ToolAcquireError::ChecksumMismatch);
        }
        checksum = Some(
            hash.iter()
                .map(|byte| byte.to_ascii_lowercase() as char)
                .collect(),
        );
    }
    checksum.ok_or(ToolAcquireError::ChecksumRequired)
}

#[cfg(test)]
mod tests;
