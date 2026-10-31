//! Provider-oriented optical character recognition for Aster.
//!
//! [`OcrClient`] owns bounded source validation and evidence while recognition
//! is delegated to an injected [`OcrProvider`]. The optional PP-OCRv6 feature
//! supplies the local default used by Aster Use without making that model the
//! library's architectural boundary.

#[cfg(feature = "ppocr-v6")]
mod assets;
#[cfg(feature = "cli")]
pub mod cli;
mod client;
#[cfg(feature = "ppocr-v6")]
mod config;
#[cfg(feature = "ppocr-v6")]
mod engine;
#[cfg(feature = "ppocr-v6")]
mod install;
#[cfg(feature = "mcp")]
pub mod mcp;
mod models;
#[cfg(feature = "ppocr-v6")]
mod postprocess;
#[cfg(feature = "ppocr-v6")]
mod ppocr_v6;
#[cfg(feature = "ppocr-v6")]
mod preprocess;
mod provider;

#[cfg(feature = "ppocr-v6")]
pub use assets::{ocr_status, OcrInstallSource, OcrRuntimeStatus};
pub use client::OcrClient;
#[cfg(feature = "ppocr-v6")]
pub use install::{install_ppocr_v6, repair_ppocr_v6, uninstall_managed_ppocr_v6};
#[cfg(feature = "mcp")]
pub use mcp::OcrMcpServer;
pub use models::{OcrBlock, OcrBoundingBox, OcrDiagnostic, OcrPoint, OcrRequest, OcrResult};
#[cfg(feature = "ppocr-v6")]
pub use ppocr_v6::{PpOcrV6Provider, PP_OCR_V6_PROVIDER_ID};
pub use provider::{
    OcrInput, OcrProvider, OcrProviderDescriptor, OcrProviderOutput, OcrProviderStatus,
};

pub use aster_use_core::{Artifact, Readiness, UseError, UseResult};

/// Read one image with the environment OCR provider.
/// Blocks keep the provider confidence. Text without a confidence is omitted.
#[cfg(feature = "ppocr-v6")]
pub fn recognize_image_bytes(
    bytes: &[u8],
    directory: &std::path::Path,
) -> Result<Vec<OcrBlock>, String> {
    if bytes.is_empty() || bytes.len() > 8 * 1024 * 1024 {
        return Err("image bounds".into());
    }
    std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let path = directory.join(format!("aster-doc-ocr-{nanos}"));
    std::fs::write(&path, bytes).map_err(|error| error.to_string())?;
    let read = path.clone();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;
    let extracted = runtime.block_on(async move {
        let client = OcrClient::from_env().map_err(|error| error.to_string())?;
        client
            .extract(OcrRequest { path: read })
            .await
            .map_err(|error| error.to_string())
    });
    let _ = std::fs::remove_file(&path);
    let result = extracted?;
    Ok(result
        .blocks
        .into_iter()
        .filter(|block| block.confidence.is_some() && !block.text.trim().is_empty())
        .collect())
}

/// Source-tree Skill root used by development hosts.
///
/// Installed products should use their release-packaged Skill directory. This
/// path is intentionally only a fallback for source builds and tests.
pub fn source_skill_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("skills")
}

#[cfg(test)]
mod tests {
    #[test]
    fn source_skill_is_owned_by_this_repository() {
        assert!(super::source_skill_root()
            .join("aster-use-ocr/SKILL.md")
            .is_file());
    }
}
