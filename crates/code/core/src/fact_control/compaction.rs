//! Session compaction for the fact-log turn engine.

use super::*;

pub(super) struct UsageStamp {
    pub(super) inner: Arc<dyn Completion>,
    pub(super) prompt_tokens: Arc<AtomicU64>,
}

impl Completion for UsageStamp {
    fn complete(
        &self,
        request: CompletionRequest,
        cancel: tokio_util::sync::CancellationToken,
    ) -> aster_effect::coding::BoxFuture<Result<ModelDecision, ActorError>> {
        let inner = Arc::clone(&self.inner);
        let prompt_tokens = Arc::clone(&self.prompt_tokens);
        Box::pin(async move {
            let decision = inner.complete(request, cancel).await?;
            let reported = inner.reported_prompt_tokens().unwrap_or(0);
            prompt_tokens.store(reported, Ordering::SeqCst);
            Ok(decision)
        })
    }

    fn reported_prompt_tokens(&self) -> Option<u64> {
        self.inner.reported_prompt_tokens()
    }
}

pub(super) struct SessionCompact {
    pub(super) client: Arc<dyn LlmClient>,
    pub(super) history: (Arc<FileLog>, String),
    pub(super) source: Option<crate::store::SessionFactSourceV1>,
    pub(super) artifacts: Option<crate::tools::ArtifactStore>,
    pub(super) events: Option<tokio::sync::mpsc::Sender<crate::agent::AgentEvent>>,
    pub(super) budget: crate::compaction::CompactionBudget,
    pub(super) fixed_prompt_tokens: Arc<AtomicU64>,
    pub(super) prompt_tokens: Arc<AtomicU64>,
}

impl Compactor for SessionCompact {
    fn compact(
        &self,
        messages: &[String],
    ) -> aster_effect::coding::BoxFuture<Result<String, ActorError>> {
        self.compact_with_tail(messages, &[])
    }

    fn compact_with_tail(
        &self,
        messages: &[String],
        kept: &[String],
    ) -> aster_effect::coding::BoxFuture<Result<String, ActorError>> {
        let client = Arc::clone(&self.client);
        let (log, thread) = self.history.clone();
        let source = self.source.clone();
        let artifacts = self.artifacts.clone();
        let events = self.events.clone();
        let budget = self.budget;
        let window = budget.max_context_tokens as u64;
        let fixed_tokens = self.fixed_prompt_tokens.load(Ordering::SeqCst) as usize;
        let used = self.prompt_tokens.load(Ordering::SeqCst);
        let messages = messages.to_vec();
        let kept = kept.to_vec();
        let retain_source = artifacts.is_some();
        Box::pin(async move {
            // The bounded original-log read may finish after caller cancellation.
            let (([messages, kept], continuation_tokens), original_text) =
                tokio::task::spawn_blocking(move || {
                    let facts = log.read(&thread)?;
                    let original = retain_source
                        .then(|| checkpoints::original_public_transcript_from_facts(&facts));
                    Ok::<_, ActorError>((
                        history_seed::with_original_tool_text(&messages, &kept, facts)?,
                        original,
                    ))
                })
                .await
                .map_err(|error| ActorError::Handler {
                    key: "compaction-input-tool-history".into(),
                    message: error.to_string(),
                })??;
            // Summarize the public observation, not its stored receipt envelope.
            let messages =
                messages
                    .into_iter()
                    .map(|line| match line.split_once('\n') {
                        Some((role @ ("tool" | "tool-error"), body)) => format!(
                            "{role}\n{}",
                            crate::compaction::render_message_for_summary(
                                &message_from_folded_tool("", body, role == "tool-error",)
                            )
                        ),
                        _ => line,
                    })
                    .collect::<Vec<_>>();
            let kept_tokens = kept
                .iter()
                .map(|line| match line.split_once('\n') {
                    Some((role @ ("tool" | "tool-error"), body)) => {
                        crate::llm::estimate_message_tokens(&[message_from_folded_tool(
                            "",
                            body,
                            role == "tool-error",
                        )])
                    }
                    _ => line.len().div_ceil(4),
                })
                .sum::<usize>()
                .saturating_add(continuation_tokens);
            let reserved_tokens = fixed_tokens.saturating_add(kept_tokens);
            let mut budget = budget.with_fixed_prompt_tokens(reserved_tokens);
            let original = match original_text {
                Some(text) => {
                    crate::compaction::CompactionSource::prepare(artifacts.as_ref(), text)
                        .await
                        .map_err(|error| ActorError::Handler {
                            key: "compaction-source".into(),
                            message: format!("{error:#}"),
                        })?
                }
                None => None,
            };
            let available = budget.summary_token_limit();
            let original = original.filter(|original| original.estimated_tokens() < available);
            budget.message_token_limit = available
                .saturating_sub(
                    original
                        .as_ref()
                        .map_or(0, crate::compaction::CompactionSource::estimated_tokens),
                )
                .max(1);
            if let Some(events) = &events {
                let _ = events
                    .send(crate::agent::AgentEvent::AutoCompact {
                        phase: "started".into(),
                        tokens_used: used,
                        context_window: window,
                        tokens_after: 0,
                    })
                    .await;
            }
            let mut summary =
                compact_handoff(client.as_ref(), &messages, budget, source.as_ref()).await;
            if let Some(original) = original {
                original
                    .attach(&mut summary)
                    .await
                    .map_err(|error| ActorError::Handler {
                        key: "compaction-source".into(),
                        message: format!("{error:#}"),
                    })?;
            }
            let summary_tokens = crate::llm::estimate_message_tokens(&[Message::user_wire(
                &format!("{}{}", crate::prompts::CONTEXT_SUMMARY_PREFIX, summary),
            )]);
            let tokens_after = reserved_tokens.saturating_add(summary_tokens) as u64;
            tracing::info!(
                fixed_prompt_tokens = fixed_tokens,
                kept_tokens,
                summary_tokens,
                estimated_prompt_tokens = tokens_after,
                target_tokens = budget.target_context_tokens,
                target_exceeded = tokens_after > budget.target_context_tokens as u64,
                "Fact compaction applied"
            );
            if let Some(events) = &events {
                let _ = events
                    .send(crate::agent::AgentEvent::AutoCompact {
                        phase: "completed".into(),
                        tokens_used: used,
                        context_window: window,
                        tokens_after,
                    })
                    .await;
            }
            Ok(summary)
        })
    }
}

