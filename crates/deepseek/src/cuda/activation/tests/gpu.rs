use super::*;
use aster_power::inference::{DevicePreference, RuntimeDevice};
use candle_core::{Device, Tensor};
use std::time::Instant;

use crate::layers::quant::{
    decode_e2m1_tensor, decode_e4m3_tensor, encode_e2m1_tensor, encode_e4m3_tensor,
};

fn bits(input: &Tensor) -> Vec<u32> {
    input
        .flatten_all()
        .unwrap()
        .to_vec1::<f32>()
        .unwrap()
        .into_iter()
        .map(f32::to_bits)
        .collect()
}
fn bytes(input: &Tensor) -> Vec<u8> {
    input.flatten_all().unwrap().to_vec1().unwrap()
}
fn nearest(input: f32, palette: &[f64]) -> u8 {
    let absolute = f64::from(input.abs()).min(*palette.last().unwrap());
    (0..palette.len())
        .min_by(|&a, &b| {
            (absolute - palette[a])
                .abs()
                .total_cmp(&(absolute - palette[b]).abs())
                .then_with(|| (a % 2).cmp(&(b % 2)))
        })
        .unwrap() as u8
}
fn check_encoders(input: &Tensor, oracle: bool) {
    let fused4 = super::super::encode_e4m3(input).unwrap();
    let fused2 = super::super::encode_e2m1(input).unwrap();
    assert_eq!(bytes(&fused4), bytes(&encode_e4m3_tensor(input).unwrap()));
    assert_eq!(bytes(&fused2), bytes(&encode_e2m1_tensor(input).unwrap()));
    if oracle {
        let values = input.flatten_all().unwrap().to_vec1::<f32>().unwrap();
        let e4: Vec<_> = (0..127)
            .map(|i| crate::reference::decode_e4m3(i).unwrap())
            .collect();
        let e2: Vec<_> = (0..8)
            .map(|i| crate::reference::decode_e2m1(i).unwrap())
            .collect();
        let expected4: Vec<_> = values
            .iter()
            .map(|x| nearest(*x, &e4) + if x.is_sign_negative() { 128 } else { 0 })
            .collect();
        let expected2: Vec<_> = values
            .chunks_exact(2)
            .map(|pair| {
                let code = |x: f32| nearest(x, &e2) + if x.is_sign_negative() { 8 } else { 0 };
                code(pair[0]) | (code(pair[1]) << 4)
            })
            .collect();
        assert_eq!(bytes(&fused4), expected4);
        assert_eq!(bytes(&fused2), expected2);
    }
}

