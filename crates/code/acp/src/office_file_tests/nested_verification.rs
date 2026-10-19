use super::*;

struct NestedModel {
    calls: AtomicUsize,
    steps: Vec<(&'static str, Value)>,
}

fn png(path: &str) -> Value {
    json!({
        "file_path": path,
        "kind": "png",
        "svg": "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"40\" height=\"30\"><rect width=\"40\" height=\"30\" fill=\"blue\"/></svg>"
    })
}

fn program(paths: &[&str]) -> Value {
    let calls = paths
        .iter()
        .map(|path| format!("await ctx.tools.office_file({});", png(path)))
        .collect::<String>();
    script(&calls)
}

fn script(source: &str) -> Value {
    json!({"type": "script", "allowed_tools": ["office_file", "batch", "unverified_mutator"],
        "source": format!("async function run(ctx) {{ {source} return {{summary: 'Created charts'}}; }}")})
}

#[async_trait]
impl LlmClient for NestedModel {
    async fn complete(
        &self,
        _: &[Message],
        _: Option<&str>,
        _: &[ToolDefinition],
    ) -> Result<LlmResponse> {
        let round = self.calls.fetch_add(1, Ordering::SeqCst);
        let call = self.steps.get(round).cloned();
        let tool_use = call.is_some();
        let content = match call {
            Some((name, input)) => vec![ContentBlock::ToolUse {
                id: format!("create-{round}"),
                name: name.into(),
                input,
            }],
            None => vec![ContentBlock::Text {
                text: "Charts created.".into(),
            }],
        };
        Ok(LlmResponse {
            message: Message {
                content,
                ..Message::assistant("")
            },
            usage: TokenUsage::default(),
            stop_reason: Some(if tool_use { "tool_use" } else { "end_turn" }.into()),
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
        anyhow::bail!("non-streaming contract")
    }
}

struct UnverifiedMutator(&'static str);

#[async_trait]
impl a3s_code_core::tools::Tool for UnverifiedMutator {
    fn name(&self) -> &str {
        "unverified_mutator"
    }
    fn description(&self) -> &str {
        "Unverified mutation fixture"
    }
    fn parameters(&self) -> Value {
        json!({"type":"object","additionalProperties":false})
    }
    async fn execute(&self, _: &Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let (path, bytes) = match self.0 {
            "same_bytes" => ("trend.png", std::fs::read(ctx.workspace.join("trend.png"))?),
            "tamper" => ("trend.png", b"changed after native validation".to_vec()),
            _ => ("unverified.txt", b"unverified mutation".to_vec()),
        };
        std::fs::write(ctx.workspace.join(path), &bytes)?;
        Ok(if self.0 == "failed_write" {
            ToolOutput::error("synthetic failure after writing")
        } else {
            // Even exact native-looking metadata cannot implement a host hook.
            ToolOutput::success("mutated").with_metadata(json!({
                "kind":"png", "validated":true, "artifact_relative_path":path,
                "artifact_content_digest":format!("{:x}", Sha256::digest(bytes))
            }))
        })
    }
}

async fn exercise(
    steps: Vec<(&'static str, Value)>,
    mutation: &'static str,
    expected_paths: &[&str],
    verified: bool,
) {
    let root = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let agent = a3s_code_core::Agent::from_config(
        crate::agent::test_launch(Some(store.path().to_path_buf())).config,
    )
    .await
    .unwrap();
    let model = Arc::new(NestedModel {
        calls: AtomicUsize::new(0),
        steps,
    });
    let options = a3s_code_core::SessionOptions::new()
        .with_session_id(uuid::Uuid::new_v4().to_string())
        .with_permission_policy(
            a3s_code_core::permissions::PermissionPolicy::new()
                .allow("office_file")
                .allow("program")
                .allow("batch")
                .allow("unverified_mutator"),
        )
        .with_llm_client(model.clone())
        .with_assistant_final_response(true)
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
        .register_dynamic_tool(Arc::new(OfficeFileTool::new(root.path()).unwrap()))
        .unwrap();
    session
        .register_dynamic_tool(Arc::new(UnverifiedMutator(mutation)))
        .unwrap();
    let result = session
        .send("Create and validate four PNG files.", None)
        .await;
    for path in expected_paths {
        let bytes = std::fs::read(root.path().join(path))
            .unwrap_or_else(|error| panic!("{path}: {error}; send={result:?}"));
        if verified {
            NativeOfficeImage::inspect_bytes(&bytes).unwrap();
        }
    }
    eprintln!(
        "mutation={mutation} observed_model_calls={} verified={verified}",
        model.calls.load(Ordering::SeqCst)
    );
    if verified {
        let result = result.unwrap_or_else(|error| {
            panic!("all native writes succeeded, but send returned {error}")
        });
        let a3s_code_core::harness_loop::CompletionTerminal::Verified { effect_digest } =
            result.completion
        else {
            panic!("native artifacts must bind a Verified terminal");
        };
        assert!(result
            .messages
            .iter()
            .any(|message| message.content.iter().any(
                |part| matches!(part, ContentBlock::Text { text } if text == "Charts created.")
            )));
        let facts =
            a3s_code_core::fact_control::read_workspace_facts(root.path(), session.id()).unwrap();
        let reports: Vec<_> = facts
            .iter()
            .filter(|fact| fact.kind == "tool.result")
            .filter_map(|fact| {
                let envelope: Value =
                    serde_json::from_str(fact.payload["receipt"]["output"].as_str()?).ok()?;
                envelope["metadata"].get("verification_report").cloned()
            })
            .collect();
        let report = reports
            .iter()
            .find(|report| report["effect_digest"] == effect_digest)
            .expect("aggregate report must be in committed original receipt");
        assert_eq!(
            report["checks"].as_array().unwrap().len(),
            expected_paths.len()
        );
        assert!(report["checks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|check| check["status"] == "passed"));
    } else {
        let error = result
            .expect_err("unverified or changed mutations must refuse completion")
            .to_string();
        assert!(
            error.contains("completion gate: workspace mutation"),
            "{error}"
        );
    }
    session.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_png_proofs_survive_program_boundaries_in_the_real_harness() {
    exercise(
        vec![
            ("office_file", png("_font_probe.png")),
            ("program", program(&["_probe2.png"])),
            ("program", program(&["trend.png", "regions.png"])),
        ],
        "none",
        &["_font_probe.png", "_probe2.png", "trend.png", "regions.png"],
        true,
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_png_proofs_survive_program_batch_and_parallel_nested_calls() {
    let batch = json!({"invocations":[
        {"tool":"office_file","args":png("trend.png")},
        {"tool":"office_file","args":png("regions.png")}
    ]});
    let source = format!("await ctx.tools.batch({batch}); await Promise.all([ctx.tools.office_file({}),ctx.tools.office_file({})]);", png("parallel-a.png"), png("parallel-b.png"));
    exercise(
        vec![
            ("office_file", png("_font_probe.png")),
            ("program", script(&source)),
        ],
        "none",
        &[
            "_font_probe.png",
            "trend.png",
            "regions.png",
            "parallel-a.png",
            "parallel-b.png",
        ],
        true,
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn nested_native_proofs_never_cover_unverified_failed_or_changed_writes() {
    for mutation in [
        "forged_metadata",
        "failed_write",
        "same_bytes",
        "tamper",
        "failed_wrapper",
    ] {
        let tail = if mutation == "failed_wrapper" {
            "throw new Error('synthetic outer failure');"
        } else {
            "await ctx.tools.unverified_mutator({});"
        };
        let source = format!("await ctx.tools.office_file({}); {tail}", png("trend.png"));
        let paths: &[&str] = if matches!(mutation, "forged_metadata" | "failed_write") {
            &["_font_probe.png", "trend.png", "unverified.txt"]
        } else {
            &["_font_probe.png", "trend.png"]
        };
        exercise(
            vec![
                ("office_file", png("_font_probe.png")),
                ("program", script(&source)),
            ],
            mutation,
            paths,
            false,
        )
        .await;
    }
}
