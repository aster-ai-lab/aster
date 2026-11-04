//! Virtual workspace path normalization and display helpers.

use super::{WorkspacePath, WorkspacePathResolver};
use anyhow::{bail, Result};
use std::path::{Component, Path, PathBuf};

/// Lexical resolver suitable for virtual/browser/DFS workspaces.
#[derive(Debug, Default)]
pub struct VirtualPathResolver;

impl WorkspacePathResolver for VirtualPathResolver {
    fn normalize(&self, input: &str) -> Result<WorkspacePath> {
        normalize_virtual_path(input)
    }
}

fn normalize_virtual_path(input: &str) -> Result<WorkspacePath> {
    let input = default_path_input(input);
    if has_windows_path_prefix(input) {
        bail!("Absolute paths are not supported by this workspace backend");
    }

    let normalized_input = input.replace('\\', "/");
    let path = Path::new(&normalized_input);
    if path.is_absolute() {
        bail!("Absolute paths are not supported by this workspace backend");
    }

    let relative = normalize_relative_path(path)?;
    Ok(pathbuf_to_workspace_path(&relative))
}

pub(super) fn default_path_input(input: &str) -> &str {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        "."
    } else {
        trimmed
    }
}

pub(super) fn has_windows_path_prefix(input: &str) -> bool {
    let bytes = input.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return true;
    }

    input.starts_with("\\\\") || input.starts_with("//")
}

pub(crate) fn validate_relative_pattern(pattern: &str, label: &str) -> Result<()> {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        bail!("{label} cannot be empty");
    }
    if has_windows_path_prefix(pattern) || Path::new(pattern).is_absolute() {
        bail!("{label} must be relative to the workspace");
    }

    let normalized = pattern.replace('\\', "/");
    if normalized.split('/').any(|component| component == "..") {
        bail!("{label} must not contain parent directory traversal");
    }

    Ok(())
}

const MAX_BRACE_EXPANSIONS: usize = 256;

/// A glob that also accepts `{a,b}` alternation, which `glob::Pattern` treats
/// as literal text. Each alternative is a separate `glob::Pattern`.
#[derive(Debug, Clone)]
pub(crate) struct BraceGlob {
    expansions: Vec<String>,
    patterns: Vec<glob::Pattern>,
}

impl BraceGlob {
    pub(crate) fn new(pattern: &str) -> Result<Self> {
        let expansions = expand_braces(pattern);
        if expansions.len() > MAX_BRACE_EXPANSIONS {
            bail!(
                "glob pattern '{pattern}' expands to more than {MAX_BRACE_EXPANSIONS} alternatives"
            );
        }
        let patterns = expansions
            .iter()
            .map(|expansion| glob::Pattern::new(expansion))
            .collect::<std::result::Result<_, _>>()
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        Ok(Self {
            expansions,
            patterns,
        })
    }

    /// The brace-free patterns, for walkers that only understand `glob` syntax.
    pub(crate) fn expansions(&self) -> &[String] {
        &self.expansions
    }

    pub(crate) fn matches(&self, text: &str) -> bool {
        self.patterns.iter().any(|pattern| pattern.matches(text))
    }

    pub(crate) fn matches_path(&self, path: &Path) -> bool {
        self.patterns
            .iter()
            .any(|pattern| pattern.matches_path(path))
    }
}

/// Expands the first balanced `{a,b}` group recursively. An unbalanced brace
/// stays literal. Output stops growing once it passes the expansion limit.
fn expand_braces(pattern: &str) -> Vec<String> {
    let Some((open, close)) = first_brace_group(pattern) else {
        return vec![pattern.to_string()];
    };
    let (head, tail) = (&pattern[..open], &pattern[close + 1..]);
    let mut out = Vec::new();
    for alternative in split_top_level_commas(&pattern[open + 1..close]) {
        for expanded in expand_braces(&format!("{head}{alternative}{tail}")) {
            out.push(expanded);
            if out.len() > MAX_BRACE_EXPANSIONS {
                return out;
            }
        }
    }
    out
}

fn first_brace_group(pattern: &str) -> Option<(usize, usize)> {
    let mut depth = 0usize;
    let mut open = 0;
    for (index, byte) in pattern.bytes().enumerate() {
        match byte {
            b'{' => {
                if depth == 0 {
                    open = index;
                }
                depth += 1;
            }
            b'}' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    return Some((open, index));
                }
            }
            _ => {}
        }
    }
    None
}

fn split_top_level_commas(body: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    for (index, byte) in body.bytes().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => {
                parts.push(&body[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(&body[start..]);
    parts
}

pub(super) fn normalize_relative_path(path: &Path) -> Result<PathBuf> {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => out.push(part),
            Component::ParentDir => {
                if !out.pop() {
                    bail!("Workspace boundary violation: path escapes workspace");
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                bail!("Absolute paths are not supported by this workspace backend");
            }
        }
    }
    Ok(out)
}

pub(super) fn pathbuf_to_workspace_path(path: &Path) -> WorkspacePath {
    let display = path.to_string_lossy().replace('\\', "/");
    WorkspacePath::from_normalized(display)
}

pub(crate) fn escape_control_chars_for_display(path: &str) -> String {
    let mut escaped = String::with_capacity(path.len());
    for ch in path.chars() {
        if ch.is_control() {
            escaped.extend(ch.escape_default());
        } else {
            escaped.push(ch);
        }
    }
    escaped
}

#[cfg(test)]
#[path = "path_brace_tests.rs"]
mod brace_tests;
