//! Project and user environment declarations in ACL.
//!
//! File layout:
//! - project: `<cwd>/.aster/env.acl`
//! - user: `$ASTER_HOME/env.acl` (default `~/.aster/env.acl`)
//!
//! Shape (hyphenated tool ids must be block labels; ACL idents disallow `-`):
//!
//! ```acl
//! env {
//!   tools {
//!     tool "uv" {
//!       version = "latest"
//!     }
//!     tool "aster-sandbox" {
//!       version = "0.1.10"
//!     }
//!   }
//!   vars {
//!     var "NODE_ENV" {
//!       value = "development"
//!     }
//!   }
//! }
//! ```

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use aster_acl::{generate, parse_acl, Block, Document, Value};

use crate::catalog::catalog_by_name;
use crate::error::ToolAcquireError;
use crate::spec::{parse_tool_name, parse_tool_version};
use crate::version::{VersionRequest, VersionRequestKind};

const ENV_FILE: &str = "env.acl";
const PROJECT_DIR: &str = ".aster";
const MAX_ENV_ACL_BYTES: usize = 1024 * 1024;

/// One closed environment declaration after merge.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct EnvConfig {
    /// Tool name → version pin (`latest` means fetch current).
    pub tools: BTreeMap<String, String>,
    /// Environment variable name → value.
    pub vars: BTreeMap<String, String>,
}

/// Editable declarations with ordered version requests for each tool.
/// Parsing, writing and resolution reject empty lists and invalid requests.
/// The scalar [`EnvConfig`] API remains available for single-version callers.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct EnvDeclarations {
    pub tools: BTreeMap<String, Vec<String>>,
    pub vars: BTreeMap<String, String>,
}

impl From<&EnvConfig> for EnvDeclarations {
    fn from(config: &EnvConfig) -> Self {
        Self {
            tools: config
                .tools
                .iter()
                .map(|(name, pin)| (name.clone(), vec![pin.clone()]))
                .collect(),
            vars: config.vars.clone(),
        }
    }
}

impl std::fmt::Debug for EnvDeclarations {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EnvDeclarations")
            .field("tools", &self.tools)
            .field("vars", &format_args!("{} redacted values", self.vars.len()))
            .finish()
    }
}

impl std::fmt::Debug for EnvConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EnvConfig")
            .field("tools", &self.tools)
            .field("vars", &format_args!("{} redacted values", self.vars.len()))
            .finish()
    }
}

/// One catalog-backed tool ready for acquire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTool {
    pub name: String,
    pub spec: String,
    pub version: Option<String>,
    pub executable: String,
}

/// Relative project path: `.aster/env.acl`.
pub fn project_env_path(project_root: impl AsRef<Path>) -> PathBuf {
    project_root.as_ref().join(PROJECT_DIR).join(ENV_FILE)
}

/// User path: `<aster_home>/env.acl` (typically `~/.aster/env.acl`).
pub fn user_env_path(aster_home: impl AsRef<Path>) -> PathBuf {
    aster_home.as_ref().join(ENV_FILE)
}

/// Parse ACL text into an env config. Unknown top-level or nested blocks fail closed.
pub fn parse_env_acl(text: &str) -> Result<EnvConfig, ToolAcquireError> {
    let declarations = parse_env_declarations(text)?;
    let mut tools = BTreeMap::new();
    for (name, versions) in declarations.tools {
        let [version] = versions.as_slice() else {
            return Err(ToolAcquireError::InvalidEnvAcl(
                "multiple versions require the environment declarations API".into(),
            ));
        };
        tools.insert(name, version.clone());
    }
    Ok(EnvConfig {
        tools,
        vars: declarations.vars,
    })
}

/// Parse scalar or ordered-array tool requests from bounded ACL text.
/// Unknown fields, empty arrays and malformed requests fail without I/O.
pub fn parse_env_declarations(text: &str) -> Result<EnvDeclarations, ToolAcquireError> {
    if text.len() > MAX_ENV_ACL_BYTES {
        return Err(ToolAcquireError::InvalidEnvAcl(format!(
            "environment declaration exceeds {MAX_ENV_ACL_BYTES} bytes"
        )));
    }
    let document = parse_acl(text)
        .map_err(|error| ToolAcquireError::InvalidEnvAcl(format!("parse failed: {error}")))?;
    apply_document(&document)
}

