use super::*;

const VISIBLE: &str = "catalog_visible";
const HIDDEN: &str = "catalog_hidden";
const LOOKUP: &str = crate::tools::deferred_catalog::LOOKUP_NAME;

struct FixtureTool(&'static str, String);

#[async_trait]
impl crate::tools::Tool for FixtureTool {
    fn name(&self) -> &str {
        self.0
    }

    fn description(&self) -> &str {
        &self.1
    }

    fn parameters(&self) -> serde_json::Value {
        let properties: serde_json::Map<_, _> = (0..128)
            .map(|index| {
                (
                    format!("field_{index}"),
                    serde_json::json!({"type": "string"}),
                )
            })
            .collect();
        serde_json::json!({"type": "object", "properties": properties})
    }

    async fn execute(
        &self,
        _args: &serde_json::Value,
        _context: &crate::tools::ToolContext,
    ) -> anyhow::Result<crate::tools::ToolOutput> {
        panic!("catalog discovery cannot execute {}", self.name());
    }
}

struct CatalogClient {
    script: ScriptedClient,
    definitions: Arc<std::sync::Mutex<Vec<Vec<ToolDefinition>>>>,
}

#[async_trait]
impl LlmClient for CatalogClient {
    fn supports_deferred_tool_catalog(&self) -> bool {
        true
    }

    async fn complete(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
    ) -> anyhow::Result<LlmResponse> {
        if tools.iter().any(|tool| tool.name == VISIBLE) {
            self.definitions.lock().unwrap().push(tools.to_vec());
        }
        self.script.complete(messages, system, tools).await
    }

    async fn complete_streaming(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        _cancel: CancellationToken,
    ) -> anyhow::Result<mpsc::Receiver<StreamEvent>> {
        let response = self.complete(messages, system, tools).await?;
        let (sender, receiver) = mpsc::channel(1);
        sender.send(StreamEvent::Done(response)).await?;
        Ok(receiver)
    }
}

#[tokio::test]
async fn deferred_catalog_lookup_uses_governed_calls_and_never_executes_a_mixed_batch() {
    use serde_json::json;

    for (requested, mixed) in [(VISIBLE, false), (HIDDEN, false), (VISIBLE, true)] {
        let root = tempfile::tempdir().unwrap();
        let agent = crate::Agent::from_config(test_config()).await.unwrap();
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let definitions = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut lookup = tool_response("lookup-1", LOOKUP, json!({"names": [requested]}));
        lookup.usage = text_response("unused").usage;
        if mixed {
            lookup
                .message
                .content
                .push(crate::llm::ContentBlock::ToolUse {
                    id: "must-not-run".into(),
                    name: VISIBLE.into(),
                    input: json!({}),
                });
        }
        let session = agent
            .build_session(
                root.path().to_string_lossy().to_string(),
                Arc::new(CatalogClient {
                    script: ScriptedClient {
                        seen: Arc::clone(&seen),
                        responses: std::sync::Mutex::new(vec![
                            lookup,
                            text_response("catalog read"),
                        ]),
                    },
                    definitions: Arc::clone(&definitions),
                }),
                &crate::SessionOptions::new()
                    .with_session_id(format!("lookup-{requested}-{mixed}"))
                    .with_planning_mode(crate::PlanningMode::Disabled)
                    .with_tool_presentation_profile(crate::ToolPresentationProfileV1::deferred())
                    .with_permission_policy(
                        crate::permissions::PermissionPolicy::new()
                            .allow(VISIBLE)
                            .deny(HIDDEN),
                    ),
            )
            .unwrap();
        for name in [VISIBLE, HIDDEN] {
            let description = format!(
                "Fixture.\n{name} documentation.\n{}",
                "Full documentation.\n".repeat(8)
            );
            session
                .register_dynamic_tool(Arc::new(FixtureTool(name, description)))
                .unwrap();
        }
        let result = session
            .send("Inspect the tool catalog", None)
            .await
            .unwrap();
        assert_eq!(result.text, "catalog read");
        assert_eq!(result.usage.total_tokens, 12);
        let seen = seen.lock().unwrap();
        let calls: Vec<_> = seen
            .iter()
            .filter(|request| request.tools.iter().any(|tool| tool == VISIBLE))
            .collect();
        assert_eq!(calls.len(), 2);
        assert!(calls[0].tools.iter().any(|name| name == LOOKUP));
        let accepted = requested == VISIBLE && !mixed;
        assert_eq!(calls[1].tools.iter().any(|name| name == LOOKUP), accepted);
        let wire = request_text(&calls[1].messages);
        assert_eq!(wire.contains("in the current tool list"), accepted);
        assert!(!wire.contains("Complete documentation."));
        let definitions = definitions.lock().unwrap();
        assert_eq!(definitions.len(), 2);
        for (index, tools) in definitions.iter().enumerate() {
            assert!(!tools.iter().any(|tool| tool.name == HIDDEN));
            let visible = tools.iter().find(|tool| tool.name == VISIBLE).unwrap();
            assert!(visible
                .description
                .contains("catalog_visible documentation."));
            assert_eq!(
                visible.parameters["properties"]["field_0"]["type"] == "string",
                index == 1
            );
        }
        assert!(session
            .durable_transcript()
            .iter()
            .all(|message| { message.tool_calls().iter().all(|call| call.name != LOOKUP) }));
    }
}
