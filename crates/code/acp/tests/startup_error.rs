use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdout, Command};

fn start(workspace: &std::path::Path, home: &std::path::Path) -> (Child, BufReader<ChildStdout>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_aster-acp"))
        .arg("--cwd")
        .arg(workspace)
        .env("HOME", home)
        .env("ASTER_LOG_FILE", "0")
        .env("ASTER_DISABLE_CC_SWITCH", "1")
        .env("ASTER_DISABLE_GROK_ACCOUNT", "1")
        .env("ASTER_DISABLE_CURSOR_ACCOUNT", "1")
        .env_remove("ASTER_CONFIG")
        .env_remove("ASTER_DEFAULT_MODEL")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("start shipped ACP");
    let stdout = BufReader::new(child.stdout.take().expect("stdout"));
    (child, stdout)
}

async fn request(
    child: &mut Child,
    stdout: &mut BufReader<ChildStdout>,
    method: &str,
    params: Value,
) -> Value {
    let request = json!({"jsonrpc":"2.0", "id":1, "method":method, "params":params});
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(format!("{request}\n").as_bytes())
        .await
        .expect("send request");
    let mut line = String::new();
    let bytes = tokio::time::timeout(Duration::from_secs(5), stdout.read_line(&mut line))
        .await
        .expect("bounded startup response")
        .expect("read response");
    assert!(
        bytes > 0,
        "ACP must return the startup failure instead of closing its channel"
    );
    serde_json::from_str(&line).expect("JSON-RPC response")
}

#[tokio::test]
async fn workspace_only_provider_returns_actionable_error_and_cannot_start_a_session() {
    let home = tempfile::tempdir().expect("home");
    let workspace = tempfile::tempdir().expect("workspace");
    std::fs::create_dir(workspace.path().join(".aster")).expect("config directory");
    std::fs::write(
        workspace.path().join(".aster/config.acl"),
        r#"
providers "repository" {
  api_key = "never-display-this-key"
  models "local" {}
}
default_model = "repository/local"
"#,
    )
    .expect("workspace ACL");
    let (mut child, mut stdout) = start(workspace.path(), home.path());
    let response = request(
        &mut child,
        &mut stdout,
        "initialize",
        json!({"protocolVersion":1}),
    )
    .await;
    let error = response["error"]["message"]
        .as_str()
        .expect("startup error");
    assert!(error.contains("repository/local"), "{response}");
    assert!(
        error.contains("workspace providers are untrusted"),
        "{response}"
    );
    assert!(
        error.contains("~/.aster/config.acl") && error.contains("ASTER_CONFIG"),
        "{response}"
    );
    assert!(!response.to_string().contains("never-display-this-key"));
    let response = request(
        &mut child,
        &mut stdout,
        "session/new",
        json!({"cwd":workspace.path(), "mcpServers":[]}),
    )
    .await;
    assert!(
        response.get("error").is_some(),
        "an invalid configuration must not create a session: {response}"
    );
    child.kill().await.expect("stop owned agent");
}

#[tokio::test]
async fn first_run_template_is_loaded_before_opening_a_session() {
    let home = tempfile::tempdir().expect("home");
    let workspace = tempfile::tempdir().expect("workspace");
    let (mut child, mut stdout) = start(workspace.path(), home.path());
    let initialized = request(
        &mut child,
        &mut stdout,
        "initialize",
        json!({"protocolVersion":1}),
    )
    .await;
    assert!(initialized.get("error").is_none(), "{initialized}");
    let opened = request(
        &mut child,
        &mut stdout,
        "session/new",
        json!({"cwd":workspace.path(),"mcpServers":[]}),
    )
    .await;
    assert_eq!(
        opened["result"]["models"]["currentModelId"], "deepseek/deepseek-flash",
        "{opened}"
    );
    assert!(home.path().join(".aster/config.acl").is_file());
    child.kill().await.expect("stop owned agent");
}

#[tokio::test]
async fn trusted_provider_still_initializes_without_contacting_the_model() {
    let home = tempfile::tempdir().expect("home");
    let workspace = tempfile::tempdir().expect("workspace");
    std::fs::create_dir(home.path().join(".aster")).expect("config directory");
    std::fs::write(
        home.path().join(".aster/config.acl"),
        include_str!("../../../../tests/fixtures/terminal/config.acl"),
    )
    .expect("host ACL");
    let (mut child, mut stdout) = start(workspace.path(), home.path());
    let response = request(
        &mut child,
        &mut stdout,
        "initialize",
        json!({"protocolVersion":1}),
    )
    .await;
    assert_eq!(response["result"]["protocolVersion"], 1, "{response}");
    assert!(response.get("error").is_none(), "{response}");
    child.kill().await.expect("stop owned agent");
}
