//! Packed index FP4 E2M1 / group-32 UE8M0 logical decoding on the bound device.
//!
//! Two adjacent logical values occupy low/high nibbles respectively. Data
//! and scale sections remain separate U8 tensors, as in an index cache block;
//! these are owned rows with an explicit deterministic encoder, not a paged cache. No
//! unquantized BF16 input can construct this packed-row handle. The public
//! boundary requires actual CUDA; small CPU proofs use the private row data.
//! Format follows the pinned MIT TileKernels torch/cast.py reference at
//! c1cf914a82e6901c679445f872051ce44ac2cc63 and DeepInfer's index-cache profile.

use aster_power::error::Result;
use aster_power::inference::InferenceLimits;
use candle_core::{DType, Device, Tensor};
use tokio_util::sync::CancellationToken;

use super::{invalid, tensor_error, validate_finite, CudaLayerDevice};
use crate::model::MAX_CONTEXT_TOKENS;

pub const INDEX_HEAD_DIM: usize = 128;
const DATA_BYTES: usize = INDEX_HEAD_DIM / 2;
const SCALE_BYTES: usize = INDEX_HEAD_DIM / 32;

pub struct Fp4IndexRows {
    data: Fp4IndexRowsData,
}

impl Fp4IndexRows {
    /// Deterministic group-32 RNE encoder for post-norm/RoPE BF16 rows.
    /// Tensor composition is a correctness control, not a fused SM90 cast.
    pub fn encode(
        device: &CudaLayerDevice,
        input: &Tensor,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        if !input.device().same_device(device.tensor_device()) {
            return Err(invalid(
                "index FP4 encoder input differs from its CUDA device",
            ));
        }
        Ok(Self {
            data: encode_rows(input, device.limits(), cancellation)?,
        })
    }

    pub fn from_packed(device: &CudaLayerDevice, data: &Tensor, scales: &Tensor) -> Result<Self> {
        if !data.device().same_device(device.tensor_device()) {
            return Err(invalid("packed index rows differ from their CUDA device"));
        }
        Ok(Self {
            data: Fp4IndexRowsData::new(device.limits(), data, scales)?,
        })
    }

    pub fn row_count(&self) -> usize {
        self.data.rows
    }
    pub fn data_bytes(&self) -> &Tensor {
        &self.data.data
    }
    pub fn scale_bytes(&self) -> &Tensor {
        &self.data.scales
    }
    pub(crate) fn rows_data(&self) -> &Fp4IndexRowsData {
        &self.data
    }
}

fn encode_rows(
    input: &Tensor,
    limits: &InferenceLimits,
    cancellation: &CancellationToken,
) -> Result<Fp4IndexRowsData> {
    check_cancel(cancellation)?;
    let (rows, width) = input.dims2().map_err(tensor_error)?;
    if input.dtype() != DType::BF16 || width != INDEX_HEAD_DIM || rows > MAX_CONTEXT_TOKENS * 32 {
        return Err(invalid(
            "index encoder requires BF16 [rows,128] post-norm/RoPE values",
        ));
    }
    if rows > 0 {
        limits.checked_elements(input.dims(), "index encoder input")?;
    }
    let bytes = (rows as u64)
        .checked_mul((INDEX_HEAD_DIM * 96) as u64)
        .and_then(|bytes| bytes.checked_add(4096))
        .ok_or_else(|| invalid("index encoder tensor bytes overflowed"))?;
    limits.checked_state_bytes(bytes, "index FP4 encoder live tensors")?;
    if rows == 0 {
        return Fp4IndexRowsData::new(
            limits,
            &Tensor::zeros((0, DATA_BYTES), DType::U8, input.device()).map_err(tensor_error)?,
            &Tensor::zeros((0, SCALE_BYTES), DType::U8, input.device()).map_err(tensor_error)?,
        );
    }
    validate_finite(input, false, "index encoder input")?;
    let math = || -> candle_core::Result<(Tensor, Tensor)> {
        let groups = rows * SCALE_BYTES;
        let x = input.detach().to_dtype(DType::F32)?.reshape((groups, 32))?;
        // Match pinned TileKernels: clamp amax at 6*2^-126, then FP32
        // reciprocal multiplication before rounding the scale up to a power.
        let amax = x
            .abs()?
            .max_keepdim(1)?
            .clamp(6_f32 * f32::MIN_POSITIVE, f32::MAX)?;
        let target = amax.affine(1. / 6., 0.)?;
        // Finite BF16 / 6 never needs a dequant exponent above 126 (byte253).
        let (scale_codes, sf) = rounded_packed_scales(&target, 253)?;
        let normalized = x.broadcast_mul(&sf.recip()?)?;
        let data = encode_e2m1(&normalized.reshape((rows, INDEX_HEAD_DIM))?)?;
        Ok((
            data,
            scale_codes
                .reshape((rows, SCALE_BYTES))?
                .to_dtype(DType::U8)?,
        ))
    };
    let (data, scales) = math().map_err(tensor_error)?;
    check_cancel(cancellation)?;
    let encoded = Fp4IndexRowsData::new(limits, &data, &scales)?;
    check_cancel(cancellation)?;
    Ok(encoded)
}

