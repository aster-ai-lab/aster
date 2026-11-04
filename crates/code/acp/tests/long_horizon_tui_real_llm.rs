//! Real GLM turns entered through the shipped TUI, with independent receipts.

#![cfg(all(test, target_os = "macos", target_arch = "aarch64"))]

#[path = "long_horizon/artifact.rs"]
mod artifact;
#[path = "long_horizon/candidate.rs"]
mod candidate;
#[path = "long_horizon/config.rs"]
mod config;
#[path = "long_horizon/driver.rs"]
mod driver;
#[path = "long_horizon/fixture.rs"]
mod fixture;
#[path = "long_horizon/live.rs"]
mod live;
#[path = "../../../../apps/aster/tests/support/python.rs"]
mod python;
#[path = "long_horizon/relay.rs"]
mod relay;
#[path = "long_horizon/runtime.rs"]
mod runtime;
#[path = "long_horizon/rust_workflow.rs"]
mod rust_workflow;
#[path = "long_horizon/upstream.rs"]
mod upstream;

use aster_core::store::SessionStore;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Instant;

const GOAL_FINISHED_TEXT: &str = "Durable goal finished";

struct TuiRun {
    pass: bool,
    events: Vec<Value>,
    session_id: Option<String>,
    elapsed_ms: u128,
}

async fn run_tui(
    campaign: &live::Campaign,
    workspace: &Path,
    assets: &Path,
    request: &str,
    expected: &[String],
    goal: bool,
    approve_task: bool,
) -> TuiRun {
    prepare_tui(campaign, workspace, goal);
    let waits = expected
        .iter()
        .enumerate()
        .map(|(i, text)| format!("wait \"result-{i}\" {{ text = {} }}\n", json!(text)))
        .collect::<String>();
    let suite = campaign.output.join("suite.acl");
    let timeout_ms = if goal { 1_800_000 } else { 600_000 };
    let approval = if approve_task {
        "wait \"focused-task-permission\" { text = \"Ctrl+o:yolo\" }\n snapshot \"task-permission\" {}\n press \"allow-task-once\" { key = \"1\" }\n"
    } else {
        ""
    };
    std::fs::write(&suite, format!(
        "suite \"glm-tui\" {{ version = 1\n scenario \"actual-tui\" {{ surface = \"tui\"\n timeout_ms = {timeout_ms}\n wait \"composer-ready\" {{ text = \"Type a message...\" }}\n terminal_paste \"request\" {{ text = {} }}\n press \"submit\" {{ key = \"Enter\" }}\n{approval}{waits}wait \"settled\" {{ text = \"Worked for\" }}\n wait \"meter\" {{ regex = \"Σ [1-9][0-9.kM]* tokens · [0-9]+\\\\.[0-9] tok/s\" }}\n snapshot \"completed\" {{}}\n terminal_recording \"recording\" {{ path = \"terminal/tui.vt\" }}\n }}\n}}\n", json!(request)
    )).unwrap();
    let trace = campaign.output.join("tui-acp.jsonl");
    let (started, output) = run_tui_engine(
        campaign,
        workspace,
        assets,
        &campaign.output,
        timeout_ms,
        None,
    )
    .await;
    let _result_timing = artifact::timing::span(artifact::timing::Phase::ResultWrite);
    let events = std::fs::read_to_string(&trace)
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    let selected = events.iter().find(|e| e["kind"] == "model-selected");
    let session_id = selected
        .and_then(|e| e["session_id"].as_str())
        .map(str::to_owned);
    let processes_absent = proxy_processes_absent(&events).await;
    std::fs::write(
        campaign.output.join("sidecar-cleanup.json"),
        serde_json::to_vec_pretty(&json!({"recorded_proxy_and_agent_absent":processes_absent}))
            .unwrap(),
    )
    .unwrap();
    let saved_usage: Result<aster_core::llm::TokenUsage, &str> = async {
        if !processes_absent {
            return Err("sidecar_still_running");
        }
        let id = session_id.as_deref().ok_or("session_not_selected")?;
        let store =
            aster_core::store::FileSessionStore::new(campaign.home.join("state").join("sessions"))
                .await
                .map_err(|_| "store_open")?;
        let snapshot = store
            .load_snapshot(id)
            .await
            .map_err(|_| "snapshot_load")?
            .ok_or("session_not_saved")?;
        Ok(snapshot.session.total_usage)
    }
    .await;
    let last_total = events
        .iter()
        .rev()
        .find_map(|event| event["packet"]["params"]["update"]["sessionTotalTokens"].as_u64());
    let usage_persisted = last_total.is_some_and(|total| {
        total > 0
            && saved_usage
                .as_ref()
                .is_ok_and(|usage| usage.total_tokens as u64 >= total)
    });
    std::fs::write(campaign.output.join("persisted-usage.json"),serde_json::to_vec_pretty(&json!({
        "saved_usage":saved_usage,"last_acp_session_total":last_total,"no_counter_rollback":usage_persisted,
        "scope":"public FileSessionStore reopened after exact sidecar absence; extracted counters only; interrupted provider billing remains unknown",
    })).unwrap()).unwrap();
    let pass = trace_pass(
        &events,
        output.status.success(),
        &campaign.model,
        processes_absent,
    ) && usage_persisted
        && (!approve_task
            || events
                .iter()
                .filter(|v| v["kind"] == "permission-request")
                .count()
                == 1);
    TuiRun {
        pass,
        events,
        session_id,
        elapsed_ms: started.elapsed().as_millis(),
    }
}

