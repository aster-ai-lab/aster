//! Complete native replicas with fixed device/cache ownership and bounded work.
//! No HTTP reference, CPU inference, cross-request KV reuse or automatic retry.

use crate::{
    execution::{XqDeviceAdmission, XqGeneratedTokens, XqNativePipeline, XqPipelinePlan},
    parallel::{XqDeviceMap, XqReplicaWidth},
    weights::{
        engram::{VerifiedEngramTables, XqEngramTableSource},
        xq::storage::XqWeights,
    },
};
use aster_power::{
    error::{PowerError, Result},
    inference::{DevicePreference, InferenceLimits, RuntimeDevice},
};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc, Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tokio::sync::mpsc as output;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XqReplicaLayout {
    TwoByFour,
    FourByTwo,
}
impl XqReplicaLayout {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "2x4" => Ok(Self::TwoByFour),
            "4x2" => Ok(Self::FourByTwo),
            _ => Err(PowerError::Config(
                "native replica layout requires 2x4 or 4x2".into(),
            )),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TwoByFour => "2x4",
            Self::FourByTwo => "4x2",
        }
    }
    pub fn width(self) -> XqReplicaWidth {
        match self {
            Self::TwoByFour => XqReplicaWidth::Four,
            Self::FourByTwo => XqReplicaWidth::Two,
        }
    }
    pub fn mappings(self) -> Vec<XqDeviceMap> {
        let width = self.width().device_count();
        (0..8 / width)
            .map(|group| {
                XqDeviceMap::new(self.width(), (group * width..(group + 1) * width).collect())
                    .expect("closed replica layouts partition eight ordinals")
            })
            .collect()
    }
}

pub enum NativeGenerationEvent {
    Token(u32),
    Finished(XqGeneratedTokens),
}

/// Dropping the actual response cancels its queued or active GPU generation.
pub struct NativeGeneration {
    pub replica: usize,
    receiver: output::Receiver<Result<NativeGenerationEvent>>,
    cancellation: CancellationToken,
    timed_out: Arc<AtomicBool>,
    timer: tokio::task::JoinHandle<()>,
    terminal: bool,
}
impl NativeGeneration {
    pub async fn next(&mut self) -> Option<Result<NativeGenerationEvent>> {
        if self.terminal {
            return None;
        }
        match self.receiver.recv().await {
            Some(Ok(NativeGenerationEvent::Finished(value))) => {
                self.terminal = true;
                self.timer.abort();
                Some(Ok(NativeGenerationEvent::Finished(value)))
            }
            Some(Ok(token)) => Some(Ok(token)),
            other => {
                self.terminal = true;
                self.timer.abort();
                let error = if self.timed_out.load(Ordering::Acquire) {
                    PowerError::InferenceDeadlineExceeded
                } else {
                    match other {
                        Some(Err(error)) => error,
                        _ => PowerError::InferenceFailed(
                            "native worker closed without a terminal receipt".into(),
                        ),
                    }
                };
                Some(Err(error))
            }
        }
    }
}
impl Drop for NativeGeneration {
    fn drop(&mut self) {
        self.cancellation.cancel();
        self.timer.abort();
    }
}

