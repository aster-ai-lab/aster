//! Private benchmark barrier: every full replica stays resident before release.
use serde::Deserialize;
use std::{
    io::{Read, Write},
    path::Path,
    time::{Duration, Instant},
};

const MODEL_SHA: &str = aster_deepseek::weights::xq::XQ_GGUF_SHA256;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Release {
    operation: String,
    model_sha256: String,
}

pub fn wait(root: &Path, ready: &serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
    wait_until(root, ready, Instant::now() + Duration::from_secs(1200))
}

#[cfg(unix)]
fn wait_until(
    root: &Path,
    ready: &serde_json::Value,
    deadline: Instant,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
    let owner = std::fs::metadata("/proc/self")?.uid();
    let metadata = std::fs::symlink_metadata(root)?;
    if root.canonicalize()? != root
        || !metadata.is_dir()
        || metadata.uid() != owner
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err("replica barrier requires its exact private process-owned directory".into());
    }
    let temporary = root.join("ready.tmp");
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)?;
    output.write_all(&serde_json::to_vec(ready)?)?;
    output.sync_all()?;
    drop(output);
    let published = root.join("ready.json");
    if published.exists() {
        return Err("replica readiness already exists".into());
    }
    std::fs::rename(temporary, published)?;
    loop {
        if Instant::now() >= deadline {
            return Err("replica barrier exceeded its bounded release deadline".into());
        }
        let gate = root.join("go.json");
        if gate.exists() {
            let metadata = std::fs::symlink_metadata(&gate)?;
            if !metadata.is_file() || metadata.uid() != owner || metadata.len() > 4096 {
                return Err("replica release is not the exact owned bounded artifact".into());
            }
            let mut bytes = Vec::new();
            std::fs::File::open(gate)?
                .take(4097)
                .read_to_end(&mut bytes)?;
            if bytes.len() > 4096 {
                return Err("replica release exceeds its byte bound".into());
            }
            let release: Release = serde_json::from_slice(&bytes)?;
            if release.operation != "generate" || release.model_sha256 != MODEL_SHA {
                return Err("replica release has an unknown operation or model identity".into());
            }
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[cfg(not(unix))]
fn wait_until(
    _root: &Path,
    _ready: &serde_json::Value,
    _deadline: Instant,
) -> Result<(), Box<dyn std::error::Error>> {
    Err("owned replica barrier qualification requires Unix process ownership".into())
}

#[cfg(all(test, unix))]
#[path = "native_barrier_tests.rs"]
mod tests;
