#![cfg(test)]

use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt, OpenOptionsSyncExt};
use std::io::Read;
use std::path::{Component, Path};

pub const LIMIT: usize = 128 * 1024;

/// Read a quiescent fixture artifact without following links or opening a FIFO.
pub fn read(root: &Path, name: &str) -> std::io::Result<Vec<u8>> {
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
