#![cfg(test)]

use super::{AsterCodeAgent, SessionSetup};
use aster_core::{sandbox::SandboxNetworkMode, CodeConfig};
use agent_client_protocol::{self as acp, Agent as _};

#[tokio::test]
async fn knowledge_compile_modes_keep_the_candidate_only_ceiling() {
    use serde_json::json;
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("wiki/local");
    std::fs::create_dir_all(&root).unwrap();
    let root = root.canonicalize().unwrap();
    std::fs::write(root.join("source.md"), "Frozen evidence.").unwrap();
    let job =
        aster_okf::begin_knowledge_maintenance(&root, &[], aster_okf::MaintenanceOperation::Ingest)
            .unwrap();
    assert_compile_ceiling(
        directory.path(),
        &job,
        json!({"root":root,"jobId":job.id}),
        "source.md",
        || {
            aster_okf::cancel_knowledge_maintenance(&root, &job.id).unwrap();
        },
    )
    .await;
    assert_eq!(
        std::fs::read_to_string(root.join("source.md")).unwrap(),
        "Frozen evidence."
    );
}

#[tokio::test]
async fn workspace_wiki_compile_modes_keep_the_candidate_only_ceiling() {
    use serde_json::json;
    let directory = tempfile::tempdir().unwrap();
    let source = b"pub fn answer() -> u8 { 42 }\n";
    std::fs::create_dir(directory.path().join("src")).unwrap();
    std::fs::write(directory.path().join("src/lib.rs"), source).unwrap();
    let input = aster_okf::WorkspaceKnowledgeInput::new(
        &directory.path().canonicalize().unwrap(),
        "tenant:permission:binding",
        &format!("sha256:{}", "a".repeat(64)),
        vec![aster_okf::WorkspaceKnowledgeSource::text("src/lib.rs", source).unwrap()],
    )
    .unwrap();
    let job = aster_okf::begin_workspace_knowledge_maintenance(&input)
        .unwrap()
        .unwrap();
    assert_compile_ceiling(
        directory.path(),
        &job,
        json!({"root":input.output_root(),"jobId":job.id,"workspaceRevision":input.binding_revision()}),
        &job.sources[0],
        || { aster_okf::cancel_workspace_knowledge_maintenance(&input, &job.id).unwrap(); },
    )
    .await;
    assert_eq!(
        std::fs::read(directory.path().join("src/lib.rs")).unwrap(),
        source
    );
    assert!(!input.output_root().join("concepts/a.md").exists());
}

