//! One admitted macOS command inside a utility VM.
//!
//! `krun_start_enter` consumes the worker, so each command gets its own guest.
//! The guest init mounts only the requested workspace and execs the image
//! agent, which runs the command script as PID 1 of a new PID namespace.
//! The fixed init text contains no caller command, environment, or path.

use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use a3s_oci_agent_protocol::AGENT_RUNTIME_SHARE_TAG;
use serde::{Deserialize, Serialize};

use crate::macos_context::{KrunContext, MacosKrunApi};
use crate::macos_runtime_share::MacosRuntimeShare;
use crate::macos_system_image::MacosSystemImage;
use crate::unix_process::{
    prepare_console_output, read_bounded_worker_output, resolve_console, terminate_and_wait,
    wait_for_worker, PinnedCurrentExecutable,
};

const WORKER_COMMAND: &str = "__macos-guest-command-worker";
const WORKER_SCHEMA_VERSION: &str = "a3s.oci.guest-command-worker.v1";
pub const GUEST_COMMAND_SCHEMA_VERSION: &str = "a3s.oci.guest-command.v1";
const MAX_WORKER_OUTPUT_BYTES: u64 = 64 * 1024;
const TIMEOUT_EXIT_CODE: i32 = 124;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuestCommandSettlement {
    Refused,
    Settled,
    Indeterminate,
    TimedOut,
}

impl GuestCommandSettlement {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Refused => "refused",
            Self::Settled => "settled",
            Self::Indeterminate => "indeterminate",
            Self::TimedOut => "timed_out",
        }
    }

    pub fn parse(value: &str) -> Result<Self, GuestCommandSettlementParseError> {
        match value {
            "refused" => Ok(Self::Refused),
            "settled" => Ok(Self::Settled),
            "indeterminate" => Ok(Self::Indeterminate),
            "timed_out" => Ok(Self::TimedOut),
            _ => Err(GuestCommandSettlementParseError),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuestCommandSettlementParseError;

impl std::fmt::Display for GuestCommandSettlementParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(
            "guest command settlement must be one of the closed set \
             [refused, settled, indeterminate, timed_out]",
        )
    }
}

