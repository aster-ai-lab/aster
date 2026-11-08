use super::{
    plan::{validate_text_tokens, EOS_TOKEN},
    XqPipelinePlan, XqStepOutput,
};
use crate::layers::xq::{check_cancel, invalid};
use aster_power::{error::Result, inference::InferenceLimits};
use serde::Serialize;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum XqStopReason {
    Eos,
    Length,
}
impl XqStopReason {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "eos" => Ok(Self::Eos),
            "length" => Ok(Self::Length),
            _ => Err(invalid("XQ stop reason requires eos or length")),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Eos => "eos",
            Self::Length => "length",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XqGenerationStats {
    pub model_steps: u64,
    pub executed_layers_by_ordinal: [u64; 8],
    pub completed_peer_copies: u64,
    pub completed_peer_bytes: u64,
    pub finite_validation_checks: u64,
    pub finite_validation_cache_hits: u64,
}

/// Monotonic nanoseconds from entry to the greedy loop, after the pipeline
/// allocated this request's private state. Choices are timed after complete
/// model execution/cache commit, before emitting the actual selected ID.
/// Later choices include any preceding callback backpressure. No SSE-fragment
/// count or transport clock participates in these measurements.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XqGenerationTimings {
    pub first_token_ns: u64,
    pub last_token_ns: u64,
    pub finished_ns: u64,
}

/// Raw IDs include a terminal EOS when emitted. The last emitted token has
/// not been fed back through the model. processed_tokens therefore equals
/// prompt_tokens + tokens.len() - 1, and pending_tail names that last token.
/// The scoped request state is released on return; this is no session handle.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XqGeneratedTokens {
    pub tokens: Vec<u32>,
    pub prompt_tokens: usize,
    pub processed_tokens: usize,
    pub pending_tail: u32,
    pub stop_reason: XqStopReason,
    pub stats: XqGenerationStats,
    pub timings: XqGenerationTimings,
}

pub(super) struct StepChoice {
    pub next: u32,
    pub position: usize,
    pub layers: [usize; 8],
    pub copies: u64,
    pub bytes: u64,
}
impl StepChoice {
    pub(super) fn from_output(output: XqStepOutput) -> Result<Self> {
        if output.accepted_tokens != output.proposed_tokens {
            return Err(invalid(
                "greedy XQ loop cannot consume a partially accepted step",
            ));
        }
        let mut bytes = 0_u64;
        for receipt in &output.peer_copies {
            bytes = bytes
                .checked_add(receipt.bytes)
                .ok_or_else(|| invalid("greedy XQ peer receipt total overflowed"))?;
        }
        Ok(Self {
            next: output.last_argmax,
            position: output.cache_position,
            layers: output.executed_layers_by_ordinal,
            copies: output.peer_copies.len() as u64,
            bytes,
        })
    }
}

pub(super) fn validate_request(
    prompt: &[u32],
    max_new: usize,
    plan: &XqPipelinePlan,
    limits: &InferenceLimits,
) -> Result<()> {
    plan.validate(limits)?;
    if prompt.is_empty()
        || max_new == 0
        || max_new > limits.max_generated_tokens
        || prompt
            .len()
            .checked_add(max_new)
            .is_none_or(|n| n > plan.context_tokens)
    {
        return Err(invalid("greedy XQ request requires nonempty prompt/output within generation and context limits"));
    }
    let ids = prompt
        .len()
        .checked_add(max_new)
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| invalid("greedy XQ token output bytes overflowed"))?;
    if ids > limits.max_input_bytes {
        return Err(invalid(
            "greedy XQ prompt and emitted token metadata exceed input budget",
        ));
    }
    validate_text_tokens(prompt, limits)
}

pub(super) fn run(
    prompt: &[u32],
    max_new: usize,
    plan: &XqPipelinePlan,
    limits: &InferenceLimits,
    cancellation: &CancellationToken,
    step: impl FnMut(&[u32]) -> Result<StepChoice>,
) -> Result<XqGeneratedTokens> {
    run_streaming(
        prompt,
        max_new,
        plan,
        limits,
        cancellation,
        step,
        |_| Ok(()),
    )
}

pub(super) fn run_streaming(
    prompt: &[u32],
    max_new: usize,
    plan: &XqPipelinePlan,
    limits: &InferenceLimits,
    cancellation: &CancellationToken,
    step: impl FnMut(&[u32]) -> Result<StepChoice>,
    emit: impl FnMut(u32) -> Result<()>,
) -> Result<XqGeneratedTokens> {
    let started = Instant::now();
    run_streaming_clocked(
        prompt,
        max_new,
        plan,
        limits,
        cancellation,
        (step, emit),
        || {
            u64::try_from(started.elapsed().as_nanos())
                .map_err(|_| invalid("native generation clock exceeds its nanosecond range"))
        },
    )
}

