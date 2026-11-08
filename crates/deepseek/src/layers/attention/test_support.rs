use crate::{
    layers::{linear::DeviceMatrix, norm::EffectiveRmsNorm, CudaLayerDevice},
    reference,
    weights::MatrixLayout,
};
use aster_power::inference::InferenceLimits;
use candle_core::{DType, Device, Tensor};

pub(super) fn device() -> CudaLayerDevice {
    CudaLayerDevice::cpu_for_tests(InferenceLimits::default())
}
pub(super) fn bf16(values: &[f32], shape: &[usize]) -> Tensor {
    Tensor::from_vec(values.to_vec(), shape, &Device::Cpu)
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap()
}
pub(super) fn floats(t: &Tensor) -> Vec<f32> {
    t.to_dtype(DType::F32)
        .unwrap()
        .flatten_all()
        .unwrap()
        .to_vec1()
        .unwrap()
}
pub(super) fn bytes(t: &Tensor) -> Vec<u8> {
    t.flatten_all().unwrap().to_vec1().unwrap()
}
pub(super) fn round(x: f32) -> f32 {
    let b = x.to_bits();
    f32::from_bits(b.wrapping_add(0x7fff + ((b >> 16) & 1)) & 0xffff0000)
}
pub(super) fn cache() -> Tensor {
    let mut v: Vec<f32> = Vec::new();
    for (c, s) in [(1., 0.), (0., 1.), (-1., 0.), (0., -1.)] {
        v.extend([c; 32]);
        v.extend([s; 32]);
    }
    Tensor::from_vec(v, (4, 64), &Device::Cpu).unwrap()
}
pub(super) fn norm_weights(width: usize) -> Vec<f32> {
    (0..width).map(|i| [1., 0.5, 2., -1.][i % 4]).collect()
}
pub(super) fn norm(device: &CudaLayerDevice, width: usize) -> EffectiveRmsNorm {
    EffectiveRmsNorm::new(
        device.clone(),
        width,
        1e-20,
        Some(bf16(&norm_weights(width), &[width])),
    )
    .unwrap()
}
pub(super) fn matrix(
    device: &CudaLayerDevice,
    rows: usize,
    cols: usize,
    data: &[u8],
    scales: &[u8],
) -> DeviceMatrix {
    DeviceMatrix::from_tensors_for_tests(
        device,
        MatrixLayout::Fp8Block32,
        [rows, cols],
        &Tensor::from_vec(data.to_vec(), (rows, cols), &Device::Cpu).unwrap(),
        Some(
            &Tensor::from_vec(
                scales.to_vec(),
                (rows.div_ceil(32), cols.div_ceil(32)),
                &Device::Cpu,
            )
            .unwrap(),
        ),
    )
    .unwrap()
}
pub(super) fn scalar_fp8(row: &[f32]) -> (Vec<u8>, Vec<u8>, Vec<f32>) {
    let palette: Vec<_> = (0..127)
        .map(|c| reference::decode_e4m3(c).unwrap() as f32)
        .collect();
    let mut data = Vec::new();
    let mut scales = Vec::new();
    let mut logical = Vec::new();
    for group in row.chunks(32) {
        let max = group.iter().map(|x| x.abs()).fold(1e-4_f32, f32::max);
        let e = (max * (1_f32 / 448.)).log2().ceil() as i32;
        let sf = 2_f32.powi(e);
        scales.push((e + 127) as u8);
        for x in group {
            let normalized = f64::from(x.abs() / sf);
            let code = (0..127)
                .min_by(|a, b| {
                    (normalized - f64::from(palette[*a]))
                        .abs()
                        .total_cmp(&(normalized - f64::from(palette[*b])).abs())
                        .then_with(|| (a % 2).cmp(&(b % 2)))
                })
                .unwrap();
            let sign = if x.is_sign_negative() { -1. } else { 1. };
            data.push(code as u8 | if x.is_sign_negative() { 128 } else { 0 });
            logical.push(palette[code] * sf * sign);
        }
    }
    (data, scales, logical)
}
pub(super) fn scalar_linear(
    input: &[f32],
    data: &[u8],
    sf: &[u8],
    rows: usize,
    cols: usize,
) -> Vec<f32> {
    (0..rows)
        .map(|r| {
            let value = (0..cols).fold(0_f64, |sum, c| {
                let weight = reference::decode_e4m3(data[r * cols + c]).unwrap()
                    * reference::decode_ue8m0(sf[(r / 32) * cols.div_ceil(32) + c / 32]).unwrap();
                sum + f64::from(input[c]) * weight
            });
            round(value as f32)
        })
        .collect()
}
pub(super) fn scalar_norm(input: &[f32], weights: &[f32]) -> Vec<f32> {
    let mean =
        (input.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>() / input.len() as f64) as f32;
    input
        .iter()
        .zip(weights)
        .map(|(x, w)| round((*x / (mean + 1e-20_f32).sqrt()) * w))
        .collect()
}
pub(super) fn scalar_rope(row: &[f32], position: usize, conjugate: bool) -> Vec<f32> {
    let mut out = row.to_vec();
    let (c, s) = [(1., 0.), (0., 1.), (-1., 0.), (0., -1.)][position];
    let s = if conjugate { -s } else { s };
    let offset = row.len() - 64;
    for pair in 0..32 {
        let i = offset + 2 * pair;
        let a = row[i];
        let b = row[i + 1];
        out[i] = round(a * c - b * s);
        out[i + 1] = round(b * c + a * s);
    }
    out
}
