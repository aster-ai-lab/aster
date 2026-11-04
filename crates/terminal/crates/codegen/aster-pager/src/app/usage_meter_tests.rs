use super::*;
use std::time::Duration;

fn usage(sequence: u64, tokens: u64) -> acp::SessionUpdate {
    let mut usage = acp::UsageUpdate::new(999, 128_000);
    usage.meta = Some(
        serde_json::json!({"callSequence":sequence,"totalTokens":tokens})
            .as_object()
            .unwrap()
            .clone(),
    );
    acp::SessionUpdate::UsageUpdate(usage)
}

#[test]
fn session_total_restores_and_counts_each_invocation_once_across_prompts() {
    let now = Instant::now();
    let mut meter = UsageMeter::default();
    assert_eq!(meter.caption(now, true), "Σ — tokens · — tok/s");
    meter.reconcile(Some(1200), None, None);
    meter.observe(&usage(1, 300), Some("p1"), now);
    meter.observe(&usage(1, 300), Some("p1"), now);
    assert_eq!(meter.caption(now, true), "Σ 1.5k tokens · — tok/s");
    meter.observe(&usage(1, 500), Some("p2"), now);
    assert_eq!(meter.caption(now, true), "Σ 2k tokens · — tok/s");
    meter.reconcile(Some(2100), Some(80), Some(4000));
    assert_eq!(meter.caption(now, false), "Σ 2.1k tokens · 20.0 tok/s");
    meter.reconcile(Some(2100), Some(80), Some(0));
    assert!(meter.caption(now, true).ends_with("— tok/s"));
}

#[test]
fn live_rate_is_explicitly_estimated_and_tool_wait_is_not_inference_time() {
    let now = Instant::now();
    let mut meter = UsageMeter::default();
    meter.reconcile(Some(1200), None, None);
    let chunk = acp::SessionUpdate::AgentThoughtChunk(acp::ContentChunk::new(
        acp::ContentBlock::Text(acp::TextContent::new("abcdefgh")),
    ));
    meter.observe(&chunk, Some("p"), now);
    meter.observe(&chunk, Some("p"), now + Duration::from_secs(1));
    assert_eq!(
        meter.caption(now + Duration::from_secs(1), true),
        "Σ 1.2k tokens · ≈4.0 tok/s"
    );
    meter.observe(&usage(1, 100), Some("p"), now + Duration::from_secs(1));
    assert_eq!(
        meter.caption(now + Duration::from_secs(60), true),
        "Σ 1.3k tokens · — tok/s"
    );
    meter.observe(&chunk, Some("p"), now + Duration::from_secs(60));
    assert_eq!(
        meter.caption(now + Duration::from_secs(61), true),
        "Σ 1.3k tokens · ≈2.0 tok/s"
    );
}

#[test]
fn missing_accounting_never_becomes_a_known_zero() {
    let now = Instant::now();
    let mut meter = UsageMeter::default();
    meter.observe(&usage(1, 100), Some("p"), now);
    assert_eq!(meter.caption(now, true), "Σ — tokens · — tok/s");
}

#[test]
fn a_cancelled_stream_does_not_keep_a_live_rate_or_leak_into_the_next_prompt() {
    let now = Instant::now();
    let mut meter = UsageMeter::default();
    let chunk = acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
        acp::ContentBlock::Text(acp::TextContent::new("abcdefgh")),
    ));
    meter.observe(&chunk, Some("p1"), now);
    assert!(
        meter
            .caption(now + Duration::from_secs(1), false)
            .ends_with("— tok/s")
    );
    meter.observe(&chunk, Some("p2"), now + Duration::from_secs(2));
    assert!(
        meter
            .caption(now + Duration::from_secs(3), true)
            .ends_with("≈2.0 tok/s")
    );
}

