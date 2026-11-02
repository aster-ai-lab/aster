use super::*;
use crate::log::MemoryLog;
use std::sync::atomic::{AtomicUsize, Ordering};

struct ObservedModel {
    calls: AtomicUsize,
    usage: Vec<Option<u64>>,
}

impl Completion for ObservedModel {
    fn complete(
        &self,
        _: CompletionRequest,
        _: CancellationToken,
    ) -> BoxFuture<Result<ModelDecision, ActorError>> {
        let index = self.calls.fetch_add(1, Ordering::SeqCst);
        let decision = if index < 3 {
            ModelDecision::Tool {
                call: ToolCall {
                    id: format!("observation-{index}"),
                    name: "observe".into(),
                    args: json!({"page":index}),
                    needs_confirmation: false,
                    text: None,
                    reasoning: None,
                },
                also: vec![],
            }
        } else {
            ModelDecision::Text {
                text: "done".into(),
            }
        };
        Box::pin(async move { Ok(decision) })
    }

    fn reported_prompt_tokens(&self) -> Option<u64> {
        self.usage[self.calls.load(Ordering::SeqCst) - 1]
    }
}

struct LargeObservation;

impl ToolRunner for LargeObservation {
    fn run(&self, _: ToolCall, _: CancellationToken) -> BoxFuture<Result<Value, ActorError>> {
        Box::pin(async { Ok(json!("x".repeat(2048))) })
    }
}

struct Summarizer(Arc<AtomicUsize>);

impl Compactor for Summarizer {
    fn compact(&self, _: &[String]) -> BoxFuture<Result<String, ActorError>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok("earlier observations".into()) })
    }
}

async fn pressure(usage: Vec<Option<u64>>) -> (usize, Vec<Fact>) {
    let compactions = Arc::new(AtomicUsize::new(0));
    let services = Arc::new(CodingServices {
        completion: Arc::new(ObservedModel {
            calls: AtomicUsize::new(0),
            usage,
        }),
        tools: Arc::new(LargeObservation),
        compactor: Arc::new(Summarizer(compactions.clone())),
    });
    let config = HarnessConfig::new(8, 1024, 64, 1, vec![], vec![])
        .unwrap()
        .with_token_compact(10_000, 1);
    let result = ingest_coding(
        &coding_actor(config),
        &MemoryLog::new(),
        services,
        "pressure",
        message_fact("request", "Observe three pages."),
        64,
    )
    .await
    .unwrap();
    assert!(result.view.schema_error.is_none());
    assert_eq!(result.view.assistant.as_deref(), Some("done"));
    (compactions.load(Ordering::SeqCst), result.log)
}

#[tokio::test]
async fn missing_provider_usage_keeps_character_compaction_available() {
    let (compactions, facts) = pressure(vec![None; 4]).await;
    eprintln!("missing usage: {compactions} compactions");
    assert!(
        compactions > 0,
        "missing usage must not disable every pressure signal"
    );
    assert!(facts
        .iter()
        .filter(|f| f.kind == "model.turn")
        .all(|f| f.payload.get("prompt_tokens").is_none()));
}

#[tokio::test]
async fn a_later_missing_usage_does_not_reuse_an_old_small_prompt() {
    let (compactions, _) = pressure(vec![Some(16), None, None, None]).await;
    eprintln!("later missing usage: {compactions} compactions");
    assert!(
        compactions > 0,
        "the first response cannot describe later grown prompts"
    );
}

#[tokio::test]
async fn fresh_reported_usage_keeps_priority_over_the_character_estimate() {
    let (compactions, _) = pressure(vec![Some(16); 4]).await;
    eprintln!("fresh known usage: {compactions} compactions");
    assert_eq!(compactions, 0);
}
