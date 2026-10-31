//! Opt-in H200 qualification. The pinned golden rows were independently
//! decoded with NumPy; F64 dots set the numeric reference before execution.
use super::*;
use aster_power::inference::{DevicePreference, InferenceLimits, RuntimeDevice};
use candle_core::Tensor;
use tokio_util::sync::CancellationToken;

use crate::layers::linear::xq::XqDeviceMatrix;
use crate::layers::CudaLayerDevice;

#[test]
#[ignore = "requires an owned H200; run explicitly with --ignored --nocapture"]
fn h200_xq_gather_preserves_distinct_repeated_nonordered_rows_and_every_golden_bit() {
    let device = CudaLayerDevice::new(
        RuntimeDevice::resolve(DevicePreference::Cuda { ordinal: 0 }).unwrap(),
        InferenceLimits::default(),
    )
    .unwrap();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../../fixtures/xq-codec-golden.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let kind = XqTensorType::from_gguf_id(case["type_id"].as_u64().unwrap() as u32).unwrap();
        let columns = case["columns"].as_u64().unwrap() as usize;
        let hex = case["raw_hex"].as_str().unwrap();
        let raw: Vec<_> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        let golden: Vec<u32> = case["expected_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_u64().unwrap() as u32)
            .collect();
        let (block_elements, block_bytes) = kind.block_layout();
        let blocks = raw.len() / block_bytes as usize;
        let mut packed = Vec::new();
        for row in 0..3 {
            let mut values = raw.clone();
            values.rotate_left((row % blocks) * block_bytes as usize);
            packed.extend(values);
        }
        let matrix =
            XqDeviceMatrix::from_bytes_for_tests(&device, kind, 3, columns, &packed).unwrap();
        let ids = [2_u32, 0, 2, 1];
        let indices = Tensor::new(&ids, device.tensor_device()).unwrap();
        let (actual, receipt) = matrix
            .gather_f32_rows(&indices, &CancellationToken::new())
            .unwrap();
        let values = actual.to_vec2::<f32>().unwrap();
        for (row, id) in ids.iter().enumerate() {
            let shift = (*id as usize % blocks) * block_elements as usize;
            for col in 0..columns {
                assert_eq!(
                    values[row][col].to_bits(),
                    golden[(col + shift) % columns],
                    "{} row={row} col={col}",
                    case["name"]
                );
            }
        }
        assert_eq!(receipt.abi, XQ_GATHER_ABI);
        assert_eq!(receipt.output_bytes, (ids.len() * columns * 4) as u64);
        let invalid = Tensor::new(&[3_u32], device.tensor_device()).unwrap();
        assert!(matrix
            .gather_f32_rows(&invalid, &CancellationToken::new())
            .is_err());
        println!(
            "qualified XQ gather {} source={} ptx={}",
            kind.as_str(),
            receipt.source_sha256,
            receipt.ptx_sha256
        );
    }
}

#[test]
#[ignore = "requires an owned H200; run explicitly with --ignored --nocapture"]
fn h200_xq_all_packed_formats_match_independent_golden_row_dots() {
    let device = CudaLayerDevice::new(
        RuntimeDevice::resolve(DevicePreference::Cuda { ordinal: 0 }).unwrap(),
        InferenceLimits::default(),
    )
    .unwrap();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../../fixtures/xq-codec-golden.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let kind = XqTensorType::from_gguf_id(case["type_id"].as_u64().unwrap() as u32).unwrap();
        let columns = case["columns"].as_u64().unwrap() as usize;
        let hex = case["raw_hex"].as_str().unwrap();
        let raw: Vec<_> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        let golden: Vec<f32> = case["expected_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| f32::from_bits(x.as_u64().unwrap() as u32))
            .collect();
        // Partial final block of four warps, multiple tokens and raw rows.
        for (tokens, rows) in [(1, 1), (3, 7)] {
            let (block_elements, block_bytes) = kind.block_layout();
            let blocks = raw.len() / block_bytes as usize;
            let mut packed = Vec::new();
            for row in 0..rows {
                let mut distinct = raw.clone();
                distinct.rotate_left((row % blocks) * block_bytes as usize);
                packed.extend(distinct);
            }
            let matrix =
                XqDeviceMatrix::from_bytes_for_tests(&device, kind, rows, columns, &packed)
                    .unwrap();
            let activations: Vec<f32> = (0..tokens * columns)
                .map(|i| ((i % 17) as f32 - 8.) * 0.015625)
                .collect();
            let input = Tensor::from_vec(
                activations.clone(),
                (tokens, columns),
                device.tensor_device(),
            )
            .unwrap();
            let (actual, receipt) = matrix
                .forward_f32_control(&input, &CancellationToken::new())
                .unwrap();
            let actual = actual.to_vec2::<f32>().unwrap();
            for token in 0..tokens {
                for row in 0..rows {
                    let shift = (row % blocks) * block_elements as usize;
                    let terms: Vec<f64> = (0..columns)
                        .map(|col| {
                            f64::from(golden[(col + shift) % columns])
                                * f64::from(activations[token * columns + col])
                        })
                        .collect();
                    let expected: f64 = terms.iter().sum();
                    let absolute_sum: f64 = terms.iter().map(|x| x.abs()).sum();
                    // Frozen F32 reduction budget, including cancellation-heavy dots.
                    let tolerance = 2e-6 * absolute_sum.max(1.);
                    assert!(
                        (f64::from(actual[token][row]) - expected).abs() <= tolerance,
                        "{} {tokens}/{rows} token={token} row={row}",
                        case["name"]
                    );
                }
            }
            assert_eq!(receipt.abi, XQ_CUDA_ABI);
            assert_eq!(receipt.source_sha256, source_sha256());
            assert_eq!(receipt.ptx_sha256.len(), 64);
            assert_eq!(receipt.scratch_bytes, 0);
            println!(
                "qualified XQ {} m={tokens} n={rows} k={columns} source={} ptx={}",
                kind.as_str(),
                receipt.source_sha256,
                receipt.ptx_sha256
            );
        }
    }
}
