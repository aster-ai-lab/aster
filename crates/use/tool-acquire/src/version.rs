//! Admitted version requests and version-list selection.
//!
//! Matching and subtraction are adapted from mise v2026.10.25 (MIT).
//! See `licenses/mise-MIT.txt` and the pinned sources in the feature research.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use versions::Versioning;

use crate::ToolAcquireError;

const MAX_REQUEST_BYTES: usize = 1024;
const MAX_RESOLUTION_DEPTH: usize = 32;
pub(crate) const MAX_VERSIONS: usize = 10_000;

/// A validated request. Filesystem identity is validated separately after resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionRequest {
    kind: VersionRequestKind,
}

/// The syntax of an admitted request; values are read through [`VersionRequest::kind`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionRequestKind {
    Version(String),
    Latest,
    Prefix(String),
    Reference {
        kind: ReferenceKind,
        value: String,
    },
    Subtract {
        amount: Vec<u32>,
        base: Box<VersionRequest>,
    },
    Path(PathBuf),
    System,
}

/// Reference spelling admitted by mise's request grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceKind {
    Ref,
    Tag,
    Branch,
    Revision,
}

impl VersionRequest {
    /// Parse bounded request syntax without filesystem or network access.
    pub fn parse(raw: &str) -> Result<Self, ToolAcquireError> {
        parse_request(raw, 0)
    }

    pub fn kind(&self) -> &VersionRequestKind {
        &self.kind
    }

    /// A stable request spelling, including normalized legacy reference prefixes.
    pub fn canonical(&self) -> String {
        match &self.kind {
            VersionRequestKind::Version(value) => value.clone(),
            VersionRequestKind::Latest => "latest".into(),
            VersionRequestKind::Prefix(value) => format!("prefix:{value}"),
            VersionRequestKind::Reference { kind, value } => {
                let prefix = match kind {
                    ReferenceKind::Ref => "ref",
                    ReferenceKind::Tag => "tag",
                    ReferenceKind::Branch => "branch",
                    ReferenceKind::Revision => "rev",
                };
                format!("{prefix}:{value}")
            }
            VersionRequestKind::Subtract { amount, base } => {
                format!("sub-{}:{}", format_components(amount), base.canonical())
            }
            VersionRequestKind::Path(path) => {
                // The private constructor builds this path only from UTF-8 request text.
                format!(
                    "path:{}",
                    path.to_str().expect("path request originated as UTF-8")
                )
            }
            VersionRequestKind::System => "system".into(),
        }
    }

    /// Match a concrete version without resolving aliases or subtraction bases.
    /// `None` means the request needs backend or context information.
    /// Latest imposes no textual constraint after resolution; this does not check freshness.
    pub fn matches_version(&self, version: &str) -> Option<bool> {
        match &self.kind {
            // A sealed latest selection may be a named or rolling release, not just a number.
            VersionRequestKind::Latest => Some(true),
            VersionRequestKind::Version(query) | VersionRequestKind::Prefix(query) => {
                Some(fuzzy_matches(query, version))
            }
            _ => None,
        }
    }
}

/// A bounded, sorted provider listing. Prerelease flags come from source metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VersionListing {
    versions: Vec<VersionEntry>,
    aliases: BTreeMap<String, String>,
}

/// One version and its provider's prerelease classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionEntry {
    version: String,
    prerelease: bool,
}

