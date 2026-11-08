//! Exact checkpoint and native execution requirements owned by this model.
//!
//! Requirement validation is not a loaded-model or GPU-execution proof. The
//! serving backend must also own verified weights and callable CUDA kernels.

use std::collections::BTreeSet;

use aster_power::error::{PowerError, Result};
use aster_power::inference::{RuntimeDevice, RuntimeDeviceIdentity, RuntimeDeviceKind};
use sha2::{Digest, Sha256};

pub const OFFICIAL_REPOSITORY: &str = "deepseek-ai/DeepSeek-V4.1-Flash-1017";
pub const OFFICIAL_REVISION: &str = "1bae0e1088993b9b82de08f59de5be5a34e42170";
pub const OFFICIAL_CONFIG_SHA256: &str =
    "8be45ce0476004a3f529fd896115a4a2e800a129ad2d3ec05b16050f52e21879";
pub const OFFICIAL_INDEX_SHA256: &str =
    "74b0686a3d2891980d5e303251b075a3bccae2c2ff650747db2620a649b98fa8";
/// Exact UTF-8 `deepseek41.config` string in the pinned XQ GGUF header.
pub const XQ_CONFIG_SHA256: &str =
    "d7637228d27528f6bd259781b5a27258068f50bf637c9c83aab784d81579669d";
pub use crate::weights::xq::{XQ_REPOSITORY, XQ_REVISION};
pub const MAX_CHECKPOINT_CONFIG_BYTES: usize = 64 * 1024;
pub const MAX_CONTEXT_TOKENS: usize = 1_048_576;
pub const H200_DEVICE_COUNT: usize = 8;

pub mod sharing;
pub mod topology;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepSeekModel {
    V41Flash1017Official,
    V41Flash1017Xq,
}

impl DeepSeekModel {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            OFFICIAL_REPOSITORY => Ok(Self::V41Flash1017Official),
            XQ_REPOSITORY => Ok(Self::V41Flash1017Xq),
            _ => Err(PowerError::Config(format!(
                "unsupported DeepSeek model; supported set: {OFFICIAL_REPOSITORY}, {XQ_REPOSITORY}"
            ))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::V41Flash1017Official => OFFICIAL_REPOSITORY,
            Self::V41Flash1017Xq => XQ_REPOSITORY,
        }
    }

    pub fn served_alias(self) -> &'static str {
        match self {
            Self::V41Flash1017Official => "deepseek-v4.1-flash-1017",
            Self::V41Flash1017Xq => "deepseek-v4.1-flash-1017-xq",
        }
    }

    pub fn revision(self) -> &'static str {
        match self {
            Self::V41Flash1017Official => OFFICIAL_REVISION,
            Self::V41Flash1017Xq => XQ_REVISION,
        }
    }

    pub fn config_sha256(self) -> &'static str {
        match self {
            Self::V41Flash1017Official => OFFICIAL_CONFIG_SHA256,
            Self::V41Flash1017Xq => XQ_CONFIG_SHA256,
        }
    }

    pub fn compression_numerics(self) -> crate::state::CompressionNumerics {
        match self {
            Self::V41Flash1017Official => crate::state::CompressionNumerics::OfficialBf16,
            Self::V41Flash1017Xq => crate::state::CompressionNumerics::XqF32,
        }
    }

    pub fn required_capabilities(self) -> &'static [NativeCapability] {
        match self {
            Self::V41Flash1017Official => &NativeCapability::BASE_TEXT,
            Self::V41Flash1017Xq => &NativeCapability::XQ_BASE_TEXT,
        }
    }

    pub fn verify_config(self, bytes: &[u8]) -> Result<VerifiedCheckpointConfig> {
        if bytes.is_empty() || bytes.len() > MAX_CHECKPOINT_CONFIG_BYTES {
            return Err(format_error("checkpoint config exceeds its byte bound"));
        }
        let metadata: serde_json::Value = serde_json::from_slice(bytes)
            .map_err(|_| format_error("checkpoint config is not valid JSON artifact metadata"))?;
        if metadata.get("model_type").and_then(|value| value.as_str()) != Some("deepseek_v41")
            || metadata.get("architectures") != Some(&serde_json::json!(["DeepseekV41ForCausalLM"]))
            || metadata
                .pointer("/text_config/model_type")
                .and_then(|value| value.as_str())
                != Some("deepseek_v41_text")
        {
            return Err(format_error("unsupported DeepSeek checkpoint model schema"));
        }
        let digest = format!("{:x}", Sha256::digest(bytes));
        if digest != self.config_sha256() {
            return Err(format_error(
                "checkpoint config does not match the selected immutable model revision and schema",
            ));
        }
        Ok(VerifiedCheckpointConfig {
            model: self,
            metadata,
        })
    }
}