fn check_cancel(cancellation: &CancellationToken) -> Result<()> {
    if cancellation.is_cancelled() {
        return Err(aster_power::error::PowerError::InferenceCancelled);
    }
    Ok(())
}

pub(crate) struct Fp4IndexRowsData {
    data: Tensor,
    scales: Tensor,
    rows: usize,
}

impl Fp4IndexRowsData {
    // Crate-private only for tensor controls and sibling CPU tests. Serving
    // receives a public Fp4IndexRows constructed through the CUDA boundary.
    pub(crate) fn new(limits: &InferenceLimits, data: &Tensor, scales: &Tensor) -> Result<Self> {
        limits.validate()?;
        let (rows, width) = data.dims2().map_err(tensor_error)?;
        if data.dtype() != DType::U8
            || scales.dtype() != DType::U8
            || width != DATA_BYTES
            || scales.dims() != [rows, SCALE_BYTES]
            || !scales.device().same_device(data.device())
            || rows > MAX_CONTEXT_TOKENS * 32
        {
            return Err(invalid("index FP4 requires U8 [rows,64] data and same-device U8 [rows,4] group-32 UE8M0 scales"));
        }
        if rows > 0 {
            limits.checked_elements(&[rows, INDEX_HEAD_DIM], "index FP4 logical rows")?;
        }
        limits.checked_state_bytes((rows as u64) * 256, "index packed-row construction")?;
        let data = data.force_contiguous().map_err(tensor_error)?.detach();
        let scales = scales.force_contiguous().map_err(tensor_error)?.detach();
        if rows > 0 {
            // The pinned cast clamps live FP4 groups before exponent
            // extraction, so committed scales have positive normal FP32
            // exponent bytes. Zero is an unwritten/padding convention in the
            // packed bitcast path, unlike general OCP E8M0's 2^-127 value.
            // Refuse that ambiguous zero rather than reinterpreting it.
            let valid = scales
                .ge(1_u8)
                .and_then(|value| value.mul(&scales.le(254_u8)?))
                .and_then(|value| value.to_dtype(DType::U32))
                .and_then(|value| value.sum_all())
                .and_then(|value| value.to_scalar::<u32>())
                .map_err(tensor_error)?;
            if valid as usize != rows * SCALE_BYTES {
                return Err(invalid(
                    "committed index packed scales require exponent bytes 1..=254; zero/padding and 255 refuse",
                ));
            }
        }
        Ok(Self { data, scales, rows })
    }

    pub(crate) fn row_count(&self) -> usize {
        self.rows
    }
    pub(crate) fn data_bytes(&self) -> &Tensor {
        &self.data
    }
    pub(crate) fn scale_bytes(&self) -> &Tensor {
        &self.scales
    }
    pub(crate) fn device(&self) -> &Device {
        self.data.device()
    }
    pub(crate) fn retained_bytes(&self) -> u64 {
        (self.rows as u64) * ((DATA_BYTES + SCALE_BYTES) as u64)
    }

