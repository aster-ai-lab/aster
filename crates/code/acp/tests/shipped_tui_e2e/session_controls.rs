use super::*;

const STALLED: &str = "SESSION-CONTROL-STALL";

pub(super) fn hold_provider(body: &str, stream: &mut TcpStream) -> bool {
    let value: serde_json::Value = serde_json::from_str(body).unwrap();
    if value["stream"] != true {
        return false;
    }
    let latest = value["messages"].as_array().and_then(|messages| {
        messages
            .iter()
            .rev()
            .find(|message| message["role"] == "user")
    });
    if !latest.is_some_and(|message| message["content"].to_string().contains(STALLED)) {
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
