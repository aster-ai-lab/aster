use super::*;
use crate::layers::attention::test_support::*;
use aster_power::inference::InferenceLimits;
use candle_core::Device;

fn value_weights() -> Vec<u8> {
    let mut v = vec![0_u8; 64 * 512];
    for r in 0..64 {
        let i = r * 512;
        match r % 32 {
            0 => {
                v[i] = 56;
                v[i + 1] = 24;
                v[i + 2] = 2;
            }
            1 => v[i] = 72,
            2 => v[i + 448] = 56,
            3 => v[i + 449] = 56,
            x => v[i + x] = if r < 32 { 56 } else { 184 },
        }
    }
    v
}
fn output_weights() -> Vec<u8> {
    let mut v = vec![0_u8; 32 * 64];
    for r in 0..32 {
        v[r * 64 + r] = 56;
        if r > 0 {
            v[r * 64 + 32 + r] = 48;
        }
    }
    v
}
fn build(device: &CudaLayerDevice) -> AttentionOutputProjection {
    AttentionOutputProjection::new(
        device.clone(),
        2,
        matrix(
            device,
            64,
            512,
            &value_weights(),
            &[vec![127; 16], vec![128; 16]].concat(),
        ),
        matrix(device, 32, 64, &output_weights(), &[127; 2]),
        cache(),
    )
    .unwrap()
}
fn context() -> Tensor {
    let mut values = vec![0_f32; 3 * 2 * 512];
    for token in 0..3 {
        for head in 0..2 {
            let offset = (token * 2 + head) * 512;
            for i in 0..32 {
                values[offset + i] = if token == 0 {
                    1.
                } else {
                    ((i * 7 + token * 5 + head * 3) % 23) as f32 / 8. - 1.
                };
            }
            values[offset + 448] = 0.25;
            values[offset + 449] = 0.5;
        }
    }
    bf16(&values, &[3, 2, 512])
}
fn scalar(context: &Tensor, positions: &[usize], conjugate: bool) -> Vec<f32> {
    let input = floats(context);
    let value = value_weights();
    let out = output_weights();
    let mut result = Vec::new();
    for (token, pos) in positions.iter().enumerate() {
        let mut intermediate = Vec::new();
        for group in 0..2 {
            let start = (token * 2 + group) * 512;
            let rotated = scalar_rope(&input[start..start + 512], *pos, conjugate);
            let cast = scalar_fp8(&rotated).2;
            intermediate.extend(scalar_linear(
                &cast,
                &value[group * 32 * 512..(group + 1) * 32 * 512],
                &[if group == 0 { 127 } else { 128 }; 16],
                32,
                512,
            ));
        }
        let cast = scalar_fp8(&intermediate).2;
        result.extend(scalar_linear(&cast, &out, &[127; 2], 32, 64));
    }
    result
}

#[test]
fn grouped_output_matches_scalar_conjugate_rope_bf16_boundary_and_group_scales() {
    let device = device();
    let owner = build(&device);
    let input = context();
    let before = floats(&input);
    let result = owner
        .forward(&input, &[1, 2, 3], &CancellationToken::new())
        .unwrap();
    assert_eq!(floats(&result), scalar(&input, &[1, 2, 3], true));
    assert_ne!(floats(&result), scalar(&input, &[1, 2, 3], false));
    // 1 + 1/16 + 1/256 rounds to BF16 1.0625, then FP8 ties to 1.0.
    // A direct F32 -> FP8 shortcut instead yields 1.125.
    assert_eq!(floats(&result)[0], 1.);
    assert_eq!(scalar_fp8(&[1.06640625, 4.]).2[0], 1.125);
    assert_eq!(owner.groups[1].source_first_row(), 32);
    assert_eq!(owner.groups[1].source_total_rows(), 64);
    assert_eq!(floats(&input), before);
}

