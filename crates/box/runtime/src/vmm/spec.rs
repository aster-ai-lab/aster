//! InstanceSpec re-exported from `aster-box-core`.
//!
//! The canonical definition lives in `aster_box_core::vmm`. Re-exported here
//! so existing callers using `crate::vmm::InstanceSpec` continue to work.

pub use aster_box_core::vmm::{
    Entrypoint, FsMount, InstanceSpec, NetworkInstanceConfig, RawBlockDevice, RootfsSource,
    TeeInstanceConfig,
};
