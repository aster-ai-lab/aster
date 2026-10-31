//! Three quantized projections with route weighting inside SwiGLU, before
//! the down-projection activation cast. Shared experts omit the route weight.
//! This composes tensor controls, not a grouped SM90 expert kernel.

use aster_power::error::Result;
use candle_core::{DType, Tensor};
use tokio_util::sync::CancellationToken;

use super::{check_cancel, invalid, DeviceMatrix, Fp8Activations, MatrixLayout};
use crate::layers::{moe::RoutedSwiGlu, CudaLayerDevice};

pub struct QuantizedExpert {
    device: CudaLayerDevice,
    gate: DeviceMatrix,
    up: DeviceMatrix,
    down: DeviceMatrix,
    activation: RoutedSwiGlu,
    hidden: usize,
    intermediate: usize,
    resident_bytes: u64,
}

impl QuantizedExpert {
    pub fn new(
        device: CudaLayerDevice,
        gate: DeviceMatrix,
        up: DeviceMatrix,
        down: DeviceMatrix,
        clip: f64,
    ) -> Result<Self> {
        let [intermediate, hidden] = gate.logical_shape();
        if !matches!(
            gate.layout(),
            MatrixLayout::Fp8Block32 | MatrixLayout::Mxfp4Block32
        ) || up.layout() != gate.layout()
            || down.layout() != gate.layout()
            || up.logical_shape() != [intermediate, hidden]
            || down.logical_shape() != [hidden, intermediate]
        {
            return Err(invalid("expert requires matching FP8 or MXFP4 gate/up/down layouts and transposed hidden geometry"));
        }
        let mut bytes = 0_u64;
        for matrix in [&gate, &up, &down] {
            if !matrix.data.device().same_device(device.tensor_device())
                || matrix.device.limits() != device.limits()
            {
                return Err(invalid(
                    "expert projections differ from the owning device/limits",
                ));
            }
            bytes = bytes
                .checked_add(matrix.retained_weight_bytes())
                .ok_or_else(|| invalid("expert resident weight budget overflowed"))?;
        }
        if bytes > device.limits().max_resident_weight_bytes {
            return Err(invalid(
                "expert projections exceed their aggregate resident weight budget",
            ));
        }
        let activation = RoutedSwiGlu::new(device.clone(), clip)?;
        Ok(Self {
            device,
            gate,
            up,
            down,
            activation,
            hidden,
            intermediate,
            resident_bytes: bytes,
        })
    }
    pub fn layout(&self) -> MatrixLayout {
        self.gate.layout()
    }
    pub fn hidden_size(&self) -> usize {
        self.hidden
    }
    pub fn intermediate_size(&self) -> usize {
        self.intermediate
    }
    pub fn retained_weight_bytes(&self) -> u64 {
        self.resident_bytes
    }
    pub(crate) fn device(&self) -> &CudaLayerDevice {
        &self.device
    }

    pub fn forward(
        &self,
        input: &Fp8Activations,
        route_weights: Option<&Tensor>,
        cancellation: &CancellationToken,
    ) -> Result<Tensor> {
        check_cancel(cancellation)?;
        let (tokens, width) = input.data_bytes().dims2().map_err(super::tensor_error)?;
        if width != self.hidden
            || !input
                .data_bytes()
                .device()
                .same_device(self.device.tensor_device())
        {
            return Err(invalid(
                "expert packed activation width differs from its hidden geometry",
            ));
        }
        let limits = self.device.limits();
        let input_elements =
            limits.checked_elements(&[tokens, self.hidden], "expert hidden activation")? as u64;
        let intermediate_elements = limits.checked_elements(
            &[tokens, self.intermediate],
            "expert intermediate activation",
        )? as u64;
        let tile = limits.checked_elements(
            &[
                32.min(self.hidden.max(self.intermediate)),
                self.hidden.max(self.intermediate),
            ],
            "expert decoded tile",
        )? as u64;
        let peak = input_elements
            .checked_mul(48)
            .and_then(|n| {
                intermediate_elements
                    .checked_mul(128)
                    .and_then(|m| n.checked_add(m))
            })
            .and_then(|n| tile.checked_mul(48).and_then(|m| n.checked_add(m)))
            .and_then(|n| n.checked_add(4096))
            .ok_or_else(|| invalid("expert tensor budget overflowed"))?;
        limits.checked_state_bytes(peak, "expert composite live tensors")?;
        // Decode the same packed activation once for both control projections.
        // Only this typed FP8 entry can reach the private expanded-math path.
        let decoded = input.decode()?;
        let gate = self
            .gate
            .forward_decoded(&decoded, DType::BF16, cancellation)?;
        let up = self
            .up
            .forward_decoded(&decoded, DType::BF16, cancellation)?;
        let weighted = self.activation.forward(&gate, &up, route_weights)?;
        let packed = Fp8Activations::encode(&self.device, &weighted, cancellation)?;
        self.down.forward_fp8(&packed, DType::BF16, cancellation)
    }
}

#[cfg(test)]
mod tests;
