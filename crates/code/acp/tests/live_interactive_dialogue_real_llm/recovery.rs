use super::*;

const CHOSEN: &str = "KIMI-RECOVERY-AMBER-842";
const OTHER: &str = "KIMI-RECOVERY-CYAN-561";
const TURN_TIMEOUT: Duration = Duration::from_secs(120);

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires the configured real provider and operator approval fixture"]
async fn real_agent_answers_then_approves_a_single_write_and_reopens() {
    tokio::task::LocalSet::new().run_until(async {
        let model = std::env::var("ASTER_LIVE_MODEL").expect("pin ASTER_LIVE_MODEL");
        let root = tempfile::tempdir().unwrap();
        let workspace = root.path().join("approval 工作区");
        std::fs::create_dir(&workspace).unwrap();
        let output = workspace.join("result.txt");
        let (gate_tx, mut gates) = tokio::sync::mpsc::unbounded_channel();
        let (mut first, conn) = spawn_live_client(
            &workspace,
            Some(&model),
            GatedToolClient { gates: gate_tx },
        );
        acp::Agent::initialize(conn.as_ref(), acp::InitializeRequest::new(acp::ProtocolVersion::V1))
            .await.unwrap();
        let session = acp::Agent::new_session(conn.as_ref(), acp::NewSessionRequest::new(workspace.clone()))
            .await.unwrap().session_id;
        acp::Agent::set_session_mode(conn.as_ref(), acp::SetSessionModeRequest::new(session.clone(), "default"))
            .await.unwrap();
        let task = format!(
            "Use ask_user with exactly two choices, {CHOSEN} and {OTHER}. Ask which marker to save. \
             Wait for the answer, then call the write tool once to write the selected value into result.txt \
             with no newline. Read result.txt back to check the exact bytes. Do not use other tools."
        );
        let started = std::time::Instant::now();
        let mut turn = Box::pin(acp::Agent::prompt(conn.as_ref(), prompt(&session, &task)));
        let result = tokio::time::timeout(TURN_TIMEOUT, async {
            let question = tokio::select! {
                gate = gates.recv() => gate.expect("question request"),
                early = &mut turn => panic!("turn ended before the question: {early:?}"),
            };
            let seen = std::time::Instant::now();
            assert_eq!(question.request.session_id, session);
            assert_eq!(question.request.meta.as_ref().unwrap()["question"]["hasChoices"], true);
            assert!(!output.exists(), "the write must wait for the answer");
            let answer = question.request.options.iter().find(|option| option.name == CHOSEN)
                .expect("the frozen question choice must be offered");
            question.reply.send(answer.option_id.0.to_string()).unwrap();
            assert!(seen.elapsed() < Duration::from_secs(10));
            println!("E12 question_ms={}", started.elapsed().as_millis());

            let approval = tokio::select! {
                gate = gates.recv() => gate.expect("approval request"),
                early = &mut turn => panic!("turn ended before the write approval: {early:?}"),
            };
            let seen = std::time::Instant::now();
            assert_eq!(approval.request.session_id, session);
            assert_eq!(approval.tool, "write");
            assert!(approval.options.iter().any(|id| id == "allow-once"));
            let raw = approval.request.tool_call.fields.raw_input.as_ref().unwrap();
            assert_eq!(raw["file_path"], "result.txt");
            assert_eq!(raw["content"], CHOSEN);
            assert!(!output.exists(), "the write must wait for the approval");
            approval.reply.send("allow-once".into()).unwrap();
            assert!(seen.elapsed() < Duration::from_secs(10));
            println!("E12 approval_ms={}", started.elapsed().as_millis());
            turn.as_mut().await
        }).await;
        drop(turn);
        assert_eq!(std::fs::read(&output).unwrap(), CHOSEN.as_bytes());
        let before = aster_core::fact_control::read_workspace_facts(&workspace, session.0.as_ref()).unwrap();
        let writes_before: Vec<_> = before.iter().filter(|fact| {
            let receipt = &fact.payload["receipt"];
            fact.kind == "tool.result" && receipt["outcome"] == "succeeded"
                && receipt["effects"]["changes"].as_array().is_some_and(|changes| !changes.is_empty())
        }).collect();
        assert_eq!(writes_before.len(), 1, "one original mutating receipt");
        assert_eq!(before.iter().filter(|fact| fact.kind == "question.answered").count(), 1);
        let completion_gate = result.as_ref().is_ok_and(|result| result.as_ref().is_err_and(|error| {
            error.data.as_ref().and_then(serde_json::Value::as_str)
                .is_some_and(|data| data.contains("no bound Passed verification"))
        }));
        println!("E12 model={model} first_elapsed_ms={} model_turns={} write_seq={} completion_gate={completion_gate}",
            started.elapsed().as_millis(), before.iter().filter(|fact| fact.kind == "model.turn").count(),
            writes_before[0].seq);
        drop(conn);
        first.kill().await.unwrap();

        let (mut second, conn, _, text) = spawn_real_agent(&workspace, Some(&model)).await;
        acp::Agent::initialize(conn.as_ref(), acp::InitializeRequest::new(acp::ProtocolVersion::V1))
            .await.unwrap();
        acp::Agent::load_session(conn.as_ref(), acp::LoadSessionRequest::new(session.clone(), workspace.clone()))
            .await.expect("cold load of the original session");
        text.lock().unwrap().clear();
        let started = std::time::Instant::now();
        let recalled = tokio::time::timeout(
            TURN_TIMEOUT,
            acp::Agent::prompt(conn.as_ref(), prompt(&session, "Use the read tool on result.txt. Quote the exact bytes. Do not write anything.")),
        ).await;
        let after = aster_core::fact_control::read_workspace_facts(&workspace, session.0.as_ref()).unwrap();
        assert_eq!(&after[..before.len()], before.as_slice(), "reload preserves the original facts");
        assert_eq!(std::fs::read(&output).unwrap(), CHOSEN.as_bytes());
        assert!(after[before.len()..].iter().all(|fact| fact.payload["receipt"]["effects"]["changes"]
            .as_array().is_none_or(Vec::is_empty)), "reload must not replay the write");
        assert!(after[before.len()..].iter().any(|fact| {
            let receipt = &fact.payload["receipt"];
            fact.kind == "tool.result" && receipt["outcome"] == "succeeded"
                && receipt["output"].as_str().is_some_and(|output| output.contains(CHOSEN))
        }), "the recovered turn must read the preserved file");
        println!("E12 reopened_elapsed_ms={} model_turns={}", started.elapsed().as_millis(),
            after[before.len()..].iter().filter(|fact| fact.kind == "model.turn").count());
        assert_eq!(recalled.expect("reopened turn deadline").unwrap().stop_reason, acp::StopReason::EndTurn);
        assert!(text.lock().unwrap().contains(CHOSEN));
        second.kill().await.unwrap();
        if !completion_gate {
            assert_eq!(result.expect("first turn deadline").unwrap().stop_reason, acp::StopReason::EndTurn);
        }
    }).await;
}