impl std::error::Error for GuestCommandSettlementParseError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuestCommandReport {
    pub schema_version: String,
    pub settlement: GuestCommandSettlement,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    pub vm_entered: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl GuestCommandReport {
    fn new(
        settlement: GuestCommandSettlement,
        exit_code: Option<i32>,
        vm_entered: bool,
        image_sha256: Option<String>,
        reason: Option<String>,
    ) -> Self {
        Self {
            schema_version: GUEST_COMMAND_SCHEMA_VERSION.to_string(),
            settlement,
            exit_code,
            vm_entered,
            image_sha256,
            reason,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct WorkerEvidence {
    schema_version: String,
    enter_attempted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

impl WorkerEvidence {
    fn initial() -> Self {
        Self {
            schema_version: WORKER_SCHEMA_VERSION.to_string(),
            enter_attempted: false,
            image_sha256: None,
            reason: None,
        }
    }
}

/// Fixed guest init. The caller command is a file in the workspace share,
/// not text in this string.
pub fn guest_init_command() -> String {
    format!(
        "mount -t virtiofs {AGENT_RUNTIME_SHARE_TAG} /run/a3s-oci-runtime && \
         cd /run/a3s-oci-runtime && \
         exec /usr/bin/a3s-oci-agent --guest-command ./.a3s-guest-command"
    )
}

pub fn guest_command(
    system_image_manifest: &Path,
    workspace: &Path,
    console: &Path,
    timeout: Duration,
) -> GuestCommandReport {
    if timeout.is_zero() {
        return GuestCommandReport::new(
            GuestCommandSettlement::TimedOut,
            Some(TIMEOUT_EXIT_CODE),
            false,
            None,
            Some("guest command timeout was already expired".to_string()),
        );
    }

    let workspace = match MacosRuntimeShare::open(workspace) {
        Ok(workspace) => workspace,
        Err(error) => {
            return GuestCommandReport::new(
                GuestCommandSettlement::Refused,
                None,
                false,
                None,
                Some(error.to_string()),
            );
        }
    };
    let identity = workspace.identity();
    let workspace_path = workspace.path().to_path_buf();
    let console = match resolve_console(console) {
        Ok(console) => console,
        Err(reason) => {
            return GuestCommandReport::new(
                GuestCommandSettlement::Refused,
                None,
                false,
                None,
                Some(reason),
            );
        }
    };
    if !system_image_manifest.is_file() {
        return GuestCommandReport::new(
            GuestCommandSettlement::Refused,
            None,
            false,
            None,
            Some(format!(
                "system image manifest is missing: {}",
                system_image_manifest.display()
            )),
        );
    }
    let executable = match PinnedCurrentExecutable::current() {
        Ok(executable) => executable,
        Err(reason) => {
            return GuestCommandReport::new(
                GuestCommandSettlement::Refused,
                None,
                false,
                None,
                Some(reason),
            );
        }
    };

    use std::os::unix::process::CommandExt;
    let mut child = match Command::new(executable.command_path())
        .arg(WORKER_COMMAND)
        .arg("--owner-pid")
        .arg(std::process::id().to_string())
        .arg("--system-image-manifest")
        .arg(system_image_manifest)
        .arg("--runtime-share")
        .arg(&workspace_path)
        .arg("--runtime-share-device")
        .arg(identity.0.to_string())
        .arg("--runtime-share-inode")
        .arg(identity.1.to_string())
        .arg("--console")
        .arg(&console)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            return GuestCommandReport::new(
                GuestCommandSettlement::Refused,
                None,
                false,
                None,
                Some(format!("failed to start the macOS guest worker: {error}")),
            );
        }
    };

    let output_reader = child.stdout.take().map(|stdout| {
        thread::spawn(move || read_bounded_worker_output(stdout, MAX_WORKER_OUTPUT_BYTES))
    });
    let worker_exit = match wait_for_worker(&mut child, timeout) {
        Ok(worker_exit) => Some(worker_exit),
        Err(error) => {
            let cleanup_error = terminate_and_wait(&mut child).err();
            return GuestCommandReport::new(
                GuestCommandSettlement::Indeterminate,
                None,
                false,
                None,
                Some(match cleanup_error {
                    Some(cleanup_error) => format!(
                        "failed to wait for the macOS guest worker: {error}; \
                         worker cleanup also failed: {cleanup_error}"
                    ),
                    None => format!("failed to wait for the macOS guest worker: {error}"),
                }),
            );
        }
    };

    let evidence = match collect_worker_evidence(output_reader) {
        Ok(evidence) => Some(evidence),
        Err(reason) => {
            return settle_report(
                true,
                None,
                worker_exit.as_ref().and_then(|exit| {
                    if exit.timed_out {
                        None
                    } else {
                        exit.status.code()
                    }
                }),
                worker_exit.as_ref().is_some_and(|exit| exit.timed_out),
                Some(reason),
            );
        }
    };
    let timed_out = worker_exit.as_ref().is_some_and(|exit| exit.timed_out);
    let exit_code = worker_exit
        .as_ref()
        .filter(|exit| !exit.timed_out)
        .and_then(|exit| exit.status.code());
    settle_report(true, evidence.as_ref(), exit_code, timed_out, None)
}

pub(crate) fn run_worker(
    system_image_manifest: &Path,
    runtime_share: &Path,
    console: &Path,
    runtime_share_identity: Option<(u64, u64)>,
) -> bool {
    let mut evidence = WorkerEvidence::initial();
    let runtime_share = match MacosRuntimeShare::open(runtime_share) {
        Ok(runtime_share) => runtime_share,
        Err(error) => return fail_worker(&mut evidence, error.to_string()),
    };
    let Some(expected) = runtime_share_identity else {
        return fail_worker(
            &mut evidence,
            "macOS HVF runtime-share handoff identity is missing".to_string(),
        );
    };
    if let Err(error) = runtime_share.verify_identity(expected) {
        return fail_worker(&mut evidence, error.to_string());
    }
    let prepared_console = match prepare_console_output(console, None) {
        Ok(console) => console,
        Err(reason) => return fail_worker(&mut evidence, reason),
    };
    let api = match MacosKrunApi::load() {
        Ok(api) => api,
        Err(error) => return fail_worker(&mut evidence, error.to_string()),
    };
    let system_image = match MacosSystemImage::load(system_image_manifest, api.runtime_provenance())
    {
        Ok(system_image) => system_image,
        Err(error) => return fail_worker(&mut evidence, error.to_string()),
    };
    if paths_overlap(&system_image, runtime_share.path()) {
        return fail_worker(
            &mut evidence,
            "immutable system image, manifest, and writable runtime share must be disjoint"
                .to_string(),
        );
    }
    evidence.image_sha256 = Some(system_image.evidence(true).system_image_sha256);
    let config = crate::fallback_config();
    let mut context = match KrunContext::create(api) {
        Ok(context) => context,
        Err(error) => return fail_worker(&mut evidence, error.to_string()),
    };
    if let Err(error) = context.set_vm_config(config) {
        return fail_worker(&mut evidence, error.to_string());
    }
    if let Err(error) = context.disable_implicit_vsock() {
        return fail_worker(&mut evidence, error.to_string());
    }
    if let Err(error) = context.set_read_only_system_image(system_image) {
        return fail_worker(&mut evidence, error.to_string());
    }
    if let Err(error) = context.add_runtime_share(AGENT_RUNTIME_SHARE_TAG, runtime_share) {
        return fail_worker(&mut evidence, error.to_string());
    }
    if let Err(error) = context.set_workdir("/") {
        return fail_worker(&mut evidence, error.to_string());
    }
    let command = guest_init_command();
    let arguments = vec!["-c".to_string(), command];
    let environment = [(
        "PATH".to_string(),
        "/usr/bin:/bin:/usr/sbin:/sbin".to_string(),
    )];
    if let Err(error) = context.set_exec("/bin/sh", &arguments, &environment) {
        return fail_worker(&mut evidence, error.to_string());
    }
    if let Err(error) = context.set_console_output(prepared_console) {
        return fail_worker(&mut evidence, error.to_string());
    }
    if let Err(error) = context.confine() {
        return fail_worker(&mut evidence, error.to_string());
    }
    evidence.enter_attempted = true;
    if let Err(error) = emit_worker_evidence(&evidence) {
        evidence.enter_attempted = false;
        evidence.reason = Some(format!("failed to emit pre-entry worker evidence: {error}"));
        let _ = emit_worker_evidence(&evidence);
        return false;
    }

    match context.start_enter() {
        Ok(status) => fail_worker(
            &mut evidence,
            format!("krun_start_enter unexpectedly returned status {status}"),
        ),
        Err(error) => fail_worker(&mut evidence, error.to_string()),
    }
}

fn paths_overlap(system_image: &MacosSystemImage, runtime_share: &Path) -> bool {
    system_image.image_path().starts_with(runtime_share)
        || runtime_share.starts_with(system_image.image_path())
        || system_image.manifest_path().starts_with(runtime_share)
        || runtime_share.starts_with(system_image.manifest_path())
}

fn settle_report(
    spawned: bool,
    evidence: Option<&WorkerEvidence>,
    exit_code: Option<i32>,
    timed_out: bool,
    read_error: Option<String>,
) -> GuestCommandReport {
    let image_sha256 = evidence.and_then(|evidence| evidence.image_sha256.clone());
    if timed_out {
        return GuestCommandReport::new(
            GuestCommandSettlement::TimedOut,
            Some(TIMEOUT_EXIT_CODE),
            evidence.is_some_and(|evidence| evidence.enter_attempted),
            image_sha256,
            Some("macOS guest worker exceeded the command timeout and was terminated".to_string()),
        );
    }
    let Some(evidence) = evidence else {
        let settlement = if spawned {
            GuestCommandSettlement::Indeterminate
        } else {
            GuestCommandSettlement::Refused
        };
        return GuestCommandReport::new(
            settlement,
            None,
            false,
            None,
            Some(
                read_error.unwrap_or_else(|| "macOS guest worker emitted no setup evidence".into()),
            ),
        );
    };
    if !evidence.enter_attempted {
        return GuestCommandReport::new(
            GuestCommandSettlement::Refused,
            None,
            false,
            image_sha256,
            evidence.reason.clone().or(read_error),
        );
    }
    if evidence.reason.is_some() {
        return GuestCommandReport::new(
            GuestCommandSettlement::Refused,
            None,
            false,
            image_sha256,
            evidence.reason.clone(),
        );
    }
    match exit_code {
        Some(exit_code) => GuestCommandReport::new(
            GuestCommandSettlement::Settled,
            Some(exit_code),
            true,
            image_sha256,
            None,
        ),
        None => GuestCommandReport::new(
            GuestCommandSettlement::Indeterminate,
            None,
            true,
            image_sha256,
            Some(
                read_error
                    .unwrap_or_else(|| "macOS guest worker exited without a guest status".into()),
            ),
        ),
    }
}

fn collect_worker_evidence(
    output_reader: Option<thread::JoinHandle<io::Result<Vec<u8>>>>,
) -> Result<WorkerEvidence, String> {
    let output_reader =
        output_reader.ok_or_else(|| "macOS guest worker stdout was unavailable".to_string())?;
    let output = output_reader
        .join()
        .map_err(|_| "macOS guest worker output reader panicked".to_string())?
        .map_err(|error| format!("failed to read macOS guest worker evidence: {error}"))?;
    parse_worker_evidence(&output)
}

fn parse_worker_evidence(output: &[u8]) -> Result<WorkerEvidence, String> {
    let mut latest = None;
    for line in output
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let evidence: WorkerEvidence = serde_json::from_slice(line)
            .map_err(|error| format!("macOS guest worker emitted invalid evidence: {error}"))?;
        if evidence.schema_version != WORKER_SCHEMA_VERSION {
            return Err(format!(
                "macOS guest worker emitted unsupported schema {}",
                evidence.schema_version
            ));
        }
        latest = Some(evidence);
    }
    latest.ok_or_else(|| "macOS guest worker emitted no setup evidence".to_string())
}

fn emit_worker_evidence(evidence: &WorkerEvidence) -> io::Result<()> {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    serde_json::to_writer(&mut output, evidence).map_err(io::Error::other)?;
    output.write_all(b"\n")?;
    output.flush()
}

fn fail_worker(evidence: &mut WorkerEvidence, reason: String) -> bool {
    evidence.reason = Some(reason);
    if let Err(error) = emit_worker_evidence(evidence) {
        eprintln!("a3s-oci-krun-shim: failed to emit guest worker failure evidence: {error}");
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{
        settle_report, GuestCommandSettlement, WorkerEvidence, GUEST_COMMAND_SCHEMA_VERSION,
        WORKER_SCHEMA_VERSION,
    };

    #[test]
    fn init_command_is_fixed_and_carries_no_caller_text() {
        let command = super::guest_init_command();
        assert!(command.contains("mount -t virtiofs a3s-oci-runtime /run/a3s-oci-runtime"));
        assert!(
            command.contains("exec /usr/bin/a3s-oci-agent --guest-command ./.a3s-guest-command")
        );
        assert!(!command.contains("setsid"));
        assert_eq!(
            GuestCommandSettlement::parse(GuestCommandSettlement::TimedOut.as_str()),
            Ok(GuestCommandSettlement::TimedOut)
        );
        assert!(GuestCommandSettlement::parse("host").is_err());
    }

    #[test]
    fn entered_exit_settles_and_a_pre_entry_failure_refuses() {
        let mut entered = WorkerEvidence::initial();
        entered.enter_attempted = true;
        entered.image_sha256 = Some("abc".into());
        let settled = settle_report(true, Some(&entered), Some(0), false, None);
        assert_eq!(settled.schema_version, GUEST_COMMAND_SCHEMA_VERSION);
        assert_eq!(settled.settlement, GuestCommandSettlement::Settled);
        assert_eq!(settled.exit_code, Some(0));
        assert!(settled.vm_entered);
        assert_eq!(settled.image_sha256.as_deref(), Some("abc"));

        let mut refused = WorkerEvidence::initial();
        refused.reason = Some("hvf unavailable".into());
        refused.schema_version = WORKER_SCHEMA_VERSION.to_string();
        let report = settle_report(true, Some(&refused), Some(1), false, None);
        assert_eq!(report.settlement, GuestCommandSettlement::Refused);
        assert!(!report.vm_entered);
        assert!(report.exit_code.is_none());

        let timed_out = settle_report(true, Some(&entered), None, true, None);
        assert_eq!(timed_out.settlement, GuestCommandSettlement::TimedOut);
        assert_eq!(timed_out.exit_code, Some(124));

        let signaled = settle_report(true, Some(&entered), None, false, None);
        assert_eq!(signaled.settlement, GuestCommandSettlement::Indeterminate);
        assert!(signaled.exit_code.is_none());
    }
}