impl VersionEntry {
    pub fn version(&self) -> &str {
        &self.version
    }
    pub fn is_prerelease(&self) -> bool {
        self.prerelease
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct VersionListingData {
    versions: Vec<VersionEntry>,
    aliases: BTreeMap<String, String>,
}

impl VersionListing {
    /// Admit provider metadata, rejecting duplicates, invalid labels and oversized lists.
    pub fn new(
        versions: Vec<(String, bool)>,
        aliases: BTreeMap<String, String>,
    ) -> Result<Self, ToolAcquireError> {
        if versions.len() > MAX_VERSIONS || aliases.len() > MAX_VERSIONS {
            return Err(ToolAcquireError::VersionListLimit);
        }
        let mut unique = BTreeMap::new();
        for (version, prerelease) in versions {
            validate_label(&version)?;
            if unique.insert(version.clone(), prerelease).is_some() {
                return Err(ToolAcquireError::InvalidVersion(version));
            }
        }
        for (alias, target) in &aliases {
            validate_label(alias)?;
            VersionRequest::parse(target)?;
        }
        let mut versions: Vec<_> = unique
            .into_iter()
            .map(|(version, prerelease)| VersionEntry {
                version,
                prerelease,
            })
            .collect();
        versions.sort_by(|a, b| compare_versions(&a.version, &b.version));
        Ok(Self { versions, aliases })
    }

    pub fn versions(&self) -> &[VersionEntry] {
        &self.versions
    }
    pub fn aliases(&self) -> &BTreeMap<String, String> {
        &self.aliases
    }

    /// Select with stable defaults. An explicitly named prerelease remains selectable.
    pub fn resolve(&self, request: &VersionRequest) -> Result<String, ToolAcquireError> {
        let effective = self.expand(request, &mut Vec::new(), 0)?;
        self.select(&effective)
    }

    /// Expand declared aliases and subtraction using this listing, retaining ordinary prefixes.
    pub fn effective_request(
        &self,
        request: &VersionRequest,
    ) -> Result<VersionRequest, ToolAcquireError> {
        self.expand(request, &mut Vec::new(), 0)
    }

    pub(crate) fn select_installed(
        &self,
        effective: &VersionRequest,
    ) -> Result<String, ToolAcquireError> {
        if matches!(effective.kind(), VersionRequestKind::Latest) {
            return self
                .versions
                .iter()
                .rev()
                .find(|entry| !entry.prerelease)
                .or_else(|| self.versions.last())
                .map(|entry| entry.version.clone())
                .ok_or_else(|| ToolAcquireError::VersionNotFound(effective.canonical()));
        }
        self.select(effective)
    }

    pub(crate) fn from_data(data: VersionListingData) -> Result<Self, ToolAcquireError> {
        Self::new(
            data.versions
                .into_iter()
                .map(|entry| (entry.version, entry.prerelease))
                .collect(),
            data.aliases,
        )
    }

    fn expand(
        &self,
        request: &VersionRequest,
        stack: &mut Vec<String>,
        depth: usize,
    ) -> Result<VersionRequest, ToolAcquireError> {
        if depth > MAX_RESOLUTION_DEPTH {
            return Err(ToolAcquireError::VersionResolutionLimit);
        }
        let alias = match request.kind() {
            VersionRequestKind::Version(value) => Some(value.as_str()),
            VersionRequestKind::Latest => Some("latest"),
            _ => None,
        };
        if let Some((name, target)) =
            alias.and_then(|name| self.aliases.get(name).map(|target| (name, target)))
        {
            if stack.iter().any(|seen| seen == name) {
                return Err(ToolAcquireError::VersionAliasCycle { name: name.into() });
            }
            stack.push(name.into());
            let result = self.expand(&VersionRequest::parse(target)?, stack, depth + 1);
            stack.pop();
            return result;
        }
        if let VersionRequestKind::Subtract { amount, base } = request.kind() {
            let base = self.expand(base, stack, depth + 1)?;
            let concrete = self.select(&base)?;
            let prefix = subtract(&concrete, amount)?;
            return self.expand(
                &VersionRequest {
                    kind: VersionRequestKind::Version(prefix),
                },
                stack,
                depth + 1,
            );
        }
        Ok(request.clone())
    }

    fn select(&self, request: &VersionRequest) -> Result<String, ToolAcquireError> {
        let (query, prefer_exact) = match request.kind() {
            VersionRequestKind::Version(value) => (value.as_str(), true),
            VersionRequestKind::Prefix(value) => (value.as_str(), false),
            VersionRequestKind::Latest => ("latest", false),
            _ => {
                return Err(ToolAcquireError::UnsupportedVersionRequest(
                    request.canonical(),
                ))
            }
        };
        if prefer_exact && self.versions.iter().any(|entry| entry.version == query) {
            return Ok(query.into());
        }
        self.versions
            .iter()
            .rev()
            .find(|entry| {
                (entry.version == query || !entry.prerelease)
                    && fuzzy_matches(query, &entry.version)
            })
            .map(|entry| entry.version.clone())
            .ok_or_else(|| ToolAcquireError::VersionNotFound(request.canonical()))
    }
}

fn parse_request(raw: &str, depth: usize) -> Result<VersionRequest, ToolAcquireError> {
    if depth > MAX_RESOLUTION_DEPTH {
        return Err(ToolAcquireError::VersionResolutionLimit);
    }
    let raw = if raw.trim_start().starts_with("path:") {
        raw.trim_start()
    } else {
        raw.trim()
    };
    if raw.is_empty() || raw.len() > MAX_REQUEST_BYTES || raw.contains('\0') {
        return Err(ToolAcquireError::InvalidVersion(raw.into()));
    }
    let reference = |kind, value: &str| -> Result<VersionRequestKind, ToolAcquireError> {
        validate_reference(value)?;
        Ok(VersionRequestKind::Reference {
            kind,
            value: value.into(),
        })
    };
    let kind = if raw.eq_ignore_ascii_case("latest") {
        VersionRequestKind::Latest
    } else if raw == "system" {
        VersionRequestKind::System
    } else if let Some((prefix, rest)) = raw.split_once(':') {
        match prefix {
            "prefix" => {
                validate_label(rest)?;
                VersionRequestKind::Prefix(rest.into())
            }
            "path" => VersionRequestKind::Path(Path::new(rest).to_path_buf()),
            "ref" => reference(ReferenceKind::Ref, rest)?,
            "tag" => reference(ReferenceKind::Tag, rest)?,
            "branch" => reference(ReferenceKind::Branch, rest)?,
            "rev" => reference(ReferenceKind::Revision, rest)?,
            _ => {
                let Some(amount) = prefix.strip_prefix("sub-") else {
                    return Err(ToolAcquireError::InvalidVersion(raw.into()));
                };
                let amount = numeric_components(amount)
                    .ok_or_else(|| ToolAcquireError::InvalidVersion(raw.into()))?;
                VersionRequestKind::Subtract {
                    amount,
                    base: Box::new(parse_request(rest, depth + 1)?),
                }
            }
        }
    } else if let Some((prefix, rest)) = raw.split_once('-') {
        match prefix {
            "ref" => reference(ReferenceKind::Ref, rest)?,
            "tag" => reference(ReferenceKind::Tag, rest)?,
            "branch" => reference(ReferenceKind::Branch, rest)?,
            "rev" => reference(ReferenceKind::Revision, rest)?,
            _ => {
                validate_label(raw)?;
                VersionRequestKind::Version(raw.into())
            }
        }
    } else {
        validate_label(raw)?;
        VersionRequestKind::Version(raw.into())
    };
    Ok(VersionRequest { kind })
}

fn validate_label(value: &str) -> Result<(), ToolAcquireError> {
    if value.is_empty()
        || value.len() > MAX_REQUEST_BYTES
        || value.starts_with(['-', '/', '\\'])
        || value.contains("..")
        || value
            .to_ascii_lowercase()
            .replace("%2e", ".")
            .contains("..")
        || value.chars().any(|c| {
            c.is_control()
                || c.is_whitespace()
                || matches!(
                    c,
                    ':' | '\\' | '\'' | '"' | '`' | '$' | ';' | '&' | '|' | '<' | '>' | '(' | ')'
                )
        })
    {
        return Err(ToolAcquireError::InvalidVersion(value.into()));
    }
    Ok(())
}

fn validate_reference(value: &str) -> Result<(), ToolAcquireError> {
    if value.is_empty() {
        return Ok(());
    }
    validate_label(value)
}

fn numeric_components(value: &str) -> Option<Vec<u32>> {
    if value.is_empty() {
        return None;
    }
    value
        .split('.')
        .map(str::parse)
        .collect::<Result<Vec<_>, _>>()
        .ok()
}

fn subtract(base: &str, amount: &[u32]) -> Result<String, ToolAcquireError> {
    let mut version =
        numeric_components(base).ok_or_else(|| ToolAcquireError::InvalidSubtraction {
            version: base.into(),
        })?;
    version.truncate(amount.len());
    for (index, sub) in amount.iter().take(version.len()).enumerate() {
        let Some(current) = version.get_mut(index) else {
            break;
        };
        if *current < *sub {
            for prior in version.iter_mut().take(index).enumerate().rev() {
                if *prior.1 > 0 {
                    *prior.1 -= 1;
                    let keep = prior.0 + 1;
                    version.truncate(keep);
                    return Ok(format_components(&version));
                }
            }
            return Ok("0".into());
        }
        *current -= sub;
    }
    Ok(format_components(&version))
}

fn format_components(parts: &[u32]) -> String {
    parts
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(".")
}

pub(crate) fn compare_versions(a: &str, b: &str) -> Ordering {
    match (Versioning::new(a), Versioning::new(b)) {
        (Some(a), Some(b)) => a.cmp(&b),
        _ => a.cmp(b),
    }
}

pub(crate) fn semver_prerelease(version: &str) -> bool {
    matches!(Versioning::new(version.strip_prefix(['v', 'V']).unwrap_or(version)), Some(Versioning::Ideal(version)) if version.pre_rel.is_some())
}

fn fuzzy_matches(query: &str, version: &str) -> bool {
    let numeric = |text: &str| text.starts_with(|c: char| c.is_ascii_digit());
    if query == "latest" {
        return numeric(version.strip_prefix('v').unwrap_or(version)) && !version.contains('\n');
    }
    let without_v = query.strip_prefix(['v', 'V']);
    let separators: &[char] = if numeric(without_v.unwrap_or(query)) {
        &['+', '-', '.']
    } else {
        &['-', '.']
    };
    let continues = |literal: &str, value: &str| {
        value.strip_prefix(literal).is_some_and(|rest| {
            if query.ends_with('-') {
                return !rest.contains('\n');
            }
            rest.is_empty()
                || rest
                    .strip_prefix(separators)
                    .is_some_and(|tail| !tail.is_empty() && !tail.contains('\n'))
        })
    };
    continues(query, version)
        || (numeric(query)
            && version
                .strip_prefix('v')
                .is_some_and(|version| continues(query, version)))
        || without_v.is_some_and(|query| continues(query, version))
}

#[cfg(test)]
mod tests;