struct Pending(Arc<AtomicUsize>);
impl Drop for Pending {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}
struct Work {
    prompt: Vec<u32>,
    maximum: usize,
    cancellation: CancellationToken,
    deadline: Instant,
    output: output::Sender<Result<NativeGenerationEvent>>,
    _pending: Pending,
}
struct Worker {
    sender: mpsc::SyncSender<Work>,
    pending: Arc<AtomicUsize>,
    healthy: Arc<AtomicBool>,
}
struct Health(Arc<AtomicBool>);
impl Drop for Health {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

pub struct XqReplicaPool {
    workers: Mutex<Vec<Worker>>,
    threads: Mutex<Vec<JoinHandle<()>>>,
    cancellation: CancellationToken,
    admissions: Vec<Vec<XqDeviceAdmission>>,
    context: usize,
    maximum_new: usize,
    capacity_per_replica: usize,
    cursor: AtomicUsize,
}
impl XqReplicaPool {
    /// The host verifies each complete shared file once before calling this.
    /// Every worker owns a separate complete pipeline and request-state owner.
    /// Product values supplied here must come from the host's validated ACL.
    pub fn load(
        weights: Arc<XqWeights>,
        tables: Arc<VerifiedEngramTables>,
        layout: XqReplicaLayout,
        plan: XqPipelinePlan,
        limits: InferenceLimits,
        capacity_per_replica: usize,
        load_budget: Duration,
    ) -> Result<Self> {
        plan.validate(&limits)?;
        if plan.placement.replica_width() != layout.width()
            || !(1..=4).contains(&capacity_per_replica)
            || load_budget < Duration::from_secs(1)
            || load_budget > Duration::from_secs(3600)
        {
            return Err(PowerError::Config("replica pool requires a matching layout, capacity 1..4 and load budget 1..3600 seconds".into()));
        }
        let cancellation = CancellationToken::new();
        let mut pool = Self {
            workers: Mutex::new(Vec::new()),
            threads: Mutex::new(Vec::new()),
            cancellation: cancellation.clone(),
            admissions: Vec::new(),
            context: plan.context_tokens,
            maximum_new: limits.max_generated_tokens,
            capacity_per_replica,
            cursor: AtomicUsize::new(0),
        };
        let deadline = Instant::now() + load_budget;
        let mut readiness = Vec::new();
        for mapping in layout.mappings() {
            let (sender, receiver) = mpsc::sync_channel(capacity_per_replica);
            let (ready, wait) = mpsc::sync_channel(1);
            let pending = Arc::new(AtomicUsize::new(0));
            let healthy = Arc::new(AtomicBool::new(false));
            let worker_health = healthy.clone();
            let weights = weights.clone();
            let tables = tables.clone();
            let plan = plan.clone();
            let limits = limits.clone();
            let cancel = cancellation.clone();
            let handle = thread::Builder::new()
                .name(format!("native-xq-{:?}", mapping.ordinals()))
                .spawn(move || {
                    let _health = Health(worker_health.clone());
                    let loaded = (|| {
                        let runtimes = mapping
                            .ordinals()
                            .iter()
                            .map(|&ordinal| {
                                RuntimeDevice::resolve(DevicePreference::Cuda { ordinal })
                            })
                            .collect::<Result<Vec<_>>>()?;
                        XqNativePipeline::load_with_device_map(
                            &weights,
                            XqEngramTableSource::VerifiedFiles(&tables),
                            &runtimes,
                            &mapping,
                            plan,
                            limits,
                            &cancel,
                        )
                    })();
                    let pipeline = match loaded {
                        Ok(value) => value,
                        Err(error) => {
                            let _ = ready.send(Err(error));
                            return;
                        }
                    };
                    worker_health.store(true, Ordering::Release);
                    if ready
                        .send(Ok(pipeline.device_admissions().to_vec()))
                        .is_err()
                    {
                        return;
                    }
                    run_worker(&pipeline, receiver, &cancel);
                })?;
            pool.workers.get_mut().map_err(lock_error)?.push(Worker {
                sender,
                pending,
                healthy,
            });
            pool.threads.get_mut().map_err(lock_error)?.push(handle);
            readiness.push(wait);
        }
        for wait in readiness {
            let result = wait.recv_timeout(deadline.saturating_duration_since(Instant::now()));
            match result {
                Ok(Ok(admissions)) => pool.admissions.push(admissions),
                Ok(Err(error)) => {
                    pool.cancellation.cancel();
                    return Err(error);
                }
                Err(_) => {
                    pool.cancellation.cancel();
                    return Err(PowerError::InferenceDeadlineExceeded);
                }
            }
        }
        Ok(pool)
    }
    pub fn admissions(&self) -> &[Vec<XqDeviceAdmission>] {
        &self.admissions
    }
    pub fn context_tokens(&self) -> usize {
        self.context
    }
    pub fn max_generated_tokens(&self) -> usize {
        self.maximum_new
    }
    pub fn is_ready(&self) -> bool {
        !self.cancellation.is_cancelled()
            && self.workers.lock().is_ok_and(|workers| {
                workers
                    .iter()
                    .all(|worker| worker.healthy.load(Ordering::Acquire))
            })
    }
    pub async fn generate(
        &self,
        prompt: Vec<u32>,
        maximum: usize,
        budget: Duration,
    ) -> Result<NativeGeneration> {
        if prompt.is_empty()
            || maximum == 0
            || maximum > self.maximum_new
            || prompt
                .len()
                .checked_add(maximum)
                .is_none_or(|end| end > self.context)
            || prompt
                .iter()
                .any(|&id| id == 129264 || id as usize >= crate::tokenizer::RAW_VOCABULARY)
            || budget.is_zero()
            || budget > Duration::from_secs(300)
        {
            return Err(PowerError::InvalidRequest("native request exceeds its text, context, generation or 1..300 second deadline bounds".into()));
        }
        if self.cancellation.is_cancelled() {
            return Err(PowerError::BackendNotAvailable(
                "native replica pool is draining".into(),
            ));
        }
        let workers = self.workers.lock().map_err(lock_error)?;
        let start = self.cursor.fetch_add(1, Ordering::Relaxed) % workers.len();
        let selected = (0..workers.len())
            .map(|offset| (start + offset) % workers.len())
            .filter(|&index| workers[index].healthy.load(Ordering::Acquire))
            .min_by_key(|&index| workers[index].pending.load(Ordering::Acquire))
            .filter(|&index| {
                workers[index].pending.load(Ordering::Acquire) < self.capacity_per_replica
            })
            .ok_or(PowerError::InferenceQueueFull {
                maximum: workers.len() * self.capacity_per_replica,
            })?;
        let worker = &workers[selected];
        worker.pending.fetch_add(1, Ordering::AcqRel);
        let cancellation = self.cancellation.child_token();
        let deadline = Instant::now() + budget;
        let (output, receiver) = output::channel(16);
        let work = Work {
            prompt,
            maximum,
            cancellation: cancellation.clone(),
            deadline,
            output,
            _pending: Pending(worker.pending.clone()),
        };
        worker.sender.try_send(work).map_err(|_| {
            PowerError::BackendNotAvailable("selected native replica stopped accepting work".into())
        })?;
        let timed_out = Arc::new(AtomicBool::new(false));
        let timeout_flag = timed_out.clone();
        let timeout_cancel = cancellation.clone();
        let timer = tokio::spawn(async move {
            tokio::time::sleep_until(deadline.into()).await;
            timeout_flag.store(true, Ordering::Release);
            timeout_cancel.cancel();
        });
        Ok(NativeGeneration {
            replica: selected,
            receiver,
            cancellation,
            timed_out,
            timer,
            terminal: false,
        })
    }
    pub fn shutdown(&self, budget: Duration) -> Result<()> {
        if budget.is_zero() || budget > Duration::from_secs(30) {
            return Err(PowerError::Config(
                "native drain requires 1..30 seconds".into(),
            ));
        }
        self.cancellation.cancel();
        let deadline = Instant::now() + budget;
        let mut threads = self.threads.lock().map_err(lock_error)?;
        while threads.iter().any(|thread| !thread.is_finished()) {
            if Instant::now() >= deadline {
                return Err(PowerError::InferenceDeadlineExceeded);
            }
            thread::sleep(Duration::from_millis(5));
        }
        for thread in threads.drain(..) {
            thread.join().map_err(|_| {
                PowerError::InferenceFailed("native worker panicked during drain".into())
            })?;
        }
        Ok(())
    }
}
impl Drop for XqReplicaPool {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

fn run_worker(
    pipeline: &XqNativePipeline<'_>,
    receiver: mpsc::Receiver<Work>,
    shutdown: &CancellationToken,
) {
    while !shutdown.is_cancelled() {
        let work = match receiver.recv_timeout(Duration::from_millis(25)) {
            Ok(work) => work,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => break,
        };
        let result = if work.cancellation.is_cancelled() {
            Err(PowerError::InferenceCancelled)
        } else if Instant::now() >= work.deadline {
            Err(PowerError::InferenceDeadlineExceeded)
        } else {
            pipeline
                .generate_greedy_streaming(
                    &work.prompt,
                    work.maximum,
                    &work.cancellation,
                    |token| send(&work, NativeGenerationEvent::Token(token)),
                )
                .and_then(|generated| send(&work, NativeGenerationEvent::Finished(generated)))
        };
        if let Err(error) = result {
            let fatal = !matches!(
                &error,
                PowerError::InferenceCancelled | PowerError::InferenceDeadlineExceeded
            );
            let _ = work.output.try_send(Err(error));
            if fatal {
                break;
            }
        }
    }
}
fn send(work: &Work, event: NativeGenerationEvent) -> Result<()> {
    let mut value = Ok(event);
    loop {
        if work.cancellation.is_cancelled() {
            return Err(PowerError::InferenceCancelled);
        }
        if Instant::now() >= work.deadline {
            return Err(PowerError::InferenceDeadlineExceeded);
        }
        match work.output.try_send(value) {
            Ok(()) => return Ok(()),
            Err(output::error::TrySendError::Closed(_)) => {
                work.cancellation.cancel();
                return Err(PowerError::InferenceCancelled);
            }
            Err(output::error::TrySendError::Full(event)) => {
                value = event;
                thread::sleep(Duration::from_millis(1));
            }
        }
    }
}
fn lock_error<T>(_: std::sync::PoisonError<T>) -> PowerError {
    PowerError::InferenceFailed("native replica ownership lock poisoned".into())
}

#[cfg(test)]
#[path = "replicas/tests.rs"]
mod tests;
