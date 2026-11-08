use super::*;
use candle_core::{DType, Device};

fn check(tensor: &Tensor, positive: bool) -> Result<()> {
    validate(tensor, positive, || {
        crate::layers::validate_finite_tensor(tensor, positive, "oracle")
    })
}

#[test]
fn clones_reuse_equal_or_stronger_checks_and_invalid_predicates_poison_commit() {
    let input = Tensor::new(&[-1_f32, 2.], &Device::Cpu).unwrap();
    let scope = Scope::begin(&InferenceLimits::default(), 0).unwrap();
    check(&input, false).unwrap();
    check(&input.clone(), false).unwrap();
    assert_eq!(scope.counts(), (1, 1));
    assert!(check(&input, true).is_err());
    assert!(scope.validate_complete().is_err());
    assert!(check(&input, false).is_err());
    drop(scope);
    let good = Tensor::ones(3, DType::F32, &Device::Cpu).unwrap();
    let scope = Scope::begin(&InferenceLimits::default(), 0).unwrap();
    check(&good, true).unwrap();
    check(&good, false).unwrap();
    check(&good, true).unwrap();
    assert_eq!(scope.counts(), (1, 2));
    scope.validate_complete().unwrap();
}

#[test]
fn new_views_and_new_steps_cannot_inherit_old_tensor_validation() {
    let input = Tensor::new(&[1_f32, f32::NAN], &Device::Cpu).unwrap();
    let first = input.narrow(0, 0, 1).unwrap();
    {
        let scope = Scope::begin(&InferenceLimits::default(), 0).unwrap();
        check(&first, true).unwrap();
        assert!(check(&input.narrow(0, 1, 1).unwrap(), false).is_err());
        assert!(scope.validate_complete().is_err());
    }
    // Candle permits mutation; the owner must end the scope before doing so.
    first
        .const_set(candle_core::scalar::Scalar::F32(f32::NAN))
        .unwrap();
    let scope = Scope::begin(&InferenceLimits::default(), 0).unwrap();
    assert!(check(&first, false).is_err());
    assert_eq!(scope.counts(), (1, 0));
}

#[test]
fn collisions_never_substitute_another_tensor_and_metadata_stays_bounded() {
    let scope = Scope::begin(&InferenceLimits::default(), 0).unwrap();
    for _ in 0..SLOTS + 1 {
        check(&Tensor::new(&[0_f32], &Device::Cpu).unwrap(), true).unwrap();
    }
    assert_eq!(scope.counts(), ((SLOTS + 1) as u64, 0));
    assert!(check(&Tensor::new(&[f32::INFINITY], &Device::Cpu).unwrap(), false).is_err());
    assert!(scope.validate_complete().is_err());
}

#[test]
fn scopes_refuse_nesting_and_excess_budget_and_end_on_error_or_thread_exit() {
    let limited = InferenceLimits {
        max_state_bytes: 1,
        ..InferenceLimits::default()
    };
    assert!(Scope::begin(&limited, 0).is_err());
    let scope = Scope::begin(&InferenceLimits::default(), 0).unwrap();
    assert!(Scope::begin(&InferenceLimits::default(), 0).is_err());
    let other = std::thread::spawn(|| {
        let scope = Scope::begin(&InferenceLimits::default(), 0).unwrap();
        let input = Tensor::new(&[1_f32], &Device::Cpu).unwrap();
        check(&input, false).unwrap();
        check(&input, false).unwrap();
        assert_eq!(scope.counts(), (1, 1));
    });
    other.join().unwrap();
    assert_eq!(scope.counts(), (0, 0));
    drop(scope);
    Scope::begin(&InferenceLimits::default(), 0)
        .unwrap()
        .validate_complete()
        .unwrap();
}

#[cfg(feature = "cuda")]
mod gpu;
