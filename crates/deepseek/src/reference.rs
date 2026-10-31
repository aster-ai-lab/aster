//! Small CPU mathematical oracles for native operator qualification.
//!
//! These routines are never a serving backend or a CUDA fallback. Scalar
//! encodings follow OCP MX 1.0. Gate and asymmetric SwiGLU semantics follow
//! the pinned TileKernels references recorded in the implementation evidence.

use aster_power::error::{PowerError, Result};

pub mod compression;

/// Fixture bound, independent of the dimensions of a production model.
pub const MAX_REFERENCE_ELEMENTS: usize = 4096;

#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceMatrix {
    rows: usize,
    columns: usize,
    values: Vec<f64>,
}

impl ReferenceMatrix {
    pub fn new(rows: usize, columns: usize, values: Vec<f64>) -> Result<Self> {
        let elements = checked_shape(rows, columns)?;
        if values.len() != elements {
            return Err(invalid("matrix payload does not match its dimensions"));
        }
        validate_values(&values)?;
        Ok(Self {
            rows,
            columns,
            values,
        })
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn columns(&self) -> usize {
        self.columns
    }

    pub fn values(&self) -> &[f64] {
        &self.values
    }
}

/// Row-major `input * weight.transpose()`, without bias.
pub fn linear(input: &ReferenceMatrix, weight: &ReferenceMatrix) -> Result<ReferenceMatrix> {
    if input.columns != weight.columns {
        return Err(invalid("linear input and weight widths differ"));
    }
    let count = checked_shape(input.rows, weight.rows)?;
    let mut values = Vec::with_capacity(count);
    for row in input.values.chunks_exact(input.columns) {
        for projection in weight.values.chunks_exact(weight.columns) {
            let mut sum = 0.0;
            for (x, w) in row.iter().zip(projection) {
                let product = finite(x * w)?;
                sum = finite(sum + product)?;
            }
            values.push(sum);
        }
    }
    ReferenceMatrix::new(input.rows, weight.rows, values)
}

/// Applies effective (already converted) weights and epsilon to one row.
pub fn rms_norm(input: &[f64], weight: &[f64], epsilon: f64) -> Result<Vec<f64>> {
    validate_values(input)?;
    validate_values(weight)?;
    if input.len() != weight.len() || !epsilon.is_finite() || epsilon <= 0.0 {
        return Err(invalid(
            "RMSNorm requires matching widths and positive finite epsilon",
        ));
    }
    let mut squares = 0.0;
    for value in input {
        squares = finite(squares + finite(value * value)?)?;
    }
    let denominator = finite(squares / input.len() as f64 + epsilon)?.sqrt();
    input
        .iter()
        .zip(weight)
        .map(|(value, scale)| finite(value / denominator * scale))
        .collect()
}

pub fn softmax(input: &[f64]) -> Result<Vec<f64>> {
    validate_values(input)?;
    let maximum = input.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let mut values: Vec<f64> = input
        .iter()
        .map(|value| finite(value - maximum).map(f64::exp))
        .collect::<Result<_>>()?;
    let sum: f64 = values.iter().sum();
    for value in &mut values {
        *value = finite(*value / sum)?;
    }
    Ok(values)
}

/// Gate is upper-clipped only; value is clipped to both signed limits.
pub fn swiglu(gate: &[f64], value: &[f64], limit: f64) -> Result<Vec<f64>> {
    validate_values(gate)?;
    validate_values(value)?;
    if gate.len() != value.len() || !limit.is_finite() || limit <= 0.0 {
        return Err(invalid(
            "SwiGLU requires matching widths and a positive finite limit",
        ));
    }
    gate.iter()
        .zip(value)
        .map(|(gate, value)| {
            let gate = gate.min(limit);
            let value = value.clamp(-limit, limit);
            // Both branches avoid overflow while preserving negative gates.
            let sigmoid = if gate >= 0.0 {
                1.0 / (1.0 + (-gate).exp())
            } else {
                let exponential = gate.exp();
                exponential / (1.0 + exponential)
            };
            finite(gate * sigmoid * value)
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceRoute {
    pub expert: usize,
    pub weight: f64,
}

/// Global noaux_tc selection, with stable logical-ID tie breaking.
/// The caller explicitly supplies the appropriate text or image bias.
pub fn topk_gate(
    logits: &[f64],
    correction_bias: &[f64],
    topk: usize,
    routed_scale: f64,
) -> Result<Vec<ReferenceRoute>> {
    validate_values(logits)?;
    validate_values(correction_bias)?;
    if logits.len() != correction_bias.len()
        || topk == 0
        || topk > logits.len()
        || !routed_scale.is_finite()
        || routed_scale <= 0.0
    {
        return Err(invalid(
            "gate requires matching widths, bounded top-k and positive finite scale",
        ));
    }
    let scores: Vec<f64> = logits
        .iter()
        .map(|value| (value.max(0.0) + (-value.abs()).exp().ln_1p()).sqrt())
        .collect();
    let rankings: Vec<f64> = scores
        .iter()
        .zip(correction_bias)
        .map(|(score, bias)| finite(score + bias))
        .collect::<Result<_>>()?;
    let mut indices: Vec<usize> = (0..scores.len()).collect();
    indices.sort_by(|left, right| {
        rankings[*right]
            .total_cmp(&rankings[*left])
            .then_with(|| left.cmp(right))
    });
    indices.truncate(topk);
    // The epsilon is part of the reference gate, even for very small scores.
    let denominator = finite(indices.iter().map(|index| scores[*index]).sum::<f64>() + 1e-20)?;
    indices
        .into_iter()
        .map(|expert| {
            Ok(ReferenceRoute {
                expert,
                weight: finite(scores[expert] / denominator * routed_scale)?,
            })
        })
        .collect()
}

/// Exact routed weighted sum; one output row per logical expert.
pub fn combine_experts(routes: &[ReferenceRoute], outputs: &ReferenceMatrix) -> Result<Vec<f64>> {
    if routes.is_empty() || routes.len() > outputs.rows {
        return Err(invalid(
            "expert routes are empty or exceed the expert count",
        ));
    }
    let mut seen = vec![false; outputs.rows];
    let mut combined = vec![0.0; outputs.columns];
    for route in routes {
        if route.expert >= outputs.rows
            || seen[route.expert]
            || !route.weight.is_finite()
            || route.weight < 0.0
        {
            return Err(invalid(
                "expert routes contain an invalid ID, duplicate or weight",
            ));
        }
        seen[route.expert] = true;
        let start = route.expert * outputs.columns;
        for (result, value) in combined
            .iter_mut()
            .zip(&outputs.values[start..start + outputs.columns])
        {
            *result = finite(*result + finite(route.weight * value)?)?;
        }
    }
    Ok(combined)
}

/// OCP E4M3 finite format; both signed NaN encodings are refused.
pub fn decode_e4m3(byte: u8) -> Result<f64> {
    let exponent = (byte >> 3) & 15;
    let mantissa = byte & 7;
    if exponent == 15 && mantissa == 7 {
        return Err(invalid("E4M3 NaN encoding is not a finite weight"));
    }
    let magnitude = if exponent == 0 {
        f64::from(mantissa) * 2.0_f64.powi(-9)
    } else {
        (1.0 + f64::from(mantissa) / 8.0) * 2.0_f64.powi(i32::from(exponent) - 7)
    };
    Ok(if byte & 128 == 0 {
        magnitude
    } else {
        -magnitude
    })
}

/// A scalar nibble only. Tensor nibble ordering belongs to the weight schema.
pub fn decode_e2m1(nibble: u8) -> Result<f64> {
    if nibble > 15 {
        return Err(invalid("E2M1 scalar requires exactly four bits"));
    }
    let magnitude = [0.0, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0][usize::from(nibble & 7)];
    Ok(if nibble & 8 == 0 {
        magnitude
    } else {
        -magnitude
    })
}

pub fn decode_ue8m0(byte: u8) -> Result<f64> {
    if byte == 255 {
        return Err(invalid("UE8M0 NaN encoding is not a finite scale"));
    }
    Ok(2.0_f64.powi(i32::from(byte) - 127))
}

fn checked_shape(rows: usize, columns: usize) -> Result<usize> {
    let count = rows
        .checked_mul(columns)
        .ok_or_else(|| invalid("reference shape multiplication overflowed"))?;
    if rows == 0 || columns == 0 || count > MAX_REFERENCE_ELEMENTS {
        return Err(invalid(
            "reference shape must contain 1 through 4096 elements",
        ));
    }
    Ok(count)
}

fn validate_values(values: &[f64]) -> Result<()> {
    if values.is_empty()
        || values.len() > MAX_REFERENCE_ELEMENTS
        || values.iter().any(|value| !value.is_finite())
    {
        return Err(invalid(
            "reference input requires 1 through 4096 finite elements",
        ));
    }
    Ok(())
}

fn finite(value: f64) -> Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(invalid("reference arithmetic produced a non-finite value"))
    }
}

fn invalid(reason: &str) -> PowerError {
    PowerError::InvalidRequest(reason.to_owned())
}

#[cfg(test)]
mod tests;
