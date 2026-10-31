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
use std::path::{Path, PathBuf};

use aster_acl::{generate, parse_acl, Block, Document, Value};

use crate::catalog::catalog_by_name;
use crate::error::ToolAcquireError;
use crate::spec::{parse_tool_name, parse_tool_version};

const ENV_FILE: &str = "env.acl";
const PROJECT_DIR: &str = ".aster";

/// One closed environment declaration after merge.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvConfig {
    /// Tool name → version pin (`latest` means fetch current).
    pub tools: BTreeMap<String, String>,
    /// Environment variable name → value.
    pub vars: BTreeMap<String, String>,
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
    let document = parse_acl(text)
        .map_err(|error| ToolAcquireError::InvalidEnvAcl(format!("parse failed: {error}")))?;
    apply_document(&document)
}

/// Load user then project layers. Missing files are empty. Project keys override user keys.
pub fn load_env_config(
    project_root: impl AsRef<Path>,
    aster_home: impl AsRef<Path>,
) -> Result<EnvConfig, ToolAcquireError> {
    let mut config = EnvConfig::default();
    if let Some(user) = read_optional(&user_env_path(aster_home))? {
        merge_into(&mut config, user);
    }
    if let Some(project) = read_optional(&project_env_path(project_root))? {
        merge_into(&mut config, project);
    }
    Ok(config)
}

/// Write a complete env config as ACL. Creates the parent directory.
pub fn write_env_acl(path: impl AsRef<Path>, config: &EnvConfig) -> Result<(), ToolAcquireError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| ToolAcquireError::io("create env acl directory", parent, error))?;
    }
    let text = render_env_acl(config);
    fs::write(path, text.as_bytes())
        .map_err(|error| ToolAcquireError::io("write env acl", path, error))?;
    Ok(())
}

/// Map declared tools through the closed catalog.
pub fn resolve_tools(config: &EnvConfig) -> Result<Vec<ResolvedTool>, ToolAcquireError> {
    let mut resolved = Vec::with_capacity(config.tools.len());
    for (name, version) in &config.tools {
        parse_tool_name(name)?;
        let catalog =
            catalog_by_name(name).ok_or_else(|| ToolAcquireError::UnknownEnvTool(name.clone()))?;
        let version = normalize_version(version)?;
        resolved.push(ResolvedTool {
            name: catalog.name.to_string(),
            spec: catalog.spec.to_string(),
            version,
            executable: catalog.executable.to_string(),
        });
    }
    Ok(resolved)
}

