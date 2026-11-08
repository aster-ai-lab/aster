//! Raw device upload in bounded Power reads; never expands quantized weights.
//! Tiled FP8 chunks preserve complete 32-row scale blocks except the final
//! checkpoint tail. Host tensor-list metadata has its own checked byte bound.

use super::{check_cancel, invalid, tensor_error, CudaLayerDevice, MatrixLayout, MatrixWeights};
use aster_power::error::Result;
use candle_core::{DType, Tensor};
use tokio_util::sync::CancellationToken;

pub(super) fn read_chunks(
    device: &CudaLayerDevice,
    weights: &MatrixWeights<'_>,
    first_row: usize,
    rows: usize,
    cancellation: &CancellationToken,
) -> Result<(Tensor, Option<Tensor>)> {
    let descriptor = weights.descriptor();
    let [total_rows, columns] = descriptor.logical_shape();
    let layout = descriptor.layout();
    let end = first_row
        .checked_add(rows)
        .ok_or_else(|| invalid("matrix upload row range overflowed"))?;
    if rows == 0
        || end > total_rows
        || (layout == MatrixLayout::Fp8Block32
            && (!first_row.is_multiple_of(32) || (!end.is_multiple_of(32) && end != total_rows)))
    {
        return Err(invalid(
            "matrix upload range exceeds its checkpoint or scale-block boundary",
        ));
    }
    let row_bytes = descriptor.weight().bytes / total_rows as u64;
    let scale_width = descriptor.scale().map_or(0, |s| s.shape[1]) as u64;
    let budget = weights.raw_read_budget_bytes();
    let tiled = layout == MatrixLayout::Fp8Block32;
    let max_rows = if tiled {
        let block_bytes = row_bytes
            .checked_mul(32)
            .and_then(|n| n.checked_add(scale_width))
            .ok_or_else(|| invalid("matrix upload block bytes overflowed"))?;
        let blocks = budget / block_bytes;
        if blocks == 0 {
            let tail_bytes = row_bytes
                .checked_mul(rows as u64)
                .and_then(|n| n.checked_add(scale_width))
                .ok_or_else(|| invalid("matrix upload tail bytes overflowed"))?;
            if rows >= 32 || tail_bytes > budget {
                return Err(invalid(
                    "matrix read budget cannot fit one aligned FP8 block or final tail",
                ));
            }
            rows
        } else {
            usize::try_from(blocks.saturating_mul(32).min(rows as u64))
                .map_err(|_| invalid("matrix upload chunk rows exceed usize"))?
        }
    } else {
        let bytes = row_bytes
            .checked_add(scale_width)
            .ok_or_else(|| invalid("matrix upload row bytes overflowed"))?;
        usize::try_from((budget / bytes).min(rows as u64))
            .map_err(|_| invalid("matrix upload chunk rows exceed usize"))?
    };
    if max_rows == 0 {
        return Err(invalid(
            "matrix read budget cannot fit one raw value/scale row",
        ));
    }
    let chunks = rows.div_ceil(max_rows) as u64;
    let raw_bytes = row_bytes
        .checked_add(scale_width)
        .and_then(|n| n.checked_mul(rows as u64))
        .ok_or_else(|| invalid("matrix upload retained bytes overflowed"))?;
    let live = raw_bytes
        .checked_mul(16)
        .and_then(|n| chunks.checked_mul(256).and_then(|m| n.checked_add(m)))
        .and_then(|n| n.checked_add(4096))
        .ok_or_else(|| invalid("matrix upload live bytes overflowed"))?;
    device
        .limits()
        .checked_state_bytes(live, "matrix upload chunks and concatenation")?;
    let dtype = match layout {
        MatrixLayout::Bf16 => DType::BF16,
        MatrixLayout::F32 => DType::F32,
        _ => DType::U8,
    };
    let physical_columns = if layout == MatrixLayout::Mxfp4Block32 {
        columns / 2
    } else {
        columns
    };
    let mut data = Vec::new();
    let mut scales = Vec::new();
    let mut first = first_row;
    while first < end {
        check_cancel(cancellation)?;
        let count = max_rows.min(end - first);
        let raw = weights.read_rows(first, count, cancellation)?;
        data.push(
            Tensor::from_raw_buffer(
                raw.weight.bytes(),
                dtype,
                &[count, physical_columns],
                device.tensor_device(),
            )
            .map_err(tensor_error)?,
        );
        if let Some(raw) = raw.scale.as_ref() {
            let scale_rows = if tiled { count.div_ceil(32) } else { count };
            scales.push(
                Tensor::from_raw_buffer(
                    raw.bytes(),
                    DType::U8,
                    &[scale_rows, columns.div_ceil(32)],
                    device.tensor_device(),
                )
                .map_err(tensor_error)?,
            );
        }
        first += count;
    }
    check_cancel(cancellation)?;
    let data = Tensor::cat(&data, 0).map_err(tensor_error)?;
    let scales = if scales.is_empty() {
        None
    } else {
        Some(Tensor::cat(&scales, 0).map_err(tensor_error)?)
    };
    check_cancel(cancellation)?;
    Ok((data, scales))
}

#[cfg(test)]
mod tests;
