#![cfg(test)]

use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt, OpenOptionsSyncExt};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Member {
    pub path: PathBuf,
    pub sha256: String,
    pub bytes: u64,
}

impl Member {
    fn open(&self) -> Result<std::fs::File, &'static str> {
        if !self.path.is_absolute()
            || self.bytes == 0
            || self.bytes.checked_add(1).is_none()
            || a3s_code_core::content_digest::validate_digest(&format!("sha256:{}", self.sha256))
                .is_err()
        {
            return Err("candidate_member_identity");
        }
        let directory = cap_std::fs::Dir::open_ambient_dir(
            self.path.parent().ok_or("candidate_parent")?,
            cap_std::ambient_authority(),
        )
        .map_err(|_| "candidate_parent")?;
        let mut options = cap_std::fs::OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No).nonblock(true);
        let file = directory
            .open_with(self.path.file_name().ok_or("candidate_name")?, &options)
            .map_err(|_| "candidate_member_open")?
            .into_std();
        let metadata = file.metadata().map_err(|_| "candidate_member_metadata")?;
        if !metadata.is_file() || metadata.len() != self.bytes {
            return Err("candidate_member_size");
        }
        Ok(file)
    }

    pub fn verify(&self) -> Result<(), &'static str> {
        let mut file = self.open()?.take(self.bytes + 1);
        let mut hash = Sha256::new();
        let mut bytes = [0; 64 * 1024];
        let mut count = 0u64;
        loop {
            let read = file.read(&mut bytes).map_err(|_| "candidate_member_read")?;
            if read == 0 {
                break;
            }
            count += read as u64;
            hash.update(&bytes[..read]);
        }
        if count != self.bytes || format!("{:x}", hash.finalize()) != self.sha256 {
            return Err("candidate_member_changed");
        }
        Ok(())
    }

    fn copy_to(&self, path: PathBuf) -> Result<(), &'static str> {
        let source = self.open()?;
        let permissions = source
            .metadata()
            .map_err(|_| "candidate_member_metadata")?
            .permissions();
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|_| "candidate_copy_open")?;
        let count = std::io::copy(&mut source.take(self.bytes + 1), &mut output)
            .map_err(|_| "candidate_copy")?;
        if count != self.bytes {
            return Err("candidate_copy_size");
        }
        std::fs::set_permissions(&path, permissions).map_err(|_| "candidate_copy_permissions")?;
        Self {
            path,
            ..self.clone()
        }
        .verify()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub schema: String,
    pub build_id: String,
    pub aster: Member,
    pub acp: Member,
}

impl Candidate {
    pub fn load(path: &Path, expected: &str) -> Result<Self, &'static str> {
        if !path.is_absolute() {
            return Err("candidate_manifest_path");
        }
        let bytes = super::artifact::read(
            path.parent().ok_or("candidate_manifest_path")?,
            path.file_name()
                .and_then(|name| name.to_str())
                .ok_or("candidate_manifest_name")?,
        )
        .map_err(|_| "candidate_manifest_read")?;
        let candidate: Self =
            serde_json::from_slice(&bytes).map_err(|_| "candidate_manifest_schema")?;
        if candidate.schema != "a3s.code.lh.candidate.v1" || candidate.build_id != expected {
            return Err("candidate_build_identity");
        }
        candidate.aster.verify()?;
        candidate.acp.verify()?;
        Ok(candidate)
    }

    pub fn from_environment(expected: &str) -> Result<Option<Self>, &'static str> {
        std::env::var_os("A3S_LH_CANDIDATE_MANIFEST")
            .map(|path| Self::load(Path::new(&path), expected))
            .transpose()
    }

    pub fn acp_binary(default: &str, expected: &str) -> PathBuf {
        match Self::from_environment(expected)
            .expect("registered candidate cannot fall back to another binary")
        {
            Some(candidate) => candidate.acp.path,
            None => PathBuf::from(default),
        }
    }

    pub fn copy_into(&self, directory: &Path) -> Result<(), &'static str> {
        self.aster.copy_to(directory.join("aster"))?;
        self.acp.copy_to(directory.join("a3s-code-acp"))?;
        Ok(())
    }
}

#[path = "candidate_tests.rs"]
mod tests;
