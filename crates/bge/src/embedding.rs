//! The local bge-m3 embedding engine.
//!
//! Lazy-loaded from the installed family directory, reloaded when that
//! directory changes, and always executed on the blocking pool behind a
//! mutex — the same contract the PP-OCRv6 provider follows.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use a3s_use_core::{UseError, UseResult};
use ort::session::builder::GraphOptimizationLevel;
use ort::session::run_options::RunOptions;
use ort::session::Session;
use ort::value::TensorRef;
use tokio::sync::{Notify, Semaphore};

use crate::assets::{bge_status, resolve_family_assets, BgeRuntimeStatus, Family};
use crate::math::l2_normalize;
use crate::tokenizer::PairTokenizer;
use crate::wire::{
    EmbeddingItem, EmbeddingRequest, EmbeddingResponse, Usage, MAX_ITEMS, MAX_ITEM_BYTES,
    MAX_MODEL_BYTES,
};

const ENGINE_BATCH_SIZE: usize = 8;
const INPUT_IDS: &str = "input_ids";
const ATTENTION_MASK: &str = "attention_mask";

/// One completed request, bound to the install receipt used by its worker.
#[derive(Debug)]
pub struct EmbeddingOutput {
    pub response: EmbeddingResponse,
    pub revision: String,
}

/// Owns a blocking request until it settles. Dropping it requests termination;
/// the worker retains its engine and run options until native execution exits.
pub struct EmbeddingOperation {
    worker: Option<tokio::task::JoinHandle<UseResult<EmbeddingOutput>>>,
    cancellation: Arc<InferenceCancellation>,
}

#[derive(Default)]
struct InferenceCancellation {
    cancelled: AtomicBool,
    run_options: Mutex<Option<Arc<RunOptions>>>,
    changed: Notify,
}

impl InferenceCancellation {
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        self.changed.notify_waiters();
        if let Ok(options) = self.run_options.lock() {
            if let Some(options) = options.as_ref() {
                // Even if native termination fails, settlement still waits for
                // the worker; cancellation never certifies a detached worker.
                let _ = options.terminate();
            }
        }
    }

    fn check(&self) -> UseResult<()> {
        if self.cancelled.load(Ordering::Acquire) {
            Err(cancelled_error())
        } else {
            Ok(())
        }
    }

    async fn cancelled(&self) {
        loop {
            let changed = self.changed.notified();
            if self.cancelled.load(Ordering::Acquire) {
                return;
            }
            changed.await;
        }
    }

    fn run_options(&self) -> UseResult<Arc<RunOptions>> {
        let options = Arc::new(
            RunOptions::new().map_err(|error| runtime_error("create inference options", error))?,
        );
        let mut active = self.run_options.lock().map_err(|_| {
            engine_error(
                "use.bge.runtime_failed",
                "The inference cancellation lock is poisoned.",
            )
        })?;
        *active = Some(Arc::clone(&options));
        // Cancellation before registration must also terminate the next Run.
        if self.cancelled.load(Ordering::Acquire) {
            let _ = options.terminate();
            return Err(cancelled_error());
        }
        Ok(options)
    }
}

impl EmbeddingOperation {
    fn start(
        admission: Arc<Semaphore>,
        work: impl FnOnce(Arc<InferenceCancellation>) -> UseResult<EmbeddingOutput> + Send + 'static,
    ) -> UseResult<Self> {
        let runtime = tokio::runtime::Handle::try_current().map_err(|error| {
            runtime_error("start an embedding worker outside a Tokio runtime", error)
        })?;
        let cancellation = Arc::new(InferenceCancellation::default());
        let worker_cancellation = Arc::clone(&cancellation);
        let worker = runtime.spawn(async move {
            worker_cancellation.check()?;
            let permit = tokio::select! {
                biased;
                _ = worker_cancellation.cancelled() => return Err(cancelled_error()),
                permit = admission.acquire_owned() => permit.map_err(|_| {
                    engine_error("use.bge.runtime_failed", "The embedding engine admission is closed.")
                })?,
            };
            worker_cancellation.check()?;
            tokio::task::spawn_blocking(move || {
                // Keep admission inside the blocking worker. Dropping the
                // provider future cannot admit another native inference early.
                let _permit = permit;
                worker_cancellation.check()?;
                let result = work(Arc::clone(&worker_cancellation));
                worker_cancellation.check()?;
                result
            })
            .await
            .map_err(|error| runtime_error("settle the local embedding worker", error))?
        });
        Ok(Self {
            worker: Some(worker),
            cancellation,
        })
    }

    pub fn cancel(&self) {
        self.cancellation.cancel();
    }

