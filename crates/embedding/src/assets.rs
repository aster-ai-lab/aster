//! Embedded model assets with pinned digests (fail-closed on drift).

use sha2::{Digest, Sha256};

pub const MODEL_BYTES: &[u8] = include_bytes!("../assets/potion-code-16M-v2.safetensors");
pub const TOKENIZER_BYTES: &[u8] = include_bytes!("../assets/potion-code-16M-v2.tokenizer.json");
pub const CONFIG_BYTES: &[u8] = include_bytes!("../assets/potion-code-16M-v2.config.json");

pub const MODEL_SHA256: &str = "75cf7a6c2171b230ad19b1e7d8e0b1aee86da5a02af8e7cacedd9921d227623c";
pub const TOKENIZER_SHA256: &str =
    "107bbdcbad4bff1d299b7a4c3a2fb17c52890688b7dd0e4c9deab79d3c4f3d45";
pub(crate) const CONFIG_SHA256: &str =
    "148e5691a6fcc553437156859701fba017a1ba5d340b170f17e0f3668fb861a7";

pub const MODEL_ID: &str = "potion-code-16M-v2";

/// Refuse load when an embedded asset drifted from its pinned digest. The
/// assets are compile-time constants, so this is a build-integrity check,
/// not a runtime download check.
pub fn verify() -> Result<(), String> {
    verify_asset("model", MODEL_BYTES, MODEL_SHA256)?;
    verify_asset("tokenizer", TOKENIZER_BYTES, TOKENIZER_SHA256)?;
    verify_asset("config", CONFIG_BYTES, CONFIG_SHA256)
}

fn verify_asset(name: &str, bytes: &[u8], pinned: &str) -> Result<(), String> {
    let actual = hex(&Sha256::digest(bytes));
    if actual != pinned {
        return Err(format!(
            "{name} asset sha256 drift: pinned {pinned}, compiled {actual}"
        ));
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "assets_tests.rs"]
mod tests;