/// Load user, ancestor, and nearest project layers, in that order.
/// Missing files are empty; nearer project keys override earlier layers.
pub fn load_env_config(
    project_root: impl AsRef<Path>,
    aster_home: impl AsRef<Path>,
) -> Result<EnvConfig, ToolAcquireError> {
    let mut config = EnvConfig::default();
    visit_env_layers(project_root.as_ref(), aster_home.as_ref(), |layer, _| {
        merge_into(&mut config, layer);
    })?;
    Ok(config)
}

#[derive(Clone, Copy)]
pub(crate) enum EnvLayerRoot<'a> {
    User,
    Project(&'a Path),
}

pub(crate) fn visit_env_layers(
    project_root: &Path,
    aster_home: &Path,
    mut visit: impl FnMut(EnvConfig, EnvLayerRoot<'_>),
) -> Result<(), ToolAcquireError> {
    visit_env_paths(project_root, aster_home, |path, root| {
        if let Some(config) = read_env_acl(path)? {
            visit(config, root);
        }
        Ok(())
    })
}

pub(crate) fn visit_declaration_layers(
    project_root: &Path,
    aster_home: &Path,
    mut visit: impl FnMut(EnvDeclarations, EnvLayerRoot<'_>),
) -> Result<(), ToolAcquireError> {
    visit_env_paths(project_root, aster_home, |path, root| {
        if let Some(config) = read_env_declarations(path)? {
            visit(config, root);
        }
        Ok(())
    })
}

fn visit_env_paths(
    project_root: &Path,
    aster_home: &Path,
    mut visit: impl FnMut(PathBuf, EnvLayerRoot<'_>) -> Result<(), ToolAcquireError>,
) -> Result<(), ToolAcquireError> {
    visit(user_env_path(aster_home), EnvLayerRoot::User)?;
    let project_root = std::path::absolute(project_root)
        .map_err(|error| ToolAcquireError::io("resolve project directory", project_root, error))?;
    let ancestors: Vec<_> = project_root.ancestors().collect();
    for ancestor in ancestors.into_iter().rev() {
        visit(project_env_path(ancestor), EnvLayerRoot::Project(ancestor))?;
    }
    Ok(())
}

/// Merge user and ancestor declarations; a nearer tool replaces its complete list.
/// Missing files are empty. Bounded read and parse errors retain their sources.
pub fn load_env_declarations(
    project: impl AsRef<Path>,
    aster_home: impl AsRef<Path>,
) -> Result<EnvDeclarations, ToolAcquireError> {
    let mut config = EnvDeclarations::default();
    visit_declaration_layers(project.as_ref(), aster_home.as_ref(), |overlay, _| {
        config.tools.extend(overlay.tools);
        config.vars.extend(overlay.vars);
    })?;
    Ok(config)
}

/// Atomically write ACL, preserving file symlinks and existing native permissions.
/// Creates the parent directory and returns validation or filesystem errors.
pub fn write_env_acl(path: impl AsRef<Path>, config: &EnvConfig) -> Result<(), ToolAcquireError> {
    let path = path.as_ref();
    let text = render_env_acl(config);
    parse_env_acl(&text)?;
    write_acl_file(path, &text)
}

/// Validate and atomically write ordered declarations, preserving native file semantics.
/// Invalid requests or variables return before the destination is changed.
pub fn write_env_declarations(
    path: impl AsRef<Path>,
    config: &EnvDeclarations,
) -> Result<(), ToolAcquireError> {
    let text = render_env_declarations(config);
    parse_env_declarations(&text)?;
    write_acl_file(path.as_ref(), &text)
}

pub(crate) fn write_acl_file(path: &Path, text: &str) -> Result<(), ToolAcquireError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| ToolAcquireError::io("create env acl directory", parent, error))?;
    }
    let destination = aster_config::fs_atomic::bind_follow_destination(path)
        .map_err(|error| ToolAcquireError::io("resolve env acl destination", path, error))?;
    aster_config::fs_atomic::write_atomically_bound(&destination, text, None)
        .map_err(|error| ToolAcquireError::io("write env acl", path, error))
}