/// Verifies config bytes only. Tensor payload integrity is a separate boundary.
#[derive(Debug, Clone)]
pub struct VerifiedCheckpointConfig {
    model: DeepSeekModel,
    metadata: serde_json::Value,
}

impl VerifiedCheckpointConfig {
    pub fn model(&self) -> DeepSeekModel {
        self.model
    }

    pub fn sha256(&self) -> &'static str {
        self.model.config_sha256()
    }

    pub fn metadata(&self) -> &serde_json::Value {
        &self.metadata
    }

    pub fn text_architecture(&self) -> Result<topology::TextArchitecture> {
        topology::TextArchitecture::from_checkpoint(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeExecution {
    Cuda,
}

impl NativeExecution {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "cuda_native" => Ok(Self::Cuda),
            _ => Err(PowerError::Config(
                "native execution must be cuda_native; CPU and HTTP fallback are not supported"
                    .into(),
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cuda => "cuda_native",
        }
    }

    /// Validates resolved Power devices and a model-owned kernel inventory.
    /// An inventory is a requirement check, never execution evidence.
    pub fn validate_runtime(
        self,
        expected_ordinals: &[usize],
        devices: &[RuntimeDevice],
        capabilities: &[NativeCapability],
    ) -> Result<()> {
        self.validate_runtime_for_model(
            DeepSeekModel::V41Flash1017Official,
            expected_ordinals,
            devices,
            capabilities,
        )
    }

    /// Requirement validation only: each checkpoint selects its own packed
    /// kernel inventory. This does not advertise a loaded or qualified model.
    pub fn validate_runtime_for_model(
        self,
        model: DeepSeekModel,
        expected_ordinals: &[usize],
        devices: &[RuntimeDevice],
        capabilities: &[NativeCapability],
    ) -> Result<()> {
        let identities = devices
            .iter()
            .map(RuntimeDevice::identity)
            .collect::<Vec<_>>();
        validate_runtime_requirements_for_model(
            cfg!(feature = "cuda"),
            model,
            expected_ordinals,
            &identities,
            capabilities,
        )
    }
}

/// Model-owned kernel groups required by the base text decoder. Optional
/// vision/speculative/replay capabilities are not silently enabled by this set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NativeCapability {
    Embedding,
    RmsNorm,
    Fp8Linear,
    Mxfp4Experts,
    MoeRouting,
    RotaryPosition,
    SparseAttention,
    KvCompression,
    SparseIndexer,
    Mhc,
    Engram,
    OutputHead,
    XqPackedLinear,
    XqPackedGather,
}

impl NativeCapability {
    pub const BASE_TEXT: [Self; 12] = [
        Self::Embedding,
        Self::RmsNorm,
        Self::Fp8Linear,
        Self::Mxfp4Experts,
        Self::MoeRouting,
        Self::RotaryPosition,
        Self::SparseAttention,
        Self::KvCompression,
        Self::SparseIndexer,
        Self::Mhc,
        Self::Engram,
        Self::OutputHead,
    ];
    pub const XQ_BASE_TEXT: [Self; 12] = [
        Self::Embedding,
        Self::RmsNorm,
        Self::XqPackedLinear,
        Self::XqPackedGather,
        Self::MoeRouting,
        Self::RotaryPosition,
        Self::SparseAttention,
        Self::KvCompression,
        Self::SparseIndexer,
        Self::Mhc,
        Self::Engram,
        Self::OutputHead,
    ];

    pub fn parse(value: &str) -> Result<Self> {
        Self::BASE_TEXT
            .into_iter()
            .chain([Self::XqPackedLinear, Self::XqPackedGather])
            .find(|item| item.as_str() == value)
            .ok_or_else(|| {
                PowerError::Config(
                    "unsupported native capability; supported set: embedding, rms_norm, fp8_linear, mxfp4_experts, moe_routing, rotary_position, sparse_attention, kv_compression, sparse_indexer, mhc, engram, output_head, xq_packed_linear, xq_packed_gather"
                        .into(),
                )
            })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Embedding => "embedding",
            Self::RmsNorm => "rms_norm",
            Self::Fp8Linear => "fp8_linear",
            Self::Mxfp4Experts => "mxfp4_experts",
            Self::MoeRouting => "moe_routing",
            Self::RotaryPosition => "rotary_position",
            Self::SparseAttention => "sparse_attention",
            Self::KvCompression => "kv_compression",
            Self::SparseIndexer => "sparse_indexer",
            Self::Mhc => "mhc",
            Self::Engram => "engram",
            Self::OutputHead => "output_head",
            Self::XqPackedLinear => "xq_packed_linear",
            Self::XqPackedGather => "xq_packed_gather",
        }
    }
}

