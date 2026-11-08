//! Exact HF backbone binding, adapted from DeepInfer's Apache-2.0
//! hugging_face/config.py and v4_modules/cross_layer.py at revision
//! 9bd9cf243fc53ef9afbf8f04d391d989117d6ab9.
//! Only hash-verified checkpoint metadata can construct a public architecture.
//! This binds cache owners and numerical profiles; it does not execute a model.

use aster_power::error::{PowerError, Result};
use serde_json::Value;

use crate::layers::kv::MainKvFormat;
use crate::layers::mhc::MhcNumerics;
use crate::layers::CudaLayerDevice;
use crate::state::index::IndexKvState;
use crate::state::kv::{KvCacheKind, KvState};
use crate::state::{CompressionKind, CompressionNumerics, CompressionState};

use super::VerifiedCheckpointConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttentionRopeProfile {
    WindowBase,
    CompressedYarn,
}

impl AttentionRopeProfile {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "window_base" => Ok(Self::WindowBase),
            "compressed_yarn" => Ok(Self::CompressedYarn),
            _ => Err(invalid(
                "unsupported attention RoPE profile; supported set: window_base, compressed_yarn",
            )),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WindowBase => "window_base",
            Self::CompressedYarn => "compressed_yarn",
        }
    }
    pub fn theta(self) -> u32 {
        match self {
            Self::WindowBase => 10_000,
            Self::CompressedYarn => 160_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompressedSources {
    kv: usize,
    index_k: usize,
    index_topk: usize,
    candidate: Option<usize>,
}

impl CompressedSources {
    pub fn kv(&self) -> usize {
        self.kv
    }
    pub fn index_k(&self) -> usize {
        self.index_k
    }
    pub fn index_topk(&self) -> usize {
        self.index_topk
    }
    pub fn candidate(&self) -> Option<usize> {
        self.candidate
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngramBinding {
    table: usize,
    rows: usize,
}

impl EngramBinding {
    pub fn table(&self) -> usize {
        self.table
    }
    pub fn rows(&self) -> usize {
        self.rows
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerBinding {
    layer: usize,
    compression: Option<CompressionKind>,
    sources: Option<CompressedSources>,
    engram: Option<EngramBinding>,
    compression_numerics: CompressionNumerics,
}

impl LayerBinding {
    pub fn compression_numerics(&self) -> CompressionNumerics {
        self.compression_numerics
    }
    pub fn layer(&self) -> usize {
        self.layer
    }
    pub fn compression(&self) -> Option<CompressionKind> {
        self.compression
    }
    pub fn sources(&self) -> Option<&CompressedSources> {
        self.sources.as_ref()
    }
    pub fn engram(&self) -> Option<&EngramBinding> {
        self.engram.as_ref()
    }
    pub fn rope_profile(&self) -> AttentionRopeProfile {
        if self.compression.is_some() {
            AttentionRopeProfile::CompressedYarn
        } else {
            AttentionRopeProfile::WindowBase
        }
    }

    pub fn new_rope_cache(
        &self,
        device: &CudaLayerDevice,
        context: usize,
        cancellation: &tokio_util::sync::CancellationToken,
    ) -> Result<crate::layers::rope::factory::AttentionRopeCache> {
        crate::layers::rope::factory::AttentionRopeCache::new(
            device,
            self.rope_profile(),
            context,
            cancellation,
        )
    }
    /// HF explicitly disables shared SWA hidden states. Every layer projects
    /// its own window KV, including compressed-KV consumers after layer 20.
    pub fn window_hidden_source(&self) -> usize {
        self.layer
    }
    pub fn is_kv_source(&self) -> bool {
        self.sources
            .as_ref()
            .is_some_and(|sources| sources.kv == self.layer)
    }
    pub fn is_index_k_source(&self) -> bool {
        self.sources
            .as_ref()
            .is_some_and(|sources| sources.index_k == self.layer)
    }
    pub fn is_index_topk_source(&self) -> bool {
        self.sources
            .as_ref()
            .is_some_and(|sources| sources.index_topk == self.layer)
    }
    pub fn is_candidate_source(&self) -> bool {
        self.sources
            .as_ref()
            .is_some_and(|sources| sources.candidate == Some(self.layer))
    }

    pub fn new_window_state(
        &self,
        device: &CudaLayerDevice,
        context: usize,
        step: usize,
    ) -> Result<KvState> {
        KvState::new(
            device,
            KvCacheKind::Window,
            MainKvFormat::Fp8Ue8m0,
            context,
            step,
        )
    }

    /// Consumers reuse their explicit source; never allocate independent,
    /// unwritten compressed state for a consumer layer.
    pub fn new_compression_state(
        &self,
        device: &CudaLayerDevice,
        context: usize,
        step: usize,
    ) -> Result<CompressionState> {
        let kind = self.owned_compression()?;
        CompressionState::new_with_numerics(
            device,
            kind,
            self.compression_numerics,
            512,
            context,
            step,
        )
    }

    pub fn new_compressed_kv_state(
        &self,
        device: &CudaLayerDevice,
        format: MainKvFormat,
        context: usize,
        step: usize,
    ) -> Result<KvState> {
        let kind = match self.owned_compression()? {
            CompressionKind::C1 => KvCacheKind::C1,
            CompressionKind::C2 => KvCacheKind::C2,
        };
        KvState::new(device, kind, format, context, step)
    }

    pub fn new_index_kv_state(
        &self,
        device: &CudaLayerDevice,
        context: usize,
        step: usize,
    ) -> Result<IndexKvState> {
        if !self.is_index_k_source() {
            return Err(invalid(
                "only an explicit index-K source may construct index cache state",
            ));
        }
        let kind = self
            .compression
            .ok_or_else(|| invalid("index-K source is missing its compression schedule"))?;
        IndexKvState::new(device, kind, context, step)
    }

    fn owned_compression(&self) -> Result<CompressionKind> {
        self.compression
            .filter(|_| self.is_kv_source())
            .ok_or_else(|| {
                invalid(
                    "only an explicit compressed-KV source may construct compressor/cache state",
                )
            })
    }
}

#[derive(Debug, Clone)]
pub struct TextArchitecture {
    layers: Vec<LayerBinding>,
    compression_numerics: CompressionNumerics,
}

impl TextArchitecture {
    pub fn from_checkpoint(config: &VerifiedCheckpointConfig) -> Result<Self> {
        Self::from_metadata_with_numerics(config.metadata(), config.model().compression_numerics())
    }

    #[cfg(test)]
    fn from_metadata(metadata: &Value) -> Result<Self> {
        Self::from_metadata_with_numerics(metadata, CompressionNumerics::OfficialBf16)
    }

    fn from_metadata_with_numerics(
        metadata: &Value,
        compression_numerics: CompressionNumerics,
    ) -> Result<Self> {
        let text = metadata
            .get("text_config")
            .ok_or_else(|| invalid("missing text_config"))?;
        // Reject future numerical/layout changes at this binding, even if a
        // later model admission implementation supports another revision.
        for (field, expected) in [
            ("num_hidden_layers", 40),
            ("hidden_size", 5120),
            ("head_dim", 512),
            ("qk_rope_head_dim", 64),
            ("sliding_window", 128),
            ("num_nextn_predict_layers", 3),
            ("rope_theta", 10_000),
            ("compress_rope_theta", 160_000),
            ("candidate_source_layer_id", 20),
            ("candidate_topk_blocks", 2048),
            ("candidate_block_size", 8),
        ] {
            if integer(text.get(field))? != expected {
                return Err(invalid("unsupported official text topology geometry"));
            }
        }
        if text
            .get("hc_shifted")
            .is_some_and(|value| value != &Value::Bool(true))
        {
            return Err(invalid("official native binding requires shifted mHC"));
        }
        let ratios = integers(text.get("compress_ratios"))?;
        if ratios.len() != 43
            || ratios[40..] != [0, 0, 0]
            || ratios[..2] != [0, 0]
            || ratios[2..20] != [2; 18]
            || ratios[20..40] != [1; 20]
        {
            return Err(invalid("unsupported backbone/draft compression schedule"));
        }
        let kv = sources(text.get("kv_source_layer_ids"))?;
        let index = sources(text.get("index_source_layer_ids"))?;
        // HF's default is KV sources, not the generic topology's indexer map.
        let index_k = sources(
            text.get("index_k_source_layer_ids")
                .or_else(|| text.get("kv_source_layer_ids")),
        )?;
        if kv != [2, 8, 14, 20] || index != [2, 8, 14, 20, 24, 28, 32, 36] || index_k != kv {
            return Err(invalid("unsupported official KV/index source sets"));
        }
        validate_refinement(&kv, &index_k, &index, &ratios)?;
        let engram_ids = integers(text.get("engram_layer_ids"))?;
        let engram_rows = integers(text.get("engram_num_embeddings"))?;
        if engram_ids != [1, 14] || engram_rows != [384_006_168, 384_016_682] {
            return Err(invalid("unsupported official Engram table binding"));
        }
        let layers = (0..40)
            .map(|layer| {
                let compression = match ratios[layer] {
                    0 => None,
                    1 => Some(CompressionKind::C1),
                    _ => Some(CompressionKind::C2),
                };
                let shared = compression.map(|_| CompressedSources {
                    kv: preceding(&kv, layer),
                    index_k: preceding(&index_k, layer),
                    index_topk: preceding(&index, layer),
                    candidate: (layer >= 20).then_some(20),
                });
                let engram =
                    engram_ids
                        .iter()
                        .position(|&id| id == layer)
                        .map(|table| EngramBinding {
                            table,
                            rows: engram_rows[table],
                        });
                LayerBinding {
                    layer,
                    compression,
                    sources: shared,
                    engram,
                    compression_numerics,
                }
            })
            .collect();
        Ok(Self {
            layers,
            compression_numerics,
        })
    }

    pub fn compression_numerics(&self) -> CompressionNumerics {
        self.compression_numerics
    }

    pub fn layers(&self) -> &[LayerBinding] {
        &self.layers
    }
    pub fn layer(&self, layer: usize) -> Result<&LayerBinding> {
        self.layers.get(layer).ok_or_else(|| {
            invalid("backbone layer must be in 0..40; LM head and DSpark are separate bindings")
        })
    }
    pub fn mhc_numerics(&self) -> MhcNumerics {
        MhcNumerics {
            norm_epsilon: 1e-20,
            pre_epsilon: 1e-6,
            sinkhorn_epsilon: 1e-6,
            sinkhorn_iterations: 20,
            post_multiplier: 2.0,
        }
    }
    pub fn use_query_head_norm(&self) -> bool {
        false
    }
    pub fn use_exponential_norm_weights(&self) -> bool {
        false
    }
    pub fn shifted_mhc(&self) -> bool {
        true
    }
    pub fn shared_window_hidden(&self) -> bool {
        false
    }
}

fn integer(value: Option<&Value>) -> Result<usize> {
    value
        .and_then(Value::as_u64)
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| invalid("topology requires nonnegative integer artifact fields"))
}
fn integers(value: Option<&Value>) -> Result<Vec<usize>> {
    let values = value
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("topology requires an integer array"))?;
    if values.len() > 43 {
        return Err(invalid("topology array exceeds model layer bound"));
    }
    values.iter().map(|value| integer(Some(value))).collect()
}
fn sources(value: Option<&Value>) -> Result<Vec<usize>> {
    let ids = integers(value)?;
    if ids.first() != Some(&2)
        || ids.iter().any(|&id| id >= 40)
        || ids.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(invalid(
            "source sets must start at layer 2 and contain increasing backbone IDs",
        ));
    }
    Ok(ids)
}
fn preceding(sources: &[usize], layer: usize) -> usize {
    // Callers validated a nonempty set beginning at 2 and layer >= 2.
    sources[sources.partition_point(|&source| source <= layer) - 1]
}
fn validate_refinement(
    kv: &[usize],
    index_k: &[usize],
    index: &[usize],
    ratios: &[usize],
) -> Result<()> {
    for layer in 2..40 {
        let (k, i, topk) = (
            preceding(kv, layer),
            preceding(index_k, layer),
            preceding(index, layer),
        );
        if preceding(kv, i) != k || preceding(index_k, topk) != i || ratios[layer] != ratios[k] {
            return Err(invalid(
                "index top-k/index-K/KV sharing crosses a coarser segment or compression ratio",
            ));
        }
    }
    if preceding(kv, 20) != 20 || !index.contains(&20) {
        return Err(invalid(
            "YOCO boundary and candidate production require source layer 20",
        ));
    }
    Ok(())
}
fn invalid(message: &str) -> PowerError {
    PowerError::InvalidRequest(message.into())
}

#[cfg(test)]
mod tests;
