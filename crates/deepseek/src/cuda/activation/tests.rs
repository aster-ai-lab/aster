use super::*;

#[test]
fn codec_plan_preserves_shapes_and_bounds_both_sides_of_packed_pairs() {
    let shape = Shape::from((3, 514));
    let plan = Plan::new(Kind::EncodeE2m1, &shape, DType::F32).unwrap();
    assert_eq!(plan.output_shape.dims(), &[3, 257]);
    assert_eq!(plan.output_elements, 771);
    assert_eq!(plan.work_items, 771);
    assert_eq!(plan.grid, 4);
    let plan = Plan::new(Kind::DecodeE2m1, &plan.output_shape, DType::U8).unwrap();
    assert_eq!(plan.output_shape, shape);
    assert_eq!(plan.output_elements, 1542);
    for kind in [Kind::EncodeE4m3, Kind::DecodeE4m3] {
        let dtype = if matches!(kind, Kind::EncodeE4m3) {
            DType::F32
        } else {
            DType::U8
        };
        let plan = Plan::new(kind, &Shape::from(()), dtype).unwrap();
        assert_eq!(plan.work_items, 1);
        assert!(plan.output_shape.dims().is_empty());
    }
    let empty = Plan::new(Kind::DecodeE2m1, &Shape::from((0, 64)), DType::U8).unwrap();
    assert_eq!(empty.output_elements, 0);
    assert_eq!(empty.grid, 1);
    assert!(Plan::new(Kind::EncodeE2m1, &Shape::from((1, 3)), DType::F32).is_err());
    assert!(Plan::new(Kind::EncodeE2m1, &Shape::from(4), DType::F32).is_err());
    assert!(Plan::new(Kind::EncodeE4m3, &Shape::from(4), DType::BF16).is_err());
    assert!(Plan::new(Kind::DecodeE4m3, &Shape::from(4), DType::F32).is_err());
    assert!(Plan::new(
        Kind::DecodeE2m1,
        &Shape::from((1, u32::MAX as usize)),
        DType::U8
    )
    .is_err());
    assert!(Plan::new(Kind::EncodeE4m3, &Shape::from((usize::MAX, 2)), DType::F32).is_err());
}

#[cfg(feature = "cuda")]
mod gpu;
