//! The local bge-reranker-v2-m3 cross-encoder.
//!
//! One query paired with every document, scored by a single-logit
//! session, mapped through a sigmoid to a (0, 1) relevance score.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use aster_use_core::{UseError, UseResult};
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::value::TensorRef;

use crate::assets::{bge_status, resolve_family_assets, BgeRuntimeStatus, Family};
use crate::math::sigmoid;
use crate::tokenizer::PairTokenizer;
use crate::wire::{RerankRequest, RerankResponse, RerankResult, Usage};

const ENGINE_BATCH_SIZE: usize = 8;
const INPUT_IDS: &str = "input_ids";
const ATTENTION_MASK: &str = "attention_mask";

/// Client for the locally installed bge-reranker-v2-m3 model.
#[derive(Clone)]
pub struct RerankClient {
    loaded: Arc<Mutex<Option<LoadedEngine>>>,
}

struct LoadedEngine {
    model_dir: PathBuf,
    install_stamp: String,
    engine: RerankEngine,
}

struct RerankEngine {
    session: Session,
    tokenizer: PairTokenizer,
    /// Some XLM-R exports declare a `token_type_ids` input even though the
    /// architecture ignores it; we feed zeros so such exports still load.
    token_type_input: Option<String>,
}

impl RerankClient {
    pub fn from_env() -> UseResult<Self> {
        Ok(Self {
            loaded: Arc::new(Mutex::new(None)),
        })
    }

    pub fn status(&self) -> BgeRuntimeStatus {
        bge_status(Family::BgeRerankerV2M3)
    }

    /// Loads the ONNX session now so the first real request pays no load
    /// latency. Runs one tiny rerank; a missing install is returned, not
    /// panicked on.
    pub async fn warm(&self) -> UseResult<()> {
        let request = RerankRequest {
            model: None,
            query: "warm".to_string(),
            documents: vec!["warm".to_string()],
            top_n: Some(1),
        };
        self.rerank(request).await.map(|_| ())
    }

    pub async fn rerank(&self, request: RerankRequest) -> UseResult<RerankResponse> {
        let loaded = Arc::clone(&self.loaded);
        tokio::task::spawn_blocking(move || {
            let assets = resolve_family_assets(Family::BgeRerankerV2M3)?;
            let mut loaded = loaded.lock().map_err(|_| {
                UseError::new(
                    "use.bge.runtime_failed",
                    "The local bge-reranker engine lock is poisoned.",
                )
            })?;
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
                    engine: RerankEngine::load(&assets.model_file, &assets.tokenizer_file)?,
                });
            }
            let engine = loaded.as_mut().ok_or_else(|| {
                UseError::new(
                    "use.bge.runtime_failed",
                    "The local bge-reranker engine failed to initialize.",
                )
            })?;
            let (scores, prompt_tokens) = engine
                .engine
                .score_all(&request.query, &request.documents)?;
            let mut results: Vec<RerankResult> = scores
                .into_iter()
                .enumerate()
                .map(|(index, relevance_score)| RerankResult {
                    index,
                    relevance_score,
                })
                .collect();
            // Highest score first; the original document order breaks ties.
            results.sort_by(|left, right| {
                right
                    .relevance_score
                    .partial_cmp(&left.relevance_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(left.index.cmp(&right.index))
            });
            results.truncate(request.effective_top_n());
            Ok(RerankResponse {
                model: request
                    .model
                    .clone()
                    .unwrap_or_else(|| Family::BgeRerankerV2M3.as_str().to_string()),
                results,
                usage: Usage {
                    prompt_tokens,
                    total_tokens: prompt_tokens,
                },
            })
        })
        .await
        .map_err(|error| {
            UseError::new(
                "use.bge.runtime_failed",
                format!("The local bge-reranker inference task failed: {error}"),
            )
        })?
    }
}

