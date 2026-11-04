#![cfg(test)]

use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
#[expect(
    dead_code,
    reason = "Shared phases are compiled into qualification binaries exercising different workflows"
)]
pub enum Phase {
    CampaignPrepare,
    ConfigLoad,
    RouteDiscovery,
    CandidateLoad,
    CandidateMemberOpen,
    CandidateMemberDigest,
    CandidateMemberCopy,
    FreezePair,
    BuildIdQuery,
    TesterCopy,
    FileDigest,
    RelayStart,
    IsolatedConfig,
    DriverSpawn,
    CampaignSpawn,
    ChildSpawn,
    Initialize,
    SessionNew,
    SessionLoad,
    Prompt,
    GoalState,
    Permission,
    IncomingRequest,
    ModelRequest,
    ModelHeaders,
    ModelFirstByte,
    ModelStream,
    ModelBufferedResponse,
    ModelUsage,
    Tool,
    ToolReplay,
    ToolMissingStart,
    ToolUnfinished,
    Compaction,
    CompactionReplay,
    CompactionMissingStart,
    CompactionUnfinished,
    Subagent,
    SubagentReplay,
    SubagentMissingStart,
    SubagentUnfinished,
    IdentityRecheck,
    DriverClose,
    ChildStop,
    ChildReap,
    WireClose,
    RelayClose,
    RelayReply,
    ArtifactRead,
    PublicEvidenceWrite,
    FixtureSetup,
    FeedPublish,
    FactRead,
    Oracle,
    ResultWrite,
    TimingOpen,
    TimingClose,
    TimingFlush,
    TuiSuiteCheck,
    TuiEngineRun,
    TuiEngineSpawn,
    TuiEngineWait,
    TuiProxyCleanup,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Subject {
    Request(usize),
    Process(u32),
    Executable(String),
    OpaqueId([u8; 32]),
}

#[derive(Serialize)]
struct Record<'a> {
    schema: &'static str,
    event: &'static str,
    pid: u32,
    sequence: u64,
    span: u64,
    phase: Phase,
    elapsed_us: u64,
    clock_origin_unix_us: u64,
    unix_time_us: u64,
    duration_us: Option<u64>,
    subject: Option<&'a Subject>,
    dropped_records: u64,
}

struct State {
    origin: Instant,
    unix_origin_us: u64,
    sequence: AtomicU64,
    writer: NonBlocking,
    path: PathBuf,
}

fn active() -> &'static Mutex<Option<Arc<State>>> {
    static ACTIVE: OnceLock<Mutex<Option<Arc<State>>>> = OnceLock::new();
    ACTIVE.get_or_init(|| Mutex::new(None))
}

fn micros(value: std::time::Duration) -> u64 {
    u64::try_from(value.as_micros()).unwrap_or(u64::MAX)
}

impl State {
    fn open(directory: &Path) -> std::io::Result<(Arc<Self>, WorkerGuard)> {
        let origin = Instant::now();
        let unix_origin_us = micros(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(std::io::Error::other)?,
        );
        let path = directory.join("phase-timing.jsonl");
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        let (writer, worker) = tracing_appender::non_blocking::NonBlockingBuilder::default()
            .buffered_lines_limit(16_384)
            .finish(file);
        let state = Arc::new(Self {
            origin,
            unix_origin_us,
            sequence: AtomicU64::new(0),
            writer,
            path,
        });
        state.write(
            "open",
            0,
            Phase::TimingOpen,
            Some(micros(origin.elapsed())),
            None,
        );
        Ok((state, worker))
    }

    fn write(
        &self,
        event: &'static str,
        span: u64,
        phase: Phase,
        duration_us: Option<u64>,
        subject: Option<&Subject>,
    ) -> u64 {
        let (sequence, bytes) = self.encode(event, span, phase, duration_us, subject);
        if let Ok(bytes) = bytes {
            let _ = self.writer.clone().write_all(&bytes);
        }
        sequence
    }

