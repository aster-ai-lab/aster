use super::*;
use crate::reference;
use aster_power::inference::InferenceLimits;
use candle_core::{Device, Var};

fn device() -> CudaLayerDevice {
    CudaLayerDevice::cpu_for_tests(InferenceLimits::default())
}
fn row(format: XqActivationFormat, data: Vec<f32>) -> Tensor {
    Tensor::from_vec(data, (1, format.columns()), &Device::Cpu).unwrap()
}
fn nearest(value: f32, palette: &[f64]) -> u8 {
    let x = f64::from(value.abs()).min(*palette.last().unwrap());
    (0..palette.len())
        .min_by(|&a, &b| {
            (x - palette[a])
                .abs()
                .total_cmp(&(x - palette[b]).abs())
                .then_with(|| (a % 2).cmp(&(b % 2)))
        })
        .unwrap() as u8
}
pub(in crate::layers::xq) fn scalar(
    format: XqActivationFormat,
    input: &[f32],
) -> (Vec<u8>, Vec<u8>, Vec<f32>) {
    let e4: Vec<f64> = (0..127)
        .map(|i| reference::decode_e4m3(i).unwrap())
        .collect();
    let e2: Vec<f64> = (0..8).map(|i| reference::decode_e2m1(i).unwrap()).collect();
    let mut codes = Vec::new();
    let mut scales = Vec::new();
    let mut decoded = Vec::new();
    for group in input.chunks_exact(format.group()) {
        let amax = group
            .iter()
            .map(|x| x.abs())
            .fold(0_f32, f32::max)
            .max(format.amax_floor());
        let target = amax / format.max_value();
        let (code, scale) = if format == XqActivationFormat::CompressedFp4 {
            let code = nearest(target, &e4);
            (code, e4[code as usize] as f32)
        } else {
            let code = (((target.to_bits() - 1) >> 23) + 1) as u8;
            (code, f32::from_bits(u32::from(code) << 23))
        };
        scales.push(code);
        for value in group {
            let normalized = (*value / scale).clamp(-format.max_value(), format.max_value());
            let palette = if format == XqActivationFormat::WindowFp8 {
                &e4
            } else {
                &e2
            };
            let code = nearest(normalized, palette);
            let signed = code
                + if normalized.is_sign_negative() {
                    if format == XqActivationFormat::WindowFp8 {
                        128
                    } else {
                        8
                    }
                } else {
                    0
                };
            codes.push(signed);
            let logical = (palette[code as usize] as f32)
                * if normalized.is_sign_negative() {
                    -1.
                } else {
                    1.
                };
            decoded.push(logical * scale);
        }
    }
    if format != XqActivationFormat::WindowFp8 {
        codes = codes.chunks_exact(2).map(|p| p[0] | p[1] << 4).collect();
    }
    (codes, scales, decoded)
}