fn prepare_tui(campaign: &live::Campaign, workspace: &Path, goal: bool) {
    let _timing = artifact::timing::span(artifact::timing::Phase::FixtureSetup);
    let config = campaign.root.path().join("tui-host.acl");
    let acl = if goal {
        config::with_rust_verification(&campaign.acl, workspace).unwrap()
    } else {
        campaign.acl.clone()
    };
    std::fs::write(&config, acl).unwrap();
    let proxy = campaign.root.path().join("tui-acp-proxy");
    std::fs::write(&proxy, tui_proxy_source()).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&proxy, std::fs::Permissions::from_mode(0o700)).unwrap();
}

async fn run_tui_engine(
    campaign: &live::Campaign,
    workspace: &Path,
    assets: &Path,
    evidence: &Path,
    timeout_ms: u64,
    resume: Option<&str>,
) -> (Instant, std::process::Output) {
    let suite = evidence.join("suite.acl");
    let config = campaign.root.path().join("tui-host.acl");
    let proxy = campaign.root.path().join("tui-acp-proxy");
    let trace = evidence.join("tui-acp.jsonl");
    let engine = std::env::var_os("ASTER_A3S_TEST_BIN").expect("explicit A3S Test executable");
    let check_timing = artifact::timing::span(artifact::timing::Phase::TuiSuiteCheck);
    let check = tokio::process::Command::new(&engine)
        .args(["check"])
        .arg(&suite)
        .arg("--json")
        .output()
        .await
        .unwrap();
    std::fs::write(evidence.join("suite-check.json"), &check.stdout).unwrap();
    assert!(
        check.status.success(),
        "TUI suite must validate before model calls"
    );
    drop(check_timing);
    let started = Instant::now();
    let _run_timing = artifact::timing::span(artifact::timing::Phase::TuiEngineRun);
    let mut command = tokio::process::Command::new(engine);
    command.env_clear();
    for name in ["PATH", "TMPDIR", "LANG", "LC_ALL"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    command
        .arg("run")
        .arg(&suite)
        .args(["--tui-executable"])
        .arg(&campaign.aster)
        .args([
            "--tui-arg=--no-plan",
            "--tui-arg=--permission-mode",
            "--tui-arg=auto",
            "--tui-working-directory",
        ])
        .arg(workspace)
        .args([
            "--tui-columns",
            "120",
            "--tui-rows",
            "40",
            "--command-timeout-ms",
        ])
        .arg(timeout_ms.to_string())
        .arg("--json")
        .env("HOME", &campaign.home)
        .env("ASTER_HOME", campaign.home.join(".aster"))
        .env("GROK_HOME", campaign.home.join("grok"))
        .env("ASTER_CONFIG", &config)
        .env("ASTER_DEFAULT_MODEL", &campaign.model)
        .env("ASTER_ACP_AGENT_BIN", &proxy)
        .env("ASTER_TUI_TEST_ACP_REAL", &campaign.binary)
        .env("ASTER_TUI_TEST_ACP_TRACE", &trace)
        .env(
            config::RELAY_KEY_ENV,
            campaign.relay.as_ref().unwrap().key(),
        )
        .env("ASTER_SANDBOX_DIR", assets)
        .env("ASTER_DISABLE_GROK_ACCOUNT", "1")
        .env("ASTER_DISABLE_CURSOR_ACCOUNT", "1")
        .env("ASTER_DISABLE_CODEX_ACCOUNT", "1")
        .env("TERM", "xterm-256color")
        .env("TERM_PROGRAM", "Apple_Terminal")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    if let Some(id) = resume {
        command
            .arg("--tui-arg=--resume")
            .arg(format!("--tui-arg={id}"));
    }
    let child = {
        let _timing = artifact::timing::span(artifact::timing::Phase::TuiEngineSpawn);
        command.spawn().unwrap()
    };
    artifact::timing::point(
        artifact::timing::Phase::TuiEngineSpawn,
        child.id().map(artifact::timing::Subject::Process),
    );
    std::fs::write(
        evidence.join("tui-launch.json"),
        serde_json::to_vec_pretty(&json!({
            "engine_pid":child.id(),"parent_pid":std::process::id(),"workspace":workspace,
            "home":campaign.home,"aster":campaign.aster,"acp":campaign.binary,
            "scope":"owned synthetic test session; no original ACL or credential values retained",
        }))
        .unwrap(),
    )
    .unwrap();
    let output = {
        let _timing = artifact::timing::span(artifact::timing::Phase::TuiEngineWait);
        child.wait_with_output().await.unwrap()
    };
    std::fs::write(evidence.join("a3s-result.json"), &output.stdout).unwrap();
    std::fs::write(evidence.join("a3s-stderr.log"), &output.stderr).unwrap();
    (started, output)
}

fn trace_pass(events: &[Value], engine_pass: bool, model: &str, processes_absent: bool) -> bool {
    let selected = events.iter().find(|e| e["kind"] == "model-selected");
    let session_id = selected.and_then(|e| e["session_id"].as_str());
    let requests = events
        .iter()
        .filter(|e| e["kind"] == "prompt-request")
        .collect::<Vec<_>>();
    let responses = events
        .iter()
        .filter(|e| e["kind"] == "prompt-response")
        .collect::<Vec<_>>();
    engine_pass
        && selected.is_some_and(|e| e["model_id"] == model)
        && session_id.is_some_and(|id| !id.is_empty())
        && requests.len() == 1
        && responses.len() == 1
        && requests[0]["rpc_id"] == responses[0]["rpc_id"]
        && requests[0]["session_id"] == responses[0]["session_id"]
        && requests[0]["session_id"].as_str() == session_id
        && responses[0]["error_code"].is_null()
        && responses[0]["stop_reason"] == "end_turn"
        && processes_absent
        && !events
            .iter()
            .any(|e| e["kind"] == "proxy-closed" && e["pending_prompts"] != 0)
}

async fn proxy_processes_absent(events: &[Value]) -> bool {
    let _timing = artifact::timing::span(artifact::timing::Phase::TuiProxyCleanup);
    let starts = events
        .iter()
        .filter(|e| e["kind"] == "proxy-started")
        .collect::<Vec<_>>();
    let [start] = starts.as_slice() else {
        return false;
    };
    let Some(pids) = ["proxy_pid", "agent_pid"]
        .iter()
        .map(|key| {
            start[key]
                .as_u64()
                .filter(|pid| *pid > 0 && *pid <= i32::MAX as u64)
        })
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    // The host kills its direct sidecar; a finally record cannot prove teardown.
    tokio::process::Command::new(python::executable())
        .args(["-c", "import os,sys\nfor pid in sys.argv[1:]:\n try: os.kill(int(pid),0)\n except ProcessLookupError: continue\n sys.exit(1)\n"])
        .args(pids.iter().map(u64::to_string))
        .output().await.is_ok_and(|result| result.status.success())
}

fn updates(run: &TuiRun) -> Vec<&Value> {
    run.events
        .iter()
        .filter(|e| e["kind"] == "packet")
        .map(|e| &e["packet"]["params"]["update"])
        .filter(|v| !v.is_null())
        .collect()
}

fn tui_proxy_source() -> String {
    let source = include_str!("../../../../apps/aster/tests/tui_913_real_llm/acp_trace_proxy.py");
    let insertion = r#"    def observe(packet, source):
        if source == "agent" and isinstance(packet.get("error"), dict):
            error = json.dumps(packet["error"])
            secret = os.environ.get("ASTER_LONG_HORIZON_RELAY_KEY")
            if secret:
                error = error.replace(secret, "[fixture credential redacted]")
            retain({"kind":"rpc-error", "rpc_id":packet.get("id"), "error":json.loads(error)})
        if source == "agent" and packet.get("method") == "session/request_permission":
            if packet.get("params", {}).get("toolCall", {}).get("rawInput", {}).get("sandbox_permissions") == "require_escalated":
                raise ValueError("host execution is outside this scenario")
        if source == "agent" and packet.get("method") in ("session/update", "_aster/session_notification", "_aster/turn/outcome"):
            retain({"kind": "packet", "packet": packet})
"#;
    assert_eq!(
        source.matches("    def observe(packet, source):\n").count(),
        1
    );
    let source = source.replacen("    def observe(packet, source):\n", insertion, 1);
    let boundary = "    try:\n        while True:\n";
    assert_eq!(source.matches(boundary).count(), 1);
    source.replacen(boundary,"    retain({\"kind\":\"proxy-started\",\"proxy_pid\":os.getpid(),\"agent_pid\":child.pid})\n    def stop_proxy(_signal, _frame):\n        raise SystemExit(0)\n    signal.signal(signal.SIGTERM, stop_proxy)\n    try:\n        while True:\n",1)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "real configured GLM, actual TUI, original C01 VM oracle and new-process Goal reload"]
async fn glm_tui_staged_rust_goal_and_process_restore() {
    tokio::task::LocalSet::new().run_until(async {
        let assets = PathBuf::from(std::env::var_os("ASTER_TEST_OCI_ASSETS").unwrap());
        let mut campaign = live::Campaign::prepare("glm-tui-staged-goal").await;
        assert_eq!(campaign.model, "glm-01/glm-5.3");
        let task = fixture::Fixture::create_seeded(campaign.root.path(), fixture::FixtureSeed::Original).unwrap();
        let baseline = task.check_in_guest(&assets).await.unwrap();
        assert!(baseline.compile_ok && !baseline.correct);
        let request = format!("/goal Complete the following task in at least two dependency-ordered stages. Accept each stage only with its current evidence. Preserve every requirement below.\n\n{}", task.request());
        let run = run_tui(&campaign, &task.workspace, &assets, &request, &[GOAL_FINISHED_TEXT.into()], true, false).await;
        let wire: Vec<Value> = run.events.iter().filter(|e| e["kind"] == "packet").filter_map(|e| {
            let packet = &e["packet"];
            match packet["method"].as_str()? {
                "session/update" => Some(packet["params"].clone()),
                "_aster/turn/outcome" => {
                    let mut event=packet.clone();
                    event["method"]="aster/turn/outcome".into();
                    Some(event)
                },
                _ => None,
            }
        }).collect();
        let checks = rust_workflow::wire_checks(&wire);
        let next = campaign.spawn(&task.workspace, Some(&assets), true);
        let restored = if let Some(id) = &run.session_id {
            let id = agent_client_protocol::SessionId::new(id.clone());
            match next.load(&id, &task.workspace).await {
                Ok(()) => next.goal_state(&id).await,
                Err(error) => Err(error),
            }
        } else { Err("tui_session_not_opened") };
        let restored_complete = restored.as_ref().is_ok_and(|v| {
            let d=&v["definition"];
            d["state"]=="completed" && d["goal"]["description"]==request.trim_start_matches("/goal ")
                && d["binding"]["session_id"].as_str()==run.session_id.as_deref()
                && d["binding"]["work_policy_digest"]==v["work_policy_digest"]
                && d["plan_accepted"]==true
                && d["plan"]["steps"].as_array().is_some_and(|s| s.len()>=2 && s.iter().all(|t| t["status"]=="completed") && s.iter().any(|t| t["dependencies"].as_array().is_some_and(|d| !d.is_empty())))
        });
        let cleanup = next.close().await;
        let oracle = task.check_in_guest(&assets).await.unwrap();
        let relay = campaign.relay.take().unwrap().close().await;
        let identity = campaign.identity_unchanged();
        let pass=run.pass && restored_complete && cleanup.is_ok() && oracle.correct && identity && relay.as_ref().is_ok_and(|s| s.upstream_calls>0 && s.listener_closed)
            && ["initial_failed_check","final_passing_check","bound_verified_outcome"].iter().all(|k| checks[*k]==true);
        std::fs::write(campaign.output.join("result.json"),serde_json::to_vec_pretty(&json!({"pass":pass,"model":campaign.model,"pair_build_id":campaign.build_id,"tui_pass":run.pass,"elapsed_ms":run.elapsed_ms,"wire_checks":checks,"restored_goal":restored,"restored_complete":restored_complete,"reload_cleanup":cleanup,"oracle":oracle,"relay":relay,"identity_unchanged":identity})).unwrap()).unwrap();
        for name in fixture::ARTIFACTS { if let Ok(bytes)=artifact::read(&task.workspace,name) {std::fs::write(campaign.output.join(name),bytes).unwrap();} }
        println!("GLM_TUI_RESULT {}", json!({"pass":pass,"evidence":campaign.output}));
        assert!(pass,"actual GLM TUI staged Goal failed; evidence retained");
    }).await;
}

async fn delegation_case(kind: &str, approve_task: bool) {
    let assets = PathBuf::from(std::env::var_os("ASTER_TEST_OCI_ASSETS").unwrap());
    let approval_label = if approve_task { "-approved" } else { "" };
    let mut campaign = live::Campaign::prepare(&format!("glm-tui-{kind}{approval_label}")).await;
    assert_eq!(campaign.model, "glm-01/glm-5.3");
    let workspace = campaign.root.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let alpha = format!("ALPHA-{}", uuid::Uuid::new_v4().simple());
    let beta = format!("BETA-{}", uuid::Uuid::new_v4().simple());
    std::fs::write(workspace.join("alpha.txt"), format!("{alpha}\n")).unwrap();
    std::fs::write(workspace.join("beta.txt"), format!("{beta}\n")).unwrap();
    if kind == "background-child" {
        let mut next = None;
        for index in (0..6).rev() {
            let name = if index == 0 {
                "alpha.txt".to_owned()
            } else {
                format!("{}.txt", uuid::Uuid::new_v4().simple())
            };
            let value = match next {
                Some(next) => json!({"next":next}),
                None => json!({"result":alpha}),
            };
            std::fs::write(workspace.join(&name), value.to_string()).unwrap();
            next = Some(name);
        }
    }
    let inputs = campaign.output.join("fixture");
    std::fs::create_dir(&inputs).unwrap();
    let originals = std::fs::read_dir(&workspace)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            let digest = artifact::digest(&path).unwrap();
            std::fs::copy(&path, inputs.join(path.file_name().unwrap())).unwrap();
            (path, digest)
        })
        .collect::<Vec<_>>();
    let (request,expected)=match kind {
        "single" => ("Use the task tool with exactly one explore child. Have that child use read to read alpha.txt and return its entire payload. Do not read it yourself. Report the actual child result. Do not modify files or run shell commands.",vec![alpha.clone()]),
        "parallel" => ("Use one task call with two independent explore children in its tasks array, concurrently: one reads alpha.txt and returns its payload; the other reads beta.txt and returns its payload. Use read in each child; do not read the files yourself. Report both actual child results. Do not modify files or run shell commands.",vec![alpha.clone(),beta.clone()]),
        "background" => ("Use background action start with name watcher and command `sleep 20; cat alpha.txt`. While it runs, read beta.txt in the parent. Use background action tail for watcher with timeout_ms 30000 to collect its actual output and completion, then stop it if still running. Report both file payloads only after observing the results. Do not modify files or request host execution.",vec![alpha.clone(),beta.clone()]),
        "background-child" => ("Use task with exactly one explore child and background=true. The child must read alpha.txt, follow each JSON next filename by another read until the final result, and return that result verbatim. Do not read that chain yourself. While the child continues, read beta.txt in the parent and return only beta.txt's payload; do not wait for the child. The child must finish independently after your parent response. Do not modify files or run shell commands.",vec![beta.clone(),"Subagent completed in".into()]),
        _=>panic!("unregistered TUI case"),
    };
    let run = run_tui(
        &campaign,
        &workspace,
        &assets,
        request,
        &expected,
        false,
        approve_task,
    )
    .await;
    let notifications = updates(&run);
    let starts = notifications
        .iter()
        .copied()
        .filter(|v| v["sessionUpdate"] == "subagent_spawned")
        .collect::<Vec<_>>();
    let finishes = notifications
        .iter()
        .copied()
        .filter(|v| v["sessionUpdate"] == "subagent_finished")
        .collect::<Vec<_>>();
    let count = if kind == "parallel" { 2 } else { 1 };
    let children_settled = starts.len() == count
        && finishes.len() == count
        && starts.iter().all(|s| {
            s["parent_session_id"].as_str() == run.session_id.as_deref()
                && s["child_session_id"]
                    .as_str()
                    .is_some_and(|id| Some(id) != run.session_id.as_deref())
                && finishes.iter().any(|f| {
                    f["subagent_id"] == s["subagent_id"]
                        && f["child_session_id"] == s["child_session_id"]
                        && f["status"] == "completed"
                })
        });
    let overlapped = kind != "parallel"
        || (children_settled
            && starts
                .iter()
                .filter_map(|v| v["startedAt"].as_u64())
                .max()
                .zip(
                    finishes
                        .iter()
                        .filter_map(|v| v["completedAt"].as_u64())
                        .min(),
                )
                .is_some_and(|(last_start, first_end)| last_start < first_end));
    let background_complete = kind != "background"
        || finishes
            .iter()
            .any(|f| f["disposition"] == "completed" && f["exitCode"] == 0);
    let background_child_complete = kind != "background-child" || {
        let dispatched = notifications.iter().any(|v| {
            v["sessionUpdate"] == "tool_call"
                && v["rawInput"]["tasks"]
                    .as_array()
                    .is_some_and(|tasks| tasks.len() == 1 && tasks[0]["background"] == true)
        });
        let end = run.events.iter().position(|v| {
            v["kind"] == "packet"
                && v["packet"]["params"]["update"]["sessionUpdate"] == "subagent_finished"
        });
        let parent = run
            .events
            .iter()
            .position(|v| v["kind"] == "prompt-response" && v["error_code"].is_null());
        dispatched
            && parent.zip(end).is_some_and(|(p, e)| p < e)
            && finishes.iter().any(|v| {
                v["tool_calls"].as_u64().is_some_and(|n| n >= 6)
                    && v["output"].as_str().is_some_and(|s| s.contains(&alpha))
            })
    };
    let parent_read_during_background = !matches!(kind, "background" | "background-child") || {
        let read_id = notifications
            .iter()
            .find(|v| {
                v["sessionUpdate"] == "tool_call"
                    && v["rawInput"]["file_path"].as_str().is_some_and(|p| {
                        Path::new(p).file_name() == Some(std::ffi::OsStr::new("beta.txt"))
                    })
            })
            .and_then(|v| v["toolCallId"].as_str());
        let start = notifications
            .iter()
            .position(|v| v["sessionUpdate"] == "subagent_spawned");
        let read = notifications.iter().position(|v| {
            v["sessionUpdate"] == "tool_call_update"
                && v["toolCallId"].as_str() == read_id
                && v["status"] == "completed"
        });
        let end = notifications
            .iter()
            .position(|v| v["sessionUpdate"] == "subagent_finished");
        read_id.is_some()
            && start
                .zip(read)
                .zip(end)
                .is_some_and(|((s, r), e)| s < r && r < e)
    };
    let unchanged = originals
        .iter()
        .all(|(path, digest)| artifact::digest(path).ok().as_ref() == Some(digest));
    let relay = campaign.relay.take().unwrap().close().await;
    let identity = campaign.identity_unchanged();
    let pass = run.pass
        && children_settled
        && overlapped
        && background_complete
        && background_child_complete
        && parent_read_during_background
        && unchanged
        && identity
        && relay
            .as_ref()
            .is_ok_and(|s| s.upstream_calls > 0 && s.listener_closed);
    std::fs::write(campaign.output.join("result.json"),serde_json::to_vec_pretty(&json!({"pass":pass,"case":kind,"approved_task_through_ui":approve_task,"model":campaign.model,"pair_build_id":campaign.build_id,"tui_pass":run.pass,"elapsed_ms":run.elapsed_ms,"children_settled":children_settled,"parallel_overlap":(kind=="parallel").then_some(overlapped),"background_process_completed_zero_exit":(kind=="background").then_some(background_complete),"background_child_outlived_parent_and_returned_chain_result":(kind=="background-child").then_some(background_child_complete),"parent_read_during_background":matches!(kind,"background"|"background-child").then_some(parent_read_during_background),"workspace_unchanged":unchanged,"source_files":originals.iter().map(|(path,hash)|json!({"name":path.file_name(),"sha256":hash})).collect::<Vec<_>>(),"starts":starts,"finishes":finishes,"relay":relay,"identity_unchanged":identity})).unwrap()).unwrap();
    println!(
        "GLM_TUI_RESULT {}",
        json!({"pass":pass,"case":kind,"evidence":campaign.output})
    );
    assert!(
        pass,
        "actual GLM TUI delegation/background failed; evidence retained"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "real configured GLM and actual TUI single child receipts"]