#[test]
fn each_token_partition_preserves_grouped_attention_output() {
    let device = device();
    let owner = build(&device);
    let input = context();
    let cancel = CancellationToken::new();
    let all = owner.forward(&input, &[0, 1, 3], &cancel).unwrap();
    let mut values = Vec::new();
    for (row, pos) in [0, 1, 3].into_iter().enumerate() {
        values.extend(floats(
            &owner
                .forward(&input.narrow(0, row, 1).unwrap(), &[pos], &cancel)
                .unwrap(),
        ));
    }
    assert_eq!(values, floats(&all));
    assert_eq!(owner.heads(), 2);
    assert_eq!(owner.hidden_size(), 32);
}

#[test]
fn raw_group_slices_preserve_sf_and_activation_columns_without_requantizing() {
    let device = device();
    let cancel = CancellationToken::new();
    let source = matrix(&device, 64, 64, &[56; 4096], &[127, 128, 129, 130]);
    let group = source.fp8_output_rows(32, 32, &cancel).unwrap();
    let ids = Tensor::new(&[0_u32, 31], &Device::Cpu).unwrap();
    let gathered = group
        .gather_rows(&ids, DType::F32, &cancel)
        .unwrap()
        .to_vec2::<f32>()
        .unwrap();
    assert_eq!(&gathered[0][..32], &[4.; 32]);
    assert_eq!(&gathered[0][32..], &[8.; 32]);
    assert_eq!(gathered[0], gathered[1]);
    assert!(source.fp8_output_rows(1, 32, &cancel).is_err());
    assert!(source.fp8_output_rows(32, 64, &cancel).is_err());
    let packed = Fp8Activations::from_packed(
        &device,
        &Tensor::full(56_u8, (2, 64), &Device::Cpu).unwrap(),
        &Tensor::new(&[[127_u8, 128], [129, 130]], &Device::Cpu).unwrap(),
    )
    .unwrap();
    let slice = packed.column_range(32, 32, &cancel).unwrap();
    assert_eq!(bytes(slice.data_bytes()), vec![56; 64]);
    assert_eq!(bytes(slice.scale_bytes()), [128, 130]);
    assert!(packed.column_range(1, 32, &cancel).is_err());
    assert!(packed.column_range(32, 64, &cancel).is_err());
}

#[test]
fn bad_group_shape_dtype_positions_cancel_and_combined_budgets_refuse() {
    let device = device();
    let cancel = CancellationToken::new();
    let owner = build(&device);
    assert!(owner.forward(&context(), &[0, 1, 4], &cancel).is_err());
    assert!(owner
        .forward(
            &context().to_dtype(DType::F32).unwrap(),
            &[0, 1, 2],
            &cancel
        )
        .is_err());
    assert!(owner
        .forward(
            &bf16(&[f32::INFINITY; 3 * 2 * 512], &[3, 2, 512]),
            &[0, 1, 2],
            &cancel
        )
        .is_err());
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(owner.forward(&context(), &[0, 1, 2], &cancelled).is_err());
    assert!(AttentionOutputProjection::new(
        device.clone(),
        0,
        matrix(&device, 64, 512, &value_weights(), &[127; 32]),
        matrix(&device, 32, 64, &output_weights(), &[127; 2]),
        cache()
    )
    .is_err());
    assert!(AttentionOutputProjection::new(
        device.clone(),
        3,
        matrix(&device, 64, 512, &value_weights(), &[127; 32]),
        matrix(&device, 32, 64, &output_weights(), &[127; 2]),
        cache()
    )
    .is_err());
    let low = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_resident_weight_bytes: 35_000,
        ..InferenceLimits::default()
    });
    assert!(AttentionOutputProjection::new(
        low.clone(),
        2,
        matrix(&low, 64, 512, &value_weights(), &[127; 32]),
        matrix(&low, 32, 64, &output_weights(), &[127; 2]),
        cache()
    )
    .is_err());
    let low = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_state_bytes: 900_000,
        ..InferenceLimits::default()
    });
    let owner = build(&low);
    let error = owner
        .forward(&context(), &[0, 1, 2], &cancel)
        .err()
        .unwrap();
    assert!(
        error
            .to_string()
            .contains("attention output composite live tensors"),
        "{error}"
    );
}
