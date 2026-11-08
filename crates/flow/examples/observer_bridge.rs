use aster_flow::{
    AsterFlowEventBridge, FlowEngine, FlowRuntime, InMemoryAsterFlowEventSink, RuntimeCommand,
    StepInvocation, WorkflowInvocation, WorkflowSpec,
};
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

struct AuditRuntime;

#[async_trait]
impl FlowRuntime for AuditRuntime {
    async fn run_workflow(
        &self,
        invocation: WorkflowInvocation,
    ) -> aster_flow::Result<RuntimeCommand> {
        let ctx = invocation.context();
        if let Some(output) = ctx.step_output("build-report") {
            return Ok(ctx.complete(output.clone()));
        }

        Ok(ctx.schedule_step(
            "build-report",
            "build_report",
            json!({ "topic": ctx.input()["topic"] }),
        ))
    }

    async fn run_step(&self, invocation: StepInvocation) -> aster_flow::Result<serde_json::Value> {
        Ok(json!({
            "title": format!("Report: {}", invocation.input["topic"].as_str().unwrap_or("unknown")),
            "ready": true,
        }))
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> aster_flow::Result<()> {
    let sink = Arc::new(InMemoryAsterFlowEventSink::new());
    let observer = Arc::new(AsterFlowEventBridge::new(sink.clone()));
    let engine = FlowEngine::builder(Arc::new(AuditRuntime))
        .with_observer(observer.clone())
        .build();
    let spec = WorkflowSpec::rust_embedded("examples.audit", "0.1.0", "examples", "main");

    let run_id = engine
        .start_with_id("observer-demo", spec, json!({ "topic": "Aster Flow" }))
        .await?;
    let snapshot = engine.snapshot(&run_id).await?;

    println!("status={:?}", snapshot.status);
    println!("observed_events:");
    for event in sink.events().await {
        println!(
            "  key={} seq={} status={:?} labels={:?}",
            event.key,
            event.sequence,
            event.status,
            event.safe_metric_labels()
        );
    }
    Ok(())
}
