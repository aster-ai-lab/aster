#![cfg(test)]

use super::*;
use a3s_code_core::llm::{
    ContentBlock, LlmClient, LlmResponse, Message, StreamEvent, TokenUsage, ToolDefinition,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

struct Model {
    calls: AtomicUsize,
}

fn successful_tool_json(text: &str) -> Value {
    let payload = text
        .strip_prefix("Tool exit code: 0\n")
        .expect("successful tool result must expose its zero exit status");
    serde_json::from_str(payload).expect("tool payload must remain JSON")
}

#[async_trait]
impl LlmClient for Model {
    async fn complete(
        &self,
        _: &[Message],
        _: Option<&str>,
        _: &[ToolDefinition],
    ) -> Result<LlmResponse> {
        let first = self.calls.fetch_add(1, Ordering::SeqCst) == 0;
        Ok(LlmResponse {
            message: Message {
                content: if first {
                    vec![ContentBlock::ToolUse {
                        id: "read-1".into(),
                        name: "office_read".into(),
                        input: json!({"file_path":"source.xlsx"}),
                    }]
                } else {
                    vec![ContentBlock::Text {
                        text: "Reported actual source read result.".into(),
                    }]
                },
                ..Message::assistant("")
            },
            usage: TokenUsage::default(),
            stop_reason: Some(if first { "tool_use" } else { "end_turn" }.into()),
            token_logprobs: vec![],
            meta: None,
        })
    }
    async fn complete_streaming(
        &self,
        _: &[Message],
        _: Option<&str>,
        _: &[ToolDefinition],
        _: tokio_util::sync::CancellationToken,
    ) -> Result<tokio::sync::mpsc::Receiver<StreamEvent>> {
        bail!("non-streaming fixture")
    }
}

#[tokio::test]
async fn live_tool_turn_reads_only_with_read_authority_and_permission() {
    let root = tempfile::tempdir().unwrap();
    workbook(root.path()).await;
    let store = tempfile::tempdir().unwrap();
    let agent = a3s_code_core::Agent::from_config(
        crate::agent::test_launch(Some(store.path().to_path_buf())).config,
    )
    .await
    .unwrap();
    for (read_allowed, permission_allowed) in [(true, true), (false, true), (true, false)] {
        let base = a3s_code_core::workspace::WorkspaceServices::local(root.path());
        let services = if read_allowed {
            base.clone()
        } else {
            a3s_code_core::workspace::WorkspaceServicesBuilder::new(
                base.workspace_ref().clone(),
                base.fs(),
            )
            .capabilities(a3s_code_core::workspace::WorkspaceCapabilities {
                read: read_allowed,
                ..base.capabilities()
            })
            .build()
        };
        let policy = if permission_allowed {
            a3s_code_core::permissions::PermissionPolicy::new().allow("office_read")
        } else {
            a3s_code_core::permissions::PermissionPolicy::new().deny("office_read")
        };
        let options = a3s_code_core::SessionOptions::new()
            .with_session_id(uuid::Uuid::new_v4().to_string())
            .with_workspace_backend(services)
            .with_permission_policy(policy)
            .with_llm_client(Arc::new(Model {
                calls: AtomicUsize::new(0),
            }))
            .with_planning_mode(a3s_code_core::PlanningMode::Disabled)
            .with_harness(
                a3s_code_core::HarnessComposeOptions::compose(
                    ["system", "tools", "budget", "compact", "infer"]
                        .into_iter()
                        .map(str::to_owned)
                        .collect(),
                    None,
                    None,
                    vec![],
                )
                .unwrap(),
            );
        let session = agent
            .session_async(root.path().to_string_lossy().into_owned(), Some(options))
            .await
            .unwrap();
        session
            .register_dynamic_tool(Arc::new(OfficeReadTool::new(root.path()).unwrap()))
            .unwrap();
        let result = session
            .send("Read the persisted workbook.", None)
            .await
            .unwrap();
        let (ok, content) = result
            .messages
            .iter()
            .flat_map(|message| &message.content)
            .find_map(|block| match block {
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                    ..
                } if tool_use_id == "read-1" => {
                    Some((!is_error.unwrap_or(false), content.as_text()))
                }
                _ => None,
            })
            .expect("actual governed tool receipt");
        assert_eq!(ok, read_allowed && permission_allowed, "{content}");
        if ok {
            let value = crate::test_support::successful_tool_json(&content);
            assert_eq!(value["kind"], "xlsx");
            assert!(value["entries"]
                .as_array()
                .unwrap()
                .iter()
                .any(|entry| entry["value"] == 12000));
        } else {
            assert!(!content.contains("12000"));
        }
        session.close().await;
    }
}
