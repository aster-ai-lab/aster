use super::*;

#[test]
fn admitted_geometries_bind_exact_block_sizes_grid_and_memory() {
    for (kind, columns, row_bytes) in [
        (XqTensorType::F32, 5, 20),
        (XqTensorType::F16, 7, 14),
        (XqTensorType::Q8_0, 64, 68),
        (XqTensorType::Q2_0, 128, 36),
        (XqTensorType::Ptq1_0, 256, 56),
        (XqTensorType::Q3K, 512, 220),
    ] {
        let plan = XqPlan::new((9, 0), kind, 3, 7, columns, 84).unwrap();
        assert_eq!(
            (plan.tokens, plan.rows, plan.columns),
            (3, 7, columns as u32)
        );
        assert_eq!(
            (plan.type_id, plan.row_bytes, plan.grid_x, plan.output_bytes),
            (kind.gguf_id(), row_bytes, 6, 84)
        );
    }
    assert_eq!(source_sha256().len(), 64);
}

#[test]
fn architecture_integer_partial_blocks_empty_and_overflow_shapes_refuse() {
    for kind in [XqTensorType::I8, XqTensorType::I32] {
        assert!(XqPlan::new((9, 0), kind, 1, 1, 256, 4).is_err());
    }
    for capability in [(8, 0), (8, 9), (9, 1), (10, 0)] {
        assert!(XqPlan::new(capability, XqTensorType::Ptq1_0, 1, 1, 128, 4).is_err());
    }
    for (m, n, k, budget) in [
        (0, 1, 128, 4),
        (1, 0, 128, 4),
        (1, 1, 0, 4),
        (1, 1, 127, 4),
        (1, 1, 128, 3),
        (usize::MAX, 1, 128, u64::MAX),
        (1, usize::MAX, 128, u64::MAX),
        (1, 1, u32::MAX as usize, u64::MAX),
        (u32::MAX as usize, u32::MAX as usize, 128, u64::MAX),
    ] {
        assert!(XqPlan::new((9, 0), XqTensorType::Ptq1_0, m, n, k, budget).is_err());
    }
}

#[cfg(feature = "cuda")]
mod gpu;

#[test]
fn gather_plan_bounds_decoded_output_independently_of_matrix_rows() {
    let plan = XqGatherPlan::new((9, 0), XqTensorType::F16, 3, 129280, 5120, 3 * 5120 * 4).unwrap();
    assert_eq!(
        (
            plan.projection.tokens,
            plan.projection.rows,
            plan.projection.columns
        ),
        (3, 129280, 5120)
    );
    assert_eq!((plan.grid_x, plan.output_bytes), (60, 61440));
    for (tokens, rows, columns, budget) in [
        (0, 7, 32, 1024),
        (3, 7, 32, 383),
        (usize::MAX, 7, 32, u64::MAX),
        (u32::MAX as usize, 7, u32::MAX as usize - 31, u64::MAX),
    ] {
        assert!(
            XqGatherPlan::new((9, 0), XqTensorType::Q8_0, tokens, rows, columns, budget).is_err()
        );
    }
    assert!(XqGatherPlan::new((8, 0), XqTensorType::F16, 1, 7, 32, 128).is_err());
    assert!(XqGatherPlan::new((9, 0), XqTensorType::I32, 1, 7, 32, 128).is_err());
}
