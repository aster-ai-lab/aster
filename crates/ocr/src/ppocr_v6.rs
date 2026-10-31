use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use aster_use_core::{Readiness, UseError, UseResult};
use async_trait::async_trait;

use crate::assets::{ocr_status, resolve_model_assets, OcrInstallSource};
use crate::config::MODEL_FAMILY;
use crate::engine::{EngineBlock, PpOcrV6Engine};
use crate::models::{OcrBlock, OcrBoundingBox, OcrPoint};
use crate::preprocess::decode_image;
use crate::provider::{
    OcrInput, OcrProvider, OcrProviderDescriptor, OcrProviderOutput, OcrProviderStatus,
};

pub const PP_OCR_V6_PROVIDER_ID: &str = "pp-ocr-v6";
const ENGINE_NAME: &str = "onnx-runtime";

/// Local PP-OCRv6 provider shipped as the default Aster Use integration.
#[derive(Clone)]
pub struct PpOcrV6Provider {
    descriptor: OcrProviderDescriptor,
    loaded: Arc<Mutex<Option<LoadedEngine>>>,
    assets: Option<crate::assets::ModelAssets>,
    max_detection_side: u32,
}

struct LoadedEngine {
    model_dir: PathBuf,
    engine: PpOcrV6Engine,
}

impl PpOcrV6Provider {
    pub fn from_env() -> UseResult<Self> {
        Ok(Self {
            descriptor: OcrProviderDescriptor::new(PP_OCR_V6_PROVIDER_ID, ENGINE_NAME, false)?,
            loaded: Arc::new(Mutex::new(None)),
            assets: None,
            max_detection_side: 4_000,
        })
    }

    /// Uses host-owned packaged assets without changing process environment variables.
    /// Detection is bounded to 1536 pixels; recognition crops retain source resolution.
    pub fn from_paths(model_dir: &std::path::Path, runtime: &std::path::Path) -> UseResult<Self> {
        let assets = crate::assets::validate_assets(model_dir, OcrInstallSource::Packaged)?;
        if !runtime.is_absolute() || !runtime.is_file() {
            return Err(UseError::new(
                "use.ocr.runtime_missing",
                "The packaged ONNX Runtime library is missing.",
            ));
        }
        ort::init_from(runtime.to_string_lossy())
            .with_name("aster-local-ocr")
            .commit()
            .map_err(|error| UseError::new("use.ocr.runtime_failed", error.to_string()))?;
        Ok(Self {
            assets: Some(assets),
            max_detection_side: 1_536,
            ..Self::from_env()?
        })
    }

    /// Loads both sessions once; callers can await this on a background startup task.
    pub async fn warmup(&self) -> UseResult<()> {
        let provider = self.clone();
        tokio::task::spawn_blocking(move || {
            let assets = provider.resolve_assets()?;
            let mut loaded = provider.loaded.lock().map_err(|_| {
                UseError::new("use.ocr.runtime_failed", "The OCR engine lock is poisoned.")
            })?;
            if loaded
                .as_ref()
                .is_none_or(|engine| engine.model_dir != assets.root)
            {
                *loaded = Some(LoadedEngine {
                    model_dir: assets.root.clone(),
                    engine: PpOcrV6Engine::load(&assets)?,
                });
            }
            Ok(())
        })
        .await
        .map_err(|error| UseError::new("use.ocr.runtime_failed", error.to_string()))?
    }

    fn resolve_assets(&self) -> UseResult<crate::assets::ModelAssets> {
        self.assets
            .clone()
            .map(Ok)
            .unwrap_or_else(resolve_model_assets)
    }
}

#[async_trait]
impl OcrProvider for PpOcrV6Provider {
    fn descriptor(&self) -> OcrProviderDescriptor {
        self.descriptor.clone()
    }

    fn diagnostic(&self) -> OcrProviderStatus {
        if let Some(assets) = &self.assets {
            return OcrProviderStatus {
                readiness: Readiness::Ready,
                model: Some(MODEL_FAMILY.into()),
                model_dir: Some(assets.root.clone()),
                message: "Packaged local PP-OCRv6 assets are available.".into(),
                suggestions: Vec::new(),
            };
        }
        let status = ocr_status();
        let (readiness, suggestions) = if status.available {
            (Readiness::Ready, Vec::new())
        } else if status.source == OcrInstallSource::Missing {
            (
                Readiness::Missing,
                vec![
                    "Run 'aster install use/ocr' to install the pinned local model bundle."
                        .to_string(),
                ],
            )
        } else {
            (
                Readiness::Broken,
                vec![
                    "Run 'aster install use/ocr --force' to restore the pinned local model bundle."
                        .to_string(),
                ],
            )
        };
        OcrProviderStatus {
            readiness,
            model: Some(status.model),
            model_dir: status.model_dir,
            message: if status.available {
                "Local PP-OCRv6 detection and recognition models are ready.".to_string()
            } else {
                status.detail
            },
            suggestions,
        }
    }

