use super::*;

#[test]
fn prompt_boundary_default_accepts_source_analysis_without_studio_recipes() {
    let options = AsterCodeAgent::core_session_options("prompt-boundary", "controlled/model", None);
    let prompt = options.prompt_slots.unwrap().build();
    assert!(prompt.contains("explanation, diagnosis, review or modification"));
    for domain in [
        "office_read",
        "date_span",
        "sequence_changes",
        "checkValidity",
    ] {
        assert!(!prompt.contains(domain), "default ACP advertises {domain}");
    }
}

#[test]
fn prompt_boundary_office_guidance_follows_admitted_host_scope() {
    for (office_tools, source_review) in [(false, false), (true, false), (true, true)] {
        let host = crate::host_context::StudioHost {
            office_tools,
            source_review,
            ..Default::default()
        };
        let prompt =
            crate::host_context::merge_studio_host(aster_core::SystemPromptSlots::default(), &host)
                .build();
        assert_eq!(
            prompt.contains("office_read"),
            office_tools && !source_review
        );
        assert_eq!(
            prompt.contains("sequence_changes"),
            office_tools && !source_review
        );
    }
}

#[test]
fn prompt_boundary_engineering_distinguishes_dispatcher_from_reviewer() {
    let policy = crate::engineering::named_policy("engineering-v1").unwrap();
    assert!(policy.instructions().contains("primary session"));
    assert!(policy.instructions().contains("delegated reviewer"));
    assert!(policy
        .instructions()
        .contains("inspect the target directly"));
    assert!(policy.instructions().contains("Do not delegate again"));
}

#[test]
fn prompt_boundary_high_effort_does_not_expand_acceptance_scope() {
    for effort in ["low", "high", "xhigh", "max"] {
        let options = AsterCodeAgent::core_session_options(
            "prompt-effort",
            "controlled/model",
            effort_limits(effort),
        );
        let prompt = options.prompt_slots.unwrap().build();
        assert!(prompt.contains("maximum, not a quota"));
        assert!(prompt.contains("Do not expand the task"), "{effort}");
        assert!(!prompt.contains("weigh at least one alternative"));
        assert!(!prompt.contains("Verify exhaustively"));
    }
}

#[test]
fn prompt_boundary_terminal_preserves_artifact_and_permission_contracts() {
    let options = AsterCodeAgent::core_session_options("prompt-terminal", "controlled/model", None);
    let mut slots = options.prompt_slots.unwrap();
    append_slot_extra(&mut slots, TERMINAL_MARKDOWN_PRESENTATION);
    let prompt = slots.build();
    assert!(prompt.contains("not browser HTML"));
    assert!(prompt.contains("standalone UTF-8 .html artifact"));
    assert!(prompt.contains("requested path"));
    assert!(prompt.contains("## Runtime Contract"));
    assert!(!prompt.contains("office_read"));
}

#[tokio::test]
#[ignore = "requires explicit ASTER_CONFIG_FILE declaring live glm-02/glm-5.3"]
async fn prompt_boundary_real_model_preserves_source_and_uses_review_worker() {
    use acp::Agent as _;

    const SOURCE: &str = "pub const MAX_LINES: usize = 3;\npub fn first_lines(lines: &[&str]) -> Vec<String> {\n    (0..=MAX_LINES).map(|i| lines[i].to_string()).collect()\n}\n";
    for (case, prompt, terminal, delegated) in [
        ("direct", "Reply with exactly READY and no other text. Do not call tools.", true, false),
        ("explain", "Read src/limit.rs and explain what first_lines actually returns and what happens for short inputs. Do not change files. Answer in English.", true, false),
        ("review", "Review src/limit.rs for correctness. Do not change files. Give concrete findings with file and line evidence. Answer in English.", false, true),
    ] {
        let root = tempfile::Builder::new().prefix("prompt bounds Δ ").tempdir().unwrap();
        std::fs::create_dir(root.path().join("src")).unwrap();
        let source = root.path().join("src/limit.rs");
        std::fs::write(&source, SOURCE).unwrap();
        let config = std::env::var_os("ASTER_CONFIG_FILE").expect("explicit live ACL");
        let mut launch = crate::config::load_launch_config(root.path().into(), Some(config.into())).unwrap();
        launch.model_id = "glm-02/glm-5.3".into();
        launch.config.default_model = Some(launch.model_id.clone());
        launch.config.sessions_dir = Some(root.path().join("store"));
        let host = AsterCodeAgent::new(launch);
        if terminal {
            host.client_terminal_markdown.set(true);
        }
        let id = host.new_session(acp::NewSessionRequest::new(root.path())).await.unwrap().session_id;
        let started = std::time::Instant::now();
        eprintln!("PROMPT_CASE_BEGIN {case} model=glm-02/glm-5.3 prompt={prompt:?}");
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(300),
            host.prompt(acp::PromptRequest::new(id.clone(), vec![acp::ContentBlock::Text(acp::TextContent::new(prompt))])),
        ).await.expect("bounded live case");
        let session = host.sessions.lock().await.get(id.0.as_ref()).unwrap().session.clone();
        let answer = session.durable_transcript().iter().rev().find(|message| message.role == "assistant").map(|message| message.text()).unwrap_or_default();
        let mut tools = Vec::new();
        for run in session.runs().await {
            for record in session.run_events(&run.id).await {
                if let AgentEvent::ToolExecutionStart { name, .. } = record.event {
                    tools.push(name);
                }
            }
        }
        eprintln!("PROMPT_CASE_RESULT {case} elapsed_ms={} usage={:?} tools={tools:?} result={result:?} answer={answer:?}", started.elapsed().as_millis(), session.recorded_usage_tokens());
        assert_eq!(std::fs::read_to_string(&source).unwrap(), SOURCE, "{case} changed source");
        assert_eq!(result.unwrap().stop_reason, acp::StopReason::EndTurn);
        if case == "direct" {
            assert_eq!(answer.trim(), "READY");
            assert!(tools.is_empty(), "direct answer called tools: {tools:?}");
        } else {
            assert!(tools.iter().any(|name| name == "read" || name == "search"), "source not inspected: {tools:?}");
            assert!(answer.contains("MAX_LINES") || answer.contains("first_lines"), "{answer}");
            assert!(answer.to_ascii_lowercase().contains("panic") || answer.to_ascii_lowercase().contains("out of bounds"), "{answer}");
        }
        if delegated {
            assert!(tools.iter().any(|name| name == "task"), "review not delegated: {tools:?}");
        }
        session.close().await;
    }
}