    /// Wait for native work to exit, including after cancellation. This does
    /// not claim a bounded native shutdown deadline; callers must qualify the
    /// installed backend before advertising that guarantee.
    pub async fn settle(mut self) -> UseResult<EmbeddingOutput> {
        let worker = self.worker.take().ok_or_else(|| {
            engine_error(
                "use.bge.runtime_failed",
                "The embedding worker was already consumed.",
            )
        })?;
        worker
            .await
            .map_err(|error| runtime_error("settle the local embedding worker", error))?
    }

    /// On cancellation, terminate the native Run and drain its worker before
    /// returning. Dropping this future also requests cancellation; admission
    /// remains worker-owned until actual settlement.
    pub async fn settle_until_cancelled(
        self,
        cancellation: impl std::future::Future<Output = ()>,
    ) -> UseResult<EmbeddingOutput> {
        let control = Arc::clone(&self.cancellation);
        let settlement = self.settle();
        tokio::pin!(settlement);
        tokio::pin!(cancellation);
        tokio::select! {
            biased;
            _ = &mut cancellation => {
                control.cancel();
                let _ = settlement.await;
                Err(cancelled_error())
            }
            output = &mut settlement => output,
        }
    }
}

impl Drop for EmbeddingOperation {
    fn drop(&mut self) {
        self.cancel();
    }
}

fn cancelled_error() -> UseError {
    engine_error(
        "use.bge.cancelled",
        "The local embedding request was cancelled.",
    )
}

fn validate_request(request: &EmbeddingRequest) -> UseResult<()> {
    if request.input.is_empty() || request.input.len() > MAX_ITEMS {
        return Err(engine_error(
            "use.bge.request_invalid",
            format!("The embedding input must contain 1-{MAX_ITEMS} strings."),
        ));
    }
    if request
        .input
        .iter()
        .any(|input| input.len() > MAX_ITEM_BYTES)
    {
        return Err(engine_error(
            "use.bge.request_invalid",
            format!("An embedding input entry exceeds the {MAX_ITEM_BYTES}-byte limit."),
        ));
    }
    if request
        .model
        .as_ref()
        .is_some_and(|model| model.len() > MAX_MODEL_BYTES)
    {
        return Err(engine_error(
            "use.bge.request_invalid",
            format!("The model field exceeds the {MAX_MODEL_BYTES}-byte limit."),
        ));
    }
    Ok(())
}

/// Client for the locally installed bge-m3 embedding model.
#[derive(Clone)]
pub struct EmbeddingClient {
    loaded: Arc<Mutex<Option<LoadedEngine>>>,
    admission: Arc<Semaphore>,
}

struct LoadedEngine {
    model_dir: PathBuf,
    install_stamp: String,
    engine: EmbeddingEngine,
}

struct EmbeddingEngine {
    session: Session,
    tokenizer: PairTokenizer,
    /// Some XLM-R exports declare a `token_type_ids` input even though the
    /// architecture ignores it; we feed zeros so such exports still load.
    token_type_input: Option<String>,
}

impl EmbeddingClient {
    pub fn from_env() -> UseResult<Self> {
        Ok(Self {
            loaded: Arc::new(Mutex::new(None)),
            admission: Arc::new(Semaphore::new(1)),
        })
    }

    pub fn status(&self) -> BgeRuntimeStatus {
        bge_status(Family::BgeM3)
    }

    /// Resolve installed assets without loading ONNX or running inference.
    /// The identity is the install receipt digest, never a path or source text.
    pub fn installed_revision(&self) -> UseResult<String> {
        let assets = resolve_family_assets(Family::BgeM3)?;
        if assets.install_stamp.is_empty() {
            return Err(engine_error(
                "use.bge.model_invalid",
                "The model receipt identity is missing.",
            ));
        }
        Ok(assets.install_stamp)
    }

    /// Loads the ONNX session now so the first real request pays no load
    /// latency. Runs one tiny embedding; a missing install is returned, not
    /// panicked on.
    pub async fn warm(&self) -> UseResult<()> {
        let request = EmbeddingRequest {
            model: None,
            input: vec!["warm".to_string()],
        };
        self.embed(request).await.map(|_| ())
    }

    pub async fn embed(&self, request: EmbeddingRequest) -> UseResult<EmbeddingResponse> {
        Ok(self.start_embedding(request)?.settle().await?.response)
    }

