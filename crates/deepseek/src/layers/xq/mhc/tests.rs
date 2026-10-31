use super::*;
use aster_power::inference::InferenceLimits;
use candle_core::{Device, Var};

fn continuation() -> (CudaLayerDevice, XqMhcContinuation) {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let embedding = Tensor::from_vec(
        vec![1.0001_f32, -2.0003, 0.7501, 3.141],
        (1, 4),
        &Device::Cpu,
    )
    .unwrap();
    let continuation =
        XqMhcContinuation::from_embedding(&device, &embedding, &CancellationToken::new()).unwrap();
    (device, continuation)
}

#[test]
fn initial_shifted_streams_and_head_preserve_every_f32_bit() {
    let (device, initial) = continuation();
    let first = initial.head_input().unwrap().to_vec2::<f32>().unwrap();
    assert_eq!(
        first[0].iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
        [1.0001_f32, -2.0003, 0.7501, 3.141]
            .iter()
            .map(|x| x.to_bits())
            .collect::<Vec<_>>()
    );
    assert_eq!(initial.residual.dims(), [1, 4, 4]);
    assert_eq!(
        initial
            .next_pre_mix
            .flatten_all()
            .unwrap()
            .to_vec1::<f32>()
            .unwrap(),
        [1., 0., 0., 0.]
    );
    let variable = Var::ones((1, 4, 4), DType::F32, &Device::Cpu).unwrap();
    let changed = initial.with_residual(variable.as_tensor()).unwrap();
    variable
        .set(&Tensor::zeros((1, 4, 4), DType::F32, &Device::Cpu).unwrap())
        .unwrap();
    assert_eq!(
        changed.head_input().unwrap().to_vec2::<f32>().unwrap(),
        [vec![1_f32; 4]]
    );
    let tiny = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_state_bytes: 1,
        ..InferenceLimits::default()
    });
    assert!(XqMhcContinuation::from_embedding(
        &tiny,
        &Tensor::zeros((1, 4), DType::F32, device.tensor_device()).unwrap(),
        &CancellationToken::new()
    )
    .is_err());
}

#[test]
fn asymmetric_mix_sinkhorn_and_post_match_independent_scalar_composition() {
    let (_device, mut prior) = continuation();
    let residual: Vec<f32> = (0..16).map(|i| i as f32 * 0.137 - 0.9).collect();
    prior.residual = Tensor::from_slice(&residual, (1, 4, 4), &Device::Cpu).unwrap();
    prior.next_pre_mix =
        Tensor::from_vec(vec![0.1_f32, 0.2, 0.3, 0.4001], (1, 4, 1), &Device::Cpu).unwrap();
    let projected: Vec<f32> = (0..24)
        .map(|i| ((i * 7 % 23) as f32 - 11.) * 0.08)
        .collect();
    let base: Vec<f32> = (0..24).map(|i| ((i * 3 % 11) as f32 - 5.) * 0.03).collect();
    let scales = [0.3_f32, -0.2, 0.7];
    let step = finish_prepare(
        prior,
        &Tensor::from_slice(&projected, (1, 24), &Device::Cpu).unwrap(),
        &Tensor::new(&scales, &Device::Cpu).unwrap(),
        &Tensor::new(base.as_slice(), &Device::Cpu).unwrap(),
    )
    .unwrap();
    let actual_in = step.input.to_vec2::<f32>().unwrap();
    for col in 0..4 {
        let expected: f64 = (0..4)
            .map(|s| f64::from(residual[s * 4 + col]) * f64::from([0.1_f32, 0.2, 0.3, 0.4001][s]))
            .sum();
        assert!((f64::from(actual_in[0][col]) - expected).abs() < 2e-7);
    }
    let sigmoid = |v: f64| 1. / (1. + (-v).exp());
    let pre: Vec<f64> = (0..4)
        .map(|i| {
            sigmoid(f64::from(projected[i]) * f64::from(scales[0]) + f64::from(base[i])) + 1e-6
        })
        .collect();
    let post: Vec<f64> = (4..8)
        .map(|i| 2. * sigmoid(f64::from(projected[i]) * f64::from(scales[1]) + f64::from(base[i])))
        .collect();
    let mut matrix = [[0_f64; 4]; 4];
    for (src, values) in matrix.iter_mut().enumerate() {
        let logits: Vec<f64> = (0..4)
            .map(|dst| {
                let i = 8 + src * 4 + dst;
                f64::from(projected[i]) * f64::from(scales[2]) + f64::from(base[i])
            })
            .collect();
        let sum: f64 = logits.iter().map(|v| v.exp()).sum();
        for dst in 0..4 {
            values[dst] = logits[dst].exp() / sum + 1e-6;
        }
    }
    for iteration in 0..20 {
        if iteration > 0 {
            for row in &mut matrix {
                let sum: f64 = row.iter().sum::<f64>() + 1e-6;
                for x in row {
                    *x /= sum;
                }
            }
        }
        for dst in 0..4 {
            let sum: f64 = (0..4).map(|src| matrix[src][dst]).sum::<f64>() + 1e-6;
            for row in &mut matrix {
                row[dst] /= sum;
            }
        }
    }
    let branch = [0.7311_f32, -0.2417, 0.9913, 1.2314];
    let output = step
        .finish(
            &Tensor::from_slice(&branch, (1, 4), &Device::Cpu).unwrap(),
            &CancellationToken::new(),
        )
        .unwrap();
    let actual = output.residual.to_vec3::<f32>().unwrap();
    let actual_pre = output
        .next_pre_mix
        .flatten_all()
        .unwrap()
        .to_vec1::<f32>()
        .unwrap();
    for dst in 0..4 {
        assert!((f64::from(actual_pre[dst]) - pre[dst]).abs() < 1e-7);
        for col in 0..4 {
            let expected: f64 = (0..4)
                .map(|src| matrix[src][dst] * f64::from(residual[src * 4 + col]))
                .sum::<f64>()
                + post[dst] * f64::from(branch[col]);
            assert!((f64::from(actual[0][dst][col]) - expected).abs() < 1e-6);
        }
    }
}

