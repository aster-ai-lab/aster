use super::*;

const QUESTION_CASE: &str = "E04-QUESTION-WRITE";
const WRITE_MARKER: &str = "E04-RECOVERY-AMBER-593";
const SOURCE: &str = "E01-VERIFIED-SOURCE-237";
const BASH_CASES: [&str; 3] = ["E01-BASH-ALTERNATE", "E02-BASH-SAME", "E02-BASH-CHANGED"];
const CRASH_CASE: &str = "E07-WRITE-BEFORE-RECEIPT";
const CRASH_FOLLOWUP_READ: &str = "E07-READ-WHILE-UNRECONCILED";
const CRASH_FOLLOWUP_SAME: &str = "E07-SAME-WRITE-WHILE-UNRECONCILED";
const CRASH_FOLLOWUP_CHANGED: &str = "E07-CHANGED-WRITE-WHILE-UNRECONCILED";
const CRASH_FOLLOWUP_OTHER: &str = "E07-OTHER-WRITE-WHILE-UNRECONCILED";
const CRASH_COMMAND: &str = "if test -f first.pid; then printf \"%s\\n\" \"$$\" > blocked-same.pid; printf once >> interrupted.txt; else printf \"%s\\n\" \"$$\" > first.pid; printf once >> interrupted.txt; exec sleep 25; fi";
const CRASH_CHANGED_COMMAND: &str =
    "printf \"%s\\n\" \"$$\" > blocked-changed.pid; printf changed >> interrupted.txt";
const CRASH_JUSTIFICATION: &str = "Inject interruption after writing the synthetic marker";

pub(super) fn response(
    request: &serde_json::Value,
    state: &mut MockState,
    streaming: bool,
) -> Option<String> {
    let messages = request["messages"].as_array()?;
    let start = messages.iter().rposition(|message| {
        message["role"] == "user"
            && message["content"].as_str().is_some_and(|content| {
                content == QUESTION_CASE
                    || content == CRASH_CASE
                    || content == CRASH_FOLLOWUP_READ
                    || content == CRASH_FOLLOWUP_SAME
                    || content == CRASH_FOLLOWUP_CHANGED
                    || content == CRASH_FOLLOWUP_OTHER
                    || BASH_CASES.contains(&content)
            })
    })?;
    if !has_tools(request) {
        return None;
    }
    let turn = serde_json::Value::Array(messages[start..].to_vec());
    let case = messages[start]["content"].as_str().unwrap();
    if case == CRASH_FOLLOWUP_READ {
        if messages[start..]
            .iter()
            .any(|message| message["role"] == "tool")
        {
            return Some(reply_text("Observed E07-READ-VALUE-419.", streaming));
        }
        return Some(reply_tool(
            "read",
            r#"{"file_path":"source.txt"}"#,
            state,
            streaming,
        ));
    }
    if matches!(
        case,
        CRASH_FOLLOWUP_SAME | CRASH_FOLLOWUP_CHANGED | CRASH_FOLLOWUP_OTHER
    ) {
        if messages[start..]
            .iter()
            .any(|message| message["role"] == "tool")
        {
            return Some(reply_text("The synthetic task is complete.", streaming));
        }
        return Some(match case {
            CRASH_FOLLOWUP_SAME => reply_tool(
                "bash",
                &serde_json::json!({
                    "command": CRASH_COMMAND,
                    "sandbox_permissions": "require_escalated",
                    "justification": CRASH_JUSTIFICATION,
                })
                .to_string(),
                state,
                streaming,
            ),
            CRASH_FOLLOWUP_CHANGED => reply_tool(
                "bash",
                &serde_json::json!({
                    "command": CRASH_CHANGED_COMMAND,
                    "sandbox_permissions": "require_escalated",
                    "justification": "Retry with altered arguments while unresolved",
                })
                .to_string(),
                state,
                streaming,
            ),
            _ => reply_tool(
                "write",
                r#"{"file_path":"unreconciled.txt","content":"new write"}"#,
                state,
                streaming,
            ),
        });
    }
    if case == CRASH_CASE {
        let arguments = serde_json::json!({
            "command": CRASH_COMMAND,
            "sandbox_permissions": "require_escalated",
            "justification": CRASH_JUSTIFICATION,
        });
        return Some(reply_tool("bash", &arguments.to_string(), state, streaming));
    }
    if BASH_CASES.contains(&case) {
        if turn.to_string().contains(SOURCE) {
            return Some(reply_text(SOURCE, streaming));
        }
        let recovery = turn.to_string().contains("[host tool recovery]");
        if recovery && case == BASH_CASES[0] {
            return Some(reply_tool(
                "read",
                r#"{"file_path":"source.txt"}"#,
                state,
                streaming,
            ));
        }
        let target = if recovery && case == BASH_CASES[2] {
            "denied-next.txt"
        } else {
            "denied-start.txt"
        };
        let arguments = serde_json::json!({"command": format!("printf forbidden > {target}")});
        return Some(reply_tool("bash", &arguments.to_string(), state, streaming));
    }
    let write = serde_json::json!({"file_path": state.write_path, "content": WRITE_MARKER});
    if let Some(answer) = returned_result(&turn, aster_core::ask_user::USER_ANSWER_SCHEMA) {
        if messages[start..].iter().any(|message| {
            message["role"] == "tool" && message["content"].to_string().contains("bytes")
        }) {
            return Some(reply_text("E04-WRITE-DONE", streaming));
        }
        assert_eq!(answer["answer"], "amber");
        return Some(reply_tool("write", &write.to_string(), state, streaming));
    }
    let question =
        serde_json::json!({"question": "Pick the recovery marker", "options": ["amber", "cyan"]});
    Some(reply_tools(
        &[
            ("ask_user", &question.to_string()),
            ("write", &write.to_string()),
        ],
        state,
        streaming,
    ))
}

