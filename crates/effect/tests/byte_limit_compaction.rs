//! A long unattended run keeps appending after the fact log reaches its byte
//! limit. The log may only drop transcript bytes already covered by a
//! compaction summary, and the actor must still fold what remains.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use aster_effect::{
    coding_actor, ingest_coding, message_fact, resume_coding, ActorError, CodingPhase,
    CodingServices, Compactor, Completion, CompletionRequest, FileLog, HarnessConfig, LogStore,
    MemoryLog, ModelDecision, NewFact, ToolCall, ToolRunner,
};
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

const THREAD: &str = "unattended";
const LIMIT: u32 = 1_000_000;
const OUTPUT_BYTES: usize = 400 * 1024;
const BATCH: usize = 3;

fn call(id: String) -> ToolCall {
    ToolCall {
        id,
        name: "read".into(),
        args: json!({ "path": "src/lib.rs" }),
        needs_confirmation: false,
        text: None,
        reasoning: None,
    }
}

/// Emits `rounds` tool batches, then a final answer.
struct BatchModel {
    rounds: usize,
    calls: AtomicUsize,
}

impl Completion for BatchModel {
    fn complete(
        &self,
        _request: CompletionRequest,
        _cancel: CancellationToken,
    ) -> aster_effect::coding::BoxFuture<Result<ModelDecision, ActorError>> {
        let round = self.calls.fetch_add(1, Ordering::SeqCst);
        let decision = if round < self.rounds {
            let mut calls = (0..BATCH).map(|index| call(format!("c{round}-{index}")));
            ModelDecision::Tool {
                call: calls.next().unwrap(),
                also: calls.collect(),
            }
        } else {
            ModelDecision::Text {
                text: "finished".into(),
            }
        };
        Box::pin(async move { Ok(decision) })
    }
}

struct BigOutput(Arc<AtomicUsize>);

impl ToolRunner for BigOutput {
    fn run(
        &self,
        call: ToolCall,
        _cancel: CancellationToken,
    ) -> aster_effect::coding::BoxFuture<Result<Value, ActorError>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            Ok(Value::String(format!(
                "{}:{}",
                call.id,
                "x".repeat(OUTPUT_BYTES)
            )))
        })
    }
}

struct Summaries(Mutex<usize>);

impl Compactor for Summaries {
    fn compact(
        &self,
        _messages: &[String],
    ) -> aster_effect::coding::BoxFuture<Result<String, ActorError>> {
        let mut count = self.0.lock().unwrap();
        *count += 1;
        let summary = format!("summary {count}");
        Box::pin(async move { Ok(summary) })
    }
}

fn services_counting(rounds: usize, executed: &Arc<AtomicUsize>) -> Arc<CodingServices> {
    Arc::new(CodingServices {
        completion: Arc::new(BatchModel {
            rounds,
            calls: AtomicUsize::new(0),
        }),
        tools: Arc::new(BigOutput(Arc::clone(executed))),
        compactor: Arc::new(Summaries(Mutex::new(0))),
    })
}

fn services(rounds: usize) -> Arc<CodingServices> {
    services_counting(rounds, &Arc::new(AtomicUsize::new(0)))
}

fn config() -> HarnessConfig {
    // One batch of outputs overflows the threshold, so every cycle compacts.
    HarnessConfig::new(1, 1_000_000, 1, 1, vec!["system".into()], Vec::new())
        .unwrap()
        .without_count_limits()
        .with_keep_tokens(0)
}

struct TailAwareSummaries {
    tails: Mutex<Vec<Vec<String>>>,
}

impl Compactor for TailAwareSummaries {
    fn compact(&self, _: &[String]) -> aster_effect::coding::BoxFuture<Result<String, ActorError>> {
        panic!("the scheduler must supply its actual retained tail")
    }

    fn compact_with_tail(
        &self,
        _: &[String],
        kept: &[String],
    ) -> aster_effect::coding::BoxFuture<Result<String, ActorError>> {
        self.tails.lock().unwrap().push(kept.to_vec());
        Box::pin(async { Ok("Earlier observations remain summarized.".into()) })
    }
}

#[tokio::test]
async fn compactor_receives_the_exact_unchanged_multi_tool_tail() {
    let compactor = Arc::new(TailAwareSummaries {
        tails: Mutex::new(Vec::new()),
    });
    let services = Arc::new(CodingServices {
        completion: Arc::new(BatchModel {
            rounds: 2,
            calls: AtomicUsize::new(0),
        }),
        tools: Arc::new(BigOutput(Arc::new(AtomicUsize::new(0)))),
        compactor: compactor.clone(),
    });
    let log = MemoryLog::new();
    let settled = ingest_coding(
        &coding_actor(config().with_keep_tokens(64)),
        &log,
        services,
        THREAD,
        message_fact("m1", "complete the observations"),
        LIMIT,
    )
    .await
    .unwrap();
    assert_eq!(settled.view.assistant.as_deref(), Some("finished"));
    let tails = compactor.tails.lock().unwrap();
    assert!(tails.len() >= 2);
    let facts = log.read(THREAD).unwrap();
    let landed = facts
        .iter()
        .filter(|fact| fact.kind == "compaction.done")
        .collect::<Vec<_>>();
    assert_eq!(landed.len(), tails.len());
    for (fact, kept) in landed.into_iter().zip(tails.iter()) {
        assert_eq!(fact.payload["kept"], json!(kept));
        assert_eq!(
            kept.iter()
                .filter(|line| line.starts_with("tool\n"))
                .count(),
            BATCH
        );
        assert_eq!(
            kept.iter()
                .filter(|line| line.starts_with("assistant\ntool "))
                .count(),
            BATCH
        );
    }
}

