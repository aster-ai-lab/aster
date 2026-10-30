//! Codex directory discovery through the actual ACP model catalog, using
//! synthetic logins and child-local environment variables. No model is called.

use std::ffi::OsStr;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

const FUTURE_JWT: &str =
    "eyJhbGciOiJub25lIn0.eyJlbWFpbCI6InVzZXJAZXhhbXBsZS50ZXN0IiwiZXhwIjo0MTAyNDQ0ODAwfQ.sig";

#[tokio::test]
async fn codex_default_directory_follows_host_home() {
    for home in [None, Some(""), Some("home")] {
        let expected = if home == Some("home") {
            "codex/home-model"
        } else {
            "codex/profile-model"
        };
        assert_eq!(catalog(home, None, None).await, vec![expected]);
    }
}

#[tokio::test]
async fn explicit_codex_homes_keep_their_priority() {
    for (aster_home, codex_home, expected) in [
        (Some("aster"), Some("codex"), "codex/aster-model"),
        (Some("aster"), None, "codex/aster-model"),
        (None, Some("codex"), "codex/codex-model"),
        (Some(""), Some("codex"), "codex/codex-model"),
        (Some(""), Some(""), "codex/home-model"),
    ] {
        assert_eq!(
            catalog(Some("home"), aster_home, codex_home).await,
            vec![expected]
        );
    }
}

async fn catalog(
    home: Option<&str>,
    aster_home: Option<&str>,
    codex_home: Option<&str>,
) -> Vec<String> {
    let root = tempfile::tempdir().expect("fixture root");
    let workspace = root.path().join("workspace 空间");
    std::fs::create_dir(&workspace).expect("workspace");
    let profile = root.path().join("profile 用户");
    let user_home = root.path().join("home 用户");
    let aster = root.path().join("aster 用户");
    let codex = root.path().join("codex 用户");
    for (directory, model) in [
        (profile.join(".codex"), "profile-model"),
        (user_home.join(".codex"), "home-model"),
        (aster.clone(), "aster-model"),
        (codex.clone(), "codex-model"),
    ] {
        write_login(&directory, model);
    }
    let config = root.path().join("config.acl");
    std::fs::write(&config, "import_accounts = [\"codex\"]\n").expect("trusted ACL");

    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_a3s-code-acp"));
    command
        .current_dir(&workspace)
        .args([OsStr::new("--config"), config.as_os_str()])
        .env("USERPROFILE", &profile)
        .env("A3S_HOME", root.path().join("state"))
        .env("A3S_STATE_HOME", root.path().join("state"))
        .env("A3S_CODE_LOG_FILE", "0")
        .env("A3S_DISABLE_CODEX_ACCOUNT", "0")
        .env_remove("A3S_DEFAULT_MODEL")
        .env_remove("A3S_CONFIG")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    for (key, value, directory) in [
        ("HOME", home, &user_home),
        ("A3S_CODEX_HOME", aster_home, &aster),
        ("CODEX_HOME", codex_home, &codex),
    ] {
        match value {
            None => command.env_remove(key),
            Some("") => command.env(key, ""),
            Some(_) => command.env(key, directory),
        };
    }
    let mut child = command.spawn().expect("start ACP");
    let mut stdin = child.stdin.take().expect("ACP stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("ACP stdout"));
    let request = json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"protocolVersion": 1, "clientCapabilities": {}}
    });
    stdin
        .write_all(format!("{request}\n").as_bytes())
        .await
        .expect("send initialize");
    stdin.flush().await.expect("flush initialize");
    let mut line = String::new();
    let read = tokio::time::timeout(Duration::from_secs(10), stdout.read_line(&mut line))
        .await
        .expect("bounded initialize")
        .expect("read initialize");
    assert!(read > 0, "ACP closed before initialization");
    let response: Value = serde_json::from_str(&line).expect("initialize JSON");
    assert!(response.get("result").is_some(), "{response}");
    child.kill().await.expect("stop ACP");
    child.wait().await.expect("reap ACP");
    response["result"]["_meta"]["modelState"]["availableModels"]
        .as_array()
        .expect("initialized model catalog")
        .iter()
        .map(|model| model["modelId"].as_str().expect("model id").to_owned())
        .collect()
}

fn write_login(directory: &Path, model: &str) {
    std::fs::create_dir_all(directory).expect("Codex directory");
    std::fs::write(
        directory.join("auth.json"),
        json!({"tokens": {"access_token": FUTURE_JWT, "account_id": "fixture-account"}})
            .to_string(),
    )
    .expect("synthetic Codex login");
    std::fs::write(
        directory.join("models_cache.json"),
        json!({"models": [{"slug": model, "visibility": "list", "supported_in_api": true}]})
            .to_string(),
    )
    .expect("synthetic model cache");
}