async fn assert_compile_ceiling(
    directory: &std::path::Path,
    job: &aster_okf::KnowledgeJob,
    scope: serde_json::Value,
    source: &str,
    stop: impl FnOnce(),
) {
    use serde_json::json;
    let frozen = std::fs::read(std::path::Path::new(&job.staging).join(source)).unwrap();
    let mut launch = super::tests::test_launch(Some(directory.join("sessions")));
    launch.workspace = job.staging.clone().into();
    let ambient_memory = directory.join("ambient-memory");
    launch.config.memory_dir = Some(ambient_memory.clone());
    let agent = AsterCodeAgent::new(launch);
    let setup = SessionSetup {
        workspace: job.staging.clone().into(),
        model_id: "fixture/model",
        effort: None,
        host: crate::host_context::StudioHost {
            knowledge_compile: Some(serde_json::from_value(scope.clone()).unwrap()),
            ..Default::default()
        },
        components: &[],
        mcp_servers: Vec::new(),
    };
    let options = agent
        .session_options_for("compile-attestation", &setup, &directory.join("sessions"))
        .await
        .unwrap();
    let attestor = options
        .completion_attestor
        .expect("candidate completion needs host byte verification");
    let candidate = std::path::Path::new(&job.staging).join("concepts/attested.md");
    std::fs::write(&candidate, b"Candidate evidence.").unwrap();
    let records = [aster_core::completion_attestor::MutatedPathRecord {
        path: "concepts/attested.md".into(),
        content_digest: aster_okf::content_generation(b"Candidate evidence."),
    }];
    let report = attestor.attest("fixture-effect-digest", &records).unwrap();
    assert_eq!(
        report.effect_digest.as_deref(),
        Some("fixture-effect-digest")
    );
    assert!(report
        .checks
        .iter()
        .all(|check| check.status == aster_core::verification::VerificationStatus::Passed));
    std::fs::write(&candidate, b"Changed after the effect receipt.").unwrap();
    assert!(attestor.attest("fixture-effect-digest", &records).is_none());
    let opened = agent
        .new_session(
            acp::NewSessionRequest::new(&job.staging).meta(Some(
                [("asterHost".into(), json!({"knowledgeCompile":scope}))]
                    .into_iter()
                    .collect(),
            )),
        )
        .await
        .unwrap();
    let session = agent
        .sessions
        .lock()
        .await
        .get(opened.session_id.0.as_ref())
        .unwrap()
        .session
        .clone();
    assert!(session
        .memory()
        .unwrap()
        .recall_similar("ambient sentinel", 5)
        .await
        .unwrap()
        .is_empty());
    assert!(!ambient_memory.exists());
    for mode in ["default", "plan", "auto", "yolo"] {
        agent
            .set_session_mode(acp::SetSessionModeRequest::new(
                opened.session_id.clone(),
                mode,
            ))
            .await
            .unwrap();
        let read = session
            .governed_tool("read", json!({"file_path":source}))
            .await
            .unwrap();
        assert_eq!(read.exit_code, 0, "{mode}: {read:?}");
        let output = session
            .governed_tool(
                "write",
                json!({"file_path":"concepts/a.md","content":"Candidate evidence."}),
            )
            .await
            .unwrap();
        assert_eq!(output.exit_code, 0, "{mode}: {output:?}");
        for (tool, args) in [
            ("write", json!({"file_path":source,"content":"Changed"})),
            ("bash", json!({"command":"true"})),
        ] {
            let refused = session.governed_tool(tool, args).await.unwrap();
            assert_ne!(refused.exit_code, 0, "{mode}: {refused:?}");
        }
        assert_eq!(
            std::fs::read(std::path::Path::new(&job.staging).join(source)).unwrap(),
            frozen
        );
    }
    let written = session
        .governed_tool(
            "batch",
            json!({"invocations":[
                {"tool":"write","args":{"file_path":"concepts/b.md","content":"Batch candidate."}},
                {"tool":"write","args":{"file_path":".wiki-changes.json","content":"{}"}}
            ]}),
        )
        .await
        .unwrap();
    assert_eq!(written.exit_code, 0, "{written:?}");
    let aliased = session.governed_tool("batch", json!({"invocations":[
        {"tool":"functions.write","args":{"file_path":"concepts/alias.md","content":"Aliased candidate"}},
        {"tool":"functions.write","args":{"file_path":".wiki-changes.json","content":"{}"}}
    ]})).await.unwrap();
    assert_eq!(aliased.exit_code, 0, "{aliased:?}");
    assert_eq!(
        std::fs::read_to_string(std::path::Path::new(&job.staging).join("concepts/alias.md"))
            .unwrap(),
        "Aliased candidate"
    );
    for member in [
        json!({"tool":"functions.write","args":{"file_path":source,"content":"Changed"}}),
        json!({"tool":"functions.bash","args":{"command":"true"}}),
        json!({"tool":"functions.batch","args":{"invocations":[{"tool":"write","args":{"file_path":"concepts/nested.md","content":"Nested"}}]}}),
    ] {
        let refused = session.governed_tool("batch", json!({"invocations":[
            {"tool":"functions.write","args":{"file_path":"concepts/alias-must-not-exist.md","content":"Changed"}},
            member
        ]})).await.unwrap();
        assert_ne!(refused.exit_code, 0, "{refused:?}");
        assert!(!std::path::Path::new(&job.staging)
            .join("concepts/alias-must-not-exist.md")
            .exists());
    }
    let refused = session.governed_tool("batch", json!({"invocations":[
        {"tool":"write","args":{"file_path":"concepts/should-not-exist.md","content":"Candidate"}},
        {"tool":"write","args":{"file_path":source,"content":"Changed"}}
    ]})).await.unwrap();
    assert_ne!(refused.exit_code, 0, "{refused:?}");
    assert!(!std::path::Path::new(&job.staging)
        .join("concepts/should-not-exist.md")
        .exists());
    stop();
    let refused = session
        .governed_tool(
            "write",
            json!({"file_path":"concepts/after-stop.md","content":"Late candidate"}),
        )
        .await
        .unwrap();
    assert_ne!(refused.exit_code, 0, "{refused:?}");
    assert!(!std::path::Path::new(&job.staging)
        .join("concepts/after-stop.md")
        .exists());
    session.close().await;
}

