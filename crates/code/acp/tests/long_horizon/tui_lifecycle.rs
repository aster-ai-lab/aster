use super::*;
use aster_core::subagent_task_tracker::SubagentStatus;
use aster_effect::LogStore;

fn trace(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn cancel_proxy() -> String {
    let source = tui_proxy_source();
    let boundary = "    def observe(packet, source):\n";
    assert_eq!(source.matches(boundary).count(), 1);
    source.replacen(
        boundary,
        r#"    cancellations = set()
    def observe(packet, source):
        if source == "client" and packet.get("method") == "_aster/subagent/cancel":
            cancellations.add(packet["id"])
            retain({"kind":"cancel-request", "packet":packet})
        if source == "agent" and packet.get("id") in cancellations:
            retain({"kind":"cancel-response", "packet":packet})
"#,
        1,
    )
}

fn settlements(workspace: &Path, session: &str) -> Vec<aster_effect::Fact> {
    let log = aster_effect::FileLog::open(aster_core::session_folder::effect_log_dir(
        workspace, session,
    ))
    .unwrap();
    log.read(&aster_core::fact_control::thread_for_session(session))
        .unwrap()
        .into_iter()
        .filter(|fact| fact.kind == "subagent.settled")
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "real configured GLM; actual TUI child cancellation, original effect settlement and fresh TUI reopen"]
async fn glm_tui_cancel_background_and_reopen_original_session() {
    tokio::task::LocalSet::new().run_until(async {
        let assets = PathBuf::from(std::env::var_os("ASTER_TEST_OCI_ASSETS").unwrap());
        let mut campaign = live::Campaign::prepare("glm-tui-cancel-reopen").await;
        assert_eq!(campaign.model, "glm-01/glm-5.3");
        let workspace = campaign.root.path().join("workspace");
        std::fs::create_dir(&workspace).unwrap();
        let mut originals = Vec::new();
        let marker = if let Some(source) = std::env::var_os("ASTER_LH_TUI_FIXTURE_DIR") {
            for entry in std::fs::read_dir(PathBuf::from(source)).unwrap() {
                let entry = entry.unwrap();
                assert!(entry.file_type().unwrap().is_file());
                let path = workspace.join(entry.file_name());
                std::fs::copy(entry.path(), &path).unwrap();
                originals.push((path.clone(), artifact::digest(&path).unwrap()));
            }
            assert_eq!(originals.len(), 35);
            let mut file = workspace.join("beta.txt");
            let mut result = None;
            for _ in 0..3 {
                let value: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
                if let Some(text) = value["result"].as_str() {
                    result = Some(text.to_owned());
                    break;
                }
                file = workspace.join(value["next"].as_str().unwrap());
            }
            result.expect("recorded three-file parent chain")
        } else {
            let marker = uuid::Uuid::new_v4().simple().to_string();
            for (entry, count) in [("alpha.txt", 32), ("beta.txt", 3)] {
                let mut next = None;
                for index in (0..count).rev() {
                    let name = if index == 0 { entry.to_owned() } else { format!("{}.txt", uuid::Uuid::new_v4().simple()) };
                    let value = next.map_or_else(|| json!({"result":marker}), |next: String| json!({"next":next}));
                    let path = workspace.join(&name);
                    std::fs::write(&path, value.to_string()).unwrap();
                    originals.push((path.clone(), artifact::digest(&path).unwrap()));
                    next = Some(name);
                }
            }
            marker
        };
        let fixture = campaign.output.join("fixture");
        std::fs::create_dir(&fixture).unwrap();
        for (path, _) in &originals {
            std::fs::copy(path, fixture.join(path.file_name().unwrap())).unwrap();
        }
        let request = "Use task with exactly one explore child and background=true. The child must read alpha.txt, follow each JSON next filename by another read until the final result, and return that result verbatim. Do not read that chain yourself. While the child continues, read beta.txt and follow its JSON next filenames to its final result in the parent. Return only that result; do not wait for the child. Do not modify files or run shell commands.";
        prepare_tui(&campaign, &workspace, false);
        std::fs::write(campaign.root.path().join("tui-acp-proxy"), cancel_proxy()).unwrap();
        let first = campaign.output.join("cancel");
        std::fs::create_dir(&first).unwrap();
        std::fs::write(first.join("suite.acl"), format!(r#"suite "glm-tui-cancel" {{
 version = 1
 scenario "cancel-original-child" {{
  surface = "tui"
  timeout_ms = 600000
  wait "ready" {{ text = "Type a message..." }}
  terminal_paste "request" {{ text = {} }}
  press "submit" {{ key = "Enter" }}
  wait "permission" {{ text = "Ctrl+o:yolo" }}
  press "allow-once" {{ key = "1" }}
  wait "parent-result" {{ text = {} }}
  wait "parent-finished" {{ text = "Worked for" }}
  wait "child-still-live" {{ text = "1 subagent still running" }}
  press "focus-scrollback" {{ key = "Tab" }}
  press "focus-dock" {{ key = "Tab" }}
  press "select-child" {{ key = "Down" }}
  press "cancel-child" {{ key = "x" }}
  press "leave-dock" {{ key = "Escape" }}
  wait "cancelled" {{ text = "Subagent cancelled in" }}
  snapshot "terminal-receipt" {{}}
  terminal_recording "vt" {{ path = "terminal/cancel.vt" }}
 }}
}}
"#, json!(request), json!(marker))).unwrap();
        let (started, output) = run_tui_engine(&campaign, &workspace, &assets, &first, 600000, None).await;
        let elapsed = started.elapsed().as_millis();
        let events = trace(&first.join("tui-acp.jsonl"));
        let absent = proxy_processes_absent(&events).await;
        let session_id = events.iter().find(|e| e["kind"] == "model-selected")
            .and_then(|e| e["session_id"].as_str()).map(str::to_owned);
        let starts: Vec<_> = events.iter().filter(|e| e["packet"]["params"]["update"]["sessionUpdate"] == "subagent_spawned")
            .map(|e| &e["packet"]["params"]["update"]).collect();
        let finishes: Vec<_> = events.iter().filter(|e| e["packet"]["params"]["update"]["sessionUpdate"] == "subagent_finished")
            .map(|e| &e["packet"]["params"]["update"]).collect();
        let cancelled = starts.len() == 1 && finishes.len() == 1
            && starts[0]["subagent_id"] == finishes[0]["subagent_id"]
            && finishes[0]["status"] == "stopped"
            && finishes[0]["tool_calls"].as_u64().is_some_and(|n| n > 0);
        let cancel_requests: Vec<_> = events.iter().filter(|e| e["kind"] == "cancel-request").collect();
        let cancel_responses: Vec<_> = events.iter().filter(|e| e["kind"] == "cancel-response").collect();
        let cancel_ack = cancel_requests.len() == 1 && cancel_responses.len() == 1
            && cancel_requests[0]["packet"]["id"] == cancel_responses[0]["packet"]["id"]
            && starts.first().is_some_and(|s| cancel_requests[0]["packet"]["params"]["subagentId"] == s["subagent_id"])
            && cancel_responses[0]["packet"]["result"]["result"]["cancelled"] == true;
        let store = aster_core::store::FileSessionStore::new(campaign.home.join("state").join("sessions")).await.unwrap();
        let snapshot = if let Some(id) = &session_id { store.load_snapshot(id).await.unwrap() } else { None };
        let saved_terminal = snapshot.as_ref().is_some_and(|s| s.subagent_tasks.len() == 1
            && s.subagent_tasks[0].status == SubagentStatus::Cancelled
            && s.subagent_tasks[0].finished_ms.is_some());
        let settled = if let Some(id) = &session_id {
            settlements(&workspace, id)
        } else { Vec::new() };
        let original_settled = settled.len() == 1 && starts.first().is_some_and(|s|
            settled[0].payload["task_id"] == s["subagent_id"]
            && settled[0].payload["child_log_cut"]["seq"].as_u64().is_some_and(|n| n > 0));
        let before_reopen_requests = campaign.relay.as_ref().unwrap().upstream_calls();
        let second = campaign.output.join("reopen");
        std::fs::create_dir(&second).unwrap();
        let mut reopen = json!({"attempted":false});
        if absent && snapshot.is_some() {
            std::fs::write(second.join("suite.acl"), format!(r#"suite "glm-tui-reopen" {{
 version = 1
 scenario "original-session" {{
  surface = "tui"
  timeout_ms = 30000
  wait "original-result" {{ text = {} }}
  wait "cancelled-history" {{ text = "Subagent cancelled" }}
  wait "composer" {{ text = "Add a follow-up" }}
  snapshot "restored" {{}}
  terminal_recording "vt" {{ path = "terminal/reopen.vt" }}
 }}
}}
"#, json!(marker))).unwrap();
            let (started, result) = run_tui_engine(&campaign, &workspace, &assets, &second, 30000, session_id.as_deref()).await;
            let reload_events = trace(&second.join("tui-acp.jsonl"));
            let cleanup = proxy_processes_absent(&reload_events).await;
            reopen = json!({"attempted":true,"pass":result.status.success(),"elapsed_ms":started.elapsed().as_millis(),
                "processes_absent":cleanup,"prompt_requests":reload_events.iter().filter(|e|e["kind"]=="prompt-request").count()});
        }
        let after_reopen_requests = campaign.relay.as_ref().unwrap().upstream_calls();
        let saved_after = if let Some(id) = &session_id { store.load_snapshot(id).await.unwrap() } else { None };
        let terminal_after_reopen = saved_after.as_ref().is_some_and(|s| s.subagent_tasks.len() == 1
            && s.subagent_tasks[0].status == SubagentStatus::Cancelled && s.subagent_tasks[0].finished_ms.is_some());
        let settlement_unchanged = session_id.as_deref().is_some_and(|id| settlements(&workspace, id) == settled);
        let usage_unchanged = snapshot.as_ref().zip(saved_after.as_ref()).is_some_and(|(a,b)|
            a.session.total_usage.total_tokens > 0 && json!(a.session.total_usage) == json!(b.session.total_usage));
        let unchanged = originals.iter().all(|(p,hash)|artifact::digest(p).ok().as_ref()==Some(hash));
        let relay = campaign.relay.take().unwrap().close().await;
        let identity = campaign.identity_unchanged();
        let pass = trace_pass(&events, output.status.success(), &campaign.model, absent)
            && cancelled && cancel_ack && saved_terminal && terminal_after_reopen && original_settled && settlement_unchanged && unchanged && usage_unchanged
            && reopen["pass"] == true && reopen["processes_absent"] == true && reopen["prompt_requests"] == 0
            && before_reopen_requests == after_reopen_requests && identity
            && relay.as_ref().is_ok_and(|s|s.upstream_calls>0 && s.listener_closed);
        std::fs::write(campaign.output.join("result.json"), serde_json::to_vec_pretty(&json!({
            "pass":pass,"model":campaign.model,"pair_build_id":campaign.build_id,"elapsed_ms":elapsed,
            "tui_pass":output.status.success(),"sidecars_absent":absent,"session_id":session_id,
            "cancel_ack":cancel_ack,"cancelled_after_real_child_tool":cancelled,"starts":starts,"finishes":finishes,
            "saved_terminal":saved_terminal,"original_effect_settled":original_settled,"settlement":settled,
            "terminal_after_reopen":terminal_after_reopen,"settlement_unchanged_on_reopen":settlement_unchanged,
            "saved_usage":snapshot.as_ref().map(|s|&s.session.total_usage),"usage_unchanged_on_reopen":usage_unchanged,
            "reopen":reopen,"requests_before_reopen":before_reopen_requests,"requests_after_reopen":after_reopen_requests,
            "workspace_unchanged":unchanged,"identity_unchanged":identity,"relay":relay,
            "input_files":originals.iter().map(|(path,hash)|json!({"name":path.file_name(),"sha256":hash})).collect::<Vec<_>>(),
            "comparison":{"baseline":null,"time_percent":null,"tokens_percent":null,"reason":"first registered cancellation/reopen case; incomplete provider usage is unknown"}
        })).unwrap()).unwrap();
        println!("GLM_TUI_RESULT {}",json!({"pass":pass,"evidence":campaign.output}));
        assert!(pass,"actual TUI cancellation/reopen failed; original evidence retained");
    }).await;
}