/// Map declared tools through the closed catalog.
pub fn resolve_tools(config: &EnvConfig) -> Result<Vec<ResolvedTool>, ToolAcquireError> {
    let mut resolved = Vec::with_capacity(config.tools.len());
    for (name, version) in &config.tools {
        let mut tool = resolve_tool_identity(name)?;
        tool.version = normalize_version(version)?;
        resolved.push(tool);
    }
    Ok(resolved)
}

/// Resolve every managed declaration in tool-name and per-tool request order.
/// This performs no I/O; unknown identities and unsupported requests return errors.
pub fn resolve_declarations(
    config: &EnvDeclarations,
) -> Result<Vec<ResolvedTool>, ToolAcquireError> {
    let mut resolved = Vec::new();
    for (name, versions) in &config.tools {
        validate_version_list(versions)?;
        for version in versions {
            let mut tool = resolve_tool_identity(name)?;
            tool.version = normalize_version(version)?;
            resolved.push(tool);
        }
    }
    Ok(resolved)
}

pub(crate) fn resolve_tool_identity(name: &str) -> Result<ResolvedTool, ToolAcquireError> {
    parse_tool_name(name)?;
    let catalog =
        catalog_by_name(name).ok_or_else(|| ToolAcquireError::UnknownEnvTool(name.to_owned()))?;
    Ok(ResolvedTool {
        name: catalog.name.to_string(),
        spec: catalog.spec.to_string(),
        version: None,
        executable: catalog.executable.to_string(),
    })
}

/// Render ACL text for an env config.
pub fn render_env_acl(config: &EnvConfig) -> String {
    render_environment(
        config
            .tools
            .iter()
            .map(|(name, version)| (name.as_str(), Value::String(version.clone()))),
        &config.vars,
    )
}

/// Render one request as a scalar and several requests as an ordered ACL array.
pub fn render_env_declarations(config: &EnvDeclarations) -> String {
    render_environment(
        config.tools.iter().map(|(name, versions)| {
            let value = match versions.as_slice() {
                [version] => Value::String(version.clone()),
                versions => Value::List(versions.iter().cloned().map(Value::String).collect()),
            };
            (name.as_str(), value)
        }),
        &config.vars,
    )
}

fn render_environment<'a>(
    tools: impl Iterator<Item = (&'a str, Value)>,
    vars: &BTreeMap<String, String>,
) -> String {
    let mut tools_blocks = Vec::new();
    for (name, version) in tools {
        let mut attributes = std::collections::HashMap::new();
        attributes.insert("version".to_string(), version);
        tools_blocks.push(Block {
            name: "tool".to_string(),
            labels: vec![name.to_owned()],
            blocks: Vec::new(),
            attributes,
        });
    }
    let mut vars_blocks = Vec::new();
    for (name, value) in vars {
        let mut attributes = std::collections::HashMap::new();
        attributes.insert("value".to_string(), Value::String(value.clone()));
        vars_blocks.push(Block {
            name: "var".to_string(),
            labels: vec![name.clone()],
            blocks: Vec::new(),
            attributes,
        });
    }
    let mut env_children = Vec::new();
    if !tools_blocks.is_empty() {
        env_children.push(Block {
            name: "tools".to_string(),
            labels: Vec::new(),
            blocks: tools_blocks,
            attributes: std::collections::HashMap::new(),
        });
    }
    if !vars_blocks.is_empty() {
        env_children.push(Block {
            name: "vars".to_string(),
            labels: Vec::new(),
            blocks: vars_blocks,
            attributes: std::collections::HashMap::new(),
        });
    }
    generate(&Document {
        blocks: vec![Block {
            name: "env".to_string(),
            labels: Vec::new(),
            blocks: env_children,
            attributes: std::collections::HashMap::new(),
        }],
    })
}