#[test]
fn shifted_step_uses_carried_mix_and_rejects_wrong_branch_precision_or_cancel() {
    let (_, initial) = continuation();
    let zeros = Tensor::zeros((1, 24), DType::F32, &Device::Cpu).unwrap();
    let scale = Tensor::ones(3, DType::F32, &Device::Cpu).unwrap();
    let base = Tensor::zeros(24, DType::F32, &Device::Cpu).unwrap();
    let step = finish_prepare(initial, &zeros, &scale, &base).unwrap();
    assert_eq!(
        step.input.to_vec2::<f32>().unwrap()[0][0].to_bits(),
        1.0001_f32.to_bits()
    );
    assert!((step.next_pre_mix().to_vec3::<f32>().unwrap()[0][0][0] - 0.500001).abs() < 1e-7);
    assert!(step
        .finish(
            &Tensor::zeros((1, 4), DType::BF16, &Device::Cpu).unwrap(),
            &CancellationToken::new()
        )
        .is_err());
    let (_, initial) = continuation();
    let step = finish_prepare(initial, &zeros, &scale, &base).unwrap();
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(step
        .finish(
            &Tensor::zeros((1, 4), DType::F32, &Device::Cpu).unwrap(),
            &cancel
        )
        .is_err());
    for site in [XqMhcSite::Attention, XqMhcSite::FeedForward] {
        assert_eq!(XqMhcSite::parse(site.as_str()).unwrap(), site);
    }
    assert!(XqMhcSite::parse("head").is_err());
}

#[test]
fn mhc_packed_constructor_rejects_wrong_tier_shape_and_parameter_budget() {
    let (device, _) = continuation();
    let projection = XqDeviceMatrix::from_bytes_for_tests(
        &device,
        XqTensorType::F16,
        24,
        16,
        &vec![0; 24 * 16 * 2],
    )
    .unwrap();
    let scale = Tensor::ones(3, DType::F32, &Device::Cpu).unwrap();
    let base = Tensor::zeros(24, DType::F32, &Device::Cpu).unwrap();
    let block = XqMhcBlock::new(device.clone(), projection, scale.clone(), base.clone()).unwrap();
    let (_, initial) = continuation();
    assert!(block.prepare(initial, &CancellationToken::new()).is_err()); // no CPU inference fallback
    let bad = XqDeviceMatrix::from_bytes_for_tests(
        &device,
        XqTensorType::F32,
        24,
        16,
        &vec![0; 24 * 16 * 4],
    )
    .unwrap();
    assert!(XqMhcBlock::new(device, bad, scale, base).is_err());
}
