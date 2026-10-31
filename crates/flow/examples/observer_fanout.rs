use aster_flow::{
    AsterFlowEventBridge, FanoutFlowEventObserver, FlowEngine, FlowRuntime, InMemoryAsterFlowEventSink,
    InMemoryFlowEventObserver, RuntimeCommand, StepInvocation, WorkflowInvocation, WorkflowSpec,
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
    let raw_observer = Arc::new(InMemoryFlowEventObserver::new());
    let aster_sink = Arc::new(InMemoryAsterFlowEventSink::new());
    let aster_bridge = Arc::new(AsterFlowEventBridge::new(aster_sink.clone()));
    let observer = Arc::new(
        FanoutFlowEventObserver::new()
            .with_observer(raw_observer.clone())
            .with_observer(aster_bridge),
    );
    let engine = FlowEngine::builder(Arc::new(AuditRuntime))
        .with_observer(observer)
        .build();
    let spec = WorkflowSpec::rust_embedded("examples.observer-fanout", "0.1.0", "examples", "main");

    let run_id = engine
        .start_with_id("observer-fanout-demo", spec, json!({ "topic": "Aster Flow" }))
        .await?;
    let snapshot = engine.snapshot(&run_id).await?;
    let raw_events = raw_observer.events().await;
    let aster_events = aster_sink.events().await;

    println!("status={:?}", snapshot.status);
    println!("raw_events={}", raw_events.len());
    println!("aster_events={}", aster_events.len());
    println!(
        "last_aster_key={}",
        aster_events
            .last()
            .map(|event| event.key.as_str())
            .unwrap_or("none")
    );
    Ok(())
}
