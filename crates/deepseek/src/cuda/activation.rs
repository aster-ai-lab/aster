//! Original strict-F32 scalar codecs. Owners retain finite/shape/budget checks.
//! Explicit nondefault profile; tensor compositions remain the numeric control.
pub const ABI: &str = "aster-activation-codecs-f32-v1";
pub const SOURCE: &str = include_str!("activation.cu");

#[cfg(any(feature = "cuda", test))]
use candle_core::{DType, Shape};

#[cfg(any(feature = "cuda", test))]
#[derive(Clone, Copy, Debug)]
enum Kind {
    EncodeE4m3,
    DecodeE4m3,
    EncodeE2m1,
    DecodeE2m1,
}

#[cfg(any(feature = "cuda", test))]
struct Plan {
    work_items: u32,
    output_elements: usize,
    output_shape: Shape,
    grid: u32,
}

#[cfg(any(feature = "cuda", test))]
impl Plan {
    fn new(kind: Kind, shape: &Shape, dtype: DType) -> candle_core::Result<Self> {
        let required = match kind {
            Kind::EncodeE4m3 | Kind::EncodeE2m1 => DType::F32,
            Kind::DecodeE4m3 | Kind::DecodeE2m1 => DType::U8,
        };
        if dtype != required {
            candle_core::bail!("activation codec input dtype differs from its explicit format");
        }
        let elements = shape.dims().iter().try_fold(1usize, |n, &dim| {
            n.checked_mul(dim)
                .ok_or_else(|| candle_core::Error::Msg("activation shape overflowed".into()))
        })?;
        let (work_items, output_elements, output_shape) = match kind {
            Kind::EncodeE4m3 | Kind::DecodeE4m3 => (elements, elements, shape.clone()),
            Kind::EncodeE2m1 => {
                let (rows, width) = shape.dims2()?;
                if !width.is_multiple_of(2) {
                    candle_core::bail!("FP4 encoding requires even row width");
                }
                (elements / 2, elements / 2, Shape::from((rows, width / 2)))
            }
            Kind::DecodeE2m1 => {
                let (rows, width) = shape.dims2()?;
                let width = width.checked_mul(2).ok_or_else(|| {
                    candle_core::Error::Msg("FP4 decoded width overflowed".into())
                })?;
                let count = elements.checked_mul(2).ok_or_else(|| {
                    candle_core::Error::Msg("FP4 decoded count overflowed".into())
                })?;
                (elements, count, Shape::from((rows, width)))
            }
        };
        if elements > u32::MAX as usize || output_elements > u32::MAX as usize {
            candle_core::bail!("activation codec input/output exceeds bounded u32 ABI");
        }
        let work_items = work_items as u32;
        Ok(Self {
            work_items,
            output_elements,
            output_shape,
            grid: u64::from(work_items).div_ceil(256).max(1) as u32,
        })
    }
}

#[cfg(feature = "cuda")]
mod execution;
#[cfg(feature = "cuda")]
pub(crate) use execution::{decode_e2m1, decode_e4m3, encode_e2m1, encode_e4m3};

#[cfg(test)]
mod tests;
