//! Typed facade for Aster application capabilities.

/// Stable user-level scope shared by standalone and embedded package hosts.
pub const PACKAGE_DEFAULT_SCOPE: &str = "user/current";

pub mod capability_registry;
pub mod cli;
mod extension_cli;
#[cfg(test)]
mod first_use;
#[cfg(feature = "extensions")]
pub mod package;
#[cfg(feature = "extensions")]
pub mod plugin_lifecycle;
#[cfg(feature = "extensions")]
pub mod plugin_manager;
#[cfg(feature = "extensions")]
pub mod plugin_runtime;
#[cfg(feature = "extensions")]
pub mod state_backup;
#[cfg(feature = "extensions")]
pub mod state_restore;
#[cfg(feature = "extensions")]
mod surface_reconciler;

#[cfg(all(test, feature = "extensions", any(unix, windows)))]
mod test_filesystem;

#[cfg(feature = "extensions")]
mod extension_host;

#[cfg(feature = "extensions")]
pub mod okf_knowledge;

pub use aster_use_core as core;

#[cfg(feature = "extensions")]
pub use aster_use_extension as extension;

#[cfg(feature = "tool-acquire")]
pub use aster_use_tool_acquire as tool_acquire;

#[cfg(feature = "reconcile")]
pub use aster_use_reconcile as reconcile;