#[test]
#[ignore = "requires owned H200; exhaustive codec numeric controls and component A/B"]
fn h200_fused_activation_codecs_match_every_byte_midpoint_stride_and_report_cost() {
    let runtime = RuntimeDevice::resolve(DevicePreference::Cuda { ordinal: 0 }).unwrap();
    let device = runtime.tensor_device();
    let all_bytes = Tensor::from_vec((0..=255_u8).collect::<Vec<_>>(), (1, 256), device).unwrap();
    let actual4 = super::super::decode_e4m3(&all_bytes).unwrap();
    assert_eq!(
        bits(&actual4),
        bits(&decode_e4m3_tensor(&all_bytes).unwrap())
    );
    let expected4: Vec<_> = (0..=255_u8)
        .map(|code| {
            crate::reference::decode_e4m3(code)
                .map(|x| (x as f32).to_bits())
                .unwrap_or(f32::NAN.to_bits())
        })
        .collect();
    assert_eq!(bits(&actual4), expected4);
    let actual2 = super::super::decode_e2m1(&all_bytes).unwrap();
    assert_eq!(
        bits(&actual2),
        bits(&decode_e2m1_tensor(&all_bytes).unwrap())
    );
    let expected2: Vec<_> = (0..=255_u8)
        .flat_map(|code| [code & 15, code >> 4])
        .map(|code| (crate::reference::decode_e2m1(code).unwrap() as f32).to_bits())
        .collect();
    assert_eq!(bits(&actual2), expected2);
    let strided_bytes = all_bytes
        .reshape((16, 16))
        .unwrap()
        .transpose(0, 1)
        .unwrap()
        .narrow(0, 1, 14)
        .unwrap();
    assert_eq!(
        bits(&super::super::decode_e4m3(&strided_bytes).unwrap()),
        bits(&decode_e4m3_tensor(&strided_bytes).unwrap())
    );
    assert_eq!(
        bits(&super::super::decode_e2m1(&strided_bytes).unwrap()),
        bits(&decode_e2m1_tensor(&strided_bytes).unwrap())
    );
    let offset_bytes = all_bytes
        .reshape((16, 16))
        .unwrap()
        .narrow(0, 1, 14)
        .unwrap();
    assert!(offset_bytes.is_contiguous());
    assert_ne!(offset_bytes.layout().start_offset(), 0);
    assert_eq!(
        bits(&super::super::decode_e4m3(&offset_bytes).unwrap()),
        bits(&decode_e4m3_tensor(&offset_bytes).unwrap())
    );
    assert_eq!(
        bits(&super::super::decode_e2m1(&offset_bytes).unwrap()),
        bits(&decode_e2m1_tensor(&offset_bytes).unwrap())
    );

    let mut values = vec![
        0_f32,
        -0.,
        f32::from_bits(1),
        -f32::from_bits(1),
        448.,
        -448.,
    ];
    let e4: Vec<_> = (0..127)
        .map(|i| crate::reference::decode_e4m3(i).unwrap() as f32)
        .collect();
    let e2: Vec<_> = (0..8)
        .map(|i| crate::reference::decode_e2m1(i).unwrap() as f32)
        .collect();
    for palette in [&e4, &e2] {
        for pair in palette.windows(2) {
            let midpoint = (pair[0] + pair[1]) * 0.5;
            for x in [midpoint.next_down(), midpoint, midpoint.next_up()] {
                values.extend([x, -x]);
            }
        }
    }
    let input = Tensor::from_vec(values.clone(), (1, values.len()), device).unwrap();
    check_encoders(&input, true);
    let mut guarded = vec![f32::NAN; values.len()];
    guarded.extend_from_slice(&values);
    guarded.extend(vec![f32::INFINITY; values.len()]);
    let offset = Tensor::from_vec(guarded, (3, values.len()), device)
        .unwrap()
        .narrow(0, 1, 1)
        .unwrap();
    assert!(offset.is_contiguous());
    assert_ne!(offset.layout().start_offset(), 0);
    check_encoders(&offset, true);
    let strided = Tensor::from_vec(values.repeat(4), (values.len(), 4), device)
        .unwrap()
        .transpose(0, 1)
        .unwrap();
    check_encoders(&strided, true);
    check_encoders(
        &input
            .to_dtype(DType::BF16)
            .unwrap()
            .to_dtype(DType::F32)
            .unwrap(),
        true,
    );
    let exceptional = Tensor::from_vec(
        vec![
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::MAX,
            -f32::MAX,
            -0.,
        ],
        (1, 6),
        device,
    )
    .unwrap();
    check_encoders(&exceptional, false);
    let empty = Tensor::zeros((0, 128), DType::F32, device).unwrap();
    assert_eq!(super::super::encode_e2m1(&empty).unwrap().dims(), &[0, 64]);
    assert!(
        super::super::encode_e4m3(&Tensor::ones((1, 4), DType::F32, &Device::Cpu).unwrap())
            .is_err()
    );
    assert!(super::super::encode_e2m1(&Tensor::ones((1, 3), DType::F32, device).unwrap()).is_err());
    assert!(super::super::decode_e4m3(&input).is_err());

    let input = Tensor::from_vec(
        (0..4096)
            .map(|i| (i as f32 - 2048.) / 512.)
            .collect::<Vec<_>>(),
        (8, 512),
        device,
    )
    .unwrap();
    let packed4 = super::super::encode_e4m3(&input).unwrap();
    let packed2 = super::super::encode_e2m1(&input).unwrap();
    let mut measurements = Vec::new();
    for repeat in 0..3 {
        for fused in if repeat % 2 == 0 {
            [false, true]
        } else {
            [true, false]
        } {
            device.synchronize().unwrap();
            let started = Instant::now();
            for _ in 0..100 {
                if fused {
                    super::super::encode_e4m3(&input).unwrap();
                    super::super::decode_e4m3(&packed4).unwrap();
                    super::super::encode_e2m1(&input).unwrap();
                    super::super::decode_e2m1(&packed2).unwrap();
                } else {
                    encode_e4m3_tensor(&input).unwrap();
                    decode_e4m3_tensor(&packed4).unwrap();
                    encode_e2m1_tensor(&input).unwrap();
                    decode_e2m1_tensor(&packed2).unwrap();
                }
            }
            device.synchronize().unwrap();
            measurements.push(serde_json::json!({"repeat":repeat+1,"fused":fused,"calls_per_codec":100,"seconds":started.elapsed().as_secs_f64()}));
        }
    }
    println!(
        "{}",
        serde_json::json!({"abi":ABI,"measurements":measurements,"full_model_promotion_claimed":false})
    );
}