async fn glm_tui_single_subagent() {
    tokio::task::LocalSet::new()
        .run_until(delegation_case("single", false))
        .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "real configured GLM and actual TUI two overlapping child runs"]
async fn glm_tui_parallel_subagents() {
    tokio::task::LocalSet::new()
        .run_until(delegation_case("parallel", false))
        .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "real configured GLM and actual TUI sandboxed background process lifecycle"]
async fn glm_tui_background_task() {
    tokio::task::LocalSet::new()
        .run_until(delegation_case("background", false))
        .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "real configured GLM; actual TUI allow-once and single child"]
async fn glm_tui_approved_single_subagent() {
    tokio::task::LocalSet::new()
        .run_until(delegation_case("single", true))
        .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "real configured GLM; actual TUI allow-once and overlapping children"]
async fn glm_tui_approved_parallel_subagents() {
    tokio::task::LocalSet::new()
        .run_until(delegation_case("parallel", true))
        .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "real configured GLM; actual TUI background child outlives parent"]
async fn glm_tui_approved_background_subagent() {
    tokio::task::LocalSet::new()
        .run_until(delegation_case("background-child", true))
        .await;
}

#[path = "long_horizon/tui_proxy_tests.rs"]
mod proxy_tests;

#[path = "long_horizon/tui_lifecycle.rs"]
mod lifecycle;
