//! The runner's credential, kept in an ACL state file only its owner reads.

use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

use aster_acl::{generate_acl, parse_acl, Block, Document, Value};

pub const REGISTRATION_TOKEN_ENV: &str = "ASTER_RUNNER_REGISTRATION_TOKEN";

/// No `Debug`: it holds the runner credential.
#[derive(Clone, PartialEq, Eq)]
pub struct RunnerState {
    pub runner_id: String,
    pub credential: String,
}

/// `Ok(None)` when the file does not exist.
pub fn load(path: &Path) -> Result<Option<RunnerState>, String> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("state file {}: {error}", path.display())),
    };
    owner_only(path, &metadata)?;
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("state file {}: {error}", path.display()))?;
    parse(&text).map(Some)
}

#[cfg(unix)]
fn owner_only(path: &Path, metadata: &std::fs::Metadata) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(format!(
            "state file {} must not be readable by group or others (chmod 600)",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn owner_only(_: &Path, _: &std::fs::Metadata) -> Result<(), String> {
    Ok(())
}

fn parse(text: &str) -> Result<RunnerState, String> {
    let malformed = || "state file must hold one runner_state block".to_string();
    let document = parse_acl(text).map_err(|_| malformed())?;
    let [block] = document.blocks.as_slice() else {
        return Err(malformed());
    };
    if block.name != "runner_state" {
        return Err(malformed());
    }
    let field = |key: &str| {
        block
            .attributes
            .get(key)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .ok_or_else(malformed)
    };
    Ok(RunnerState {
        runner_id: field("runner_id")?,
        credential: field("credential")?,
    })
}

/// Writes the state with mode 0600 (its directory 0700 when created),
/// replacing any previous file.
pub fn store(path: &Path, state: &RunnerState) -> Result<(), String> {
    let failed = |error: std::io::Error| format!("state file {}: {error}", path.display());
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            create_private_dir(parent).map_err(failed)?;
        }
    }
    let mut attributes = HashMap::new();
    attributes.insert("runner_id".into(), Value::String(state.runner_id.clone()));
    attributes.insert("credential".into(), Value::String(state.credential.clone()));
    let text = generate_acl(&Document {
        blocks: vec![Block {
            name: "runner_state".into(),
            labels: Vec::new(),
            blocks: Vec::new(),
            attributes,
        }],
    });
    let staged = path.with_extension("acl.tmp");
    let _ = std::fs::remove_file(&staged);
    let mut file = private_file(&staged).map_err(failed)?;
    file.write_all(text.as_bytes()).map_err(failed)?;
    file.sync_all().map_err(failed)?;
    std::fs::rename(&staged, path).map_err(failed)
}

#[cfg(unix)]
pub fn create_private_dir(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
}

#[cfg(not(unix))]
pub fn create_private_dir(path: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(path)
}

#[cfg(unix)]
pub fn private_file(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
pub fn private_file(path: &Path) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
}

#[cfg(all(test, unix))]
#[path = "state_tests.rs"]
mod tests;
