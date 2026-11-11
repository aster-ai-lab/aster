use super::*;

const STALLED: &str = "SESSION-CONTROL-STALL";
const QUEUE_HOLD: &str = "QUEUE-HOLD";

fn latest_user(body: &serde_json::Value) -> Option<&serde_json::Value> {
    body["messages"]
        .as_array()?
        .iter()
        .rev()
        .find(|message| message["role"] == "user")
        .map(|message| &message["content"])
}

pub(super) fn response(body: &serde_json::Value, streaming: bool) -> Option<String> {
    let text = latest_user(body)?.as_str()?;
    text.starts_with("QUEUE-")
        .then(|| reply_text(&format!("{text}-DONE"), streaming))
}

pub(super) fn hold_provider(
    body: &str,
    stream: &TcpStream,
    response: &str,
    queue_release: &mut Option<std::sync::mpsc::Receiver<()>>,
) -> bool {
    let value: serde_json::Value = serde_json::from_str(body).unwrap();
    if value["stream"] != true {
        return false;
    }
    let latest = latest_user(&value);
    if latest.and_then(serde_json::Value::as_str) == Some(QUEUE_HOLD) {
        if let Some(release) = queue_release.take() {
            let mut stream = stream.try_clone().unwrap();
            let response = response.to_owned();
            std::thread::spawn(move || {
                if release.recv_timeout(Duration::from_secs(30)).is_ok() {
                    let _ = write_http(&mut stream, &response);
                }
            });
            return true;
        }
    }
    if !latest.is_some_and(|text| text.to_string().contains(STALLED)) {
        return false;
    }
    let mut stream = stream.try_clone().unwrap();
    std::thread::spawn(move || {
        let mut byte = [0];
        let _ = stream.read(&mut byte);
    });
    true
}

#[test]
fn queue_drains_three_follow_ups_in_order_once() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace");
    let home = root.path().join("home");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    let (address, state) = spawn_mock(&workspace.join("unused"), &workspace.join("late"));
    let (release, held) = std::sync::mpsc::channel();
    state.lock().unwrap().queue_release = Some(held);
    let config = write_acl(&home, &workspace, &format!("http://{address}/v1"));
    let source = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, source + "\nmemory { llm_extraction = false }\n").unwrap();
    let product_home = home.join(".aster");
    let mut owned = child_env(&binaries::acp_binary(), &config, &home);
    owned.push(("ASTER_HOME".into(), product_home.display().to_string()));
    owned.push(("ASTER_DISABLE_CODEX_ACCOUNT".into(), "1".into()));
    let env = owned
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect::<Vec<_>>();
    let mut pty = open_pty_cleared(
        &tui_binary(),
        &["--cwd", workspace.to_str().unwrap(), "--fullscreen"],
        &env,
        &workspace,
    );
    let primary_prompts = || {
        state
            .lock()
            .unwrap()
            .bodies
            .iter()
            .filter_map(|raw| {
                let body: serde_json::Value = serde_json::from_str(raw).unwrap();
                if body["stream"] != true {
                    return None;
                }
                latest_user(&body)?.as_str().map(str::to_owned)
            })
            .collect::<Vec<_>>()
    };
    pty.wait_for_frame(Duration::from_secs(45));
    pty.type_text(QUEUE_HOLD);
    pty.wait_for(QUEUE_HOLD, Duration::from_secs(5));
    pty.enter();
    pty.wait_for("[stop]", Duration::from_secs(20));
    let follow_ups = ["QUEUE-FIRST", "QUEUE-SECOND", "QUEUE-THIRD"];
    for (index, prompt) in follow_ups.iter().enumerate() {
        pty.type_text(prompt);
        pty.wait_for(prompt, Duration::from_secs(5));
        pty.enter();
        pty.wait_for(&format!("#{} {prompt}", index + 1), Duration::from_secs(5));
    }
    assert_eq!(primary_prompts(), [QUEUE_HOLD]);
    release.send(()).unwrap();
    let expected = [QUEUE_HOLD, follow_ups[0], follow_ups[1], follow_ups[2]];
    pty.wait_for("QUEUE-THIRD-DONE", Duration::from_secs(20));
    let settled = pty.pump(Duration::from_millis(500));
    for (index, prompt) in follow_ups.iter().enumerate() {
        assert!(
            !settled.contains(&format!("#{} {prompt}", index + 1)),
            "settled prompt returned to the queue\n{settled}"
        );
    }
    let info = pty.open_session_info(Duration::from_secs(15));
    let session = session_id_from_screen(&info);
    assert!(!session.is_empty(), "{info}");
    pty.escape();
    pty.focus_composer();
    pty.type_text("/quit");
    pty.enter();
    assert!(pty.child.wait().unwrap().success());
    assert_eq!(primary_prompts(), expected);

    use aster_core::store::{FileSessionStore, SessionStore};
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let snapshot = runtime.block_on(async {
        FileSessionStore::new(product_home.join("session-store"))
            .await
            .unwrap()
            .load_snapshot(&session)
            .await
            .unwrap()
            .unwrap()
    });
    let records = &snapshot.run_records;
    assert_eq!(records.len(), expected.len());
    for prompt in expected {
        let matching: Vec<_> = records
            .iter()
            .filter(|record| record.snapshot.prompt == prompt)
            .collect();
        assert_eq!(matching.len(), 1, "expected one admitted Run for {prompt}");
        assert_eq!(
            matching[0].snapshot.status,
            aster_core::run::RunStatus::Completed
        );
        assert_eq!(
            matching[0].snapshot.result_text.as_deref(),
            Some(format!("{prompt}-DONE").as_str())
        );
    }
    let run_ids: std::collections::HashSet<_> =
        records.iter().map(|record| &record.snapshot.id).collect();
    assert_eq!(run_ids.len(), expected.len());
}

