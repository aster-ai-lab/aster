//! Explicit pasted review material. Task instructions never become source text.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum ReviewTextSource {
    Text { text: String },
}

impl ReviewTextSource {
    pub(crate) fn text(&self) -> Result<&str, &'static str> {
        let Self::Text { text } = self;
        if text.trim().is_empty() || text.contains('\0') || text.len() > 256 * 1024 {
            return Err("Review text must be nonempty and at most 256 KiB.");
        }
        Ok(text)
    }

    pub(super) fn revision(&self) -> Result<String, &'static str> {
        Ok(format!("{:x}", Sha256::digest(self.text()?.as_bytes())))
    }
}

#[cfg(test)]
#[path = "review_text_tests.rs"]
mod tests;