/// Read one bounded UTF-8 ACL declaration. Only a missing file is treated as empty.
pub fn read_env_acl(path: impl AsRef<Path>) -> Result<Option<EnvConfig>, ToolAcquireError> {
    read_env_text(path.as_ref())?
        .map(|text| parse_env_acl(&text))
        .transpose()
}

/// Read one bounded declaration without losing ordered version requests.
/// Only a missing file returns `None`; malformed bytes and I/O failures return errors.
pub fn read_env_declarations(
    path: impl AsRef<Path>,
) -> Result<Option<EnvDeclarations>, ToolAcquireError> {
    read_env_text(path.as_ref())?
        .map(|text| parse_env_declarations(&text))
        .transpose()
}

fn read_env_text(path: &Path) -> Result<Option<String>, ToolAcquireError> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(ToolAcquireError::io("open env acl", path, error)),
    };
    let mut text = String::new();
    file.take(MAX_ENV_ACL_BYTES as u64 + 1)
        .read_to_string(&mut text)
        .map_err(|error| ToolAcquireError::io("read env acl", path, error))?;
    Ok(Some(text))
}

pub(crate) fn merge_into(base: &mut EnvConfig, overlay: EnvConfig) {
    for (name, version) in overlay.tools {
        base.tools.insert(name, version);
    }
    for (name, value) in overlay.vars {
        base.vars.insert(name, value);
    }
}

fn apply_document(document: &Document) -> Result<EnvDeclarations, ToolAcquireError> {
    let env = match document.blocks.as_slice() {
        [] => return Ok(EnvDeclarations::default()),
        [env] if env.name == "env" => env,
        _ => {
            return Err(ToolAcquireError::InvalidEnvAcl(
                "document must contain a single env block".to_string(),
            ));
        }
    };
    if !env.labels.is_empty() {
        return Err(ToolAcquireError::InvalidEnvAcl(
            "env block must not have labels".to_string(),
        ));
    }
    if !env.attributes.is_empty() {
        return Err(ToolAcquireError::InvalidEnvAcl(
            "env block must not have attributes".to_string(),
        ));
    }
    let mut config = EnvDeclarations::default();
    let mut saw_tools = false;
    let mut saw_vars = false;
    for child in &env.blocks {
        match child.name.as_str() {
            "tools" => {
                if saw_tools {
                    return Err(ToolAcquireError::InvalidEnvAcl(
                        "duplicate tools block".to_string(),
                    ));
                }
                saw_tools = true;
                apply_tools(child, &mut config)?;
            }
            "vars" => {
                if saw_vars {
                    return Err(ToolAcquireError::InvalidEnvAcl(
                        "duplicate vars block".to_string(),
                    ));
                }
                saw_vars = true;
                apply_vars(child, &mut config)?;
            }
            other => {
                return Err(ToolAcquireError::InvalidEnvAcl(format!(
                    "unknown env child block: {other}"
                )));
            }
        }
    }
    Ok(config)
}