#[tokio::test(flavor = "current_thread")]
async fn shipped_agent_reloads_a_write_without_a_terminal_receipt() {
    crash_after_write(false).await;
}

#[tokio::test(flavor = "current_thread")]
async fn shipped_agent_blocks_new_mutations_until_crashed_write_is_reconciled() {
    crash_after_write(true).await;
}

fn cleanup_verified_crash_child(pid_file: &Path) {
    let Ok(raw) = std::fs::read_to_string(pid_file) else {
        return;
    };
    let Ok(pid) = raw.trim().parse::<u32>() else {
        return;
    };
    let Ok(output) = std::process::Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "stat=,pgid=,command="])
        .output()
    else {
        return;
    };
    let status = String::from_utf8_lossy(&output.stdout);
    let mut fields = status.split_whitespace();
    let Some(stat) = fields.next() else {
        return;
    };
    let Some(pgid) = fields.next().and_then(|value| value.parse::<u32>().ok()) else {
        return;
    };
    let cmd = fields.collect::<Vec<_>>().join(" ");
    if !stat.starts_with('Z')
        && pgid == pid
        && (cmd.contains("sleep 25") || cmd.contains("interrupted.txt"))
    {
        let _ = std::process::Command::new("kill")
            .args(["-TERM", &format!("-{pgid}")])
            .status();
    }
}