pub(super) fn run_streaming_clocked(
    prompt: &[u32],
    max_new: usize,
    plan: &XqPipelinePlan,
    limits: &InferenceLimits,
    cancellation: &CancellationToken,
    callbacks: (
        impl FnMut(&[u32]) -> Result<StepChoice>,
        impl FnMut(u32) -> Result<()>,
    ),
    mut now_ns: impl FnMut() -> Result<u64>,
) -> Result<XqGeneratedTokens> {
    let (mut step, mut emit) = callbacks;
    validate_request(prompt, max_new, plan, limits)?;
    check_cancel(cancellation)?;
    let mut tokens = Vec::new();
    tokens
        .try_reserve_exact(max_new)
        .map_err(|_| invalid("greedy XQ cannot reserve bounded output IDs"))?;
    let mut expected_layers = [0; 8];
    for &ordinal in plan.placement.layer_ordinals() {
        expected_layers[ordinal] += 1;
    }
    let mut stats = XqGenerationStats::default();
    let mut position = 0_usize;
    let mut execute = |ids: &[u32]| -> Result<u32> {
        check_cancel(cancellation)?;
        #[cfg(feature = "xq-validation-cache")]
        let _ = super::validation_cache::take_last_counts();
        let output = step(ids)?;
        #[cfg(feature = "xq-validation-cache")]
        if let Some((checks, hits)) = super::validation_cache::take_last_counts() {
            stats.finite_validation_checks = stats
                .finite_validation_checks
                .checked_add(checks)
                .ok_or_else(|| invalid("native finite validation count overflowed"))?;
            stats.finite_validation_cache_hits = stats
                .finite_validation_cache_hits
                .checked_add(hits)
                .ok_or_else(|| invalid("native validation cache hit count overflowed"))?;
        }
        check_cancel(cancellation)?;
        let next_position = position
            .checked_add(ids.len())
            .ok_or_else(|| invalid("greedy XQ processed position overflowed"))?;
        if output.position != next_position || output.layers != expected_layers {
            return Err(invalid(
                "greedy XQ step did not complete its exact cache position and forty-layer schedule",
            ));
        }
        if output.next as usize >= crate::tokenizer::RAW_VOCABULARY {
            return Err(invalid(
                "greedy XQ engine returned an out-of-vocabulary argmax",
            ));
        }
        stats.model_steps = stats
            .model_steps
            .checked_add(1)
            .ok_or_else(|| invalid("greedy XQ model-step count overflowed"))?;
        for (total, count) in stats
            .executed_layers_by_ordinal
            .iter_mut()
            .zip(output.layers)
        {
            *total = total
                .checked_add(count as u64)
                .ok_or_else(|| invalid("greedy XQ layer-call count overflowed"))?;
        }
        stats.completed_peer_copies = stats
            .completed_peer_copies
            .checked_add(output.copies)
            .ok_or_else(|| invalid("greedy XQ peer-copy count overflowed"))?;
        stats.completed_peer_bytes = stats
            .completed_peer_bytes
            .checked_add(output.bytes)
            .ok_or_else(|| invalid("greedy XQ peer bytes overflowed"))?;
        position = next_position;
        Ok(output.next)
    };
    let mut next = 0;
    for chunk in prompt.chunks(plan.max_step_tokens) {
        next = execute(chunk)?;
    }
    let mut first_token_ns = None;
    let mut last_token_ns = 0;
    let stop_reason = loop {
        check_cancel(cancellation)?;
        validate_text_tokens(&[next], limits)?;
        tokens.push(next);
        let selected_ns = now_ns()?;
        if selected_ns < last_token_ns {
            return Err(invalid("native generation clock moved backwards"));
        }
        first_token_ns.get_or_insert(selected_ns);
        last_token_ns = selected_ns;
        emit(next)?;
        check_cancel(cancellation)?;
        if next == EOS_TOKEN {
            break XqStopReason::Eos;
        }
        if tokens.len() == max_new {
            break XqStopReason::Length;
        }
        next = execute(&[next])?;
    };
    check_cancel(cancellation)?;
    let pending_tail = *tokens
        .last()
        .ok_or_else(|| invalid("greedy XQ returned no generated token"))?;
    let finished_ns = now_ns()?;
    if finished_ns < last_token_ns {
        return Err(invalid(
            "native generation final clock precedes its last selected token",
        ));
    }
    Ok(XqGeneratedTokens {
        tokens,
        prompt_tokens: prompt.len(),
        processed_tokens: position,
        pending_tail,
        stop_reason,
        stats,
        timings: XqGenerationTimings {
            first_token_ns: first_token_ns
                .ok_or_else(|| invalid("native generation lacks a selected-token timestamp"))?,
            last_token_ns,
            finished_ns,
        },
    })
}
