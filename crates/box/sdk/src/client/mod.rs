//! Direct runtime-backed management API for aster-box.
//!
//! This module intentionally returns typed Rust data instead of parsing CLI
//! tables or JSON text. Container metadata is read from the shared `boxes.json`
//! state model, while image, volume, network, and snapshot operations call
//! `aster-box-runtime` stores directly.

use std::collections::{HashMap, HashSet};
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::Arc;
#[cfg(unix)]
use std::time::Instant;

use aster_box_core::log::{json_log_path, LogDriver, LogEntry, RuntimeConsoleFilter};
use aster_box_core::network::{IsolationMode, NetworkConfig, NetworkEndpoint, NetworkMode};
use aster_box_core::platform::Platform;
use aster_box_core::snapshot::SnapshotMetadata;
#[cfg(unix)]
use aster_box_core::vmm::parse_signal_name;
use aster_box_core::volume::VolumeConfig;
use aster_box_core::{
    CreateExecutionRequest, ExecOutput, ExecRequest, ExecutionEventBatch, ExecutionEventsRequest,
    ExecutionGeneration, ExecutionId, ExecutionLease, ExecutionManager, ExecutionProcessInventory,
    ExecutionReservation, ExecutionResourceUpdate, ExecutionSessionManager, ExecutionSnapshot,
    ExecutionSnapshotId, ExecutionStats, ExecutionStatus, FileRequest, FileResponse,
    FilesystemRequest, FilesystemResponse, KillExecutionOptions, KillOutcome, OperationId,
    ReconcileOutcome, RestartExecutionOptions, StoredImage,
};
#[cfg(unix)]
use aster_box_runtime::is_process_alive;
use aster_box_runtime::oci::BuildResult as RuntimeBuildResult;
use aster_box_runtime::{
    is_process_alive_with_identity, load_resolved_image_config, BuildConfig as RuntimeBuildConfig,
    BuildNetworkPolicy, ImagePuller, ImageReference, ImageStore, ManagedExecutionState,
    NetworkStore, OciImage, PushResult, RegistryAuth, RegistryProtocol, RegistryPusher,
    SignaturePolicy, SnapshotStore, VolumeStore,
};
use serde::{Deserialize, Serialize};
use sysinfo::{Pid, System};

#[cfg(all(test, unix))]
use aster_box_runtime::pid_start_time;
#[cfg(unix)]
use aster_box_runtime::{AttestationReport, AttestationRequest, ExecClient, PtyClient};

use crate::box_state::{BoxRecord, StateFile};

include!("types.rs");
include!("summaries.rs");
include!("config.rs");
include!("core.rs");
include!("builders.rs");
include!("lifecycle.rs");
include!("support.rs");

#[cfg(test)]
mod tests;
