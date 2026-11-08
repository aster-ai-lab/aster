//! Exact expert-to-type/slot binding from the exporter's two I32 maps.
//! Tensor names are the schema in pinned tools/v41/v41_gguf.py, not inferred
//! from catalog spelling. Gate/up share a map; down has independent slots.

use aster_power::error::Result;
use tokio_util::sync::CancellationToken;

use super::gguf::XqGgufHeader;
use super::storage::{XqTensorRead, XqWeights};
use super::{invalid, XqTensorType};

const EXPERTS: usize = 384;
const LAYERS: usize = 40;
const HIDDEN: u64 = 5120;
const INTERMEDIATE: u64 = 2304;
const TIERS: [XqTensorType; 3] = [XqTensorType::Q2_0, XqTensorType::Q3K, XqTensorType::Ptq1_0];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XqExpertRole {
    Gate,
    Up,
    Down,
}

impl XqExpertRole {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "gate" => Ok(Self::Gate),
            "up" => Ok(Self::Up),
            "down" => Ok(Self::Down),
            _ => Err(invalid(
                "unsupported XQ expert role; supported set: gate, up, down",
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gate => "gate",
            Self::Up => "up",
            Self::Down => "down",
        }
    }

    fn dimensions(self) -> (u64, u64) {
        if self == Self::Down {
            (HIDDEN, INTERMEDIATE)
        } else {
            (INTERMEDIATE, HIDDEN)
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Slot {
    kind: XqTensorType,
    index: u64,
}

/// Immutable layout derived from authenticated maps, checked against every
/// expert stack in this layer. Global expert IDs remain in ascending order.
#[derive(Debug)]
pub struct XqExpertLayout {
    layer: usize,
    gate_up: Vec<Slot>,
    down: Vec<Slot>,
}

/// A single expert's rows inside one typed stack, without a decoded copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XqExpertProjection {
    tensor_name: String,
    kind: XqTensorType,
    first_row: u64,
    rows: u64,
    columns: u64,
}

impl XqExpertLayout {
    pub fn load(
        weights: &XqWeights,
        layer: usize,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        if layer >= LAYERS {
            return Err(invalid("XQ expert layer exceeds the pinned 40-layer model"));
        }
        let read = |suffix: &str| -> Result<Vec<i32>> {
            let raw = weights.read_tensor(&format!("blk.{layer}.{suffix}"), cancellation)?;
            if raw.tensor().kind != XqTensorType::I32 || raw.tensor().shape != [EXPERTS as u64] {
                return Err(invalid(
                    "XQ expert type map requires exact I32[384] geometry",
                ));
            }
            raw.tensor()
                .kind
                .decode_i32_row(raw.bytes(), EXPERTS, (EXPERTS * 4) as u64)
        };
        let gate_up = read("ffn_exps_gu_type")?;
        let down = read("ffn_exps_down_type")?;
        Self::from_maps(weights.header(), layer, &gate_up, &down)
    }

    fn from_maps(header: &XqGgufHeader, layer: usize, gu: &[i32], down: &[i32]) -> Result<Self> {
        if layer >= LAYERS {
            return Err(invalid("XQ expert layer exceeds the pinned 40-layer model"));
        }
        let (gate_up, gu_counts) = slots(gu)?;
        let (down, down_counts) = slots(down)?;
        for (role, counts) in [
            (XqExpertRole::Gate, gu_counts),
            (XqExpertRole::Up, gu_counts),
            (XqExpertRole::Down, down_counts),
        ] {
            for (kind, count) in TIERS.into_iter().zip(counts) {
                let name = tensor_name(layer, role, kind)?;
                let tensor = header.tensors().get(&name);
                if count == 0 {
                    if tensor.is_some() {
                        return Err(invalid(
                            "XQ expert stack exists for an empty type partition",
                        ));
                    }
                    continue;
                }
                let tensor = tensor.ok_or_else(|| {
                    invalid("XQ expert type partition has no corresponding tensor")
                })?;
                let (rows, columns) = role.dimensions();
                if tensor.kind != kind || tensor.shape != [columns, rows, count] {
                    return Err(invalid(
                        "XQ expert stack type or geometry differs from its authenticated map",
                    ));
                }
            }
        }
        Ok(Self {
            layer,
            gate_up,
            down,
        })
    }

    pub fn layer(&self) -> usize {
        self.layer
    }

    pub fn projection(&self, expert: usize, role: XqExpertRole) -> Result<XqExpertProjection> {
        let map = if role == XqExpertRole::Down {
            &self.down
        } else {
            &self.gate_up
        };
        let slot = map
            .get(expert)
            .ok_or_else(|| invalid("XQ expert ID exceeds the pinned 384-expert layer"))?;
        let (rows, columns) = role.dimensions();
        let first_row = slot
            .index
            .checked_mul(rows)
            .ok_or_else(|| invalid("XQ expert slot offset overflowed"))?;
        Ok(XqExpertProjection {
            tensor_name: tensor_name(self.layer, role, slot.kind)?,
            kind: slot.kind,
            first_row,
            rows,
            columns,
        })
    }
}

impl XqExpertProjection {
    pub fn tensor_name(&self) -> &str {
        &self.tensor_name
    }
    pub fn kind(&self) -> XqTensorType {
        self.kind
    }
    pub fn first_row(&self) -> u64 {
        self.first_row
    }
    pub fn rows(&self) -> u64 {
        self.rows
    }
    pub fn columns(&self) -> u64 {
        self.columns
    }

    /// Relative row bounds stay inside this expert, even if a neighboring
    /// expert occupies the next contiguous rows in the same source tensor.
    pub fn read_rows(
        &self,
        weights: &XqWeights,
        first: u64,
        rows: u64,
        cancellation: &CancellationToken,
    ) -> Result<XqTensorRead> {
        let first = self.row_offset(first, rows)?;
        let read = weights.read_rows(&self.tensor_name, first, rows, cancellation)?;
        if read.tensor().kind != self.kind || read.tensor().shape[0] != self.columns {
            return Err(invalid(
                "XQ expert read differs from its bound projection layout",
            ));
        }
        Ok(read)
    }

    fn row_offset(&self, first: u64, rows: u64) -> Result<u64> {
        let end = first
            .checked_add(rows)
            .ok_or_else(|| invalid("XQ expert row range overflowed"))?;
        if rows == 0 || end > self.rows {
            return Err(invalid("XQ expert row range crosses its expert boundary"));
        }
        self.first_row
            .checked_add(first)
            .ok_or_else(|| invalid("XQ expert absolute row overflowed"))
    }
}

fn slots(types: &[i32]) -> Result<(Vec<Slot>, [u64; 3])> {
    if types.len() != EXPERTS {
        return Err(invalid("XQ expert map requires exactly 384 entries"));
    }
    let mut counts = [0; 3];
    let mut output = Vec::with_capacity(EXPERTS);
    for value in types {
        let tier = match value {
            42 => 0,
            11 => 1,
            143 => 2,
            _ => {
                return Err(invalid(
                    "unsupported XQ expert type; supported set: Q2_0=42, Q3_K=11, PTQ1_0=143",
                ))
            }
        };
        output.push(Slot {
            kind: TIERS[tier],
            index: counts[tier],
        });
        counts[tier] += 1;
    }
    Ok((output, counts))
}

fn tensor_name(layer: usize, role: XqExpertRole, kind: XqTensorType) -> Result<String> {
    let suffix = match kind {
        XqTensorType::Q2_0 => "q2_0",
        XqTensorType::Q3K => "q3_k",
        XqTensorType::Ptq1_0 => "ptq1_0",
        _ => {
            return Err(invalid(
                "XQ expert stack type is outside the pinned tier set",
            ))
        }
    };
    Ok(format!(
        "blk.{layer}.ffn_{}_exps.{suffix}.weight",
        role.as_str()
    ))
}

#[cfg(test)]
mod tests;
