//! A live OKF job grants snapshot reads and candidate writes, never general coding authority.

use std::path::{Component, Path, PathBuf};

use aster_core::permissions::{
    canonical_tool_name, PermissionChecker, PermissionDecision, PermissionPolicy,
};
use serde::Deserialize;
use serde_json::Value;

const TOOLS: &[&str] = &[
    "read",
    "ls",
    "search",
    "write",
    "edit",
    "batch",
    "update_plan",
];

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompileScope {
    root: PathBuf,
    job_id: String,
    #[serde(default)]
    workspace_revision: Option<String>,
}

impl CompileScope {
    fn owned_workspace(&self) -> Result<PathBuf, String> {
        match &self.workspace_revision {
            Some(revision) => {
                aster_okf::workspace_knowledge_compile_workspace(&self.root, &self.job_id, revision)
            }
            None => aster_okf::knowledge_compile_workspace(&self.root, &self.job_id),
        }
        .map_err(|error| error.to_string())
    }
}

pub(crate) struct CompileGuard {
    scope: CompileScope,
    workspace: PathBuf,
}

impl CompileGuard {
    pub(crate) fn new(scope: &CompileScope, workspace: &Path) -> Result<Self, String> {
        let owned = scope.owned_workspace()?;
        if workspace != owned {
            return Err("knowledge.compile_workspace_mismatch".into());
        }
        Ok(Self {
            scope: scope.clone(),
            workspace: owned,
        })
    }

    fn path(&self, raw: &str) -> Option<PathBuf> {
        if raw.is_empty()
            || raw.len() > 4096
            || raw.contains(['\\', ':'])
            || raw.chars().any(char::is_control)
        {
            return None;
        }
        let path = Path::new(raw);
        let relative = if path.is_absolute() {
            path.strip_prefix(&self.workspace).ok()?
        } else {
            path
        };
        let mut result = PathBuf::new();
        let mut absolute = self.workspace.clone();
        for component in relative.components() {
            match component {
                Component::CurDir => continue,
                Component::Normal(part) => {
                    result.push(part);
                    absolute.push(part);
                    match std::fs::symlink_metadata(&absolute) {
                        Ok(meta) if meta.file_type().is_symlink() => return None,
                        Ok(_) => {}
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                        Err(_) => return None,
                    }
                }
                _ => return None,
            }
        }
        Some(result)
    }

    fn allows(&self, name: &str, args: &Value) -> bool {
        if !args.is_object() {
            return false;
        }
        let name = canonical_tool_name(name);
        let Some(name) = TOOLS
            .iter()
            .copied()
            .find(|tool| tool.eq_ignore_ascii_case(name))
        else {
            return false;
        };
        match name {
            "read" => match (args.get("file_path"), args.get("files")) {
                (Some(path), None) => path.as_str().is_some_and(|path| self.path(path).is_some()),
                (None, Some(files)) => files.as_array().is_some_and(|files| {
                    !files.is_empty()
                        && files.len() <= 32
                        && files.iter().all(|file| {
                            file.get("path")
                                .and_then(Value::as_str)
                                .is_some_and(|path| self.path(path).is_some())
                        })
                }),
                _ => false,
            },
            "ls" | "search" => {
                let path = args.get("path").map_or(Some("."), Value::as_str);
                path.is_some_and(|path| self.path(path).is_some())
                    && (name == "ls"
                        || matches!(
                            args.get("mode").and_then(Value::as_str),
                            Some("grep" | "glob")
                        ))
            }
            "write" | "edit" => args
                .get("file_path")
                .and_then(Value::as_str)
                .and_then(|path| self.path(path))
                .is_some_and(|path| {
                    if path == Path::new(".wiki-changes.json") {
                        return true;
                    }
                    path.to_str().is_some_and(aster_okf::knowledge_page_path)
                }),
            "batch" => args
                .get("invocations")
                .and_then(Value::as_array)
                .is_some_and(|calls| {
                    !calls.is_empty()
                        && calls.len() <= 32
                        && calls.iter().all(|call| {
                            let Some(name) = call.get("tool").and_then(Value::as_str) else {
                                return false;
                            };
                            !canonical_tool_name(name).eq_ignore_ascii_case("batch")
                                && call.get("args").is_some_and(|args| self.allows(name, args))
                        })
                }),
            "update_plan" => true,
            _ => false,
        }
    }
}

impl PermissionChecker for CompileGuard {
    fn expose_to_model(&self, name: &str) -> bool {
        TOOLS
            .iter()
            .any(|tool| tool.eq_ignore_ascii_case(canonical_tool_name(name)))
    }

    fn check(&self, name: &str, args: &Value) -> PermissionDecision {
        let live = self
            .scope
            .owned_workspace()
            .is_ok_and(|workspace| workspace == self.workspace);
        if live && self.allows(name, args) {
            PermissionDecision::Allow
        } else {
            PermissionDecision::Deny
        }
    }
}

pub(crate) fn policy(denies: &[String]) -> PermissionPolicy {
    let mut policy = PermissionPolicy {
        default_decision: PermissionDecision::Deny,
        ..Default::default()
    }
    .allow_all(TOOLS);
    for rule in denies {
        policy = policy.deny(rule);
    }
    policy
}

#[cfg(test)]
#[path = "knowledge_compile_tests.rs"]
mod tests;