    pub(crate) fn decode_prefix(&self, count: usize, limits: &InferenceLimits) -> Result<Tensor> {
        if count > self.rows {
            return Err(invalid("index decode prefix exceeds stored rows"));
        }
        self.check_decode_budget(count, limits)?;
        let data = self.data.narrow(0, 0, count).map_err(tensor_error)?;
        let scales = self.scales.narrow(0, 0, count).map_err(tensor_error)?;
        decode_parts(&data, &scales, count)
    }

    pub(crate) fn decode_selected(
        &self,
        indices: &Tensor,
        limits: &InferenceLimits,
    ) -> Result<Tensor> {
        let count = indices.dims1().map_err(tensor_error)?;
        if indices.dtype() != DType::U32 || !indices.device().same_device(self.device()) {
            return Err(invalid(
                "packed index gather requires same-device U32 row IDs",
            ));
        }
        self.check_decode_budget(count, limits)?;
        if count == 0 {
            return Tensor::zeros((0, INDEX_HEAD_DIM), DType::BF16, self.device())
                .map_err(tensor_error);
        }
        let valid = indices
            .lt(self.rows as u32)
            .and_then(|value| value.to_dtype(DType::U32))
            .and_then(|value| value.sum_all())
            .and_then(|value| value.to_scalar::<u32>())
            .map_err(tensor_error)?;
        if valid as usize != count {
            return Err(invalid(
                "packed index gather contains out-of-range/padded row IDs",
            ));
        }
        let data = self.data.index_select(indices, 0).map_err(tensor_error)?;
        let scales = self.scales.index_select(indices, 0).map_err(tensor_error)?;
        decode_parts(&data, &scales, count)
    }

    fn check_decode_budget(&self, count: usize, limits: &InferenceLimits) -> Result<()> {
        if count > 0 {
            limits.checked_elements(&[count, INDEX_HEAD_DIM], "decoded index rows")?;
        }
        // Named tensors only, including retained packed storage; hidden backend
        // workspace/allocator overhead needs separate model-wide qualification.
        let bytes = (count as u64)
            .checked_mul((INDEX_HEAD_DIM * 64) as u64)
            .and_then(|n| n.checked_add(self.retained_bytes()))
            .and_then(|n| n.checked_add(4096))
            .ok_or_else(|| invalid("index decode bytes overflowed"))?;
        limits.checked_state_bytes(bytes, "index FP4 decode live tensors")?;
        Ok(())
    }
}

fn decode_parts(data: &Tensor, scales: &Tensor, rows: usize) -> Result<Tensor> {
    if rows == 0 {
        return Tensor::zeros((0, INDEX_HEAD_DIM), DType::BF16, data.device())
            .map_err(tensor_error);
    }
    let math = || -> candle_core::Result<Tensor> {
        let device = data.device();
        let values = decode_e2m1(data)?.reshape((rows, SCALE_BYTES, 32))?;
        // Validated packed exponent bytes 1..254 map to exact FP32 powers.
        // Zero/padding and 255 were refused before any device lookup; this
        // packed-cache boundary does not replace general OCP E8M0 semantics.
        let scale_lut: Vec<_> = (0..255_u32)
            .map(|code| f32::from_bits(code << 23))
            .collect();
        let scale_ids = scales.flatten_all()?.to_dtype(DType::U32)?;
        let scales = Tensor::from_vec(scale_lut, 255, device)?
            .index_select(&scale_ids, 0)?
            .reshape((rows, SCALE_BYTES, 1))?;
        values
            .broadcast_mul(&scales)?
            .reshape((rows, INDEX_HEAD_DIM))?
            .to_dtype(DType::BF16)
    };
    let output = math().map_err(tensor_error)?.detach();
    validate_finite(&output, false, "decoded FP4 index values")?;
    Ok(output)
}