/// Keep fitting prefixes losslessly; otherwise use the shared summary budget.
/// Mandatory original obligations are sealed outside the discretionary allowance.
pub(super) async fn compact_handoff(
    client: &dyn LlmClient,
    messages: &[String],
    budget: crate::compaction::CompactionBudget,
    source: Option<&crate::store::SessionFactSourceV1>,
) -> String {
    let joined = messages.join("\n");
    let summary_token_limit = budget.summary_token_limit();
    if joined.len() <= summary_token_limit.saturating_mul(4) {
        let mut carried = crate::compaction_carry::obligations_from_folded(messages);
        carried.durable_source = source.cloned();
        let handoff = crate::compaction_carry::seal_summary(&joined, &carried);
        if handoff.contains(&joined) {
            return handoff;
        }
    }
    match crate::compaction::summarize_folded_prefix(client, messages, budget, source).await {
        Ok(summary) => summary,
        Err(_) => fallback_handoff(messages, budget, source),
    }
}

fn fallback_handoff(
    messages: &[String],
    budget: crate::compaction::CompactionBudget,
    source: Option<&crate::store::SessionFactSourceV1>,
) -> String {
    let mut carried = crate::compaction_carry::obligations_from_folded(messages);
    carried.durable_source = source.cloned();
    crate::compaction_carry::cap_fallback_goals(&mut carried);
    let summary = crate::compaction_carry::strip_model_sections(&bounded_summary(messages));
    let summary =
        crate::compaction::truncate_summary_to_token_limit(&summary, budget.summary_token_limit());
    crate::compaction_carry::seal_summary(&summary, &carried)
}

/// Keep a short handoff when the prefix is long. A joined transcript would
/// put the same tokens back into the next prompt.
pub(super) fn bounded_summary(messages: &[String]) -> String {
    const MAX_CHARS: usize = 32_000;
    let joined = messages.join("\n");
    if joined.len() <= MAX_CHARS {
        return joined;
    }
    let head_end = char_boundary_at(&joined, 8_000);
    let tail_start = char_boundary_before(&joined, joined.len().saturating_sub(8_000));
    format!(
        "{}\n\n[earlier transcript compacted]\n\n{}",
        &joined[..head_end],
        &joined[tail_start..]
    )
}

pub(super) fn char_boundary_at(text: &str, index: usize) -> usize {
    let index = index.min(text.len());
    (index..=text.len())
        .find(|cursor| text.is_char_boundary(*cursor))
        .unwrap_or(text.len())
}

pub(super) fn char_boundary_before(text: &str, index: usize) -> usize {
    let index = index.min(text.len());
    (0..=index)
        .rev()
        .find(|cursor| text.is_char_boundary(*cursor))
        .unwrap_or(0)
}