async fn crash_after_write(try_new_mutation: bool) {
    tokio::task::LocalSet::new().run_until(async {
        let root = tempfile::tempdir().unwrap();
        let workspace = root.path().join("interruption 工作区");
        let home = root.path().join("home");
        std::fs::create_dir(&workspace).unwrap();
        std::fs::create_dir(&home).unwrap();
        std::fs::write(workspace.join("source.txt"), "E07-READ-VALUE-419").unwrap();
        let output = workspace.join("interrupted.txt");
        let first_pid = workspace.join("first.pid");
        let (address, state) = spawn_mock(&workspace.join("unused"), &workspace.join("unused"));
        let config = write_acl(&home, &workspace, &format!("http://{address}/v1"));
        let mut first = connect_agent(&workspace, &config, &home).await;
        let session = acp::Agent::new_session(first.conn.as_ref(), acp::NewSessionRequest::new(workspace.clone()))
            .await.unwrap().session_id;
        let pending = tokio::task::spawn_local({
            let conn = first.conn.clone();
            let session = session.clone();
            async move { acp::Agent::prompt(conn.as_ref(), user_prompt(&session, CRASH_CASE)).await }
        });
        answer_gate(&mut first, "allow-once").await;
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if std::fs::read(&output).is_ok_and(|bytes| bytes == b"once")
                    && first_pid.is_file()
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        }).await.expect("marker must be written before cutting the process");
        let before = aster_core::fact_control::read_workspace_facts(&workspace, session.0.as_ref()).unwrap();
        let prepared = before.iter().find(|fact| fact.kind == "tool.prepared").unwrap();
        assert!(!before.iter().any(|fact| fact.kind == "tool.result" || fact.kind == "turn.complete"));
        stop_agent(first).await;
        assert!(tokio::time::timeout(Duration::from_secs(10), pending).await.unwrap().unwrap().is_err());
        cleanup_verified_crash_child(&first_pid);

        let calls = state.lock().unwrap().bodies.len();
        let mut reopened = connect_agent(&workspace, &config, &home).await;
        let loaded = acp::Agent::load_session(reopened.conn.as_ref(), acp::LoadSessionRequest::new(session.clone(), workspace.clone())).await;
        let after = aster_core::fact_control::read_workspace_facts(&workspace, session.0.as_ref()).unwrap();
        assert_eq!(&after[..before.len()], before.as_slice());
        assert_eq!(std::fs::read(&output).unwrap(), b"once");
        assert_eq!(state.lock().unwrap().bodies.len(), calls);
        assert!(after[before.len()..].iter().all(|fact| fact.kind != "tool.result" && fact.kind != "turn.complete"));
        println!("E07 prepared_seq={} load_ok={} write_count=1 load_requests=0 terminal_receipt=0", prepared.seq, loaded.is_ok());
        loaded.expect("the workspace state must remain available for reconciliation");
        if !try_new_mutation {
            stop_agent(reopened).await;
            return;
        }

        // Same-session read-only positive control: allowed without approval,
        // observes the real workspace bytes, and leaves the unresolved effect open.
        let before_read_len = after.len();
        let read_turn = tokio::time::timeout(
            Duration::from_secs(30),
            acp::Agent::prompt(reopened.conn.as_ref(), user_prompt(&session, CRASH_FOLLOWUP_READ)),
        )
        .await;
        assert!(reopened.gates.try_recv().is_err(), "read-only positive control must not ask approval");
        let read_facts = aster_core::fact_control::read_workspace_facts(&workspace, session.0.as_ref()).unwrap();
        let read_receipts: Vec<_> = read_facts[before_read_len..]
            .iter()
            .filter_map(|fact| fact.payload.get("receipt"))
            .collect();
        assert_eq!(read_receipts.len(), 1);
        assert_eq!(read_receipts[0]["outcome"], "succeeded");
        assert!(read_receipts[0]["output"].as_str().unwrap().contains("E07-READ-VALUE-419"));
        assert!(read_receipts[0]["effects"]["changes"].as_array().unwrap().is_empty());
        assert_eq!(std::fs::read(&output).unwrap(), b"once");
        let read_err = read_turn
            .expect("read control must return a bounded result")
            .expect_err("read control must not clear the unresolved mutation gate");
        assert!(
            read_err
                .data
                .as_ref()
                .is_some_and(|data| data.to_string().contains("tool_effect_unresolved")),
            "{read_err:?}"
        );

        // Negative cases on the same Session: same-args bash, changed-args bash,
        // and other-file write. All must record 0 ACP approvals, 0 process starts,
        // unchanged file bytes, and persisted denied receipts.
        for (prompt_text, blocked_marker) in [
            (CRASH_FOLLOWUP_SAME, Some(workspace.join("blocked-same.pid"))),
            (CRASH_FOLLOWUP_CHANGED, Some(workspace.join("blocked-changed.pid"))),
            (CRASH_FOLLOWUP_OTHER, None),
        ] {
            let prior_facts = aster_core::fact_control::read_workspace_facts(&workspace, session.0.as_ref()).unwrap();
            let prior_len = prior_facts.len();
            let resumed = tokio::time::timeout(
                Duration::from_secs(30),
                acp::Agent::prompt(reopened.conn.as_ref(), user_prompt(&session, prompt_text)),
            )
            .await;
            assert!(
                reopened.gates.try_recv().is_err(),
                "{prompt_text} must be blocked before requesting ACP approval"
            );
            if let Some(marker) = &blocked_marker {
                assert!(!marker.exists(), "{prompt_text} must have zero process starts");
            }
            assert!(!workspace.join("unreconciled.txt").exists(), "an unknown effect must block the next mutating call");
            assert_eq!(std::fs::read(&output).unwrap(), b"once");
            let resumed_facts = aster_core::fact_control::read_workspace_facts(&workspace, session.0.as_ref()).unwrap();
            let new_receipts: Vec<_> = resumed_facts[prior_len..]
                .iter()
                .filter_map(|fact| fact.payload.get("receipt"))
                .collect();
            assert_eq!(new_receipts.len(), 1, "{prompt_text} must record one denied receipt");
            assert_eq!(new_receipts[0]["outcome"], "denied");
            assert!(new_receipts[0]["exit_code"].is_null());
            assert!(new_receipts[0]["effects"]["changes"].as_array().unwrap().is_empty());
            assert!(
                new_receipts[0]["output"]
                    .as_str()
                    .is_some_and(|text| text.contains("tool_effect_unresolved")),
                "{:?}",
                new_receipts[0]
            );
            let error = resumed
                .expect("reconciliation must return a bounded result")
                .expect_err("the unfinished invocation must not report success");
            assert!(
                error
                    .data
                    .as_ref()
                    .is_some_and(|data| data.to_string().contains("tool_effect_unresolved")),
                "{error:?}"
            );
            let new_facts: Vec<_> = resumed_facts[prior_len..].iter().map(|fact| fact.kind.as_str()).collect();
            println!(
                "E07 case={prompt_text} blocked_start=0 approvals=0 new_file={} new_facts={new_facts:?}",
                workspace.join("unreconciled.txt").exists()
            );
        }
        stop_agent(reopened).await;
    }).await;
}

