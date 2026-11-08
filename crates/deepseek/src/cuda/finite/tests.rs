use super::*;

#[test]
fn finite_flag_grid_covers_partial_warps_empty_inputs_and_u32_boundary() {
    assert_eq!(geometry(0).unwrap(), (0, 1));
    assert_eq!(geometry(257).unwrap(), (257, 2));
    assert_eq!(geometry(u32::MAX as usize).unwrap(), (u32::MAX, 16_777_216));
    if usize::BITS > 32 {
        assert!(geometry(u32::MAX as usize + 1).is_err());
    }
}

#[cfg(feature = "cuda")]
mod gpu;
