#![cfg(test)]

//! Calls an orchestrator dispatches inside a fact run cross the same gates
//! as the call the model made directly.

use super::*;

struct SideEffect {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl crate::tools::Tool for SideEffect {
    fn name(&self) -> &str {
        "side_effect"
    }

    fn description(&self) -> &str {
        "Count one side effect"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type": "object", "properties": {}})
    }

    async fn execute(
        &self,
        _args: &serde_json::Value,
        _ctx: &crate::tools::ToolContext,
    ) -> anyhow::Result<crate::tools::ToolOutput> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(crate::tools::ToolOutput::success("side-effect-ran"))
    }
}

async fn run_orchestrated(session_id: &str, call: LlmResponse) -> usize {
    let workspace = tempfile::tempdir().unwrap();
    let agent = crate::Agent::from_config(test_config()).await.unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let session = session_for(
        &agent,
        workspace.path(),
        session_id,
        Arc::new(ScriptedClient {
            seen: Arc::new(std::sync::Mutex::new(Vec::new())),
            responses: std::sync::Mutex::new(vec![call, text_response("done")]),
        }),
        &crate::SessionOptions::new().with_permission_policy(
            crate::permissions::PermissionPolicy::new()
                .allow("*")
                .deny("side_effect"),
        ),
    );
    session
        .register_dynamic_tool(Arc::new(SideEffect {
            calls: Arc::clone(&calls),
        }))
        .unwrap();
    session.send("orchestrate", None).await.unwrap();
    calls.load(Ordering::SeqCst)
}

#[tokio::test]
async fn fact_run_batch_nested_call_obeys_the_session_deny() {
    let ran = run_orchestrated(
        "nested-batch-deny",
        tool_response(
            "batch-1",
            "batch",
            serde_json::json!({"invocations": [{"tool": "side_effect", "args": {}}]}),
        ),
    )
    .await;
    assert_eq!(ran, 0, "a denied tool must not run through batch");
}

#[tokio::test]
async fn fact_run_program_nested_call_obeys_the_session_deny() {
    let ran = run_orchestrated(
        "nested-program-deny",
        tool_response(
            "program-1",
            "program",
            serde_json::json!({
                "type": "script",
                "language": "javascript",
                "source": "async function run(ctx) { return await ctx.tool('side_effect', {}); }",
                "allowed_tools": ["side_effect"]
            }),
        ),
    )
    .await;
    assert_eq!(ran, 0, "a denied tool must not run through program");
}

struct ProjectionClient {
    definitions: Arc<std::sync::Mutex<Vec<Vec<ToolDefinition>>>>,
}

#[tokio::test]
async fn fact_run_without_permission_or_confirmation_presents_no_tools() {
    let root = tempfile::tempdir().unwrap();
    let agent = crate::Agent::from_config(test_config()).await.unwrap();
    let definitions = Arc::new(std::sync::Mutex::new(Vec::new()));
    let session = agent
        .build_session(
            root.path().display().to_string(),
            Arc::new(ProjectionClient {
                definitions: Arc::clone(&definitions),
            }),
            &crate::SessionOptions::new()
                .with_session_id("projection-without-governance")
                .with_planning_mode(crate::PlanningMode::Disabled),
        )
        .unwrap();
    assert!(session
        .tool_definitions()
        .iter()
        .any(|tool| tool.name == "Skill"));
    let result = session
        .send("Inspect workspace evidence", None)
        .await
        .unwrap();
    assert_eq!(result.text, "projection captured");
    let definitions = definitions.lock().unwrap();
    assert!(!definitions.is_empty());
    assert!(definitions.iter().all(Vec::is_empty));
}