// Shared, crate-internal format math. Public owning codecs enforce actual
// CUDA, finite inputs, shapes, scalar ranges and allocation budgets first.
pub(super) fn rounded_packed_scales(
    target: &Tensor,
    max_byte: u8,
) -> candle_core::Result<(Tensor, Tensor)> {
    let (groups, width) = target.dims2()?;
    if width != 1 || groups == 0 || target.dtype() != DType::F32 || !(1..=254).contains(&max_byte) {
        candle_core::bail!(
            "packed scale search requires positive FP32 group targets and exponent bound 1..254"
        );
    }
    let device = target.device();
    let lut = Tensor::from_vec(
        (0..255_u32)
            .map(|code| f32::from_bits(code << 23))
            .collect::<Vec<_>>(),
        255,
        device,
    )?;
    let mut low = Tensor::ones((groups, 1), DType::F32, device)?;
    let mut high = Tensor::full(f32::from(max_byte), (groups, 1), device)?;
    for _ in 0..8 {
        let middle = low.add(&high)?.affine(0.5, 0.)?.floor()?;
        let scale = lut
            .index_select(&middle.flatten_all()?.to_dtype(DType::U32)?, 0)?
            .reshape((groups, 1))?;
        let below = scale.lt(target)?;
        low = below.where_cond(&middle.affine(1., 1.)?, &low)?;
        high = below.where_cond(&high, &middle)?;
    }
    let scales = lut
        .index_select(&low.flatten_all()?.to_dtype(DType::U32)?, 0)?
        .reshape((groups, 1))?;
    Ok((low, scales))
}

pub(super) fn encode_e2m1(normalized: &Tensor) -> candle_core::Result<Tensor> {
    #[cfg(feature = "xq-fused-activation-codecs")]
    if normalized.device().is_cuda() {
        return crate::cuda::activation::encode_e2m1(normalized);
    }
    encode_e2m1_tensor(normalized)
}

pub(crate) fn encode_e2m1_tensor(normalized: &Tensor) -> candle_core::Result<Tensor> {
    let (rows, dim) = normalized.dims2()?;
    let device = normalized.device();
    let absolute = normalized.abs()?;
    let mut codes = Tensor::zeros((rows, dim), DType::F32, device)?;
    for (lower, midpoint) in [0.25_f32, 0.75, 1.25, 1.75, 2.5, 3.5, 5.]
        .into_iter()
        .enumerate()
    {
        let higher = if lower % 2 == 0 {
            absolute.gt(midpoint)?
        } else {
            absolute.ge(midpoint)?
        };
        codes = higher.where_cond(
            &Tensor::full((lower + 1) as f32, (rows, dim), device)?,
            &codes,
        )?;
    }
    // The owning codecs bound normalized magnitudes so reciprocal underflow
    // is absent; reciprocal sign also preserves negative zero.
    let negative = normalized.recip()?.lt(0_f32)?;
    codes = negative.where_cond(&codes.affine(1., 8.)?, &codes)?;
    let pairs = codes.reshape((rows, dim / 2, 2))?;
    pairs
        .narrow(2, 0, 1)?
        .squeeze(2)?
        .add(&pairs.narrow(2, 1, 1)?.squeeze(2)?.affine(16., 0.)?)?
        .to_dtype(DType::U8)
}

pub(super) fn decode_e2m1(data: &Tensor) -> candle_core::Result<Tensor> {
    #[cfg(feature = "xq-fused-activation-codecs")]
    if data.device().is_cuda() {
        return crate::cuda::activation::decode_e2m1(data);
    }
    decode_e2m1_tensor(data)
}

