//! Explicit small XQ floating parameters over authenticated Power ranges.
//! Large/quantized matrices use XqDeviceMatrix, never a full F32 decode here.

use aster_power::error::{PowerError, Result};
use candle_core::{DType, Tensor};
use tokio_util::sync::CancellationToken;

use crate::layers::{tensor_error, validate_finite, CudaLayerDevice};

use super::{storage::XqWeights, XqTensorType};

const MAX_PARAMETER_BYTES: u64 = 16 * 1024 * 1024;

impl XqWeights {
    /// The model owner supplies the exact name, stored type and GGUF shape.
    /// F16 values convert exactly to F32 on the admitted device. No filename
    /// suffix, role spelling or integer container chooses a numeric meaning.
    pub fn load_f32_parameter(
        &self,
        device: &CudaLayerDevice,
        name: &str,
        kind: XqTensorType,
        gguf_shape: &[u64],
        cancellation: &CancellationToken,
    ) -> Result<Tensor> {
        check_cancel(cancellation)?;
        let dtype = match kind {
            XqTensorType::F32 => DType::F32,
            XqTensorType::F16 => DType::F16,
            _ => {
                return Err(invalid(
                    "XQ small parameters require explicit F32 or F16 storage",
                ))
            }
        };
        let tensor = self.tensor(name)?;
        if tensor.kind != kind || tensor.shape != gguf_shape || tensor.bytes > MAX_PARAMETER_BYTES {
            return Err(invalid(
                "XQ parameter type/shape/bytes differ from its bounded declaration",
            ));
        }
        let shape: Vec<usize> = gguf_shape
            .iter()
            .rev()
            .map(|&value| {
                usize::try_from(value)
                    .map_err(|_| invalid("XQ parameter dimension exceeds the host address range"))
            })
            .collect::<Result<_>>()?;
        let elements = device
            .limits()
            .checked_elements(&shape, "XQ F32 parameter")?;
        let output_bytes = (elements as u64)
            .checked_mul(4)
            .ok_or_else(|| invalid("XQ parameter output bytes overflowed"))?;
        if output_bytes > device.limits().max_resident_weight_bytes {
            return Err(invalid("XQ F32 parameter exceeds its resident budget"));
        }
        let live = output_bytes
            .checked_mul(16)
            .and_then(|bytes| bytes.checked_add(tensor.bytes))
            .ok_or_else(|| invalid("XQ parameter live byte bound overflowed"))?;
        device
            .limits()
            .checked_state_bytes(live, "XQ raw parameter, conversion and finite validation")?;
        let raw = self.read_tensor(name, cancellation)?;
        let columns = gguf_shape[0];
        kind.validate_floating_payload(
            raw.bytes(),
            columns,
            tensor.bytes / kind.row_bytes(columns)?,
        )?;
        let output = Tensor::from_raw_buffer(raw.bytes(), dtype, &shape, device.tensor_device())
            .and_then(|tensor| tensor.to_dtype(DType::F32))
            .map_err(tensor_error)?
            .detach();
        validate_finite(&output, false, "XQ F32 parameter")?;
        check_cancel(cancellation)?;
        Ok(output)
    }
}

fn invalid(reason: &str) -> PowerError {
    PowerError::InvalidFormat(reason.into())
}

fn check_cancel(cancellation: &CancellationToken) -> Result<()> {
    if cancellation.is_cancelled() {
        return Err(PowerError::InferenceCancelled);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
