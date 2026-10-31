//! Power composition for genuine loaded native XQ replicas. No remote engine.
use crate::{
    execution::XqStopReason,
    model::DeepSeekModel,
    prompt::{render_text, TextMessage, ThinkingMode, XqTextCodec},
    replicas::{NativeGeneration, NativeGenerationEvent, XqReplicaPool},
    weights::xq::{gguf::XQ_GGUF_BYTES, XQ_GGUF_SHA256},
};
use aster_power::{
    backend::{
        think_parser::ThinkBlockParser,
        types::{
            ChatRequest, ChatResponseChunk, CompletionRequest, CompletionResponseChunk,
            EffectivePromptDigest, EmbeddingRequest, EmbeddingResponse, MessageContent,
        },
        Backend,
    },
    error::{PowerError, Result},
    model::manifest::{ModelFormat, ModelManifest},
};
use async_trait::async_trait;
use futures::{stream, Stream};
use std::{
    path::PathBuf,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

pub struct NativeXqBackend {
    pool: Arc<XqReplicaPool>,
    codec: Arc<XqTextCodec>,
    path: PathBuf,
    mode: ThinkingMode,
    effort: u8,
    loaded: AtomicBool,
}
impl NativeXqBackend {
    pub fn new(
        pool: Arc<XqReplicaPool>,
        codec: Arc<XqTextCodec>,
        verified_model_path: PathBuf,
        mode: ThinkingMode,
        effort: u8,
    ) -> Result<Self> {
        if !verified_model_path.is_absolute()
            || !(1..=100).contains(&effort)
            || pool.admissions().len() < 2
            || pool.max_generated_tokens() > 32
            || !pool.is_ready()
        {
            return Err(invalid("native backend requires an absolute verified model path, multiple loaded replicas and effort 1..100"));
        }
        Ok(Self {
            pool,
            codec,
            path: verified_model_path,
            mode,
            effort,
            loaded: AtomicBool::new(false),
        })
    }
    fn check_name(&self, name: &str) -> Result<()> {
        if name != DeepSeekModel::V41Flash1017Xq.served_alias()
            || !self.loaded.load(Ordering::Acquire)
        {
            return Err(PowerError::ModelNotFound(name.into()));
        }
        Ok(())
    }
    fn tokens(&self, request: &ChatRequest) -> Result<Vec<u32>> {
        validate_chat_options(request, self.pool.context_tokens())?;
        let messages = request
            .messages
            .iter()
            .map(|message| {
                if message.name.is_some()
                    || message.tool_calls.is_some()
                    || message.tool_call_id.is_some()
                    || message.images.is_some()
                {
                    return Err(invalid(
                        "native text profile refuses named, tool or image messages",
                    ));
                }
                let MessageContent::Text(content) = &message.content else {
                    return Err(invalid(
                        "native text profile requires plain text message content",
                    ));
                };
                Ok(TextMessage {
                    role: message.role.clone(),
                    content: content.clone(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        if !messages.last().is_some_and(|message| {
            message.role == "user" || (messages.len() > 1 && message.role == "system")
        }) {
            return Err(invalid(
                "native chat requires a final user or mid-conversation system message",
            ));
        }
        let prompt = render_text(&messages, self.mode, self.effort)?;
        let maximum = request.max_tokens.unwrap_or(32) as usize;
        if maximum == 0
            || maximum > self.pool.max_generated_tokens()
            || maximum >= self.pool.context_tokens()
        {
            return Err(invalid(
                "native max_tokens exceeds the loaded generation/context profile",
            ));
        }
        self.codec
            .encode(&prompt, self.pool.context_tokens() - maximum)
    }
}

#[async_trait]
impl Backend for NativeXqBackend {
    fn name(&self) -> &str {
        "deepseek-native-xq"
    }
    fn supports(&self, _: &ModelFormat) -> bool {
        false
    }
    fn supports_manifest(&self, manifest: &ModelManifest) -> bool {
        exact_manifest(manifest, &self.path)
    }
    fn reports_exact_chat_usage(&self) -> bool {
        true
    }
    async fn load(&self, manifest: &ModelManifest) -> Result<()> {
        if !self.supports_manifest(manifest) || !self.pool.is_ready() {
            return Err(invalid("native backend refuses an unbound manifest, unavailable pool or unsupported overrides"));
        }
        self.loaded.store(true, Ordering::Release);
        Ok(())
    }
    async fn unload(&self, name: &str) -> Result<()> {
        self.check_name(name)?;
        self.loaded.store(false, Ordering::Release);
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || pool.shutdown(Duration::from_secs(30)))
            .await
            .map_err(|_| PowerError::InferenceFailed("native drain task failed".into()))?
    }
    async fn chat(
        &self,
        name: &str,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatResponseChunk>> + Send>>> {
        self.check_name(name)?;
        let ids = self.tokens(&request)?;
        let count = ids.len();
        let generation = self
            .pool
            .generate(
                ids,
                request.max_tokens.unwrap_or(32) as usize,
                Duration::from_secs(300),
            )
            .await?;
        let mut parser = ThinkBlockParser::new();
        if self.mode == ThinkingMode::Thinking {
            parser.feed("<think>");
        }
        let state = ChatState {
            generation,
            codec: self.codec.clone(),
            decoder: Decoder::default(),
            parser,
            prompt_tokens: count,
            ended: false,
        };
        Ok(Box::pin(stream::unfold(state, |mut state| async move {
            if state.ended {
                return None;
            }
            let result = state.next().await;
            if result.is_err() {
                state.ended = true;
            }
            Some((result, state))
        })))
    }
    async fn effective_chat_prompt_digest(
        &self,
        name: &str,
        request: &ChatRequest,
    ) -> Result<Option<EffectivePromptDigest>> {
        self.check_name(name)?;
        Ok(Some(EffectivePromptDigest::chat_prompt_token_ids(
            self.name(),
            &self.tokens(request)?,
        )))
    }
    async fn complete(
        &self,
        _: &str,
        _: CompletionRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<CompletionResponseChunk>> + Send>>> {
        Err(PowerError::BackendNotAvailable(
            "native qualification currently exposes chat; raw completion adapter is not enabled"
                .into(),
        ))
    }
    async fn embed(&self, _: &str, _: EmbeddingRequest) -> Result<EmbeddingResponse> {
        Err(PowerError::BackendNotAvailable(
            "native XQ has no embedding API".into(),
        ))
    }
}

fn exact_manifest(manifest: &ModelManifest, path: &std::path::Path) -> bool {
    manifest.name == DeepSeekModel::V41Flash1017Xq.served_alias()
        && manifest.format == ModelFormat::Gguf
        && manifest.sha256 == XQ_GGUF_SHA256
        && manifest.size == XQ_GGUF_BYTES
        && manifest.path == path
        && manifest.system_prompt.is_none()
        && manifest.template_override.is_none()
        && manifest
            .default_parameters
            .as_ref()
            .is_none_or(|value| value.is_empty())
        && manifest.adapter_path.is_none()
        && manifest.adapter_artifact.is_none()
        && manifest.projector_path.is_none()
        && manifest.projector_artifact.is_none()
        && manifest.external_draft.is_none()
        && manifest.messages.is_empty()
}

fn validate_chat_options(request: &ChatRequest, context: usize) -> Result<()> {
    let value = serde_json::to_value(request)?;
    let fields = value
        .as_object()
        .ok_or_else(|| invalid("invalid native request envelope"))?;
    for (name, value) in fields {
        if value.is_null() {
            continue;
        }
        let allowed = match name.as_str() {
            "messages" | "max_tokens" | "stream" => true,
            "temperature" => value.as_f64() == Some(0.0),
            "top_p" => value.as_f64() == Some(1.0),
            "top_k" | "num_parallel" => value.as_u64() == Some(1),
            "num_ctx" => value
                .as_u64()
                .is_some_and(|count| count > 0 && count <= context as u64),
            "stream_options" => value.as_object().is_some_and(|options| {
                options.len() == 1
                    && options
                        .get("include_usage")
                        .is_some_and(|value| value.is_boolean())
            }),
            _ => false,
        };
        if !allowed {
            return Err(PowerError::InvalidRequest(format!(
                "native greedy text profile refuses option '{name}'"
            )));
        }
    }
    Ok(())
}

#[derive(Default)]
struct Decoder {
    ids: Vec<u32>,
    prefix: String,
    prefix_index: usize,
    all: Vec<u32>,
    emitted: String,
}
impl Decoder {
    fn token(&mut self, codec: &XqTextCodec, id: u32) -> Result<String> {
        if self.all.len() >= 32 {
            return Err(invalid("native stream exceeded its token receipt bound"));
        }
        self.all.push(id);
        if id == 1 {
            return Ok(String::new());
        }
        let piece = tokenizers::tokenizer::step_decode_stream(
            codec.tokenizer(),
            vec![id],
            false,
            &mut self.ids,
            &mut self.prefix,
            &mut self.prefix_index,
        )
        .map_err(|_| invalid("native incremental UTF-8 decoding failed"))?
        .unwrap_or_default();
        self.emitted.push_str(&piece);
        Ok(piece)
    }
    fn finish(&self, codec: &XqTextCodec) -> Result<String> {
        let ids: Vec<_> = self.all.iter().copied().filter(|&id| id != 1).collect();
        let complete = codec
            .tokenizer()
            .decode(&ids, false)
            .map_err(|_| invalid("native final UTF-8 decoding failed"))?;
        complete
            .strip_prefix(&self.emitted)
            .map(str::to_owned)
            .ok_or_else(|| invalid("native final decoded text changed an emitted prefix"))
    }
}
struct ChatState {
    generation: NativeGeneration,
    codec: Arc<XqTextCodec>,
    decoder: Decoder,
    parser: ThinkBlockParser,
    prompt_tokens: usize,
    ended: bool,
}
impl ChatState {
    async fn next(&mut self) -> Result<ChatResponseChunk> {
        let event = self
            .generation
            .next()
            .await
            .ok_or_else(|| invalid("native chat closed without a complete generation"))??;
        let mut result = ChatResponseChunk {
            content: String::new(),
            thinking_content: None,
            done: false,
            prompt_tokens: Some(self.prompt_tokens as u32),
            done_reason: None,
            prompt_eval_duration_ns: None,
            tool_calls: None,
            upstream_timings: None,
        };
        let (content, thinking) = match event {
            NativeGenerationEvent::Token(id) => {
                self.parser.feed(&self.decoder.token(&self.codec, id)?)
            }
            NativeGenerationEvent::Finished(generated) => {
                if generated.tokens != self.decoder.all
                    || generated.prompt_tokens != self.prompt_tokens
                {
                    return Err(invalid(
                        "native final token receipt disagrees with the streamed IDs",
                    ));
                }
                let (mut content, mut thinking) =
                    self.parser.feed(&self.decoder.finish(&self.codec)?);
                let (tail, thought) = self.parser.flush();
                content.push_str(&tail);
                thinking.push_str(&thought);
                result.done = true;
                result.done_reason = Some(
                    match generated.stop_reason {
                        XqStopReason::Eos => "stop",
                        XqStopReason::Length => "length",
                    }
                    .into(),
                );
                result.upstream_timings = Some(native_metadata(
                    &generated,
                    self.prompt_tokens,
                    self.generation.replica,
                ));
                self.ended = true;
                (content, thinking)
            }
        };
        result.content = content;
        if !thinking.is_empty() {
            result.thinking_content = Some(thinking);
        }
        Ok(result)
    }
}
fn native_metadata(
    generated: &crate::execution::XqGeneratedTokens,
    prompt: usize,
    replica: usize,
) -> serde_json::Value {
    // Power's existing suppress_token_metrics policy removes the entire
    // `timings` object. Keep every exact native counter/clock behind that same
    // established boundary, including replica-local execution receipts.
    serde_json::json!({"source":"power-native-token-ids",
        "usage":{"prompt_tokens":prompt,"completion_tokens":generated.tokens.len(),"total_tokens":prompt+generated.tokens.len()},
        "timings":{"native_generation":{"clock":"monotonic-nanoseconds","decoding_algorithm":"greedy-one-selected-id-per-model-step",
        "origin":"greedy-loop-after-request-state-allocation","choice_boundary":"complete-model-step-and-cache-commit-before-id-callback","includes_prior_callback_backpressure":true,
        "timings":generated.timings,"decode_intervals":generated.tokens.len().saturating_sub(1),"terminal_eos_included":generated.stop_reason==XqStopReason::Eos,
        "replica_index":replica,"stats_ordinal_scope":"replica-local","stats":generated.stats,
        "finite_validation_policy":if cfg!(feature="xq-validation-cache") {"step-local-immutable-tensor-cache-v1"} else {"immediate-per-call"},
        "readonly_kernel_input_policy":if cfg!(feature="xq-borrowed-kernel-inputs") {"borrow-contiguous-copy-strided-v1"} else {"force-copy-v1"}}}})
}
fn invalid(message: &str) -> PowerError {
    PowerError::InvalidRequest(message.into())
}

#[cfg(test)]
#[path = "backend/tests.rs"]
mod tests;
