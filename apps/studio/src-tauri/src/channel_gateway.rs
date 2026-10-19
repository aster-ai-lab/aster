//! Supervises the node process that carries ToBot traffic for message channels.
//! The renderer stores the channel list and posts it here. This module writes a
//! private config and starts `scripts/channel-gateway.mjs` when one channel can
//! serve ToBot. WeChat stays in the list and does not start an adapter.
//! Ready rules match that script.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

use serde_json::{Value, json};

struct Gateway {
    child: Option<Child>,
    plane_url: String,
}

static GATEWAY: Mutex<Option<Gateway>> = Mutex::new(None);

pub fn boot(token: &str, plane_url: &str) {
    if cfg!(test) || token.len() < 8 {
        return;
    }
    let mut guard = GATEWAY
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if guard.is_none() {
        *guard = Some(Gateway {
            child: None,
            plane_url: plane_url.to_owned(),
        });
    }
}

pub fn channels_from_body(body: &[u8]) -> Result<Vec<Value>, u16> {
    let value: Value = serde_json::from_slice(body).map_err(|_| 400u16)?;
    let rows = value
        .get("channels")
        .and_then(Value::as_array)
        .ok_or(400u16)?;
    if rows.len() > 64 {
        return Err(400);
    }
    Ok(rows.clone())
}

/// An active channel whose handler is ToBot and whose credentials can connect.
pub fn channel_serves_tobot(channel: &Value) -> bool {
    if channel.get("isActive").and_then(Value::as_bool) != Some(true) {
        return false;
    }
    if !serves_agent(channel) {
        return false;
    }
    let kind = channel.get("type").and_then(Value::as_str).unwrap_or("");
    let config = channel.get("config").cloned().unwrap_or(Value::Null);
    ready(kind, &config)
}

fn serves_agent(channel: &Value) -> bool {
    match channel.get("agentId") {
        None | Some(Value::Null) => true,
        Some(Value::String(id)) => {
            let trimmed = id.trim();
            trimmed.is_empty() || trimmed == "tobot"
        }
        Some(_) => false,
    }
}

fn field<'a>(config: &'a Value, key: &str) -> &'a str {
    config.get(key).and_then(Value::as_str).unwrap_or("").trim()
}

fn ready(kind: &str, config: &Value) -> bool {
    match kind {
        "telegram" | "discord" => !field(config, "bot_token").is_empty(),
        "feishu" => !field(config, "app_id").is_empty() && !field(config, "app_secret").is_empty(),
        "slack" => !field(config, "bot_token").is_empty() && !field(config, "app_token").is_empty(),
        "qq" => !field(config, "app_id").is_empty() && !field(config, "client_secret").is_empty(),
        _ => false,
    }
}

pub fn apply(token: &str, plane_url: Option<&str>, body: &[u8]) -> Result<String, u16> {
    let channels = channels_from_body(body)?;
    if cfg!(test) {
        return Ok(json!({ "ok": true }).to_string());
    }
    let Some(prefix) = token.get(..8) else {
        return Err(400);
    };
    let plane = plane_url
        .map(str::to_owned)
        .or_else(|| {
            GATEWAY
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .as_ref()
                .map(|gateway| gateway.plane_url.clone())
        })
        .unwrap_or_default();
    let path = std::env::temp_dir().join(format!("a3s-studio-channels-{prefix}.json"));
    let document = json!({
        "planeUrl": plane,
        "token": token,
        "channels": channels,
    });
    let bytes = serde_json::to_vec(&document).map_err(|_| 500u16)?;
    write_private(&path, &bytes).map_err(|_| 500u16)?;
    let wanted = !plane.is_empty() && channels.iter().any(channel_serves_tobot);
    let mut guard = GATEWAY
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let gateway = guard.get_or_insert_with(|| Gateway {
        child: None,
        plane_url: plane.clone(),
    });
    if !plane.is_empty() {
        gateway.plane_url = plane;
    }
    if wanted {
        ensure_child(gateway, &path);
    } else {
        stop_child(&mut gateway.child);
    }
    Ok(json!({ "ok": true }).to_string())
}

fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../scripts/channel-gateway.mjs")
}

fn child_is_running(child: &mut Child) -> bool {
    matches!(child.try_wait(), Ok(None))
}

fn ensure_child(gateway: &mut Gateway, config_path: &Path) {
    if gateway.child.as_mut().is_some_and(child_is_running) {
        return;
    }
    gateway.child = None;
    let script = script_path();
    if !script.is_file() {
        eprintln!("studio: channel gateway script is missing");
        return;
    }
    let mut command = Command::new("node");
    command.arg(&script).arg(config_path);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    if let Some(dir) = script.parent().and_then(Path::parent) {
        command.current_dir(dir);
    }
    match command.spawn() {
        Ok(child) => gateway.child = Some(child),
        Err(err) => eprintln!("studio: channel gateway did not start: {err}"),
    }
}

fn stop_child(slot: &mut Option<Child>) {
    let Some(child) = slot.as_mut() else {
        return;
    };
    let _ = child.kill();
    let _ = child.wait();
    *slot = None;
}

#[cfg(test)]
mod tests {
    use super::{channel_serves_tobot, channels_from_body};
    use serde_json::json;

    #[test]
    fn accepts_a_channel_list_and_rejects_a_bad_body() {
        let rows = channels_from_body(br#"{"channels":[{"id":"1","type":"telegram"}]}"#)
            .expect("channels");
        assert_eq!(rows.len(), 1);
        assert_eq!(channels_from_body(b"{}").unwrap_err(), 400);
        assert_eq!(channels_from_body(b"not-json").unwrap_err(), 400);
    }

    #[test]
    fn starts_only_an_active_tobot_channel_with_credentials() {
        let telegram = json!({
            "type": "telegram",
            "isActive": true,
            "agentId": "tobot",
            "config": { "bot_token": "token" }
        });
        assert!(channel_serves_tobot(&telegram));
        let unbound = json!({
            "type": "discord",
            "isActive": true,
            "agentId": null,
            "config": { "bot_token": "token" }
        });
        assert!(channel_serves_tobot(&unbound));
        let other = json!({
            "type": "telegram",
            "isActive": true,
            "agentId": "agent-1",
            "config": { "bot_token": "token" }
        });
        assert!(!channel_serves_tobot(&other));
        let idle = json!({
            "type": "feishu",
            "isActive": true,
            "config": { "app_id": "", "app_secret": "" }
        });
        assert!(!channel_serves_tobot(&idle));
        let wechat = json!({
            "type": "wechat",
            "isActive": true,
            "agentId": "tobot",
            "config": { "token_path": "/tmp/wechat" }
        });
        assert!(!channel_serves_tobot(&wechat));
        let slack = json!({
            "type": "slack",
            "isActive": true,
            "config": { "bot_token": "xoxb", "app_token": "" }
        });
        assert!(!channel_serves_tobot(&slack));
        let qq = json!({
            "type": "qq",
            "isActive": true,
            "config": { "app_id": "app", "client_secret": "secret" }
        });
        assert!(channel_serves_tobot(&qq));
    }
}