#[tokio::test]
async fn long_run_past_the_byte_limit_keeps_running_across_compactions() {
    let dir = tempfile::tempdir().unwrap();
    let log = FileLog::open(dir.path()).unwrap();
    let actor = coding_actor(config());
    // 3 * 400 KiB per round is ~1.2 MiB; 40 rounds is ~48 MiB, three times the cap.
    let services = services(40);

    let settled = ingest_coding(
        &actor,
        &log,
        services,
        THREAD,
        message_fact("m1", "keep going"),
        LIMIT,
    )
    .await
    .unwrap_or_else(|error| panic!("run hard-failed: {error:?}"));

    assert_eq!(settled.view.schema_error, None);
    assert_eq!(settled.view.phase, CodingPhase::Done);
    assert_eq!(settled.view.assistant.as_deref(), Some("finished"));

    let facts = log.read(THREAD).unwrap();
    assert!(facts.iter().any(|fact| fact.kind == "compaction.done"));
    assert!(facts.iter().any(|fact| fact.kind == "compaction.pruned"));
    let reopened = resume_coding(
        &coding_actor(config()),
        &FileLog::open(dir.path()).unwrap(),
        self::services(0),
        THREAD,
        LIMIT,
    )
    .await
    .unwrap();
    assert_eq!(reopened.view.schema_error, None);
    assert_eq!(reopened.view.assistant.as_deref(), Some("finished"));
}

/// The log keeps its tool allowance across the compactions that tombstone the
/// facts which counted it, exactly as a log that never prunes does.
#[tokio::test]
async fn tool_allowance_is_the_same_with_and_without_pruning() {
    // 3 MiB per round: ten rounds cross the byte limit well before the allowance ends.
    let budget = || {
        HarnessConfig::new(30, 1_000_000, 1, 1, vec!["system".into()], Vec::new())
            .unwrap()
            .with_step_limit(None)
            .unwrap()
            .with_keep_tokens(0)
    };
    let run = |log: Box<dyn LogStore>| async move {
        let executed = Arc::new(AtomicUsize::new(0));
        let settled = ingest_coding(
            &coding_actor(budget()),
            log.as_ref(),
            services_counting(40, &executed),
            THREAD,
            message_fact("m1", "keep going"),
            LIMIT,
        )
        .await
        .unwrap_or_else(|error| panic!("run hard-failed: {error:?}"));
        (
            settled.view.assistant,
            settled.view.schema_error,
            executed.load(Ordering::SeqCst),
        )
    };
    let dir = tempfile::tempdir().unwrap();
    let pruned = run(Box::new(FileLog::open(dir.path()).unwrap())).await;
    assert!(std::fs::read_to_string(dir.path().join("unattended.jsonl"))
        .unwrap()
        .contains("compaction.pruned"));
    let whole = run(Box::new(MemoryLog::new())).await;
    assert_eq!(pruned, whole);
    assert_eq!(whole.0.as_deref(), Some("budget"));
    assert_eq!(whole.2, 30);
}

/// A log tombstoned by the first fix (no carried counters, `tool.prepared`
/// left in place) folds and continues instead of failing its schema.
#[tokio::test]
async fn log_tombstoned_before_counters_were_carried_keeps_running() {
    let first = MemoryLog::new();
    ingest_coding(
        &coding_actor(config()),
        &first,
        services(6),
        THREAD,
        message_fact("m1", "keep going"),
        LIMIT,
    )
    .await
    .unwrap();
    let facts = first.read(THREAD).unwrap();
    let anchor = facts
        .iter()
        .rposition(|fact| fact.kind == "compaction.done")
        .expect("the run compacted");

    let legacy = MemoryLog::new();
    for (index, fact) in facts.iter().enumerate() {
        let tombstone = index < anchor
            && matches!(
                fact.kind.as_str(),
                "user.message" | "model.turn" | "tool.result" | "question.answered"
            );
        let mut payload = fact.payload.clone();
        if fact.kind == "compaction.done" {
            payload.as_object_mut().unwrap().remove("carry");
        }
        let next = NewFact {
            kind: if tombstone {
                "compaction.pruned"
            } else {
                fact.kind.as_str()
            }
            .into(),
            key: fact.key.clone(),
            payload: if tombstone {
                serde_json::json!({})
            } else {
                payload
            },
        };
        legacy
            .append(THREAD, &[next], fact.cause.as_deref())
            .unwrap();
    }
    let settled = ingest_coding(
        &coding_actor(config()),
        &legacy,
        services(3),
        THREAD,
        message_fact("m2", "and again"),
        LIMIT,
    )
    .await
    .unwrap_or_else(|error| panic!("legacy log hard-failed: {error:?}"));
    assert_eq!(settled.view.schema_error, None);
    assert_eq!(settled.view.assistant.as_deref(), Some("finished"));
}