#[cfg(test)]
fn validate_runtime_requirements(
    cuda_compiled: bool,
    expected_ordinals: &[usize],
    devices: &[RuntimeDeviceIdentity],
    capabilities: &[NativeCapability],
) -> Result<()> {
    validate_runtime_requirements_for_model(
        cuda_compiled,
        DeepSeekModel::V41Flash1017Official,
        expected_ordinals,
        devices,
        capabilities,
    )
}

fn validate_runtime_requirements_for_model(
    cuda_compiled: bool,
    model: DeepSeekModel,
    expected_ordinals: &[usize],
    devices: &[RuntimeDeviceIdentity],
    capabilities: &[NativeCapability],
) -> Result<()> {
    if !cuda_compiled {
        return Err(unavailable(
            "native DeepSeek requires the aster-deepseek cuda build feature",
        ));
    }
    let expected = expected_ordinals.iter().copied().collect::<BTreeSet<_>>();
    if expected_ordinals.len() != H200_DEVICE_COUNT
        || expected.len() != H200_DEVICE_COUNT
        || expected.iter().any(|ordinal| *ordinal >= H200_DEVICE_COUNT)
        || devices.len() != H200_DEVICE_COUNT
    {
        return Err(unavailable(
            "native deployment requires eight distinct visible CUDA devices",
        ));
    }
    for (ordinal, device) in expected_ordinals.iter().zip(devices) {
        if device.kind != RuntimeDeviceKind::Cuda || device.ordinal != Some(*ordinal) {
            return Err(unavailable(
                "resolved native device differs from the requested CUDA rank",
            ));
        }
    }
    let supplied = capabilities.iter().copied().collect::<BTreeSet<_>>();
    if supplied.len() != capabilities.len() {
        return Err(unavailable(
            "native kernel inventory contains duplicate capabilities",
        ));
    }
    let missing = model
        .required_capabilities()
        .iter()
        .copied()
        .filter(|item| !supplied.contains(item))
        .map(NativeCapability::as_str)
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(unavailable(&format!(
            "native kernel inventory is incomplete: {}",
            missing.join(", ")
        )));
    }
    Ok(())
}

fn unavailable(message: &str) -> PowerError {
    PowerError::BackendNotAvailable(message.to_owned())
}

fn format_error(message: &str) -> PowerError {
    PowerError::InvalidFormat(message.to_owned())
}

#[cfg(test)]
mod tests;