#[test]
fn empty_launch_model_is_omitted_and_named() {
    let empty = AsterCodeAgent::core_session_options("session", "  ", None);
    assert!(empty.model.is_none());
    let set = AsterCodeAgent::core_session_options("session", "mock/gpt", None);
    assert_eq!(set.model.as_deref(), Some("mock/gpt"));
    assert_eq!(
        super::configured_model_id("").unwrap_err(),
        "no model is configured; open /config and save ~/.aster/config.acl, then start aster again"
    );
    assert_eq!(
        super::configured_model_id(" mock/gpt ").unwrap(),
        "mock/gpt"
    );
}

#[tokio::test]
async fn blank_launch_model_session_and_prompt_use_the_no_model_sentence() {
    let root = tempfile::tempdir().unwrap();
    let mut launch = super::tests::test_launch(Some(root.path().join("sessions")));
    launch.workspace = root.path().to_path_buf();
    launch.model_id.clear();
    let agent = AsterCodeAgent::new(launch);
    let created = agent
        .new_session(acp::NewSessionRequest::new(root.path()))
        .await
        .expect_err("a blank launch model refuses session creation");
    assert_eq!(
        created.data,
        Some(serde_json::json!(super::NO_MODEL_CONFIGURED))
    );
    assert!(
        !format!("{created:?}").contains("must use 'provider/model' format"),
        "{created:?}"
    );

    *agent.model_id.lock().await = "anthropic/claude-sonnet-4-20250514".to_string();
    let opened = agent
        .new_session(acp::NewSessionRequest::new(root.path()))
        .await
        .expect("a provider/model id opens a session");
    *agent.model_id.lock().await = "  ".to_string();
    let prompted = agent
        .prompt(acp::PromptRequest::new(
            opened.session_id,
            vec![acp::ContentBlock::Text(acp::TextContent::new("hello"))],
        ))
        .await
        .expect_err("a blank model refuses the prompt");
    assert_eq!(
        prompted.data,
        Some(serde_json::json!(super::NO_MODEL_CONFIGURED))
    );
    assert!(
        !format!("{prompted:?}").contains("must use 'provider/model' format"),
        "{prompted:?}"
    );
}

#[tokio::test]
async fn initialize_refuses_invalid_prompt_metadata_instead_of_silently_ignoring_it() {
    for (key, value) in [
        ("rules", serde_json::json!(42)),
        ("rules", serde_json::Value::Null),
        ("systemPromptOverride", serde_json::json!(["not a string"])),
        (
            "systemPromptOverride",
            serde_json::json!("Replace the whole system prompt"),
        ),
    ] {
        let agent = AsterCodeAgent::new(super::tests::test_launch(None));
        let error = agent
            .initialize(
                acp::InitializeRequest::new(acp::ProtocolVersion::V1)
                    .meta(Some([(key.into(), value)].into_iter().collect())),
            )
            .await
            .expect_err("unsupported prompt metadata must be explicit");
        assert!(format!("{error:?}").contains(key));
    }
}