    fn encode(
        &self,
        event: &'static str,
        span: u64,
        phase: Phase,
        duration_us: Option<u64>,
        subject: Option<&Subject>,
    ) -> (u64, serde_json::Result<Vec<u8>>) {
        let sequence = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        let elapsed_us = micros(self.origin.elapsed());
        let record = Record {
            schema: "aster.test.phase-timing.v1",
            event,
            pid: std::process::id(),
            sequence,
            span: if span == 0 { sequence } else { span },
            phase,
            elapsed_us,
            clock_origin_unix_us: self.unix_origin_us,
            unix_time_us: self.unix_origin_us.saturating_add(elapsed_us),
            duration_us,
            subject,
            dropped_records: u64::try_from(self.writer.error_counter().dropped_lines())
                .unwrap_or(u64::MAX),
        };
        // This sink accepts only typed timing metadata, never task or transport payloads.
        let bytes = serde_json::to_vec(&record).map(|mut bytes| {
            bytes.push(b'\n');
            bytes
        });
        (sequence, bytes)
    }
}

pub struct Guard {
    state: Arc<State>,
    worker: Option<WorkerGuard>,
}

impl std::fmt::Debug for Guard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TimingGuard")
            .field("records", &self.state.sequence.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

impl Guard {
    pub fn open(directory: &Path) -> std::io::Result<Self> {
        let (state, worker) = State::open(directory)?;
        // shortcut: Qualification runs one exact campaign per process; use explicit contexts if it starts batching campaigns.
        let mut installed = active()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if installed.is_some() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "a timing campaign is already active",
            ));
        }
        *installed = Some(state.clone());
        Ok(Self {
            state,
            worker: Some(worker),
        })
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        let started = Instant::now();
        let span = self.state.write("begin", 0, Phase::TimingFlush, None, None);
        self.state.write("close", 0, Phase::TimingClose, None, None);
        active()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        // The existing appender drains its writer on shutdown, including assertion unwinding.
        self.worker.take();
        let (_, bytes) = self.state.encode(
            "end",
            span,
            Phase::TimingFlush,
            Some(micros(started.elapsed())),
            None,
        );
        if let Ok(bytes) = bytes {
            // The queue is drained; append its measured shutdown interval through the same schema.
            let _ = std::fs::OpenOptions::new()
                .append(true)
                .open(&self.state.path)
                .and_then(|mut file| file.write_all(&bytes));
        }
    }
}

pub struct Span {
    state: Option<Arc<State>>,
    started: Instant,
    id: u64,
    phase: Phase,
    subject: Option<Subject>,
}

impl std::fmt::Debug for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TimingSpan")
            .field("id", &self.id)
            .field("phase", &self.phase)
            .finish_non_exhaustive()
    }
}

pub fn span(phase: Phase) -> Span {
    span_for(phase, None)
}

pub fn span_for(phase: Phase, subject: Option<Subject>) -> Span {
    let state = active()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    Span::new(state, phase, subject)
}

impl Span {
    fn new(state: Option<Arc<State>>, phase: Phase, subject: Option<Subject>) -> Self {
        let started = Instant::now();
        let id = state.as_ref().map_or(0, |state| {
            state.write("begin", 0, phase, None, subject.as_ref())
        });
        Self {
            state,
            started,
            id,
            phase,
            subject,
        }
    }
}

pub fn point(phase: Phase, subject: Option<Subject>) {
    let state = active()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    if let Some(state) = state {
        state.write("point", 0, phase, None, subject.as_ref());
    }
}

pub fn opaque_id(value: &str) -> Subject {
    use sha2::Digest;
    Subject::OpaqueId(sha2::Sha256::digest(value.as_bytes()).into())
}

impl Drop for Span {
    fn drop(&mut self) {
        if let Some(state) = &self.state {
            state.write(
                "end",
                self.id,
                self.phase,
                Some(micros(self.started.elapsed())),
                self.subject.as_ref(),
            );
        }
    }
}

#[path = "timing_tests.rs"]
mod tests;