    /// Start one cancellable request using the same lazily loaded engine as
    /// `embed`. No installer or remote provider is involved.
    pub fn start_embedding(&self, request: EmbeddingRequest) -> UseResult<EmbeddingOperation> {
        validate_request(&request)?;
        let loaded = Arc::clone(&self.loaded);
        EmbeddingOperation::start(Arc::clone(&self.admission), move |cancellation| {
            let assets = resolve_family_assets(Family::BgeM3)?;
            if assets.install_stamp.is_empty() {
                return Err(engine_error(
                    "use.bge.model_invalid",
                    "The model receipt identity is missing.",
                ));
            }
            cancellation.check()?;
            let mut loaded = loaded.lock().map_err(|_| {
                UseError::new(
                    "use.bge.runtime_failed",
                    "The local bge-m3 engine lock is poisoned.",
                )
            })?;
            cancellation.check()?;
            // A forced reinstall keeps the canonical path but changes the
            // receipt digest — the stamp, not the path, is the reload signal.
            let should_load = loaded
                .as_ref()
                .map(|loaded| {
                    loaded.model_dir != assets.root || loaded.install_stamp != assets.install_stamp
                })
                .unwrap_or(true);
            if should_load {
                *loaded = Some(LoadedEngine {
                    model_dir: assets.root.clone(),
                    install_stamp: assets.install_stamp.clone(),
                    engine: EmbeddingEngine::load(&assets.model_file, &assets.tokenizer_file)?,
                });
            }
            cancellation.check()?;
            let engine = loaded.as_mut().ok_or_else(|| {
                UseError::new(
                    "use.bge.runtime_failed",
                    "The local bge-m3 engine failed to initialize.",
                )
            })?;
            let options = cancellation.run_options()?;
            let (mut embeddings, prompt_tokens) =
                engine
                    .engine
                    .embed_all(&request.input, &cancellation, &options)?;
            embeddings.truncate(request.input.len());
            let mut data = Vec::with_capacity(embeddings.len());
            for (index, embedding) in embeddings.into_iter().enumerate() {
                data.push(EmbeddingItem {
                    object: "embedding",
                    index,
                    embedding,
                });
            }
            let response = EmbeddingResponse {
                object: "list",
                data,
                model: request
                    .model
                    .clone()
                    .unwrap_or_else(|| Family::BgeM3.as_str().to_string()),
                usage: Usage {
                    prompt_tokens,
                    total_tokens: prompt_tokens,
                },
            };
            Ok(EmbeddingOutput {
                response,
                revision: assets.install_stamp,
            })
        })
    }
}

impl EmbeddingEngine {
    fn load(model_file: &Path, tokenizer_file: &Path) -> UseResult<Self> {
        let (session, token_type_input) = load_session(model_file)?;
        let tokenizer = PairTokenizer::load(tokenizer_file)?;
        Ok(Self {
            session,
            tokenizer,
            token_type_input,
        })
    }

    fn embed_all(
        &mut self,
        texts: &[String],
        cancellation: &InferenceCancellation,
        options: &RunOptions,
    ) -> UseResult<(Vec<Vec<f32>>, u64)> {
        let mut embeddings = Vec::with_capacity(texts.len());
        let mut total_tokens = 0_u64;
        for batch in texts.chunks(ENGINE_BATCH_SIZE) {
            cancellation.check()?;
            let (ids, mask, tokens) = self.tokenizer.encode_batch(batch)?;
            cancellation.check()?;
            total_tokens += tokens;
            let rows = ids.len();
            let columns = ids.first().map(Vec::len).unwrap_or(0);
            let flat_ids = ids.into_iter().flatten().collect::<Vec<_>>();
            let flat_mask = mask.into_iter().flatten().collect::<Vec<_>>();
            let output = run_encoder(
                &mut self.session,
                &flat_ids,
                &flat_mask,
                rows,
                columns,
                self.token_type_input.as_deref(),
                options,
            )?;
            let hidden = output.hidden;
            let stride = columns.checked_mul(hidden).ok_or_else(|| {
                engine_error(
                    "use.bge.provider_output_invalid",
                    "The bge-m3 row stride overflowed.",
                )
            })?;
            for row in 0..rows {
                let start = row
                    .checked_mul(stride)
                    .filter(|offset| offset + hidden <= output.data.len())
                    .ok_or_else(|| {
                        engine_error(
                            "use.bge.provider_output_invalid",
                            "The bge-m3 CLS row escapes the output tensor.",
                        )
                    })?;
                let mut vector = output.data[start..start + hidden].to_vec();
                if vector.iter().any(|value| !value.is_finite()) {
                    return Err(engine_error(
                        "use.bge.provider_output_invalid",
                        "The bge-m3 encoder produced a non-finite hidden state.",
                    ));
                }
                l2_normalize(&mut vector);
                embeddings.push(vector);
            }
        }
        Ok((embeddings, total_tokens))
    }
}

