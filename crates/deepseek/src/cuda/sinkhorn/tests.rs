use super::*;

#[test]
fn sinkhorn_plan_binds_sm90_exact_geometry_and_validation_live_bytes() {
    let plan = Plan::new((9, 0), &[5, 4, 4], 5 * 64 * 32).unwrap();
    assert_eq!((plan.tokens, plan.grid, plan.output_bytes), (5, 2, 320));
    for shape in [vec![0, 4, 4], vec![4097, 4, 4], vec![3, 16], vec![3, 4, 3]] {
        assert!(Plan::new((9, 0), &shape, u64::MAX).is_err());
    }
    assert!(Plan::new((9, 0), &[5, 4, 4], 5 * 64 * 32 - 1).is_err());
    assert!(Plan::new((8, 0), &[5, 4, 4], u64::MAX).is_err());
}

#[cfg(feature = "cuda")]
mod gpu;