#[tokio::test]
async fn fact_run_skill_visibility_keeps_execution_permission_and_confirmation_gates() {
    for (mode, expected_visible, expected_body) in [
        ("allow", true, true),
        ("deny", false, false),
        // The fact runtime's existing disabled-HITL contract executes Ask.
        ("ask_without_confirmation", true, true),
        ("ask_with_confirmation", true, true),
        ("confirmation_only", true, true),
    ] {
        let root = tempfile::tempdir().unwrap();
        let agent = crate::Agent::from_config(test_config()).await.unwrap();
        let skills = Arc::new(crate::skills::SkillRegistry::new());
        skills.register_unchecked(Arc::new(crate::skills::Skill {
            name: "visibility-skill".into(),
            description: "Controlled visibility fixture".into(),
            allowed_tools: None,
            disable_model_invocation: false,
            kind: crate::skills::SkillKind::Instruction,
            content: "CONTROLLED-SKILL-BODY-ONLY-AFTER-ADMISSION".into(),
            tags: vec![],
            version: None,
        }));
        let mut options = crate::SessionOptions::new().with_skill_registry(skills);
        if mode != "confirmation_only" {
            let policy = match mode {
                "allow" => crate::permissions::PermissionPolicy::new().allow("Skill"),
                "deny" => crate::permissions::PermissionPolicy::new()
                    .allow("*")
                    .deny("Skill"),
                _ => crate::permissions::PermissionPolicy::new(),
            };
            options = options.with_permission_policy(policy);
        }
        if matches!(mode, "ask_with_confirmation" | "confirmation_only") {
            options =
                options.with_confirmation_manager(Arc::new(crate::hitl::AutoApproveConfirmation));
        }
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let session = session_for(
            &agent,
            root.path(),
            mode,
            Arc::new(ScriptedClient {
                seen: Arc::clone(&seen),
                responses: std::sync::Mutex::new(vec![
                    tool_response(
                        "controlled-skill",
                        "Skill",
                        serde_json::json!({"skill_name":"visibility-skill"}),
                    ),
                    text_response("done"),
                ]),
            }),
            &options,
        );
        session
            .send("Load the controlled skill", None)
            .await
            .unwrap();
        let seen = seen.lock().unwrap();
        assert_eq!(
            seen[0].tools.iter().any(|name| name == "Skill"),
            expected_visible,
            "{mode}"
        );
        assert_eq!(
            seen.iter().any(|request| request_text(&request.messages)
                .contains("CONTROLLED-SKILL-BODY-ONLY-AFTER-ADMISSION")),
            expected_body,
            "{mode}"
        );
    }
}

#[async_trait]
impl LlmClient for ProjectionClient {
    async fn complete(
        &self,
        _: &[Message],
        _: Option<&str>,
        tools: &[ToolDefinition],
    ) -> anyhow::Result<LlmResponse> {
        self.definitions.lock().unwrap().push(tools.to_vec());
        Ok(text_response("projection captured"))
    }
    async fn complete_streaming(
        &self,
        _: &[Message],
        _: Option<&str>,
        _: &[ToolDefinition],
        _: CancellationToken,
    ) -> anyhow::Result<mpsc::Receiver<StreamEvent>> {
        anyhow::bail!("unused")
    }
}