#[tokio::test]
async fn initialize_rules_append_without_replacing_host_or_unattended_contracts() {
    for unattended in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let mut launch = super::tests::test_launch(None);
        launch.workspace = root.path().into();
        let mut agent = AsterCodeAgent::new(launch);
        if unattended {
            agent =
                agent.with_unattended(crate::unattended::Unattended::new(10_000, None).unwrap());
        }
        agent
            .initialize(
                acp::InitializeRequest::new(acp::ProtocolVersion::V1).meta(Some(
                    [
                        ("rules".into(), serde_json::json!("保留原文 APPEND_RULE_42")),
                        ("systemPromptOverride".into(), serde_json::json!(" \n")),
                    ]
                    .into_iter()
                    .collect(),
                )),
            )
            .await
            .unwrap();
        let setup = SessionSetup {
            workspace: root.path().into(),
            model_id: "fixture/model",
            effort: None,
            host: crate::host_context::StudioHost {
                guidelines: "Standing host instruction".into(),
                source_review: true,
                ..Default::default()
            },
            components: &[],
            mcp_servers: Vec::new(),
        };
        let options = agent
            .session_options_for("rules", &setup, &root.path().join("store"))
            .await
            .unwrap();
        assert!(
            options.permission_policy.is_some(),
            "review restrictions stay installed"
        );
        assert!(options.isolated_context);
        assert!(options.hook_executor.is_none());
        let prompt = options.prompt_slots.unwrap().build();
        assert_eq!(prompt.matches("APPEND_RULE_42").count(), 1);
        assert!(prompt.contains("保留原文"));
        if unattended {
            assert!(prompt.contains("Host note: A host admission bind"));
        } else {
            assert!(prompt.contains("Standing host instruction"));
            assert!(prompt.contains(super::SESSION_OUTCOME_BOUNDARY));
        }
    }
}

#[test]
fn trusted_acl_presentation_reaches_the_host_session_without_changing_default() {
    let root = tempfile::tempdir().unwrap();
    let setup = SessionSetup {
        workspace: root.path().to_owned(),
        model_id: "fixture/model",
        effort: None,
        host: Default::default(),
        components: &[],
        mcp_servers: Vec::new(),
    };
    for source in [
        "",
        "tool_presentation { mode = \"code\" }",
        "tool_presentation { mode = \"disabled\" }",
    ] {
        let config = CodeConfig::from_acl(source).unwrap();
        let options = AsterCodeAgent::host_session_options(
            "presentation",
            &setup,
            &config,
            &root.path().join("store"),
        )
        .unwrap();
        assert_eq!(
            options.tool_presentation_profile,
            config.tool_presentation_profile
        );
        assert!(options.permission_checker.is_some());
        assert!(options.confirmation_policy.unwrap().enabled);
    }
}

#[test]
fn disk_attestor_is_installed_only_for_opted_in_non_review_sessions() {
    let root = tempfile::tempdir().unwrap();
    for (enabled, review) in [(false, false), (true, false), (true, true)] {
        let setup = SessionSetup {
            workspace: root.path().to_owned(),
            model_id: "fixture/model",
            effort: None,
            host: crate::host_context::StudioHost {
                completion_attestation: enabled
                    .then_some(crate::host_context::CompletionAttestation::DiskContent),
                source_review: review,
                ..Default::default()
            },
            components: &[],
            mcp_servers: Vec::new(),
        };
        let options = AsterCodeAgent::host_session_options(
            "attestation",
            &setup,
            &CodeConfig::default(),
            &root.path().join("store"),
        )
        .unwrap();
        assert_eq!(options.completion_attestor.is_some(), enabled && !review);
    }
}