fn load_session(path: &Path) -> UseResult<(Session, Option<String>)> {
    let session = Session::builder()
        .map_err(|error| runtime_error("create an ONNX Runtime session", error))?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(|error| runtime_error("configure graph optimization", error))?
        .commit_from_file(path)
        .map_err(|error| runtime_error("load the bge-m3 ONNX model", error))?;
    let names: Vec<&str> = session
        .inputs
        .iter()
        .map(|input| input.name.as_str())
        .collect();
    if !names.contains(&INPUT_IDS) || !names.contains(&ATTENTION_MASK) {
        return Err(engine_error(
            "use.bge.model_invalid",
            format!(
                "The bge-m3 model must expose '{INPUT_IDS}' and '{ATTENTION_MASK}' inputs; found {names:?}."
            ),
        ));
    }
    let token_type_input = names
        .iter()
        .find(|name| **name == "token_type_ids")
        .map(|name| (*name).to_string());
    Ok((session, token_type_input))
}

struct EncoderOutput {
    hidden: usize,
    data: Vec<f32>,
}

fn run_encoder(
    session: &mut Session,
    flat_ids: &[i64],
    flat_mask: &[i64],
    rows: usize,
    columns: usize,
    token_type_input: Option<&str>,
    options: &RunOptions,
) -> UseResult<EncoderOutput> {
    let expected = rows
        .checked_mul(columns)
        .filter(|total| *total == flat_ids.len());
    if expected.is_none() || flat_ids.len() != flat_mask.len() {
        return Err(engine_error(
            "use.bge.provider_input_invalid",
            "The bge-m3 batch tensor length does not match its shape.",
        ));
    }
    let ids = TensorRef::from_array_view(([rows, columns], flat_ids))
        .map_err(|error| runtime_error("create the input_ids tensor", error))?;
    let mask = TensorRef::from_array_view(([rows, columns], flat_mask))
        .map_err(|error| runtime_error("create the attention_mask tensor", error))?;
    let outputs = match token_type_input {
        None => session
            .run_with_options(
                ort::inputs![INPUT_IDS => ids, ATTENTION_MASK => mask],
                options,
            )
            .map_err(|error| runtime_error("run bge-m3 inference", error))?,
        Some(name) => {
            let zeros = vec![0_i64; flat_ids.len()];
            let token_types = TensorRef::from_array_view(([rows, columns], zeros.as_slice()))
                .map_err(|error| runtime_error("create the token_type_ids tensor", error))?;
            session
                .run_with_options(
                    ort::inputs![INPUT_IDS => ids, ATTENTION_MASK => mask, name => token_types],
                    options,
                )
                .map_err(|error| runtime_error("run bge-m3 inference", error))?
        }
    };
    // Prefer the named hidden state; quantized exports sometimes keep extra
    // intermediate outputs.
    let fallback = outputs.values().next();
    let (shape, data): (Vec<i64>, Vec<f32>) = if let Some(output) = outputs.get("last_hidden_state")
    {
        let (shape, data) = output
            .try_extract_tensor::<f32>()
            .map_err(|error| runtime_error("read the bge-m3 output tensor", error))?;
        (shape.to_vec(), data.into())
    } else if let Some(output) = fallback {
        let (shape, data) = output
            .try_extract_tensor::<f32>()
            .map_err(|error| runtime_error("read the bge-m3 output tensor", error))?;
        (shape.to_vec(), data.into())
    } else {
        return Err(engine_error(
            "use.bge.provider_output_invalid",
            "The bge-m3 inference returned no output tensor.",
        ));
    };
    let shape = shape
        .iter()
        .map(|dimension| {
            usize::try_from(*dimension).map_err(|_| {
                engine_error(
                    "use.bge.provider_output_invalid",
                    format!("The bge-m3 output contains an invalid dimension {dimension}."),
                )
            })
        })
        .collect::<UseResult<Vec<_>>>()?;
    if shape.len() != 3 || shape[0] != rows || shape[1] != columns || shape[2] == 0 {
        return Err(engine_error(
            "use.bge.provider_output_invalid",
            format!(
                "The bge-m3 encoder output must be [batch, sequence, hidden]; found {shape:?}."
            ),
        ));
    }
    let dimensions = shape
        .iter()
        .try_fold(1_usize, |total, dimension| total.checked_mul(*dimension))
        .ok_or_else(|| {
            engine_error(
                "use.bge.provider_output_invalid",
                "The bge-m3 encoder output dimensions overflowed.",
            )
        })?;
    if dimensions != data.len() {
        return Err(engine_error(
            "use.bge.provider_output_invalid",
            "The bge-m3 encoder output length does not match its batch shape.",
        ));
    }
    Ok(EncoderOutput {
        hidden: shape[2],
        data: data.to_vec(),
    })
}

fn runtime_error(action: &str, error: impl std::fmt::Display) -> UseError {
    UseError::new(
        "use.bge.runtime_failed",
        format!("Failed to {action}: {error}"),
    )
}

fn engine_error(code: &str, message: impl Into<String>) -> UseError {
    UseError::new(code, message)
}

#[cfg(test)]
#[path = "embedding_tests.rs"]
mod tests;
