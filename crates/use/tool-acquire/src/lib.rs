//! Machine-local installer for a closed set of command-line tool specs.
//!
//! This crate records installs under a host-supplied root. It does not publish
//! a package generation, and its receipts are not package-manager
//! authority.

mod acquire;
mod archive;
mod catalog;
mod env_acl;
mod env_context;
mod env_lock;
mod error;
mod http;
mod installation;
mod lock;
mod prune;
mod receipt;
mod source;
mod spec;
mod store;
mod version;

pub use acquire::AcquireRequest;
pub use archive::{ArchiveFormat, ArchivePayload, InstallationPaths};
pub use catalog::{catalog, catalog_by_name, CatalogTool};
pub use env_acl::{
    load_env_config, load_env_declarations, nearest_project_env_declarations, parse_env_acl,
    parse_env_declarations, project_env_path, read_env_acl, read_env_declarations, render_env_acl,
    render_env_declarations, resolve_declarations, resolve_tools, user_env_path, write_env_acl,
    write_env_declarations, EnvConfig, EnvDeclarations, ResolvedTool,
};
pub use env_context::{
    find_env_executable, load_env_config_with_sources, load_env_declarations_with_sources,
    resolve_env_tool, EnvToolOrigin, ExternalToolPaths, LoadedEnvConfig, LoadedEnvDeclarations,
    ResolvedEnvTool,
};
pub use env_lock::{project_env_lock_path, EnvLock};
pub use error::ToolAcquireError;
pub use http::{
    DownloadProgress, FetchPolicy, HttpSource, InstalledToolchain, NodeSignatureError,
    PreparedToolchain, ToolchainPlugin,
};
pub use prune::ToolPrunePlan;
pub use receipt::{ToolFailureRecord, ToolReceipt, FAILURE_SCHEMA, RECEIPT_SCHEMA};
pub use source::{CompanionFile, InstallPayload, StaticSource, ToolPayload, ToolQuery, ToolSource};
pub use spec::{parse_tool_spec, CoreRuntime, ToolBackend, ToolSpec};
pub use store::{LocalApply, ToolStore};
pub use version::{
    ReferenceKind, VersionEntry, VersionListing, VersionRequest, VersionRequestKind,
};