#[tokio::test]
async fn fact_run_provider_projection_retains_admitted_schema_and_profile_identity() {
    use crate::tools::{ToolPresentationModeV1, ToolPresentationProfileV1};
    for mode in [
        ToolPresentationModeV1::Adaptive,
        ToolPresentationModeV1::Code,
    ] {
        let root = tempfile::tempdir().unwrap();
        let agent = crate::Agent::from_config(test_config()).await.unwrap();
        let definitions = Arc::new(std::sync::Mutex::new(Vec::new()));
        let profile = ToolPresentationProfileV1::for_mode(mode);
        let session = agent
            .build_session(
                root.path().display().to_string(),
                Arc::new(ProjectionClient {
                    definitions: Arc::clone(&definitions),
                }),
                &crate::SessionOptions::new()
                    .with_session_id(format!("projection-{}", mode.as_str()))
                    .with_memory(Arc::new(a3s_memory::InMemoryStore::new()))
                    .with_planning_mode(crate::PlanningMode::Disabled)
                    .with_permission_policy(
                        crate::permissions::PermissionPolicy::new()
                            .allow("program(*)")
                            .allow("read(*)")
                            .deny("write(*)"),
                    )
                    .with_tool_presentation_profile(profile.clone()),
            )
            .unwrap();
        let source = session.tool_definitions();
        session
            .send("Inspect workspace evidence", None)
            .await
            .unwrap();
        let presented = definitions.lock().unwrap().first().unwrap().clone();
        assert!(!presented.is_empty());
        for tool in &presented {
            assert_eq!(
                tool.parameters,
                source
                    .iter()
                    .find(|source| source.name == tool.name)
                    .unwrap()
                    .parameters
            );
            assert_ne!(tool.name, "write");
        }
        if mode == ToolPresentationModeV1::Code {
            assert_eq!(presented.len(), 1);
            assert_eq!(presented[0].name, "program");
            assert!(presented[0].description.contains("read("));
            assert!(!presented[0].description.contains("write("));
        }
        let runs = session.runs().await;
        let events = session.run_events(&runs[0].id).await;
        let snapshots = events
            .iter()
            .filter_map(|record| match &record.event {
                crate::AgentEvent::ModelPresentationBound { snapshot } => Some(snapshot),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(!snapshots.is_empty());
        assert!(snapshots.iter().all(|snapshot| snapshot.profile == profile));
        assert!(snapshots
            .iter()
            .any(|snapshot| snapshot.presented_tool_count == presented.len()));
        assert!(snapshots
            .iter()
            .all(|snapshot| !snapshot.snapshot_digest.is_empty()));
    }
}

struct DelegatedProjectionClient {
    parent_calls: AtomicUsize,
    child_tools: Arc<std::sync::Mutex<Vec<Vec<String>>>>,
}

#[async_trait]
impl LlmClient for DelegatedProjectionClient {
    async fn complete(
        &self,
        _: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
    ) -> anyhow::Result<LlmResponse> {
        if system
            .unwrap_or_default()
            .contains("CHILD-CODE-PROFILE-FIXTURE")
        {
            self.child_tools
                .lock()
                .unwrap()
                .push(tools.iter().map(|tool| tool.name.clone()).collect());
            return Ok(text_response("child profile observed"));
        }
        if tools.is_empty() {
            return Ok(text_response("[]"));
        }
        if self.parent_calls.fetch_add(1, Ordering::SeqCst) == 0 {
            assert_eq!(
                tools
                    .iter()
                    .map(|tool| tool.name.as_str())
                    .collect::<Vec<_>>(),
                ["program"]
            );
            return Ok(tool_response(
                "delegate-through-program",
                "program",
                serde_json::json!({
                    "type":"script","allowed_tools":["task"],
                    "source":"async function run(ctx) { return await ctx.tool('task', {agent:'profile-worker',description:'inspect',prompt:'Reply ready without tools.'}); }"
                }),
            ));
        }
        Ok(text_response("done"))
    }
    async fn complete_streaming(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        _: CancellationToken,
    ) -> anyhow::Result<mpsc::Receiver<StreamEvent>> {
        let response = self.complete(messages, system, tools).await?;
        let (sender, receiver) = mpsc::channel(1);
        sender.send(StreamEvent::Done(response)).await.unwrap();
        Ok(receiver)
    }
}

#[tokio::test]
async fn fact_run_delegated_main_turn_inherits_the_code_profile() {
    let root = tempfile::tempdir().unwrap();
    let agent = crate::Agent::from_config(test_config()).await.unwrap();
    let child_tools = Arc::new(std::sync::Mutex::new(Vec::new()));
    let client = Arc::new(DelegatedProjectionClient {
        parent_calls: AtomicUsize::new(0),
        child_tools: Arc::clone(&child_tools),
    });
    let mut child_policy =
        crate::permissions::PermissionPolicy::new().allow_all(&["program", "read"]);
    child_policy.default_decision = crate::permissions::PermissionDecision::Deny;
    let session = agent
        .build_session(
            root.path().display().to_string(),
            client,
            &crate::SessionOptions::new()
                .with_session_id("nested-profile-inheritance")
                .with_memory(Arc::new(a3s_memory::InMemoryStore::new()))
                .with_planning_mode(crate::PlanningMode::Disabled)
                .with_continuation(false)
                .with_auto_delegation_enabled(false)
                .with_manual_delegation_enabled(true)
                .with_permission_policy(
                    crate::permissions::PermissionPolicy::new()
                        .allow_all(&["program", "task", "read"])
                        .deny("write"),
                )
                .with_tool_presentation_profile(crate::tools::ToolPresentationProfileV1::code())
                .with_worker_agent(
                    crate::subagent::WorkerAgentSpec::implementer(
                        "profile-worker",
                        "Inspect controlled evidence",
                    )
                    .with_permissions(child_policy)
                    .with_prompt("CHILD-CODE-PROFILE-FIXTURE"),
                ),
        )
        .unwrap();
    session.send("Delegate the inspection", None).await.unwrap();
    let child_tools = child_tools.lock().unwrap();
    assert!(
        !child_tools.is_empty(),
        "no actual child provider call was observed"
    );
    assert!(child_tools.iter().all(|tools| tools == &["program"]));
}

struct ChildBudget {
    checked: Arc<std::sync::Mutex<Vec<String>>>,
    admitted_children: AtomicUsize,
}

#[async_trait]
impl crate::budget::BudgetGuard for ChildBudget {
    async fn check_before_tool(&self, _: &str, name: &str) -> crate::budget::BudgetDecision {
        self.checked.lock().unwrap().push(name.to_owned());
        if name == "side_effect" && self.admitted_children.fetch_add(1, Ordering::SeqCst) >= 1 {
            crate::budget::BudgetDecision::Deny {
                resource: "nested-calls".into(),
                reason: "one child admitted".into(),
            }
        } else {
            crate::budget::BudgetDecision::Allow
        }
    }
}

#[tokio::test]
async fn fact_run_program_children_share_the_run_budget_and_keep_denied_receipts() {
    let root = tempfile::tempdir().unwrap();
    let agent = crate::Agent::from_config(test_config()).await.unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let checked = Arc::new(std::sync::Mutex::new(Vec::new()));
    let session = session_for(
        &agent,
        root.path(),
        "nested-program-budget",
        Arc::new(ScriptedClient {
            seen: Arc::new(std::sync::Mutex::new(Vec::new())),
            responses: std::sync::Mutex::new(vec![
                tool_response(
                    "program-budget",
                    "program",
                    serde_json::json!({
                        "type":"script", "allowed_tools":["side_effect"],
                        "source":"async function run(ctx) { const first = await ctx.tool('side_effect', {}); const denied = await ctx.tool('side_effect', {}); return {first:first.exitCode,denied:denied.exitCode}; }"
                    }),
                ),
                text_response("done"),
            ]),
        }),
        &crate::SessionOptions::new()
            .with_permission_policy(crate::permissions::PermissionPolicy::new().allow("*"))
            .with_budget_guard(Arc::new(ChildBudget {
                checked: Arc::clone(&checked),
                admitted_children: AtomicUsize::new(0),
            })),
    );
    session
        .register_dynamic_tool(Arc::new(SideEffect {
            calls: Arc::clone(&calls),
        }))
        .unwrap();
    let _ = session.send("Run the bounded orchestration", None).await;
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        checked
            .lock()
            .unwrap()
            .iter()
            .filter(|name| name.as_str() == "side_effect")
            .count(),
        2
    );
    let runs = session.runs().await;
    assert!(runs[0].status.is_terminal());
    let events = session.run_events(&runs[0].id).await;
    // The fact Run retains nested receipts on its original program result;
    // public child events are a separate projection and may be absent.
    let receipts = events
        .iter()
        .find_map(|event| match &event.event {
            crate::AgentEvent::ToolEnd {
                name,
                metadata: Some(metadata),
                ..
            } if name == "program" => metadata
                .pointer("/program/tool_calls")
                .and_then(serde_json::Value::as_array),
            _ => None,
        })
        .unwrap();
    assert_eq!(receipts.len(), 2);
    assert_eq!(receipts[0]["exit_code"], 0);
    assert_ne!(receipts[1]["exit_code"], 0);
}

struct CancellableChild {
    started: Arc<tokio::sync::Notify>,
    settled: Arc<AtomicUsize>,
}

#[async_trait]
impl crate::tools::Tool for CancellableChild {
    fn name(&self) -> &str {
        "cancellable_child"
    }
    fn description(&self) -> &str {
        "Wait for the original Run cancellation"
    }
    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type":"object"})
    }
    async fn execute(
        &self,
        _: &serde_json::Value,
        ctx: &crate::tools::ToolContext,
    ) -> anyhow::Result<crate::tools::ToolOutput> {
        self.started.notify_one();
        ctx.cancellation_token().cancelled().await;
        self.settled.fetch_add(1, Ordering::SeqCst);
        Ok(crate::tools::ToolOutput::error("cancelled child settled"))
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn fact_run_cancellation_settles_program_children_on_the_original_run() {
    let root = tempfile::tempdir().unwrap();
    let agent = crate::Agent::from_config(test_config()).await.unwrap();
    let started = Arc::new(tokio::sync::Notify::new());
    let settled = Arc::new(AtomicUsize::new(0));
    let session = Arc::new(session_for(
        &agent,
        root.path(),
        "nested-program-cancel",
        Arc::new(ScriptedClient {
            seen: Arc::new(std::sync::Mutex::new(Vec::new())),
            responses: std::sync::Mutex::new(vec![
                tool_response(
                    "program-cancel",
                    "program",
                    serde_json::json!({
                        "type":"script", "allowed_tools":["cancellable_child"], "limits":{"timeoutMs":30_000},
                        "source":"async function run(ctx) { return await ctx.tool('cancellable_child', {}); }"
                    }),
                ),
                text_response("done"),
            ]),
        }),
        &crate::SessionOptions::new()
            .with_permission_policy(crate::permissions::PermissionPolicy::new().allow("*")),
    ));
    session
        .register_dynamic_tool(Arc::new(CancellableChild {
            started: Arc::clone(&started),
            settled: Arc::clone(&settled),
        }))
        .unwrap();
    let running = Arc::clone(&session);
    let send =
        tokio::spawn(async move { running.send("Run cancellable orchestration", None).await });
    tokio::time::timeout(std::time::Duration::from_secs(5), started.notified())
        .await
        .unwrap();
    let runs = session.runs().await;
    assert!(session.cancel_run(&runs[0].id).await);
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), send)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(settled.load(Ordering::SeqCst), 1);
    assert_eq!(
        session.run_snapshot(&runs[0].id).await.unwrap().status,
        crate::RunStatus::Cancelled
    );
    let events = session.run_events(&runs[0].id).await;
    assert!(events.iter().any(|event| matches!(&event.event, crate::AgentEvent::ToolEnd {name,exit_code,..} if name == "program" && *exit_code != 0)));
}