#[tokio::test(flavor = "current_thread")]
async fn shipped_agent_bash_denials_use_one_recovery_round() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let root = tempfile::tempdir().unwrap();
            let workspace = root.path().join("bash recovery 工作区");
            let home = root.path().join("home");
            std::fs::create_dir(&workspace).unwrap();
            std::fs::create_dir(&home).unwrap();
            std::fs::write(workspace.join("source.txt"), SOURCE).unwrap();
            let (address, state) = spawn_mock(&workspace.join("unused"), &workspace.join("unused"));
            let config = write_acl(&home, &workspace, &format!("http://{address}/v1"));
            let agent = connect_agent(&workspace, &config, &home).await;
            for case in BASH_CASES {
                let session = acp::Agent::new_session(
                    agent.conn.as_ref(),
                    acp::NewSessionRequest::new(workspace.clone()),
                )
                .await
                .unwrap()
                .session_id;
                let admitted = goal_ext(
                    &agent,
                    "aster/permission_posture",
                    serde_json::json!({
                        "sessionId": session.0.as_ref(), "posture": "yolo", "deny": "bash(*)",
                    }),
                )
                .await;
                assert_eq!(admitted["result"]["permissionPosture"], "yolo");
                let start = state.lock().unwrap().bodies.len();
                let response = tokio::time::timeout(
                    Duration::from_secs(30),
                    acp::Agent::prompt(agent.conn.as_ref(), user_prompt(&session, case)),
                )
                .await
                .unwrap();
                let facts =
                    aster_core::fact_control::read_workspace_facts(&workspace, session.0.as_ref())
                        .unwrap();
                let receipts: Vec<_> = facts
                    .iter()
                    .filter_map(|fact| fact.payload.get("receipt"))
                    .collect();
                assert_eq!(receipts.len(), 2);
                assert_eq!(receipts[0]["outcome"], "denied");
                assert!(receipts[0]["exit_code"].is_null());
                let admission = &receipts[0]["identity"]["admission"];
                assert!(admission["run_id"].is_string());
                assert_eq!(admission, &receipts[1]["identity"]["admission"]);
                if case == BASH_CASES[0] {
                    assert_eq!(response.unwrap().stop_reason, acp::StopReason::EndTurn);
                    assert_eq!(receipts[1]["outcome"], "succeeded");
                    assert!(receipts[1]["output"].as_str().unwrap().contains(SOURCE));
                    assert!(agent_text(&agent, &session, 0).contains(SOURCE));
                } else {
                    let error = response.unwrap_err();
                    assert!(error
                        .data
                        .unwrap()
                        .to_string()
                        .contains("run.tool_refusal_cycle"));
                    assert_eq!(receipts[1]["outcome"], "denied");
                    assert!(receipts[1]["exit_code"].is_null());
                    assert_ne!(
                        receipts[0]["identity"]["tool_call_id"],
                        receipts[1]["identity"]["tool_call_id"]
                    );
                }
                assert!(!workspace.join("denied-start.txt").exists());
                assert!(!workspace.join("denied-next.txt").exists());
                let requests = bodies_since(&state.lock().unwrap(), start);
                assert_eq!(
                    requests
                        .iter()
                        .filter(|body| body.contains("[host tool recovery]"))
                        .count(),
                    1
                );
                let model_turns = facts
                    .iter()
                    .filter(|fact| fact.kind == "model.turn")
                    .count();
                assert_eq!(model_turns, if case == BASH_CASES[0] { 3 } else { 2 });
                println!("{case} receipts=2 model_turns={model_turns} denied_start=0");
            }
            stop_agent(agent).await;
        })
        .await;
}

