//! Model assets with pinned digests (fail-closed on drift).
//!
//! Load order for [`load_verified`]:
//! 1. `ASTER_EMBEDDING_DIR` when set and populated;
//! 2. `{executable_parent}/embedding/` when populated;
//! 3. compile-time bundled bytes when the `bundled-assets` feature is on.
//!
//! Byte tables under `bundled-assets` are `static`, not `const`, so
//! `include_bytes!` is not duplicated across codegen units.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

pub const MODEL_SHA256: &str = "75cf7a6c2171b230ad19b1e7d8e0b1aee86da5a02af8e7cacedd9921d227623c";
pub const TOKENIZER_SHA256: &str =
    "107bbdcbad4bff1d299b7a4c3a2fb17c52890688b7dd0e4c9deab79d3c4f3d45";
pub(crate) const CONFIG_SHA256: &str =
    "148e5691a6fcc553437156859701fba017a1ba5d340b170f17e0f3668fb861a7";

pub const MODEL_ID: &str = "potion-code-16M-v2";

pub const MODEL_FILE: &str = "potion-code-16M-v2.safetensors";
pub const TOKENIZER_FILE: &str = "potion-code-16M-v2.tokenizer.json";
pub const CONFIG_FILE: &str = "potion-code-16M-v2.config.json";

/// Directory name next to the host executable that carries the three asset files.
pub const EMBEDDING_DIR_NAME: &str = "embedding";

/// Environment override for the asset directory (absolute or relative path).
pub const EMBEDDING_DIR_ENV: &str = "ASTER_EMBEDDING_DIR";

#[cfg(feature = "bundled-assets")]
pub static MODEL_BYTES: &[u8] = include_bytes!("../assets/potion-code-16M-v2.safetensors");
#[cfg(feature = "bundled-assets")]
pub static TOKENIZER_BYTES: &[u8] = include_bytes!("../assets/potion-code-16M-v2.tokenizer.json");
#[cfg(feature = "bundled-assets")]
pub static CONFIG_BYTES: &[u8] = include_bytes!("../assets/potion-code-16M-v2.config.json");

/// Owned asset bytes after digest verification.
#[derive(Clone)]
pub struct VerifiedAssets {
    pub model: Vec<u8>,
    pub tokenizer: Vec<u8>,
    pub config: Vec<u8>,
    pub source: AssetSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetSource {
    Directory,
    #[cfg(feature = "bundled-assets")]
    Bundled,
}

/// Resolve and verify assets from disk (preferred) or the compile-time bundle.
pub fn load_verified() -> Result<VerifiedAssets, String> {
    if let Some(dir) = resolve_embedding_dir() {
        if dir_has_assets(&dir) {
            return load_from_dir(&dir);
        }
    }
    #[cfg(feature = "bundled-assets")]
    {
        verify_bundled()?;
        return Ok(VerifiedAssets {
            model: MODEL_BYTES.to_vec(),
            tokenizer: TOKENIZER_BYTES.to_vec(),
            config: CONFIG_BYTES.to_vec(),
            source: AssetSource::Bundled,
        });
    }
    #[cfg(not(feature = "bundled-assets"))]
    {
        Err(format!(
            "embedding assets not found: set {EMBEDDING_DIR_ENV} or place \
             {MODEL_FILE}, {TOKENIZER_FILE}, and {CONFIG_FILE} under \
             <exe>/{EMBEDDING_DIR_NAME}/ (this build has no bundled assets)"
        ))
    }
}

/// Load and verify the three pinned files from `dir`.
pub fn load_from_dir(dir: &Path) -> Result<VerifiedAssets, String> {
    let model = read_asset(dir, MODEL_FILE, MODEL_SHA256)?;
    let tokenizer = read_asset(dir, TOKENIZER_FILE, TOKENIZER_SHA256)?;
    let config = read_asset(dir, CONFIG_FILE, CONFIG_SHA256)?;
    Ok(VerifiedAssets {
        model,
        tokenizer,
        config,
        source: AssetSource::Directory,
    })
}

/// Prefer `ASTER_EMBEDDING_DIR`, else `{exe_dir}/embedding`.
pub fn resolve_embedding_dir() -> Option<PathBuf> {
    if let Ok(raw) = std::env::var(EMBEDDING_DIR_ENV) {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed));
        }
    }
    let exe = std::env::current_exe().ok()?;
    let parent = exe.parent()?;
    Some(parent.join(EMBEDDING_DIR_NAME))
}

fn dir_has_assets(dir: &Path) -> bool {
    dir.join(MODEL_FILE).is_file()
        && dir.join(TOKENIZER_FILE).is_file()
        && dir.join(CONFIG_FILE).is_file()
}

fn read_asset(dir: &Path, file: &str, pinned: &str) -> Result<Vec<u8>, String> {
    let path = dir.join(file);
    let bytes = std::fs::read(&path).map_err(|error| {
        format!(
            "cannot read embedding asset {}: {error}",
            path.display()
        )
    })?;
    verify_asset(file, &bytes, pinned)?;
    Ok(bytes)
}

#[cfg(feature = "bundled-assets")]
fn verify_bundled() -> Result<(), String> {
    verify_asset("model", MODEL_BYTES, MODEL_SHA256)?;
    verify_asset("tokenizer", TOKENIZER_BYTES, TOKENIZER_SHA256)?;
    verify_asset("config", CONFIG_BYTES, CONFIG_SHA256)?;
    Ok(())
}

/// Refuse load when bytes drifted from the pinned digest.
pub fn verify_asset(name: &str, bytes: &[u8], pinned: &str) -> Result<(), String> {
    let actual = hex(&Sha256::digest(bytes));
    if actual != pinned {
        return Err(format!(
            "{name} asset sha256 drift: pinned {pinned}, compiled {actual}"
        ));
    }
    Ok(())
}

/// Back-compat for callers that only check the bundled tables.
#[cfg(feature = "bundled-assets")]
pub fn verify() -> Result<(), String> {
    verify_bundled()
}

#[cfg(not(feature = "bundled-assets"))]
pub fn verify() -> Result<(), String> {
    load_verified().map(|_| ())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "assets_tests.rs"]
mod tests;
