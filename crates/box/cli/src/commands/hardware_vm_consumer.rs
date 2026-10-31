use aster_box_core::config::ExecutionIsolation;
use aster_box_runtime::hardware_vm::{
    open_vm_only, select_isolation, stored_profile_bytes, ContractError, IsolationSelection,
    VmOnlyFeature,
};
use clap::ValueEnum;

use super::{resolve_isolation, IsolationArg};

#[test]
fn omitted_isolation_selects_hardware_vm_and_microvm_stays_rejected() {
    assert_eq!(resolve_isolation(None), ExecutionIsolation::Microvm);
    assert_eq!(
        resolve_isolation(Some(IsolationArg::Sandbox)),
        ExecutionIsolation::Sandbox
    );
    assert!(IsolationArg::from_str("microvm", true).is_err());
    assert!(IsolationArg::from_str("sandbox", true).is_ok());

    let live = std::env::var("ASTER_BOX_OCI_MIGRATION").ok();
    let baseline = stored_profile_bytes(None);
    assert_eq!(stored_profile_bytes(live.as_deref()), baseline);
    for migration in [None, Some("off"), Some("microvm"), Some("all")] {
        assert_eq!(stored_profile_bytes(migration), baseline);
        assert_eq!(
            select_isolation(None, migration),
            Ok(IsolationSelection::HardwareVm)
        );
        assert_eq!(
            select_isolation(Some("microvm"), migration),
            Err(ContractError::MicrovmSpellingRejected)
        );
        let sandbox = select_isolation(Some("sandbox"), migration).expect("sandbox");
        for feature in [
            VmOnlyFeature::Profile,
            VmOnlyFeature::Snapshot,
            VmOnlyFeature::ControlChannel,
        ] {
            assert_eq!(
                open_vm_only(sandbox, feature),
                Err(ContractError::SandboxRejectsVmOnly)
            );
        }
    }
}