#[test]
fn typed_setup_preserves_host_safety_and_context_without_activation() {
    let root = tempfile::tempdir().expect("workspace");
    let skills = root.path().join("configured-skills");
    std::fs::create_dir(&skills).expect("skill directory");
    let store = root.path().join("must-not-create-store");
    let config = CodeConfig {
        sandbox_network: SandboxNetworkMode::Deny,
        sandbox_network_allow: vec!["https://host.example.test".into()],
        skill_dirs: vec!["configured-skills".into()],
        ..Default::default()
    };
    let setup = SessionSetup {
        workspace: root.path().to_owned(),
        model_id: "fixture/exact-model-id",
        effort: None,
        host: crate::host_context::StudioHost {
            guidelines: "Preserve these host instructions".into(),
            output_language: Some("zh-CN".into()),
            ..Default::default()
        },
        components: &[],
        mcp_servers: Vec::new(),
    };
    let options = AsterCodeAgent::host_session_options("exact-session-id", &setup, &config, &store)
        .expect("host options");
    assert_eq!(options.model.as_deref(), Some("fixture/exact-model-id"));
    assert_eq!(options.session_id.as_deref(), Some("exact-session-id"));
    assert_eq!(options.require_assistant_final_response, Some(true));
    assert!(options.code_intelligence);
    assert_eq!(options.sandbox_network, config.sandbox_network);
    assert_eq!(options.sandbox_network_allow, config.sandbox_network_allow);
    assert!(options.permission_checker.is_some());
    let confirmation = options
        .confirmation_policy
        .expect("interactive confirmation");
    assert!(confirmation.enabled);
    assert!(matches!(
        confirmation.timeout_action,
        aster_core::hitl::TimeoutAction::Reject
    ));
    assert!(confirmation.yolo_lanes.is_empty());
    assert!(options.skill_dirs.contains(&skills));
    assert!(options
        .watch_skill_dirs
        .contains(&root.path().join(".aster/skills")));
    assert!(!store.exists());
    let slots = options.prompt_slots.expect("prompt slots");
    assert_eq!(slots.output_language.as_deref(), Some("zh-CN"));
    let extra = slots.extra.expect("host context");
    assert!(extra.contains("Preserve these host instructions"));
    assert!(extra.contains("bm25"));
    assert!(extra.contains("code_symbols"));
    assert!(extra.contains("语义检索关闭"));
    assert!(options.hook_executor.is_some());
}

#[test]
fn host_session_status_names_live_search_and_closed_semantic() {
    let root = tempfile::tempdir().expect("workspace");
    let store = root.path().join("store");
    let setup = SessionSetup {
        workspace: root.path().to_owned(),
        model_id: "fixture/model",
        effort: None,
        host: Default::default(),
        components: &[],
        mcp_servers: Vec::new(),
    };
    let trust = root.path().join("hooks-trust.json");
    let options = AsterCodeAgent::core_session_options("host-session", setup.model_id, setup.effort);
    let mut slots = crate::host_context::merge_studio_host(
        options.prompt_slots.clone().unwrap_or_default(),
        &setup.host,
    );
    let options = AsterCodeAgent::host_session_options_with_hook_roots(
        options,
        &mut slots,
        &setup,
        &CodeConfig::default(),
        &store,
        None,
        Some(&trust),
        None,
    )
    .expect("host options");
    assert!(options.hook_executor.is_some());
    let extra = options
        .prompt_slots
        .expect("prompt slots")
        .extra
        .expect("capability status");
    assert!(extra.contains("bm25"));
    assert!(extra.contains("code_symbols"));
    assert!(extra.contains("语义检索关闭"));
    assert!(extra.contains("memory: 耐久集合"));
    assert!(extra.contains("hooks: 0"));
    assert!(extra.contains("mcp: 0"));
    assert!(extra.contains("pending hooks: 0"));
    assert!(!extra.contains("semantic, hybrid"));
}

#[tokio::test]
async fn trusted_session_hook_runs_and_a_pending_hook_does_not() {
    let root = tempfile::tempdir().expect("workspace");
    let hooks_dir = root.path().join(".aster");
    std::fs::create_dir_all(&hooks_dir).expect("hook directory");
    std::fs::write(
        hooks_dir.join("hooks.json"),
        r#"{
            "hooks": {
                "SessionStart": [{
                    "hooks": [
                        {"type": "command", "command": "printf x > trusted-ran", "timeout": 15},
                        {"type": "command", "command": "printf x > pending-ran", "timeout": 15}
                    ]
                }]
            }
        }"#,
    )
    .expect("hooks.json");
    let trust = root.path().join("hooks-trust.json");
    let preview =
        aster_core::hooks::CommandHookExecutor::discover(root.path(), None, trust.clone())
            .expect("discover");
    let listed = preview.status_value();
    let short_id = listed["hooks"]
        .as_array()
        .expect("hooks")
        .iter()
        .find(|hook| {
            hook["command"]
                .as_str()
                .is_some_and(|command| command.contains("trusted-ran"))
        })
        .expect("trusted hook")["shortId"]
        .as_str()
        .expect("short id");
    preview.manage(&format!("trust {short_id}")).expect("trust");

    let store = root.path().join("store");
    let setup = SessionSetup {
        workspace: root.path().to_owned(),
        model_id: "fixture/model",
        effort: None,
        host: Default::default(),
        components: &[],
        mcp_servers: Vec::new(),
    };
    let options = AsterCodeAgent::core_session_options("hook-session", setup.model_id, setup.effort);
    let mut slots = crate::host_context::merge_studio_host(
        options.prompt_slots.clone().unwrap_or_default(),
        &setup.host,
    );
    let options = AsterCodeAgent::host_session_options_with_hook_roots(
        options,
        &mut slots,
        &setup,
        &CodeConfig::default(),
        &store,
        None,
        Some(&trust),
        None,
    )
    .expect("host options");
    let extra = options
        .prompt_slots
        .as_ref()
        .and_then(|slots| slots.extra.as_deref())
        .expect("capability status");
    assert!(extra.contains("hooks: 2"));
    assert!(extra.contains("pending hooks: 1"));
    assert!(extra.contains("语义检索关闭"));

    let executor = options.hook_executor.expect("hook executor");
    executor
        .fire(&aster_core::hooks::HookEvent::SessionStart(
            aster_core::hooks::SessionStartEvent {
                session_id: "hook-session".to_string(),
                system_prompt: None,
                model_provider: "fixture".to_string(),
                model_name: "model".to_string(),
            },
        ))
        .await;
    assert!(root.path().join("trusted-ran").is_file());
    assert!(!root.path().join("pending-ran").exists());
}