    async fn recognize(&self, input: OcrInput) -> UseResult<OcrProviderOutput> {
        let loaded = Arc::clone(&self.loaded);
        let provider = self.clone();
        tokio::task::spawn_blocking(move || {
            let assets = provider.resolve_assets()?;
            let mut loaded = loaded.lock().map_err(|_| {
                UseError::new(
                    "use.ocr.runtime_failed",
                    "The local PP-OCRv6 engine lock is poisoned.",
                )
            })?;
            // Bound decoded images as well as inference when clones share a provider.
            let image = decode_image(input.bytes())?;
            let should_load = loaded
                .as_ref()
                .map(|loaded| loaded.model_dir != assets.root)
                .unwrap_or(true);
            if should_load {
                *loaded = Some(LoadedEngine {
                    model_dir: assets.root.clone(),
                    engine: PpOcrV6Engine::load(&assets)?,
                });
            }
            let engine = loaded.as_mut().ok_or_else(|| {
                UseError::new(
                    "use.ocr.runtime_failed",
                    "The local PP-OCRv6 engine failed to initialize.",
                )
            })?;
            build_output(engine.engine.extract(&image, provider.max_detection_side)?)
        })
        .await
        .map_err(|error| {
            UseError::new(
                "use.ocr.runtime_failed",
                format!("The local PP-OCRv6 inference task failed: {error}"),
            )
        })?
    }
}

fn build_output(blocks: Vec<EngineBlock>) -> UseResult<OcrProviderOutput> {
    let blocks = blocks
        .into_iter()
        .map(|block| {
            let [first, second, third, fourth] = block.polygon;
            let polygon = [
                ocr_point(first)?,
                ocr_point(second)?,
                ocr_point(third)?,
                ocr_point(fourth)?,
            ];
            let min_x = polygon.iter().map(|point| point.x).min().unwrap_or(0);
            let max_x = polygon.iter().map(|point| point.x).max().unwrap_or(0);
            let min_y = polygon.iter().map(|point| point.y).min().unwrap_or(0);
            let max_y = polygon.iter().map(|point| point.y).max().unwrap_or(0);
            Ok(OcrBlock {
                page: 1,
                text: block.text,
                confidence: Some(block.confidence),
                detection_confidence: Some(block.detection_confidence),
                polygon: Some(polygon),
                bounding_box: Some(OcrBoundingBox {
                    x: min_x,
                    y: min_y,
                    width: max_x.saturating_sub(min_x),
                    height: max_y.saturating_sub(min_y),
                }),
            })
        })
        .collect::<UseResult<Vec<_>>>()?;
    let text = blocks
        .iter()
        .filter(|block| !block.text.trim().is_empty())
        .map(|block| block.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    Ok(OcrProviderOutput {
        model: Some(MODEL_FAMILY.to_string()),
        text,
        blocks,
        warnings: Vec::new(),
    })
}

fn ocr_point(point: imageproc::point::Point<f32>) -> UseResult<OcrPoint> {
    Ok(OcrPoint {
        x: finite_coordinate(point.x)?,
        y: finite_coordinate(point.y)?,
    })
}

fn finite_coordinate(value: f32) -> UseResult<u32> {
    if !value.is_finite() || value < 0.0 || value > u32::MAX as f32 {
        return Err(UseError::new(
            "use.ocr.provider_output_invalid",
            "PP-OCRv6 returned an invalid polygon coordinate.",
        ));
    }
    Ok(value.round() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_provider_is_local_and_explicit() {
        let provider = PpOcrV6Provider::from_env().unwrap();
        assert_eq!(provider.descriptor().id, PP_OCR_V6_PROVIDER_ID);
        assert_eq!(provider.descriptor().engine, ENGINE_NAME);
        assert!(!provider.descriptor().sends_source_off_device);
    }
}