#[tokio::test]
#[ignore = "requires explicit live ACL, glm-02/glm-5.3, Python and configured native sandbox"]
async fn prompt_boundary_real_model_repairs_and_passes_host_checker() {
    use acp::Agent as _;

    let root = tempfile::Builder::new()
        .prefix("prompt repair Δ ")
        .tempdir()
        .unwrap();
    const CHECK: &str = "from limit import first_lines\nassert first_lines([]) == []\nassert first_lines([1,2]) == [1,2]\nassert first_lines([1,2,3,4,5]) == [1,2,3]\nprint('checks passed')\n";
    std::fs::write(
        root.path().join("limit.py"),
        "def first_lines(lines):\n    return list(lines[:4])\n",
    )
    .unwrap();
    std::fs::write(root.path().join("check.py"), CHECK).unwrap();
    let baseline = std::process::Command::new("python3")
        .arg("check.py")
        .current_dir(root.path())
        .output()
        .unwrap();
    assert!(!baseline.status.success(), "fixture must expose the defect");
    let config = std::env::var_os("ASTER_CONFIG_FILE").expect("explicit live ACL");
    let mut launch =
        crate::config::load_launch_config(root.path().into(), Some(config.into())).unwrap();
    launch.model_id = "glm-02/glm-5.3".into();
    launch.config.default_model = Some(launch.model_id.clone());
    launch.config.sessions_dir = Some(root.path().join("store"));
    launch.config.verification_contracts =
        vec![aster_core::config::WorkspaceVerificationContract {
            workspace: root.path().into(),
            contract: aster_core::verification::VerificationContractV1::new(
                "prompt-repair",
                "python3 check.py",
                ["limit.py"],
            )
            .unwrap(),
        }];
    let host = AsterCodeAgent::new(launch);
    let meta = serde_json::json!({"permissionPosture":"yolo"})
        .as_object()
        .unwrap()
        .clone();
    let id = host
        .new_session(acp::NewSessionRequest::new(root.path()).meta(meta))
        .await
        .unwrap()
        .session_id;
    const PROMPT: &str = "Fix limit.py so first_lines returns at most the first three inputs, preserving order and accepting empty and short inputs. Inspect and run the existing python3 check.py checker. Do not modify check.py, install dependencies, add files or change unrelated behavior. Finish after the required checker passes.";
    let started = std::time::Instant::now();
    eprintln!("PROMPT_CASE_BEGIN repair model=glm-02/glm-5.3 prompt={PROMPT:?}");
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(420),
        host.prompt(acp::PromptRequest::new(
            id.clone(),
            vec![acp::ContentBlock::Text(acp::TextContent::new(PROMPT))],
        )),
    )
    .await
    .expect("bounded live repair");
    let checked = std::process::Command::new("python3")
        .arg("check.py")
        .current_dir(root.path())
        .output()
        .unwrap();
    let session = host
        .sessions
        .lock()
        .await
        .get(id.0.as_ref())
        .unwrap()
        .session
        .clone();
    eprintln!("PROMPT_CASE_RESULT repair elapsed_ms={} usage={:?} result={result:?} independent_check={:?}", started.elapsed().as_millis(), session.recorded_usage_tokens(), checked.status);
    assert_eq!(
        std::fs::read_to_string(root.path().join("check.py")).unwrap(),
        CHECK
    );
    assert!(
        checked.status.success(),
        "independent checker: {}",
        String::from_utf8_lossy(&checked.stderr)
    );
    assert_eq!(result.unwrap().stop_reason, acp::StopReason::EndTurn);
    session.close().await;
}
