use super::*;
use crate::{
    layers::rope::{RopeDirection, RopeLayout, RotaryTransform},
    model::DeepSeekModel,
};
use aster_power::{error::PowerError, inference::InferenceLimits};
use candle_core::DType;

fn device() -> CudaLayerDevice {
    CudaLayerDevice::cpu_for_tests(InferenceLimits::default())
}
fn floats(t: &Tensor) -> Vec<f32> {
    t.flatten_all().unwrap().to_vec1().unwrap()
}
fn golden() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../../../fixtures/rope-fp32-cpu-golden.json"
    ))
    .unwrap()
}
fn decoded_bits(values: &serde_json::Value) -> Vec<f32> {
    values
        .as_array()
        .unwrap()
        .iter()
        .map(|v| f32::from_bits(v.as_u64().unwrap() as u32))
        .collect()
}

#[test]
fn both_profiles_match_independent_pytorch_cpu_frequency_fixture_with_declared_ulp_budget() {
    let golden = golden();
    let device = device();
    let cancel = CancellationToken::new();
    assert_eq!(golden["cuda_initialized_before"], false);
    assert_eq!(golden["cuda_initialized_after"], false);
    assert_eq!(
        golden["test_budgets_declared_before_comparison"]["cpu_frequency_max_ulp"],
        4
    );
    for row in golden["profiles"].as_array().unwrap() {
        let profile = AttentionRopeProfile::parse(row["profile"].as_str().unwrap()).unwrap();
        let built = AttentionRopeCache::new(&device, profile, 8, &cancel).unwrap();
        assert_eq!(built.profile(), profile);
        assert_eq!(built.context_tokens(), 8);
        assert_eq!(built.inverse_frequencies().dtype(), DType::F32);
        assert_eq!(built.cos_sin().dtype(), DType::F32);
        assert_eq!(built.retained_bytes(), (8 * 64 + 32) * 4);
        let expected = decoded_bits(&row["inverse_frequency_bits"]);
        for (i, (a, b)) in floats(built.inverse_frequencies())
            .iter()
            .zip(expected)
            .enumerate()
        {
            assert!(
                a.to_bits().abs_diff(b.to_bits()) <= 4,
                "{} frequency {i}: {a:?} vs {b:?}",
                profile.as_str()
            );
        }
        assert_eq!(floats(built.inverse_frequencies())[0], 1.);
        if profile == AttentionRopeProfile::CompressedYarn {
            // Independent scalar Python-double correction-range calculation,
            // only for the integer cutoff, not frequency or phase generation.
            let cutoff = |rot: f64| {
                64. * (65536. / (rot * 2. * std::f64::consts::PI)).ln() / (2. * 160000_f64.ln())
            };
            assert_eq!((cutoff(32.).floor(), cutoff(1.).ceil()), (15., 25.));
            assert_eq!(row["correction_range"], serde_json::json!([15, 25]));
        }
    }
}