#[test]
fn profiles_have_exact_geometry_and_unknown_values_refuse() {
    for (format, columns, group, bytes) in [
        (XqActivationFormat::WindowFp8, 512, 32, 528),
        (XqActivationFormat::CompressedFp4, 512, 16, 288),
        (XqActivationFormat::IndexFp4, 128, 32, 68),
    ] {
        assert_eq!(XqActivationFormat::parse(format.as_str()).unwrap(), format);
        assert_eq!(format.columns(), columns);
        assert_eq!(format.group(), group);
        let encoded = XqQuantizedActivation::encode(
            &device(),
            format,
            &row(format, vec![0.; columns]),
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(encoded.retained_bytes(), bytes);
        assert_eq!(encoded.format(), format);
        assert_eq!(encoded.row_count(), 1);
    }
    for bad in ["", "auto", "bf16", "fp8", "fp4"] {
        assert!(XqActivationFormat::parse(bad).is_err());
    }
}

#[test]
fn f32_division_quantization_matches_independent_scalar_codes_scales_and_decoded_bits() {
    for format in [
        XqActivationFormat::WindowFp8,
        XqActivationFormat::CompressedFp4,
        XqActivationFormat::IndexFp4,
    ] {
        let mut input: Vec<f32> = (0..format.columns())
            .map(|i| ((i * 29 % 131) as f32 - 65.) * 0.023 + 0.0001)
            .collect();
        input[0] = -0.;
        input[1] = 0.;
        input[2] = 1.25001;
        input[3] = -1.74999;
        let (data, scales, decoded) = scalar(format, &input);
        let encoded = XqQuantizedActivation::encode(
            &device(),
            format,
            &row(format, input),
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(encoded.data_bytes().to_vec2::<u8>().unwrap(), [data]);
        assert_eq!(encoded.scale_bytes().to_vec2::<u8>().unwrap(), [scales]);
        let actual = encoded.decode_f32(&CancellationToken::new()).unwrap();
        assert_eq!(actual.dtype(), DType::F32);
        assert_eq!(
            actual.to_vec2::<f32>().unwrap()[0]
                .iter()
                .map(|x| x.to_bits())
                .collect::<Vec<_>>(),
            decoded.iter().map(|x| x.to_bits()).collect::<Vec<_>>()
        );
    }
}

#[test]
fn compressed_midpoint_distinguishes_division_from_reciprocal_multiplication() {
    let format = XqActivationFormat::CompressedFp4;
    let mut input = vec![0_f32; 512];
    input[0] = f32::from_bits(0x3cc3ffff);
    input[15] = 6. * 0.013671875;
    assert_eq!(
        input[0] / 0.013671875,
        f32::from_bits(1.75_f32.to_bits() - 1)
    );
    assert_eq!(input[0] * (1_f32 / 0.013671875), 1.75);
    let encoded = XqQuantizedActivation::encode(
        &device(),
        format,
        &row(format, input),
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(encoded.scale_bytes().to_vec2::<u8>().unwrap()[0][0], 7);
    assert_eq!(encoded.data_bytes().to_vec2::<u8>().unwrap()[0][0] & 15, 3);
}

#[test]
fn input_is_not_precast_to_bf16_and_typed_storage_preserves_every_byte() {
    for format in [
        XqActivationFormat::WindowFp8,
        XqActivationFormat::CompressedFp4,
        XqActivationFormat::IndexFp4,
    ] {
        let mut input = vec![0.; format.columns()];
        input[0] = if format == XqActivationFormat::WindowFp8 {
            1.06251
        } else {
            1.25001
        };
        input[format.group() - 1] = format.max_value();
        let tensor = row(format, input);
        let cancel = CancellationToken::new();
        let actual = XqQuantizedActivation::encode(&device(), format, &tensor, &cancel).unwrap();
        let rounded = tensor
            .to_dtype(DType::BF16)
            .unwrap()
            .to_dtype(DType::F32)
            .unwrap();
        let changed = XqQuantizedActivation::encode(&device(), format, &rounded, &cancel).unwrap();
        assert_ne!(
            actual.data_bytes().to_vec2::<u8>().unwrap(),
            changed.data_bytes().to_vec2::<u8>().unwrap()
        );
        let data = actual.data_bytes().to_vec2::<u8>().unwrap();
        let scales = actual.scale_bytes().to_vec2::<u8>().unwrap();
        if format == XqActivationFormat::IndexFp4 {
            let packed = actual.into_index_rows(&cancel).unwrap();
            assert_eq!(packed.data_bytes().to_vec2::<u8>().unwrap(), data);
            assert_eq!(packed.scale_bytes().to_vec2::<u8>().unwrap(), scales);
        } else {
            let records = actual
                .into_main_rows(&cancel)
                .unwrap()
                .records()
                .to_vec2::<u8>()
                .unwrap();
            assert_eq!(records[0], [data[0].clone(), scales[0].clone()].concat());
        }
    }
}

#[test]
fn empty_rows_have_typed_storage_and_input_variables_do_not_alias_encoded_bytes() {
    for format in [
        XqActivationFormat::WindowFp8,
        XqActivationFormat::CompressedFp4,
        XqActivationFormat::IndexFp4,
    ] {
        let cancel = CancellationToken::new();
        let d = device();
        let empty = Tensor::zeros((0, format.columns()), DType::F32, d.tensor_device()).unwrap();
        let encoded = XqQuantizedActivation::encode(&d, format, &empty, &cancel).unwrap();
        assert_eq!(encoded.retained_bytes(), 0);
        assert_eq!(
            encoded.decode_f32(&cancel).unwrap().dims(),
            [0, format.columns()]
        );
        if format == XqActivationFormat::IndexFp4 {
            assert_eq!(encoded.into_index_rows(&cancel).unwrap().row_count(), 0);
        } else {
            assert_eq!(encoded.into_main_rows(&cancel).unwrap().row_count(), 0);
        }
        let variable = Var::ones((1, format.columns()), DType::F32, d.tensor_device()).unwrap();
        let encoded =
            XqQuantizedActivation::encode(&d, format, variable.as_tensor(), &cancel).unwrap();
        let before = encoded.data_bytes().to_vec2::<u8>().unwrap();
        variable
            .set(&Tensor::zeros((1, format.columns()), DType::F32, d.tensor_device()).unwrap())
            .unwrap();
        assert_eq!(encoded.data_bytes().to_vec2::<u8>().unwrap(), before);
    }
}

#[test]
fn bad_geometry_precision_nonfinite_extreme_grid_budget_and_cancellation_refuse() {
    let format = XqActivationFormat::WindowFp8;
    let d = device();
    let cancel = CancellationToken::new();
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, f32::MAX] {
        assert!(
            XqQuantizedActivation::encode(&d, format, &row(format, vec![value; 512]), &cancel)
                .is_err()
        );
    }
    assert!(XqQuantizedActivation::encode(
        &d,
        format,
        &row(format, vec![0.; 512]).to_dtype(DType::BF16).unwrap(),
        &cancel
    )
    .is_err());
    assert!(XqQuantizedActivation::encode(
        &d,
        format,
        &Tensor::zeros((1, 128), DType::F32, d.tensor_device()).unwrap(),
        &cancel
    )
    .is_err());
    let strict = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_state_bytes: 4095,
        ..InferenceLimits::default()
    });
    assert!(
        XqQuantizedActivation::encode(&strict, format, &row(format, vec![0.; 512]), &cancel)
            .is_err()
    );
    assert!(XqQuantizedActivation::encode(
        &d,
        XqActivationFormat::CompressedFp4,
        &row(XqActivationFormat::CompressedFp4, vec![3000.; 512]),
        &cancel
    )
    .is_err());
    let input = row(format, vec![0.; 512]);
    let packed = XqQuantizedActivation::encode(&d, format, &input, &cancel).unwrap();
    assert!(packed.into_index_rows(&cancel).is_err());
    let index = XqActivationFormat::IndexFp4;
    let packed =
        XqQuantizedActivation::encode(&d, index, &row(index, vec![0.; 128]), &cancel).unwrap();
    assert!(packed.into_main_rows(&cancel).is_err());
    cancel.cancel();
    assert!(XqQuantizedActivation::encode(&d, format, &input, &cancel).is_err());
}