#[test]
fn host_contract_setup_is_scoped_and_refuses_invalid_roots_before_activation() {
    use aster_core::config::WorkspaceVerificationContract;
    use aster_core::verification::VerificationContractV1;

    let root = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let store = root.path().join("not-created");
    let mut config = CodeConfig {
        verification_contracts: vec![WorkspaceVerificationContract {
            workspace: root.path().to_owned(),
            contract: VerificationContractV1::new("host", "python3 gate.py", ["src/**"]).unwrap(),
        }],
        ..Default::default()
    };
    let mut setup = SessionSetup {
        workspace: root.path().to_owned(),
        model_id: "fixture/model",
        effort: None,
        host: Default::default(),
        components: &[],
        mcp_servers: Vec::new(),
    };
    let options = AsterCodeAgent::host_session_options("session", &setup, &config, &store).unwrap();
    assert_eq!(options.verification_contracts.len(), 1);
    assert_eq!(options.verification_contracts[0].command, "python3 gate.py");
    assert!(options.permission_checker.is_some());
    setup.workspace = other.path().to_owned();
    assert!(
        AsterCodeAgent::host_session_options("session", &setup, &config, &store)
            .unwrap()
            .verification_contracts
            .is_empty()
    );
    config.verification_contracts[0].workspace = root.path().join("missing");
    assert!(AsterCodeAgent::host_session_options("session", &setup, &config, &store).is_err());
    assert!(!store.exists());
}