pub(crate) fn decode_e2m1_tensor(data: &Tensor) -> candle_core::Result<Tensor> {
    let (rows, width) = data.dims2()?;
    let bytes = data.to_dtype(DType::F32)?;
    let high = bytes.affine(1. / 16., 0.)?.floor()?;
    let low = bytes.sub(&high.affine(16., 0.)?)?;
    let nibbles = Tensor::stack(&[low, high], 2)?
        .flatten_all()?
        .to_dtype(DType::U32)?;
    Tensor::new(
        &[
            0_f32, 0.5, 1., 1.5, 2., 3., 4., 6., -0., -0.5, -1., -1.5, -2., -3., -4., -6.,
        ],
        data.device(),
    )?
    .index_select(&nibbles, 0)?
    .reshape((rows, width * 2))
}

pub(super) fn e4m3_positive_palette() -> [f32; 127] {
    std::array::from_fn(|code| {
        let exponent = code >> 3;
        let mantissa = code & 7;
        if exponent == 0 {
            mantissa as f32 * 2_f32.powi(-9)
        } else {
            (1. + mantissa as f32 / 8.) * 2_f32.powi(exponent as i32 - 7)
        }
    })
}

pub(super) fn encode_e4m3(normalized: &Tensor) -> candle_core::Result<Tensor> {
    #[cfg(feature = "xq-fused-activation-codecs")]
    if normalized.device().is_cuda() {
        return crate::cuda::activation::encode_e4m3(normalized);
    }
    encode_e4m3_tensor(normalized)
}

pub(crate) fn encode_e4m3_tensor(normalized: &Tensor) -> candle_core::Result<Tensor> {
    let device = normalized.device();
    let absolute = normalized.abs()?;
    let palette = e4m3_positive_palette();
    let midpoints: Vec<_> = (0..127)
        .map(|code| {
            if code == 126 {
                f32::MAX
            } else {
                (palette[code] + palette[code + 1]) * 0.5
            }
        })
        .collect();
    let thresholds = Tensor::from_vec(midpoints, 127, device)?;
    let parity = Tensor::from_vec(
        (0..127).map(|code| (code % 2) as f32).collect::<Vec<_>>(),
        127,
        device,
    )?;
    let shape = normalized.shape();
    let mut low = Tensor::zeros(shape, DType::F32, device)?;
    let mut high = Tensor::full(126_f32, shape, device)?;
    for _ in 0..7 {
        let middle = low.add(&high)?.affine(0.5, 0.)?.floor()?;
        let ids = middle.flatten_all()?.to_dtype(DType::U32)?;
        let midpoint = thresholds.index_select(&ids, 0)?.reshape(shape)?;
        let odd = parity.index_select(&ids, 0)?.reshape(shape)?;
        let higher = absolute
            .gt(&midpoint)?
            .to_dtype(DType::F32)?
            .add(&absolute.eq(&midpoint)?.to_dtype(DType::F32)?.mul(&odd)?)?
            .gt(0_f32)?;
        low = higher.where_cond(&middle.affine(1., 1.)?, &low)?;
        high = higher.where_cond(&high, &middle)?;
    }
    normalized
        .recip()?
        .lt(0_f32)?
        .where_cond(&low.affine(1., 128.)?, &low)?
        .to_dtype(DType::U8)
}

pub(super) fn decode_e4m3(data: &Tensor) -> candle_core::Result<Tensor> {
    #[cfg(feature = "xq-fused-activation-codecs")]
    if data.device().is_cuda() {
        return crate::cuda::activation::decode_e4m3(data);
    }
    decode_e4m3_tensor(data)
}

pub(crate) fn decode_e4m3_tensor(data: &Tensor) -> candle_core::Result<Tensor> {
    let positive = e4m3_positive_palette();
    let lut: Vec<_> = (0..256)
        .map(|byte| {
            if byte & 127 == 127 {
                f32::NAN
            } else if byte & 128 == 0 {
                positive[byte & 127]
            } else {
                -positive[byte & 127]
            }
        })
        .collect();
    Tensor::from_vec(lut, 256, data.device())?
        .index_select(&data.flatten_all()?.to_dtype(DType::U32)?, 0)?
        .reshape(data.shape())
}

#[cfg(test)]
mod tests;
