use super::*;
use candle_core::{DType, Device};

fn values(input: &Tensor) -> Vec<u32> {
    input
        .to_dtype(DType::F32)
        .unwrap()
        .flatten_all()
        .unwrap()
        .to_vec1::<f32>()
        .unwrap()
        .into_iter()
        .map(f32::to_bits)
        .collect()
}

#[test]
fn readonly_contiguous_views_keep_identity_and_offsets_while_control_still_copies() {
    let source =
        Tensor::from_vec(vec![f32::NAN, -0., 1., 2., f32::INFINITY], 5, &Device::Cpu).unwrap();
    let view = source.narrow(0, 1, 3).unwrap();
    assert!(view.is_contiguous());
    assert_eq!(view.layout().start_offset(), 1);
    let borrowed = prepare_with_policy(&view, Policy::BorrowContiguous).unwrap();
    assert_eq!(borrowed.id(), view.id());
    assert_eq!(borrowed.layout().start_offset(), 1);
    assert_eq!(
        values(&borrowed),
        vec![(-0_f32).to_bits(), 1_f32.to_bits(), 2_f32.to_bits()]
    );
    let frozen = prepare_with_policy(&view, Policy::FrozenCopy).unwrap();
    assert_ne!(frozen.id(), view.id());
    assert_eq!(frozen.layout().start_offset(), 0);
    assert_eq!(values(&frozen), values(&borrowed));
    assert_eq!(values(&prepare(&view).unwrap()), values(&view));
}

#[test]
fn strided_and_byte_inputs_preserve_every_logical_element_without_retained_snapshot_aliases() {
    let source = Tensor::from_vec(vec![-0_f32, 1., 2., 3., 4., 5.], (2, 3), &Device::Cpu).unwrap();
    let view = source.transpose(0, 1).unwrap();
    assert!(!view.is_contiguous());
    for policy in [Policy::FrozenCopy, Policy::BorrowContiguous] {
        let prepared = prepare_with_policy(&view, policy).unwrap();
        assert_ne!(prepared.id(), view.id());
        assert!(prepared.is_contiguous());
        assert_eq!(prepared.layout().start_offset(), 0);
        assert_eq!(values(&prepared), values(&view));
    }
    let bytes = Tensor::from_vec(vec![255_u8, 0, 128, 7, 254], 5, &Device::Cpu)
        .unwrap()
        .narrow(0, 1, 3)
        .unwrap();
    let prepared = prepare_with_policy(&bytes, Policy::BorrowContiguous).unwrap();
    assert_eq!(prepared.id(), bytes.id());
    assert_eq!(prepared.to_vec1::<u8>().unwrap(), vec![0, 128, 7]);
}
