use super::*;

#[derive(Default)]
struct CapturedInput(Mutex<Vec<String>>);

#[async_trait::async_trait]
impl LlmClient for CapturedInput {
    async fn complete(
        &self,
        _: &[Message],
        _: Option<&str>,
        _: &[ToolDefinition],
    ) -> anyhow::Result<LlmResponse> {
        panic!("this fixture uses the real streaming path")
    }

    async fn complete_streaming(
        &self,
        _: &[Message],
        system: Option<&str>,
        _: &[ToolDefinition],
        _: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<tokio::sync::mpsc::Receiver<StreamEvent>> {
        self.0
            .lock()
            .unwrap()
            .push(system.unwrap_or_default().into());
        let (sender, receiver) = tokio::sync::mpsc::channel(1);
        sender.send(StreamEvent::Done(response())).await.unwrap();
        Ok(receiver)
    }
}

#[tokio::test]
async fn model_guidance_uses_current_host_evidence_without_retaining_final_proof() {
    for changed in [false, true] {
        let (workspace, mut surface, _, _) = surface().await;
        let path = "result 甲.txt";
        std::fs::write(workspace.path().join(path), "original").unwrap();
        let client = Arc::new(CapturedInput::default());
        let agent = crate::agent::AgentLoop::new(
            client.clone(),
            Arc::new(crate::tools::ToolExecutor::new(
                workspace.path().display().to_string(),
            )),
            crate::tools::ToolContext::new(workspace.path().to_path_buf()),
            crate::agent::AgentConfig {
                require_assistant_final_response: true,
                completion_attestor: Some(Arc::new(
                    crate::completion_attestor::DiskContentAttestor::new(workspace.path()),
                )),
                ..Default::default()
            },
        )
        .with_model_generation_admission(crate::llm::ModelGenerationAdmission::default());
        let invocation = agent.invocation_context(
            surface.run_id.clone().unwrap(),
            Some(&surface.session_id),
            None,
            surface.cancel.clone(),
        );
        surface.agent = invocation.bind_agent_loop(&agent);
        surface.ledger.lock().unwrap().observe_tool(
            "write",
            0,
            Some(&serde_json::json!({"file_path":path,"after":"original"})),
        );
        if changed {
            std::fs::write(workspace.path().join(path), "changed").unwrap();
        }
        let missing_context = crate::harness_loop::model_verification_context(
            surface
                .agent
                .decide_fact_completion_gate(&surface.ledger.lock().unwrap(), &[]),
        )
        .expect("without host evidence the mutation is incomplete");
        let (completion, _) =
            LiveCompletion::new(client.clone(), PermissionPolicy::default(), vec![]);
        let result = completion
            .with_surface(surface.clone())
            .complete(
                CompletionRequest {
                    system: vec![],
                    tools: vec![],
                    summary: String::new(),
                    messages: vec!["user\nFinish the work.".into()],
                },
                surface.cancel.clone(),
            )
            .await
            .unwrap();
        assert!(matches!(result, ModelDecision::Text { .. }));
        let systems = client.0.lock().unwrap();
        assert_eq!(systems.len(), 1);
        assert_eq!(
            systems[0].contains(&missing_context),
            changed,
            "guidance must reflect current bytes, including host verification"
        );
        assert!(
            surface.reports.lock().unwrap().is_empty(),
            "a model preview must not become final completion proof"
        );
    }
}