fn apply_tools(block: &Block, config: &mut EnvDeclarations) -> Result<(), ToolAcquireError> {
    if !block.labels.is_empty() || !block.attributes.is_empty() {
        return Err(ToolAcquireError::InvalidEnvAcl(
            "tools block must only contain tool children".to_string(),
        ));
    }
    for child in &block.blocks {
        if child.name != "tool" {
            return Err(ToolAcquireError::InvalidEnvAcl(format!(
                "unknown tools child block: {}",
                child.name
            )));
        }
        if child.labels.len() != 1 {
            return Err(ToolAcquireError::InvalidEnvAcl(
                "tool block requires exactly one label".to_string(),
            ));
        }
        if !child.blocks.is_empty() {
            return Err(ToolAcquireError::InvalidEnvAcl(
                "tool block must not nest further blocks".to_string(),
            ));
        }
        let name = child.labels[0].clone();
        parse_tool_name(&name)?;
        if config.tools.contains_key(&name) {
            return Err(ToolAcquireError::InvalidEnvAcl(format!(
                "duplicate tool: {name}"
            )));
        }
        let versions = match child.attributes.get("version") {
            None => vec!["latest".to_string()],
            Some(Value::String(value)) => vec![value.clone()],
            Some(Value::List(values)) => values
                .iter()
                .map(|value| {
                    value.as_str().map(str::to_owned).ok_or_else(|| {
                        ToolAcquireError::InvalidEnvAcl(format!(
                            "tool {name} versions must be strings"
                        ))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
            Some(_) => {
                return Err(ToolAcquireError::InvalidEnvAcl(format!(
                    "tool {name} version must be a string or array"
                )));
            }
        };
        for key in child.attributes.keys() {
            if key != "version" {
                return Err(ToolAcquireError::InvalidEnvAcl(format!(
                    "unknown tool attribute: {key}"
                )));
            }
        }
        validate_version_list(&versions)?;
        for version in &versions {
            // V1 locks can bind literal labels that overlap today's request operators.
            if parse_tool_version(version).is_err() {
                let _ = normalize_version(version)?;
            }
        }
        config.tools.insert(name, versions);
    }
    Ok(())
}

fn apply_vars(block: &Block, config: &mut EnvDeclarations) -> Result<(), ToolAcquireError> {
    if !block.labels.is_empty() || !block.attributes.is_empty() {
        return Err(ToolAcquireError::InvalidEnvAcl(
            "vars block must only contain var children".to_string(),
        ));
    }
    for child in &block.blocks {
        if child.name != "var" {
            return Err(ToolAcquireError::InvalidEnvAcl(format!(
                "unknown vars child block: {}",
                child.name
            )));
        }
        if child.labels.len() != 1 {
            return Err(ToolAcquireError::InvalidEnvAcl(
                "var block requires exactly one label".to_string(),
            ));
        }
        if !child.blocks.is_empty() {
            return Err(ToolAcquireError::InvalidEnvAcl(
                "var block must not nest further blocks".to_string(),
            ));
        }
        let name = child.labels[0].clone();
        validate_env_name(&name)?;
        if config.vars.contains_key(&name) {
            return Err(ToolAcquireError::InvalidEnvAcl(format!(
                "duplicate var: {name}"
            )));
        }
        let value = match child.attributes.get("value") {
            Some(Value::String(value)) => value.clone(),
            Some(_) => {
                return Err(ToolAcquireError::InvalidEnvAcl(format!(
                    "var {name} value must be a string"
                )));
            }
            None => {
                return Err(ToolAcquireError::InvalidEnvAcl(format!(
                    "var {name} is missing value"
                )));
            }
        };
        for key in child.attributes.keys() {
            if key != "value" {
                return Err(ToolAcquireError::InvalidEnvAcl(format!(
                    "unknown var attribute: {key}"
                )));
            }
        }
        config.vars.insert(name, value);
    }
    Ok(())
}

fn normalize_version(raw: &str) -> Result<Option<String>, ToolAcquireError> {
    let request = VersionRequest::parse(raw)?;
    if matches!(request.kind(), VersionRequestKind::Latest) {
        Ok(None)
    } else {
        Ok(Some(request.canonical()))
    }
}

pub(crate) fn validate_version_list(versions: &[String]) -> Result<(), ToolAcquireError> {
    if versions.is_empty() || versions.len() > crate::version::MAX_VERSIONS {
        return Err(ToolAcquireError::InvalidEnvAcl(
            "tool requires between 1 and 10000 version requests".into(),
        ));
    }
    Ok(())
}

fn validate_env_name(name: &str) -> Result<(), ToolAcquireError> {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return Err(ToolAcquireError::InvalidEnvAcl(
            "empty environment variable name".to_string(),
        ));
    };
    if !(first.is_ascii_alphabetic() || first == '_')
        || !chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return Err(ToolAcquireError::InvalidEnvAcl(format!(
            "invalid environment variable name: {name}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
