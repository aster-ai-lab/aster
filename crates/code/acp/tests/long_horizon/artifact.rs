#![cfg(test)]

use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt, OpenOptionsSyncExt};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Component, Path};

#[path = "timing.rs"]
pub mod timing;

pub const LIMIT: usize = 128 * 1024;

/// Capture public handoffs after a fixture run without exporting model reasoning.
pub fn write_public_compaction_evidence(
    directory: &Path,
    facts: &[aster_effect::Fact],
) -> std::io::Result<()> {
    let _timing = timing::span(timing::Phase::PublicEvidenceWrite);
    const LIMIT: usize = 2 * 1024 * 1024;
    let mut entries = Vec::new();
    let mut remaining = LIMIT - 512;
    let mut omitted = 0;
    for fact in facts.iter().filter(|fact| fact.kind == "compaction.done") {
        let entry = serde_json::json!({
            "seq":fact.seq, "summary":fact.payload["summary"], "kept":fact.payload["kept"]
        });
        let bytes = serde_json::to_vec(&entry).map_err(std::io::Error::other)?;
        let cost = bytes.len().saturating_add(1);
        if cost > remaining {
            omitted += 1;
            continue;
        }
        remaining -= cost;
        entries.push(entry);
    }
    let bytes = serde_json::to_vec(&serde_json::json!({
        "schema":"aster.code.public-compaction-evidence.v1", "entries":entries,
        "omitted_entries":omitted, "content_limit_bytes":LIMIT, "private_reasoning_omitted":true
    }))
    .map_err(std::io::Error::other)?;
    std::fs::write(directory.join("public-compaction-handoffs.json"), bytes)
}

/// Hash the bytes of a quiescent qualification input without loading it all into memory.
pub fn digest(path: &Path) -> std::io::Result<String> {
    let _timing = timing::span(timing::Phase::FileDigest);
    let mut file = std::fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut bytes = [0; 64 * 1024];
    loop {
        let count = file.read(&mut bytes)?;
        if count == 0 {
            break;
        }
        digest.update(&bytes[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

/// Read a quiescent fixture artifact without following links or opening a FIFO.
pub fn read(root: &Path, name: &str) -> std::io::Result<Vec<u8>> {
    let _timing = timing::span(timing::Phase::ArtifactRead);
    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err(std::io::Error::other("artifact must be one relative file"));
    }
    let directory = cap_std::fs::Dir::open_ambient_dir(root, cap_std::ambient_authority())?;
    let metadata = directory.symlink_metadata(name)?;
    if !metadata.is_file() || metadata.len() > LIMIT as u64 {
        return Err(std::io::Error::other("artifact type or size refused"));
    }
    let mut options = cap_std::fs::OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No).nonblock(true);
    let file = directory.open_with(name, &options)?.into_std();
    let metadata = file.metadata()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() != 1 {
            return Err(std::io::Error::other("artifact hard link refused"));
        }
    }
    if !metadata.is_file() || metadata.len() > LIMIT as u64 {
        return Err(std::io::Error::other("artifact type or size changed"));
    }
    let mut bytes = Vec::new();
    file.take(LIMIT as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > LIMIT {
        return Err(std::io::Error::other("artifact exceeds capture bound"));
    }
    Ok(bytes)
}

#[path = "artifact_tests.rs"]
mod tests;