#[test]
fn a_new_request_waiting_for_its_first_chunk_does_not_display_the_previous_rate() {
    let now = Instant::now();
    let mut meter = UsageMeter::default();
    meter.reconcile(Some(1500), Some(80), Some(4000));
    assert_eq!(meter.caption(now, false), "Σ 1.5k tokens · 20.0 tok/s");
    assert_eq!(meter.caption(now, true), "Σ 1.5k tokens · — tok/s");
}

#[test]
fn absolute_totals_preserve_live_generation_and_the_last_measured_rate() {
    let now = Instant::now();
    let mut meter = UsageMeter::default();
    meter.reconcile(Some(1200), Some(80), Some(4000));
    meter.reconcile(Some(1400), None, None);
    assert_eq!(meter.caption(now, false), "Σ 1.4k tokens · 20.0 tok/s");
    let chunk = acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
        acp::ContentBlock::Text(acp::TextContent::new("abcdefgh")),
    ));
    meter.observe(&chunk, Some("new"), now);
    meter.reconcile(Some(1500), None, None);
    assert_eq!(
        meter.caption(now + Duration::from_secs(1), true),
        "Σ 1.5k tokens · ≈2.0 tok/s"
    );
}

#[test]
fn absolute_call_totals_do_not_double_count_watch_updates_or_roll_back_late_totals() {
    let now = Instant::now();
    for notification_first in [false, true] {
        let mut meter = UsageMeter::default();
        meter.reconcile(Some(1200), None, None);
        let mut update = usage(1, 300);
        if let acp::SessionUpdate::UsageUpdate(ref mut call) = update {
            call.meta
                .as_mut()
                .unwrap()
                .insert("sessionTotalTokens".into(), 1500.into());
        }
        if notification_first {
            meter.reconcile(Some(1500), None, None);
        }
        meter.observe(&update, Some("p"), now);
        meter.observe(&update, Some("p"), now);
        if !notification_first {
            meter.reconcile(Some(1500), None, None);
        }
        assert_eq!(meter.caption(now, true), "Σ 1.5k tokens · — tok/s");
        meter.reconcile(Some(1600), None, None);
        meter.observe(&update, Some("p"), now);
        meter.reconcile(Some(1500), None, None);
        assert_eq!(meter.caption(now, true), "Σ 1.6k tokens · — tok/s");
    }
}

#[test]
fn controller_text_and_usage_updates_preserve_the_last_measured_generation_rate() {
    let now = Instant::now();
    let mut meter = UsageMeter::default();
    meter.reconcile(Some(215780), Some(473), Some(3641));
    let mut total = usage(1, 65311);
    if let acp::SessionUpdate::UsageUpdate(ref mut call) = total {
        call.meta
            .as_mut()
            .unwrap()
            .insert("sessionTotalTokens".into(), 219813.into());
    }
    meter.observe(&total, Some("goal"), now);
    let controller = acp::SessionUpdate::AgentMessageChunk(
        acp::ContentChunk::new(acp::ContentBlock::Text(acp::TextContent::new(
            "Durable goal finished",
        )))
        .meta(Some(
            [("asterGenerationDelta".into(), false.into())]
                .into_iter()
                .collect(),
        )),
    );
    meter.observe(&controller, Some("goal"), now);
    assert_eq!(meter.caption(now, false), "Σ 219.8k tokens · 129.9 tok/s");
    assert_eq!(meter.caption(now, true), "Σ 219.8k tokens · — tok/s");
    let generated = acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
        acp::ContentBlock::Text(acp::TextContent::new("abcdefgh")),
    ));
    meter.observe(&generated, Some("goal"), now);
    assert!(meter.caption(now, false).ends_with("— tok/s"));
    assert!(
        meter
            .caption(now + Duration::from_secs(1), true)
            .ends_with("≈2.0 tok/s")
    );
    meter.reconcile(None, Some(80), Some(4000));
    meter.observe(
        &acp::SessionUpdate::UserMessageChunk(acp::ContentChunk::new(acp::ContentBlock::Text(
            acp::TextContent::new("next request"),
        ))),
        Some("next"),
        now,
    );
    assert!(meter.caption(now, false).ends_with("— tok/s"));
}