#[tokio::test]
async fn source_review_has_no_tools_even_after_a_yolo_mode_change() {
    use agent_client_protocol::{self as acp, Agent as _};
    use serde_json::json;

    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("private.txt"),
        "not part of the supplied snapshot",
    )
    .unwrap();
    let mut launch = super::test_launch(Some(root.path().join("sessions")));
    launch.workspace = root.path().into();
    let ambient_memory = root.path().join("ambient-memory");
    launch.config.memory_dir = Some(ambient_memory.clone());
    let ambient_wiki = root.path().join("ambient-wiki");
    std::fs::create_dir(&ambient_wiki).unwrap();
    std::fs::write(
        ambient_wiki.join("private.md"),
        "Private personal evidence.",
    )
    .unwrap();
    let host = AsterCodeAgent::new(launch);
    let meta =
        json!({"asterHost":{"sourceReview":true,"officeTools":true,"workspaceWikiSearch":true,"workspaceMemorySearch":true,"assets":{"wiki":ambient_wiki}}})
            .as_object()
            .unwrap()
            .clone();
    let id = host
        .new_session(acp::NewSessionRequest::new(root.path()).meta(meta))
        .await
        .unwrap()
        .session_id;
    let session = host
        .sessions
        .lock()
        .await
        .get(id.0.as_ref())
        .unwrap()
        .session
        .clone();
    assert!(!session.tool_definitions().is_empty());
    assert!(session
        .presented_tool_definitions("Review the supplied text.")
        .unwrap()
        .is_empty());
    assert!(!ambient_memory.exists());
    assert!(session
        .memory()
        .unwrap()
        .recall_similar("Private personal evidence", 5)
        .await
        .unwrap()
        .is_empty());
    for name in ["open_local_asset", "search_wiki", "search_memory"] {
        assert!(!session
            .tool_definitions()
            .iter()
            .any(|tool| tool.name == name));
    }
    host.set_session_mode(acp::SetSessionModeRequest::new(id.clone(), "yolo"))
        .await
        .unwrap();
    assert!(session
        .presented_tool_definitions("Read private.txt using bash or read.")
        .unwrap()
        .is_empty());
    let result = session
        .governed_tool("read", json!({"file_path":"private.txt"}))
        .await
        .unwrap();
    assert_ne!(result.exit_code, 0);
    assert!(result.output.contains("Permission denied"));
    assert!(!result.output.contains("not part of the supplied snapshot"));
    for name in ["office_read", "office_file", "output_check"] {
        assert!(!session
            .tool_definitions()
            .iter()
            .any(|tool| tool.name == name));
    }
    session.close().await;

    let ordinary = host
        .new_session(acp::NewSessionRequest::new(root.path()))
        .await
        .unwrap();
    let ordinary = host
        .sessions
        .lock()
        .await
        .get(ordinary.session_id.0.as_ref())
        .unwrap()
        .session
        .clone();
    assert!(!ordinary
        .presented_tool_definitions("Read private.txt.")
        .unwrap()
        .is_empty());
    for name in ["office_read", "office_file"] {
        assert!(!ordinary
            .tool_definitions()
            .iter()
            .any(|tool| tool.name == name));
    }
    assert!(ordinary
        .tool_definitions()
        .iter()
        .any(|tool| tool.name == "output_check"));
    ordinary.close().await;
}

#[tokio::test]
async fn open_workspace_browser_follows_the_user_browser_pane_and_document_host() {
    let root = tempfile::tempdir().unwrap();
    let mut launch = super::test_launch(Some(root.path().join("sessions")));
    launch.workspace = root.path().into();
    let agent = AsterCodeAgent::new(launch);
    for (metadata, enabled) in [
        (serde_json::json!({}), false),
        (
            serde_json::json!({"asterHost":{"userBrowserPane":true}}),
            false,
        ),
        (
            serde_json::json!({
                "asterStudioDocuments": true,
                "asterHost": {"userBrowserPane": true}
            }),
            true,
        ),
    ] {
        let opened = agent
            .new_session(
                acp::NewSessionRequest::new(root.path())
                    .meta(metadata.as_object().unwrap().clone()),
            )
            .await
            .unwrap();
        let session = agent
            .sessions
            .lock()
            .await
            .get(opened.session_id.0.as_ref())
            .unwrap()
            .session
            .clone();
        let tools = session
            .presented_tool_definitions("Open https://github.com in the workspace browser.")
            .unwrap();
        assert_eq!(
            tools
                .iter()
                .any(|tool| tool.name == "open_workspace_browser"),
            enabled,
            "{metadata}"
        );
        session.close().await;
    }
}

#[tokio::test]
async fn office_tools_follow_only_the_explicit_host_capability() {
    let root = tempfile::tempdir().unwrap();
    let mut launch = super::test_launch(Some(root.path().join("sessions")));
    launch.workspace = root.path().into();
    let agent = AsterCodeAgent::new(launch);
    for (metadata, enabled) in [
        (serde_json::json!({}), false),
        (serde_json::json!({"asterHost":{"officeTools":false}}), false),
        (serde_json::json!({"asterHost":{"officeTools":true}}), true),
    ] {
        let opened = agent
            .new_session(
                acp::NewSessionRequest::new(root.path())
                    .meta(metadata.as_object().unwrap().clone()),
            )
            .await
            .unwrap();
        let session = agent
            .sessions
            .lock()
            .await
            .get(opened.session_id.0.as_ref())
            .unwrap()
            .session
            .clone();
        let tools = session
            .presented_tool_definitions("Create an Office workbook.")
            .unwrap();
        for name in ["office_read", "office_file"] {
            assert_eq!(
                tools.iter().any(|tool| tool.name == name),
                enabled,
                "{name}"
            );
        }
        assert!(tools.iter().any(|tool| tool.name == "output_check"));
        session.close().await;
    }
}