#[test]
fn resume_command_opens_and_restores_the_selected_session() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace");
    let home = root.path().join("home");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    let (address, state) = spawn_mock(&workspace.join("unused"), &workspace.join("late"));
    let config = write_acl(&home, &workspace, &format!("http://{address}/v1"));
    let owned = child_env(&binaries::acp_binary(), &config, &home);
    let env = owned
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect::<Vec<_>>();
    let marker = "RESUME-POPUP-PRIOR-TURN";
    let id = seed_marked_session(&env, &workspace, &state, marker);
    let mut pty = open_pty_cleared(
        &tui_binary(),
        &["--cwd", workspace.to_str().unwrap()],
        &env,
        &workspace,
    );
    pty.wait_for_frame(Duration::from_secs(45));
    pty.focus_composer();
    pty.type_text("/resume");
    pty.enter();
    pty.wait_for("Resume session", Duration::from_secs(15));
    pty.type_text(marker);
    pty.wait_for(marker, Duration::from_secs(15));
    pty.pump(Duration::from_millis(500));
    pty.enter();
    pty.wait_for(marker, Duration::from_secs(20));
    let info = pty.open_session_info(Duration::from_secs(15));
    assert_eq!(session_id_from_screen(&info), id, "{info}");
}

#[test]
fn mouse_stop_ctrl_c_and_send_now_release_a_stalled_turn() {
    for gesture in ["mouse", "keyboard", "send-now"] {
        let root = tempfile::tempdir().unwrap();
        let workspace = root.path().join("workspace");
        let home = root.path().join("home");
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::create_dir_all(&home).unwrap();
        let (address, state) = spawn_mock(&workspace.join("unused"), &workspace.join("late"));
        let config = write_acl(&home, &workspace, &format!("http://{address}/v1"));
        let source = std::fs::read_to_string(&config).unwrap();
        std::fs::write(&config, source + "\nmemory { llm_extraction = false }\n").unwrap();
        let owned = child_env(&binaries::acp_binary(), &config, &home);
        let env = owned
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect::<Vec<_>>();
        let mut pty = open_pty_cleared(
            &tui_binary(),
            &["--cwd", workspace.to_str().unwrap()],
            &env,
            &workspace,
        );
        pty.wait_for_frame(Duration::from_secs(45));
        pty.type_text(STALLED);
        pty.enter();
        wait_body(&state, STALLED, Duration::from_secs(20));
        let running = pty.wait_for("[stop]", Duration::from_secs(10));
        match gesture {
            "mouse" => {
                let (row, line) = running
                    .lines()
                    .enumerate()
                    .find(|(_, line)| line.contains("[stop]"))
                    .unwrap();
                let column = line.chars().position(|ch| ch == '[').unwrap();
                pty.write_bytes(
                    format!(
                        "\x1b[<0;{};{}M\x1b[<0;{};{}m",
                        column + 2,
                        row + 1,
                        column + 2,
                        row + 1
                    )
                    .as_bytes(),
                );
            }
            "keyboard" => pty.write_bytes(b"\x03"),
            _ => {
                pty.type_text("CONTROL-NEXT");
                pty.enter();
                pty.wait_for("CONTROL-NEXT", Duration::from_secs(5));
                pty.enter();
            }
        }
        if gesture != "send-now" {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                let screen = pty.pump(Duration::from_millis(100));
                if !screen.contains("[stop]") && !screen.contains("Cancelling") {
                    break;
                }
                assert!(Instant::now() < deadline, "{gesture} stayed busy\n{screen}");
            }
            pty.type_text("CONTROL-NEXT");
            pty.enter();
        }
        pty.wait_for("CONTROL-NEXT-DONE", Duration::from_secs(15));
        let settled = pty.pump(Duration::from_millis(800));
        assert!(
            !settled.contains("#1") && !settled.contains("Cancelling"),
            "{gesture} replayed or remained busy\n{settled}"
        );
        let calls = state
            .lock()
            .unwrap()
            .bodies
            .iter()
            .filter(|body| body.contains("CONTROL-NEXT"))
            .count();
        assert!(calls > 0, "{gesture} did not admit the next prompt");
    }
}
