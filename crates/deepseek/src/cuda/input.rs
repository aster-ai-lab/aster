//! Inputs borrowed only for read-only custom kernels with fresh outputs.
//!
//! This helper must never replace an owning weight/cache snapshot: a contiguous
//! view can alias caller storage. Kernel arguments track reads and storage
//! lifetimes through cudarc. Every custom kernel still slices its actual layout
//! offsets; noncontiguous views retain the real copy path. No data is mutated.
use candle_core::Tensor;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Policy {
    FrozenCopy,
    BorrowContiguous,
}

pub(crate) const POLICY: Policy = if cfg!(feature = "xq-borrowed-kernel-inputs") {
    Policy::BorrowContiguous
} else {
    Policy::FrozenCopy
};

pub(crate) fn prepare(input: &Tensor) -> candle_core::Result<Tensor> {
    prepare_with_policy(input, POLICY)
}

pub(crate) fn prepare_with_policy(input: &Tensor, policy: Policy) -> candle_core::Result<Tensor> {
    match policy {
        Policy::FrozenCopy => input.force_contiguous(),
        Policy::BorrowContiguous => input.contiguous(),
    }
}

#[cfg(test)]
mod tests;