#[test]
fn shipped_tui_questions_before_approving_a_batched_write() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("recovery 工作区");
    let home = root.path().join("home");
    std::fs::create_dir(&workspace).unwrap();
    std::fs::create_dir(&home).unwrap();
    let output = workspace.join("result.txt");
    let (addr, state) = spawn_mock(&output, &workspace.join("unused"));
    let config = write_acl(&home, &workspace, &format!("http://{addr}/v1"));
    let owned = child_env(&binaries::acp_binary(), &config, &home);
    let env: Vec<_> = owned
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    let mut pty = open_pty(
        &tui_binary(),
        &[
            "--cwd",
            workspace.to_str().unwrap(),
            "--fullscreen",
            "--trust",
            "--no-plan",
        ],
        &env,
        &workspace,
    );
    pty.wait_for(MODEL_ID, Duration::from_secs(45));
    pty.type_text(QUESTION_CASE);
    pty.enter();
    pty.wait_for("Pick the recovery marker", Duration::from_secs(30));
    assert!(!output.exists(), "the sibling write must remain parked");
    pty.write_bytes(b"1");
    pty.wait_for("Yes, proceed", Duration::from_secs(30));
    assert!(!output.exists(), "the write has not received consent");
    pty.write_bytes(b"1");
    let started = Instant::now();
    while !output.is_file() {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "approved file was not written"
        );
        pty.pump(Duration::from_millis(25));
    }
    assert_eq!(std::fs::read(&output).unwrap(), WRITE_MARKER.as_bytes());
    let stop = pty.wait_for("completion gate:", Duration::from_secs(30));
    assert!(stop.contains("no bound Passed verification"));
    let log = log_text(&workspace);
    assert_eq!(log.matches("\"type\":\"question.answered\"").count(), 1);
    let mutating_results: Vec<_> = log
        .lines()
        .filter_map(|line| {
            let fact: serde_json::Value = serde_json::from_str(line).ok()?;
            (fact["type"] == "tool.result"
                && fact["payload"]["receipt"]["effects"]["changes"]
                    .as_array()
                    .is_some_and(|changes| !changes.is_empty()))
            .then_some(fact)
        })
        .collect();
    assert_eq!(mutating_results.len(), 1);
    assert!(state.lock().unwrap().bodies.iter().any(|body| {
        let request: serde_json::Value = serde_json::from_str(body).unwrap();
        returned_result(
            &request["messages"],
            aster_core::ask_user::USER_ANSWER_SCHEMA,
        )
        .is_some_and(|answer| answer["answer"] == "amber")
    }));
    pty.type_text("/quit");
    pty.enter();
    assert!(pty.child.wait().unwrap().success());
}

