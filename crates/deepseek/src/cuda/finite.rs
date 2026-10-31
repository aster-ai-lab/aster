//! An explicit experimental validation profile: one flag kernel, one scalar read.
//! Keep the tensor control available for qualification. No precision changes.
#[cfg(any(feature = "cuda", test))]
use aster_power::error::{PowerError, Result};

pub const ABI: &str = "aster-finite-f32-flag-v1";
pub const SOURCE: &str = include_str!("finite.cu");

#[cfg(any(feature = "cuda", test))]
fn geometry(elements: usize) -> Result<(u32, u32)> {
    let elements = u32::try_from(elements)
        .map_err(|_| PowerError::InvalidRequest("validation element count exceeds u32".into()))?;
    let grid = u64::from(elements).div_ceil(256).max(1) as u32;
    Ok((elements, grid))
}

#[cfg(feature = "cuda")]
mod execution;
#[cfg(feature = "cuda")]
pub(crate) use execution::validate;

#[cfg(test)]
mod tests;