#[tokio::test]
async fn malformed_office_capability_refuses_before_session_admission() {
    let root = tempfile::tempdir().unwrap();
    let mut launch = super::test_launch(Some(root.path().join("sessions")));
    launch.workspace = root.path().into();
    let agent = AsterCodeAgent::new(launch);
    let metadata = serde_json::json!({"asterHost":{"officeTools":"true"}});
    let error = agent
        .new_session(
            acp::NewSessionRequest::new(root.path()).meta(metadata.as_object().unwrap().clone()),
        )
        .await
        .unwrap_err();
    assert!(format!("{error:?}").contains("officeTools must be a boolean"));
    assert!(agent.sessions.lock().await.is_empty());
}

#[tokio::test]
async fn durable_collection_stays_closed_until_the_first_recall() {
    let root = tempfile::tempdir().expect("workspace");
    let memory_dir = root.path().join(".aster/memory");
    std::fs::create_dir_all(memory_dir.join("items")).expect("legacy items");
    std::fs::write(
        memory_dir.join("items/legacy-1.json"),
        r#"{
            "id": "legacy-1",
            "content": "legacy memory about e2e fixtures",
            "timestamp": "2026-01-01T00:00:00Z",
            "importance": 0.7,
            "tags": ["legacy"],
            "memory_type": "semantic",
            "metadata": {},
            "access_count": 0,
            "last_accessed": null
        }"#,
    )
    .expect("write legacy item");
    let store = root.path().join("session-store");
    let setup = SessionSetup {
        workspace: root.path().to_owned(),
        model_id: "fixture/model",
        effort: None,
        host: Default::default(),
        components: &[],
        mcp_servers: Vec::new(),
    };
    let config = CodeConfig {
        default_model: Some("fixture/model".to_string()),
        providers: vec![aster_core::ProviderConfig {
            protocol: None,
            name: "fixture".to_string(),
            api_key: Some("test-key".to_string()),
            base_url: None,
            headers: Default::default(),
            session_id_header: None,
            models: vec![aster_core::ModelConfig {
                id: "model".to_string(),
                name: "model".to_string(),
                family: "fixture".to_string(),
                api_key: None,
                base_url: None,
                headers: Default::default(),
                session_id_header: None,
                attachment: false,
                reasoning: false,
                tool_call: true,
                temperature: true,
                release_date: None,
                modalities: Default::default(),
                cost: Default::default(),
                limit: Default::default(),
            }],
        }],
        ..Default::default()
    };
    let options = AsterCodeAgent::host_session_options("memory-session", &setup, &config, &store)
        .expect("host options");
    assert!(options.memory_store.is_some());
    assert!(!memory_dir.join("durable_memory").exists());
    let extra = options
        .prompt_slots
        .as_ref()
        .and_then(|slots| slots.extra.as_deref())
        .expect("capability status");
    assert!(extra.contains("memory: 耐久集合"));

    let agent = aster_core::Agent::from_config(config)
        .await
        .expect("agent");
    let session = agent
        .session_async(root.path().to_string_lossy().to_string(), Some(options))
        .await
        .expect("session");
    assert!(!memory_dir.join("durable_memory").exists());
    assert!(memory_dir.join("items").is_dir());

    let found = session
        .memory()
        .expect("memory")
        .recall_similar("legacy e2e fixtures", 5)
        .await
        .expect("recall");
    assert_eq!(found.len(), 1);
    assert!(found[0].content.contains("legacy memory"));
    assert!(memory_dir.join("durable_memory").exists());
    assert!(memory_dir.join("items.imported").is_dir());
    assert!(!memory_dir.join("items").exists());

    let again = session
        .memory()
        .expect("memory")
        .recall_similar("legacy e2e fixtures", 5)
        .await
        .expect("second recall");
    assert_eq!(again.len(), 1);
    assert!(!memory_dir.join("items").exists());
    session.close().await;
}