/// Render ACL text for an env config.
pub fn render_env_acl(config: &EnvConfig) -> String {
    let mut tools_blocks = Vec::new();
    for (name, version) in &config.tools {
        let mut attributes = std::collections::HashMap::new();
        attributes.insert("version".to_string(), Value::String(version.clone()));
        tools_blocks.push(Block {
            name: "tool".to_string(),
            labels: vec![name.clone()],
            blocks: Vec::new(),
            attributes,
        });
    }
    let mut vars_blocks = Vec::new();
    for (name, value) in &config.vars {
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

fn read_optional(path: &Path) -> Result<Option<EnvConfig>, ToolAcquireError> {
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(path)
        .map_err(|error| ToolAcquireError::io("read env acl", path, error))?;
    Ok(Some(parse_env_acl(&text)?))
}

fn merge_into(base: &mut EnvConfig, overlay: EnvConfig) {
    for (name, version) in overlay.tools {
        base.tools.insert(name, version);
    }
    for (name, value) in overlay.vars {
        base.vars.insert(name, value);
    }
}

fn apply_document(document: &Document) -> Result<EnvConfig, ToolAcquireError> {
    let mut env_blocks = document.blocks.iter().filter(|block| block.name == "env");
    let Some(env) = env_blocks.next() else {
        if document.blocks.is_empty() {
            return Ok(EnvConfig::default());
        }
        return Err(ToolAcquireError::InvalidEnvAcl(
            "document must contain a single env block".to_string(),
        ));
    };
    if env_blocks.next().is_some() {
        return Err(ToolAcquireError::InvalidEnvAcl(
            "duplicate env block".to_string(),
        ));
    }
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
    let mut config = EnvConfig::default();
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

fn apply_tools(block: &Block, config: &mut EnvConfig) -> Result<(), ToolAcquireError> {
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
        let version = match child.attributes.get("version") {
            None => "latest".to_string(),
            Some(Value::String(value)) => value.clone(),
            Some(_) => {
                return Err(ToolAcquireError::InvalidEnvAcl(format!(
                    "tool {name} version must be a string"
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
        let _ = normalize_version(&version)?;
        config.tools.insert(name, version);
    }
    Ok(())
}

fn apply_vars(block: &Block, config: &mut EnvConfig) -> Result<(), ToolAcquireError> {
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
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ToolAcquireError::InvalidVersion(raw.to_string()));
    }
    if trimmed.eq_ignore_ascii_case("latest") {
        return Ok(None);
    }
    parse_tool_version(trimmed)?;
    Ok(Some(trimmed.to_string()))
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
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn parses_labeled_tools_and_vars() {
        let config = parse_env_acl(
            r#"
env {
  tools {
    tool "uv" {
      version = "latest"
    }
    tool "aster-sandbox" {
      version = "0.1.10"
    }
  }
  vars {
    var "NODE_ENV" {
      value = "development"
    }
  }
}
"#,
        )
        .unwrap();
        assert_eq!(config.tools.get("uv").map(String::as_str), Some("latest"));
        assert_eq!(
            config.tools.get("aster-sandbox").map(String::as_str),
            Some("0.1.10")
        );
        assert_eq!(
            config.vars.get("NODE_ENV").map(String::as_str),
            Some("development")
        );
    }

    #[test]
    fn rejects_unknown_child_blocks() {
        let error = parse_env_acl(
            r#"
env {
  tasks {}
}
"#,
        )
        .unwrap_err();
        assert!(matches!(error, ToolAcquireError::InvalidEnvAcl(_)));
    }

    #[test]
    fn resolve_requires_catalog_names() {
        let config = parse_env_acl(
            r#"
env {
  tools {
    tool "not-a-real-tool" {
      version = "1"
    }
  }
}
"#,
        )
        .unwrap();
        let error = resolve_tools(&config).unwrap_err();
        assert!(matches!(error, ToolAcquireError::UnknownEnvTool(_)));
    }

    #[test]
    fn round_trip_write_and_load_merges_layers() {
        let temporary = tempdir().unwrap();
        let aster_home = temporary.path().join("aster-home");
        let project = temporary.path().join("project");
        fs::create_dir_all(&aster_home).unwrap();
        fs::create_dir_all(project.join(".aster")).unwrap();

        write_env_acl(
            user_env_path(&aster_home),
            &EnvConfig {
                tools: BTreeMap::from([("uv".into(), "0.1.0".into())]),
                vars: BTreeMap::from([("A".into(), "user".into()), ("B".into(), "keep".into())]),
            },
        )
        .unwrap();
        write_env_acl(
            project_env_path(&project),
            &EnvConfig {
                tools: BTreeMap::from([("gh".into(), "latest".into())]),
                vars: BTreeMap::from([("A".into(), "project".into())]),
            },
        )
        .unwrap();

        let merged = load_env_config(&project, &aster_home).unwrap();
        assert_eq!(merged.tools.get("uv").map(String::as_str), Some("0.1.0"));
        assert_eq!(merged.tools.get("gh").map(String::as_str), Some("latest"));
        assert_eq!(merged.vars.get("A").map(String::as_str), Some("project"));
        assert_eq!(merged.vars.get("B").map(String::as_str), Some("keep"));
    }

    #[test]
    fn resolve_maps_catalog_and_latest() {
        let config = EnvConfig {
            tools: BTreeMap::from([
                ("uv".into(), "latest".into()),
                ("aster-sandbox".into(), "0.1.10".into()),
            ]),
            vars: BTreeMap::new(),
        };
        let resolved = resolve_tools(&config).unwrap();
        assert_eq!(resolved.len(), 2);
        let uv = resolved.iter().find(|entry| entry.name == "uv").unwrap();
        assert!(uv.version.is_none());
        assert_eq!(uv.spec, "github:astral-sh/uv");
        let sandbox = resolved
            .iter()
            .find(|entry| entry.name == "aster-sandbox")
            .unwrap();
        assert_eq!(sandbox.version.as_deref(), Some("0.1.10"));
    }
}