#[tokio::test(flavor = "current_thread")]
async fn shipped_agent_cold_load_does_not_execute_a_parked_call() {
    tokio::task::LocalSet::new().run_until(async {
        for parked_after_question in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let workspace = root.path().join("cold restart 工作区");
            let home = root.path().join("home");
            std::fs::create_dir(&workspace).unwrap();
            std::fs::create_dir(&home).unwrap();
            let workspace = workspace.canonicalize().unwrap();
            let output = workspace.join("result.txt");
            let (address, state) = spawn_mock(&output, &workspace.join("unused"));
            let config = write_acl(&home, &workspace, &format!("http://{address}/v1"));
            let mut first = connect_agent(&workspace, &config, &home).await;
            let session = acp::Agent::new_session(first.conn.as_ref(), acp::NewSessionRequest::new(workspace.clone()))
                .await.unwrap().session_id;
            slash(&first, &session, DURABLE_TOKEN).await;
            let objective = "Keep the recovery marker pending until the operator answers";
            slash(&first, &session, &format!("/goal {objective}")).await;
            let bound = goal_state(&first, &session).await;
            assert_eq!(bound["definition"]["goal"]["description"], objective);
            let pending = tokio::task::spawn_local({
                let conn = first.conn.clone();
                let session = session.clone();
                async move { acp::Agent::prompt(conn.as_ref(), user_prompt(&session, QUESTION_CASE)).await }
            });
            if parked_after_question {
                answer_gate(&mut first, "question-0").await;
            }
            let gate = tokio::time::timeout(Duration::from_secs(30), first.gates.recv())
                .await.unwrap().unwrap();
            let old_reply = if parked_after_question { "allow-once" } else { "question-0" };
            assert!(gate.option_ids.iter().any(|id| id == old_reply));
            assert!(!output.exists());
            let before = aster_core::fact_control::read_workspace_facts(&workspace, session.0.as_ref()).unwrap();
            let calls = state.lock().unwrap().bodies.len();
            stop_agent(first).await;
            assert!(tokio::time::timeout(Duration::from_secs(10), pending).await.unwrap().unwrap().is_err());
            let _ = gate.reply.send(old_reply.into());

            let mut resumed = connect_agent(&workspace, &config, &home).await;
            acp::Agent::load_session(resumed.conn.as_ref(), acp::LoadSessionRequest::new(session.clone(), workspace.clone()))
                .await.unwrap();
            let after = aster_core::fact_control::read_workspace_facts(&workspace, session.0.as_ref()).unwrap();
            assert_eq!(&after[..before.len()], before.as_slice());
            assert_eq!(state.lock().unwrap().bodies.len(), calls, "session/load must not infer");
            assert!(!output.exists(), "old consent must not start a write in the new process");
            assert!(resumed.replayed.borrow().iter().any(|(id, replay, text)|
                id == session.0.as_ref() && *replay && text.contains(DURABLE_TOKEN)));
            let reissued_gate = resumed.gates.try_recv().ok().map(|gate| gate.option_ids);
            let restored = goal_state(&resumed, &session).await;
            assert_eq!(restored["definition"]["binding"], bound["definition"]["binding"]);
            let blocked_steps = restored["definition"]["waits"].as_array()
                .map_or(0, |waits| waits.iter().filter(|wait| wait["observed"].is_null()).count());
            println!("E06 phase={old_reply} original_facts={} load_facts={} load_requests=0 reissued_gate={reissued_gate:?} restored_goal_waits={blocked_steps}",
                before.len(), after.len());
            stop_agent(resumed).await;
        }
    }).await;
}