#[test]
fn long_positions_match_independent_polar_fixture_for_identical_fp32_frequencies() {
    let golden = golden();
    let device = device();
    let positions: Vec<f32> = golden["positions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as f32)
        .collect();
    for row in golden["profiles"].as_array().unwrap() {
        let inverse = Tensor::from_vec(
            decoded_bits(&row["inverse_frequency_bits"]),
            32,
            device.tensor_device(),
        )
        .unwrap();
        let positions = Tensor::new(positions.as_slice(), device.tensor_device()).unwrap();
        let actual = cache_rows(&inverse, &positions).unwrap();
        let expected: Vec<_> = row["cos_sin_bits"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(decoded_bits)
            .collect();
        assert_eq!(actual.dims(), [6, 64]);
        for (i, (a, b)) in floats(&actual).iter().zip(expected).enumerate() {
            assert!(
                (*a - b).abs() <= 2.5e-7,
                "{} sample {i}: {a:?} vs {b:?}",
                row["profile"]
            );
        }
        assert_eq!(&floats(&actual)[..32], &[1.; 32]);
        assert_eq!(&floats(&actual)[32..64], &[0.; 32]);
    }
}

#[test]
fn phase_multiplication_rounds_to_fp32_before_cos_sin() {
    let device = device();
    let inverse = 0.3_f32;
    let position = 1048575_f32;
    let actual = floats(
        &cache_rows(
            &Tensor::new(&[inverse], device.tensor_device()).unwrap(),
            &Tensor::new(&[position], device.tensor_device()).unwrap(),
        )
        .unwrap(),
    );
    let phase32 = inverse * position;
    let expected = [
        f64::from(phase32).cos() as f32,
        f64::from(phase32).sin() as f32,
    ];
    let wrong_phase = f64::from(inverse) * f64::from(position);
    let wrong = [wrong_phase.cos() as f32, wrong_phase.sin() as f32];
    for (a, b) in actual.iter().zip(expected) {
        assert!((*a - b).abs() <= 2.5e-7);
    }
    assert!(
        actual
            .iter()
            .zip(wrong)
            .map(|(a, b)| (*a - b).abs())
            .fold(0_f32, f32::max)
            > 1e-3
    );
}

#[test]
fn growing_context_preserves_existing_cache_bits_and_fixed_yarn_factor() {
    let device = device();
    let cancel = CancellationToken::new();
    for profile in [
        AttentionRopeProfile::WindowBase,
        AttentionRopeProfile::CompressedYarn,
    ] {
        let small = AttentionRopeCache::new(&device, profile, 16, &cancel).unwrap();
        let large = AttentionRopeCache::new(&device, profile, 128, &cancel).unwrap();
        let bits = |t: &Tensor| floats(t).iter().map(|v| v.to_bits()).collect::<Vec<_>>();
        assert_eq!(
            bits(small.inverse_frequencies()),
            bits(large.inverse_frequencies())
        );
        assert_eq!(
            bits(small.cos_sin()),
            bits(&large.cos_sin().narrow(0, 0, 16).unwrap())
        );
        // The builder's actual FP32 frequencies independently drive scalar
        // phase/trig math. This checks assembly, not GPU powf equivalence.
        let freq = floats(large.inverse_frequencies());
        let values = floats(large.cos_sin());
        for position in [1, 7, 127] {
            for i in 0..32 {
                let phase = (position as f32) * freq[i];
                assert!(
                    (values[position * 64 + i] - (f64::from(phase).cos() as f32)).abs() <= 2.5e-7
                );
                assert!(
                    (values[position * 64 + 32 + i] - (f64::from(phase).sin() as f32)).abs()
                        <= 2.5e-7
                );
            }
        }
    }
}

#[test]
fn built_cache_connects_main_and_index_tail_rotations_and_conjugate_output() {
    let device = device();
    let cancel = CancellationToken::new();
    let built =
        AttentionRopeCache::new(&device, AttentionRopeProfile::CompressedYarn, 128, &cancel)
            .unwrap();
    let mut tails = Vec::new();
    for width in [512, 128] {
        let transform = RotaryTransform::from_cache(
            device.clone(),
            width,
            width - 64,
            RopeLayout::Interleaved,
            built.cos_sin().clone(),
        )
        .unwrap();
        let data: [f32; 64] = std::array::from_fn(|i| if i % 2 == 0 { 1. } else { -2. });
        let input = Tensor::from_vec(
            [vec![3.25; width - 64], data.to_vec()].concat(),
            (1, 1, width),
            device.tensor_device(),
        )
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap();
        let output = transform
            .forward(&input, &[127], RopeDirection::Forward, &cancel)
            .unwrap();
        let values = floats(&output.to_dtype(DType::F32).unwrap());
        assert_eq!(&values[..width - 64], vec![3.25; width - 64]);
        tails.push(values[width - 64..].to_vec());
        let back = transform
            .forward(&output, &[127], RopeDirection::Conjugate, &cancel)
            .unwrap();
        for (a, b) in floats(&back.to_dtype(DType::F32).unwrap())
            .iter()
            .zip(floats(&input.to_dtype(DType::F32).unwrap()))
        {
            assert!((*a - b).abs() <= 0.015625);
        }
    }
    assert_eq!(tails[0], tails[1]);
}

#[test]
fn layer_factory_binds_window_and_compressed_profiles_from_verified_official_metadata() {
    let cfg = DeepSeekModel::V41Flash1017Official
        .verify_config(include_bytes!("../../../../fixtures/official-config.json"))
        .unwrap();
    let architecture = cfg.text_architecture().unwrap();
    let device = device();
    let cancel = CancellationToken::new();
    for (id, expected) in [
        (0, AttentionRopeProfile::WindowBase),
        (1, AttentionRopeProfile::WindowBase),
        (2, AttentionRopeProfile::CompressedYarn),
        (39, AttentionRopeProfile::CompressedYarn),
    ] {
        let layer = architecture.layer(id).unwrap();
        assert_eq!(
            layer.new_rope_cache(&device, 8, &cancel).unwrap().profile(),
            expected
        );
    }
}

#[test]
fn factory_refuses_cancelled_context_tensor_state_and_resident_overruns_before_allocation() {
    let device = device();
    let profile = AttentionRopeProfile::CompressedYarn;
    let cancel = CancellationToken::new();
    for context in [
        0,
        usize::MAX,
        MAX_CONTEXT_TOKENS + 1,
        device.limits().max_context_tokens + 1,
    ] {
        assert!(AttentionRopeCache::new(&device, profile, context, &cancel).is_err());
    }
    for limits in [
        InferenceLimits {
            max_tensor_elements: 63,
            ..InferenceLimits::default()
        },
        InferenceLimits {
            max_state_bytes: 34000,
            ..InferenceLimits::default()
        },
        InferenceLimits {
            max_resident_weight_bytes: 383,
            ..InferenceLimits::default()
        },
    ] {
        let bounded = CudaLayerDevice::cpu_for_tests(limits);
        assert!(AttentionRopeCache::new(&bounded, profile, 1, &cancel).is_err());
    }
    cancel.cancel();
    assert!(matches!(
        AttentionRopeCache::new(&device, profile, 8, &cancel),
        Err(PowerError::InferenceCancelled)
    ));
}

#[test]
fn direct_powf_path_is_distinct_from_tensor_log_exp_substitution() {
    let device = device();
    let cancel = CancellationToken::new();
    let direct = inverse_frequencies(&device, AttentionRopeProfile::WindowBase, &cancel).unwrap();
    let base = Tensor::full(10000_f32, 32, device.tensor_device()).unwrap();
    let exponents = Tensor::arange(0_f32, 32_f32, device.tensor_device())
        .unwrap()
        .affine(1. / 32., 0.)
        .unwrap();
    let substitute = base.pow(&exponents).unwrap().recip().unwrap();
    assert!(floats(&direct)
        .iter()
        .zip(floats(&substitute))
        .any(|(a, b)| a.to_bits() != b.to_bits()));
    assert!(
        golden()["profiles"][0]["pow_logexp_different_count"]
            .as_u64()
            .unwrap()
            > 0
    );
}