impl RerankEngine {
    fn load(model_file: &Path, tokenizer_file: &Path) -> UseResult<Self> {
        let session = Session::builder()
            .map_err(|error| runtime_error("create an ONNX Runtime session", error))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|error| runtime_error("configure graph optimization", error))?
            .commit_from_file(model_file)
            .map_err(|error| runtime_error("load the bge-reranker ONNX model", error))?;
        let names: Vec<&str> = session
            .inputs
            .iter()
            .map(|input| input.name.as_str())
            .collect();
        if !names.contains(&INPUT_IDS) || !names.contains(&ATTENTION_MASK) {
            return Err(engine_error(
                "use.bge.model_invalid",
                format!(
                    "The bge-reranker model must expose '{INPUT_IDS}' and '{ATTENTION_MASK}' inputs; found {names:?}."
                ),
            ));
        }
        let token_type_input = names
            .iter()
            .find(|name| **name == "token_type_ids")
            .map(|name| (*name).to_string());
        let tokenizer = PairTokenizer::load(tokenizer_file)?;
        Ok(Self {
            session,
            tokenizer,
            token_type_input,
        })
    }

    fn score_all(&mut self, query: &str, documents: &[String]) -> UseResult<(Vec<f32>, u64)> {
        let mut scores = Vec::with_capacity(documents.len());
        let mut total_tokens = 0_u64;
        for batch in documents.chunks(ENGINE_BATCH_SIZE) {
            let (ids, mask, tokens) = self.tokenizer.encode_pairs(query, batch)?;
            total_tokens += tokens;
            let rows = ids.len();
            let columns = ids.first().map(Vec::len).unwrap_or(0);
            let flat_ids = ids.into_iter().flatten().collect::<Vec<_>>();
            let flat_mask = mask.into_iter().flatten().collect::<Vec<_>>();
            let logits = run_cross_encoder(
                &mut self.session,
                &flat_ids,
                &flat_mask,
                rows,
                columns,
                self.token_type_input.as_deref(),
            )?;
            for logit in logits {
                if !logit.is_finite() {
                    return Err(engine_error(
                        "use.bge.provider_output_invalid",
                        "The bge-reranker produced a non-finite logit.",
                    ));
                }
                scores.push(sigmoid(logit));
            }
        }
        Ok((scores, total_tokens))
    }
}

fn run_cross_encoder(
    session: &mut Session,
    flat_ids: &[i64],
    flat_mask: &[i64],
    rows: usize,
    columns: usize,
    token_type_input: Option<&str>,
) -> UseResult<Vec<f32>> {
    if rows
        .checked_mul(columns)
        .map(|total| total == flat_ids.len())
        != Some(true)
        || flat_ids.len() != flat_mask.len()
    {
        return Err(engine_error(
            "use.bge.provider_input_invalid",
            "The bge-reranker batch tensor length does not match its shape.",
        ));
    }
    let ids = TensorRef::from_array_view(([rows, columns], flat_ids))
        .map_err(|error| runtime_error("create the input_ids tensor", error))?;
    let mask = TensorRef::from_array_view(([rows, columns], flat_mask))
        .map_err(|error| runtime_error("create the attention_mask tensor", error))?;
    let outputs = match token_type_input {
        None => session
            .run(ort::inputs![INPUT_IDS => ids, ATTENTION_MASK => mask])
            .map_err(|error| runtime_error("run bge-reranker inference", error))?,
        Some(name) => {
            let zeros = vec![0_i64; flat_ids.len()];
            let token_types = TensorRef::from_array_view(([rows, columns], zeros.as_slice()))
                .map_err(|error| runtime_error("create the token_type_ids tensor", error))?;
            session
                .run(ort::inputs![INPUT_IDS => ids, ATTENTION_MASK => mask, name => token_types])
                .map_err(|error| runtime_error("run bge-reranker inference", error))?
        }
    };
    // Dynamic-quantized exports may keep an intermediate node as an extra
    // output; the score lives in `logits`, so select by name.
    let fallback = outputs.values().next();
    let (shape, data): (Vec<i64>, Vec<f32>) = if let Some(output) = outputs.get("logits") {
        let (shape, data) = output
            .try_extract_tensor::<f32>()
            .map_err(|error| runtime_error("read the bge-reranker output tensor", error))?;
        (shape.to_vec(), data.into())
    } else if let Some(output) = fallback {
        let (shape, data) = output
            .try_extract_tensor::<f32>()
            .map_err(|error| runtime_error("read the bge-reranker output tensor", error))?;
        (shape.to_vec(), data.into())
    } else {
        return Err(engine_error(
            "use.bge.provider_output_invalid",
            "The bge-reranker inference returned no output tensor.",
        ));
    };
    let shape = shape
        .iter()
        .map(|dimension| {
            usize::try_from(*dimension).map_err(|_| {
                engine_error(
                    "use.bge.provider_output_invalid",
                    format!("The bge-reranker output contains an invalid dimension {dimension}."),
                )
            })
        })
        .collect::<UseResult<Vec<_>>>()?;
    let elements = shape
        .iter()
        .try_fold(1_usize, |total, dimension| total.checked_mul(*dimension))
        .ok_or_else(|| {
            engine_error(
                "use.bge.provider_output_invalid",
                "The bge-reranker output dimensions overflowed.",
            )
        })?;
    if elements != data.len() {
        return Err(engine_error(
            "use.bge.provider_output_invalid",
            "The bge-reranker output length does not match its shape.",
        ));
    }
    match shape.as_slice() {
        [count] if *count == rows => Ok(data.to_vec()),
        [count, one] if *count == rows && *one == 1 => Ok(data.to_vec()),
        other => Err(engine_error(
            "use.bge.provider_output_invalid",
            format!(
                "The bge-reranker output must be one logit per pair ([{rows}] or [{rows}, 1]); found {other:?}."
            ),
        )),
    }
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
