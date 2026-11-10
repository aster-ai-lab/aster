use super::*;
use serde_json::Value;

pub(super) fn response(body: &Value, streaming: bool) -> Option<String> {
    let prompt = body["messages"]
        .as_array()?
        .iter()
        .rev()
        .find(|message| message["role"] == "user")?["content"]
        .to_string();
    ["CAPABILITY-PROBE-LOW", "CAPABILITY-PROBE-DEFAULT"]
        .into_iter()
        .find(|marker| prompt.contains(marker))
        .map(|marker| reply_text(&format!("{marker}-DONE"), streaming))
}

#[test]
fn discovery_applies_effort_reset_and_restart() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace");
    let home = root.path().join("home");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    let (address, state) = spawn_mock(&workspace.join("unused"), &workspace.join("unused-late"));
    let config = write_acl(&home, &workspace, &format!("http://{address}/v1"));
    let source = std::fs::read_to_string(&config).unwrap();
    std::fs::write(
        &config,
        source.replace(
            "reasoning_control = { mode = \"effort\", levels = [\"none\", \"low\", \"high\"] }",
            "",
        ) + "\nmemory { llm_extraction = false }\n",
    )
    .unwrap();
    let acp = binaries::acp_binary();
    let mut owned = child_env(&acp, &config, &home);
    owned.push((
        "ASTER_HOME".into(),
        home.join(".aster").display().to_string(),
    ));
    owned.push(("ASTER_DISABLE_CODEX_ACCOUNT".into(), "1".into()));
    let env = owned
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect::<Vec<_>>();
    let binary = tui_binary();
    let mut pty = open_pty_cleared(
        &binary,
        &["--cwd", workspace.to_str().unwrap()],
        &env,
        &workspace,
    );
    pty.wait_for_frame(Duration::from_secs(45));
    pty.wait_for("mock/gpt (high)", Duration::from_secs(20));
    pty.type_text("/effort ");
    let menu = pty.wait_for("Default (high)", Duration::from_secs(10));
    assert!(
        !menu.contains("Balanced reasoning"),
        "the server does not offer medium\n{menu}"
    );
    assert!(
        !menu.contains("Extended reasoning"),
        "the server does not offer xhigh\n{menu}"
    );
    pty.escape();
    pty.pump(Duration::from_millis(300));
    for (command, marker, expected) in [
        ("/effort low", "CAPABILITY-PROBE-LOW", Some("low")),
        ("/effort default", "CAPABILITY-PROBE-DEFAULT", None),
    ] {
        pty.focus_composer();
        pty.type_text(command);
        pty.enter();
        pty.wait_for(
            if expected.is_some() {
                "mock/gpt (low)"
            } else {
                "mock/gpt (high)"
            },
            Duration::from_secs(15),
        );
        pty.focus_composer();
        pty.type_text(marker);
        pty.enter();
        pty.wait_for(&format!("{marker}-DONE"), Duration::from_secs(20));
        let body: Value = serde_json::from_str(&wait_body_all(
            &state,
            &[marker],
            Duration::from_secs(10),
            true,
        ))
        .unwrap();
        assert_eq!(
            body.get("reasoning_effort").and_then(Value::as_str),
            expected
        );
    }
    pty.focus_composer();
    pty.type_text("/model mock/alt");
    pty.enter();
    pty.wait_for("mock/alt (low)", Duration::from_secs(15));
    pty.focus_composer();
    pty.type_text("/quit");
    pty.enter();
    pty.child.wait().unwrap();
    drop(pty);
    let saved = aster_core::CodeConfig::from_file(&config).unwrap();
    assert_eq!(saved.default_model.as_deref(), Some("mock/alt"));
    assert_eq!(
        saved.reasoning_effort,
        Some(aster_core::config::ReasoningEffort::Default)
    );
    let next = root.path().join("next workspace");
    std::fs::create_dir(&next).unwrap();
    let mut pty = open_pty_cleared(&binary, &["--cwd", next.to_str().unwrap()], &env, &next);
    pty.wait_for_frame(Duration::from_secs(45));
    pty.wait_for("mock/alt (low)", Duration::from_secs(20));
    pty.focus_composer();
    pty.type_text("/quit");
    pty.enter();
    pty.child.wait().unwrap();
}
