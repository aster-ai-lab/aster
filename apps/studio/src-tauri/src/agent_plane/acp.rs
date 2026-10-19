//! Newline-delimited JSON-RPC over the a3s-code ACP agent's stdio.

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{Notify, mpsc, oneshot};

const STDERR_TAIL: usize = 800;
const SANDBOX_ASSETS_ENV: &str = "ASTER_SANDBOX_DIR";

/// Parent `ASTER_SANDBOX_DIR` is left as inherited. A qualified `sandbox/`
/// beside the agent is passed only when that variable is unset.
fn qualified_sibling_sandbox(binary: &Path) -> Option<std::path::PathBuf> {
    if std::env::var_os(SANDBOX_ASSETS_ENV).is_some() {
        return None;
    }
    let directory = binary.parent()?.join("sandbox");
    let qualified = [
        "a3s-oci",
        "a3s-oci-krun-shim",
        "system-image.json",
        "bundle/config.json",
        "bundle/rootfs/bin/bash",
    ]
    .into_iter()
    .all(|relative| directory.join(relative).is_file());
    if !qualified {
        return None;
    }
    directory.canonicalize().ok()
}

/// Messages the agent sends us without being asked.
#[derive(Debug)]
pub enum Inbound {
    Notification {
        method: String,
        params: Value,
    },
    Request {
        id: Value,
        method: String,
        params: Value,
    },
}

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>>;

pub struct AcpClient {
    stdin: tokio::sync::Mutex<ChildStdin>,
    next_id: AtomicU64,
    pending: Pending,
    child: tokio::sync::Mutex<Child>,
    stderr_tail: Arc<Mutex<String>>,
    exited: Arc<AtomicBool>,
    exit_notify: Arc<Notify>,
}

impl AcpClient {
    pub fn spawn(
        binary: &Path,
        args: &[String],
        cwd: &Path,
    ) -> std::io::Result<(Arc<Self>, mpsc::UnboundedReceiver<Inbound>)> {
        let (gateway_origin, gateway_key) = crate::providers::gateway_decision_env();
        let mut command = Command::new(binary);
        command
            .args(args)
            .current_dir(cwd)
            .env("A3S_GATEWAY_ORIGIN", &gateway_origin)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Some(key) = gateway_key {
            command.env("A3S_GATEWAY_KEY", key);
        }
        if let Some(directory) = qualified_sibling_sandbox(binary) {
            command.env(SANDBOX_ASSETS_ENV, directory);
        }
        let mut child = command.spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| std::io::Error::other("no stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| std::io::Error::other("no stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| std::io::Error::other("no stderr"))?;
        let pending: Pending = Arc::default();
        let stderr_tail = Arc::new(Mutex::new(String::new()));
        let exited = Arc::new(AtomicBool::new(false));
        let exit_notify = Arc::new(Notify::new());
        let (inbound, receiver) = mpsc::unbounded_channel();

        tokio::spawn(read_stdout(
            stdout,
            Arc::clone(&pending),
            inbound,
            Arc::clone(&exited),
            Arc::clone(&exit_notify),
        ));
        tokio::spawn(read_stderr(stderr, Arc::clone(&stderr_tail)));

        Ok((
            Arc::new(Self {
                stdin: tokio::sync::Mutex::new(stdin),
                next_id: AtomicU64::new(1),
                pending,
                child: tokio::sync::Mutex::new(child),
                stderr_tail,
                exited,
                exit_notify,
            }),
            receiver,
        ))
    }

    /// Resolves when the agent stdout closes. A later wait still resolves.
    pub async fn exited(&self) {
        let notified = self.exit_notify.notified();
        tokio::pin!(notified);
        if self.exited.load(Ordering::Acquire) {
            return;
        }
        notified.await;
    }

    pub async fn request(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (sender, receiver) = oneshot::channel();
        self.pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(id, sender);
        let line = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        if let Err(error) = self.write(&line).await {
            self.pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .remove(&id);
            return Err(error);
        }
        match receiver.await {
            Ok(result) => result,
            Err(_) => Err(self.exit_message()),
        }
    }

    pub async fn notify(&self, method: &str, params: Value) -> Result<(), String> {
        self.write(&json!({ "jsonrpc": "2.0", "method": method, "params": params }))
            .await
    }

    pub async fn respond(&self, id: Value, result: Value) -> Result<(), String> {
        self.write(&json!({ "jsonrpc": "2.0", "id": id, "result": result }))
            .await
    }

    pub async fn respond_error(&self, id: Value, code: i64, message: &str) -> Result<(), String> {
        self.write(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": code, "message": message },
        }))
        .await
    }

    pub async fn kill(&self) {
        let _ = self.child.lock().await.kill().await;
    }

    #[cfg(test)]
    pub async fn process_id(&self) -> Option<u32> {
        self.child.lock().await.id()
    }

    async fn write(&self, value: &Value) -> Result<(), String> {
        let mut line = value.to_string();
        line.push('\n');
        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|_| self.exit_message())?;
        stdin.flush().await.map_err(|_| self.exit_message())
    }

    fn exit_message(&self) -> String {
        let tail = self
            .stderr_tail
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .trim()
            .to_owned();
        if tail.is_empty() {
            "a3s-code exited".to_owned()
        } else {
            format!("a3s-code exited: {tail}")
        }
    }
}

async fn read_stdout(
    stdout: tokio::process::ChildStdout,
    pending: Pending,
    inbound: mpsc::UnboundedSender<Inbound>,
    exited: Arc<AtomicBool>,
    exit_notify: Arc<Notify>,
) {
    let mut lines = BufReader::new(stdout).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let method = message
            .get("method")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let id = message.get("id").cloned().filter(|id| !id.is_null());
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        match (id, method) {
            (Some(id), Some(method)) => {
                let _ = inbound.send(Inbound::Request { id, method, params });
            }
            (None, Some(method)) => {
                let _ = inbound.send(Inbound::Notification { method, params });
            }
            (Some(id), None) => {
                let Some(id) = id.as_u64() else { continue };
                let sender = pending
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .remove(&id);
                if let Some(sender) = sender {
                    let result = match message.get("error") {
                        Some(error) => Err(acp_error_text(error)),
                        None => Ok(message.get("result").cloned().unwrap_or(Value::Null)),
                    };
                    let _ = sender.send(result);
                }
            }
            (None, None) => {}
        }
    }
    // Dropping the senders fails every in-flight request with the exit message.
    pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
    exited.store(true, Ordering::Release);
    exit_notify.notify_waiters();
}

/// JSON-RPC `message` for an internal failure is the generic string
/// "Internal error". The provider reason is in `data`.
fn acp_error_text(error: &Value) -> String {
    let data = error.get("data");
    if let Some(text) = data.and_then(Value::as_str) {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            return trimmed.to_owned();
        }
    }
    if let Some(text) = data
        .and_then(|value| value.get("message"))
        .and_then(Value::as_str)
    {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            return trimmed.to_owned();
        }
    }
    error
        .get("message")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or("a3s-code request failed")
        .to_owned()
}

async fn read_stderr(mut stderr: tokio::process::ChildStderr, tail: Arc<Mutex<String>>) {
    let mut buffer = [0u8; 4096];
    while let Ok(read) = stderr.read(&mut buffer).await {
        if read == 0 {
            break;
        }
        let mut tail = tail.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        tail.push_str(&String::from_utf8_lossy(&buffer[..read]));
        if tail.len() > STDERR_TAIL {
            let cut = tail.len() - STDERR_TAIL;
            let boundary = (cut..tail.len())
                .find(|index| tail.is_char_boundary(*index))
                .unwrap_or(tail.len());
            tail.replace_range(..boundary, "");
        }
    }
}

struct AdmittedWiki {
    wiki: String,
    client: Weak<AcpClient>,
}

fn admitted_wikis() -> &'static Mutex<Vec<AdmittedWiki>> {
    static ADMITTED: std::sync::OnceLock<Mutex<Vec<AdmittedWiki>>> = std::sync::OnceLock::new();
    ADMITTED.get_or_init(|| Mutex::new(Vec::new()))
}

pub(crate) fn admit_wiki_client(wiki: &str, client: &Arc<AcpClient>) {
    if wiki.is_empty() {
        return;
    }
    let mut guard = admitted_wikis()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    guard.retain(|entry| entry.wiki != wiki && entry.client.upgrade().is_some());
    guard.push(AdmittedWiki {
        wiki: wiki.to_owned(),
        client: Arc::downgrade(client),
    });
}

fn same_directory(left: &str, right: &Path) -> bool {
    let Ok(left) = std::fs::canonicalize(left) else {
        return false;
    };
    let Ok(right) = std::fs::canonicalize(right) else {
        return false;
    };
    left == right
}

/// Ask the a3s-code process that already owns this wiki. No session means not-ready.
pub(crate) async fn search_admitted_wiki(wiki: &Path, query: &str) -> Result<Value, String> {
    let client = {
        let guard = admitted_wikis()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        guard
            .iter()
            .find(|entry| same_directory(&entry.wiki, wiki))
            .and_then(|entry| entry.client.upgrade())
    };
    let Some(client) = client else {
        return Ok(json!({ "phase": "not-ready", "hits": [] }));
    };
    let value = match client
        .request(
            "_a3s/wiki/search",
            json!({ "wiki": wiki.display().to_string(), "query": query }),
        )
        .await
    {
        Ok(value) => value,
        Err(_) => return Ok(json!({ "phase": "not-ready", "hits": [] })),
    };
    Ok(published_search_body(&value))
}

fn published_search_body(value: &Value) -> Value {
    let phase = value
        .get("phase")
        .and_then(Value::as_str)
        .unwrap_or("not-ready");
    if phase != "published" {
        return json!({ "phase": "not-ready", "hits": [] });
    }
    let hits = value
        .get("hits")
        .and_then(Value::as_array)
        .map(|hits| {
            hits.iter()
                .filter_map(|hit| {
                    let name = hit.get("name").and_then(Value::as_str)?;
                    let passage = hit.get("passage").and_then(Value::as_str)?;
                    Some(json!({ "name": name, "passage": passage }))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({ "phase": "published", "hits": hits })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_review_error_prefers_the_provider_reason() {
        let nested = json!({
            "code": -32603,
            "message": "Internal error",
            "data": { "message": "context length exceeded" }
        });
        assert_eq!(acp_error_text(&nested), "context length exceeded");
        let text = json!({
            "message": "Internal error",
            "data": "payload too large"
        });
        assert_eq!(acp_error_text(&text), "payload too large");
        assert_eq!(
            acp_error_text(&json!({ "message": "rate limit" })),
            "rate limit"
        );
    }
}

#[cfg(all(test, unix))]
mod sandbox_dir_handoff {
    use super::AcpClient;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    fn lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    struct EnvGuard {
        previous: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn set(value: Option<&std::ffi::OsStr>) -> Self {
            let previous = std::env::var_os("ASTER_SANDBOX_DIR");
            write_sandbox_dir(value);
            Self { previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            write_sandbox_dir(self.previous.take().as_deref());
        }
    }

    fn write_sandbox_dir(value: Option<&std::ffi::OsStr>) {
        // SAFETY: every sandbox_dir test holds `lock` until this guard drops.
        match value {
            Some(value) => unsafe { std::env::set_var("ASTER_SANDBOX_DIR", value) },
            None => unsafe { std::env::remove_var("ASTER_SANDBOX_DIR") },
        }
    }

    fn write_agent(dir: &Path) -> PathBuf {
        let path = dir.join("agent");
        std::fs::write(
            &path,
            "#!/bin/sh\nprintf '%s' \"$ASTER_SANDBOX_DIR\" > sandbox-dir\n",
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).unwrap();
        path
    }

    fn write_companions(sandbox: &Path) {
        for relative in [
            "a3s-oci",
            "a3s-oci-krun-shim",
            "system-image.json",
            "bundle/config.json",
            "bundle/rootfs/bin/bash",
        ] {
            let path = sandbox.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"stub").unwrap();
        }
    }

    async fn written(cwd: &Path, agent: &Path) -> String {
        let (client, _inbound) = AcpClient::spawn(agent, &[], cwd).expect("script agent starts");
        let output = cwd.join("sandbox-dir");
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while !output.is_file() {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("agent script did not write sandbox-dir");
        drop(client);
        std::fs::read_to_string(output).unwrap()
    }

    #[tokio::test]
    async fn inherited_sandbox_dir_reaches_the_agent_process() {
        let _lock = lock();
        let root = tempfile::tempdir().unwrap();
        let explicit = root.path().join("explicit-dir");
        let _env = EnvGuard::set(Some(explicit.as_os_str()));
        let agent = write_agent(root.path());
        assert_eq!(
            written(root.path(), &agent).await,
            explicit.to_string_lossy()
        );
    }

    #[tokio::test]
    async fn valid_sibling_sandbox_dir_is_passed_when_unset() {
        let _lock = lock();
        let _env = EnvGuard::set(None);
        let root = tempfile::tempdir().unwrap();
        let bin = root.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let agent = write_agent(&bin);
        let sandbox = bin.join("sandbox");
        write_companions(&sandbox);
        assert_eq!(
            written(root.path(), &agent).await,
            sandbox.canonicalize().unwrap().to_string_lossy()
        );
    }

    #[tokio::test]
    async fn missing_sibling_sandbox_dir_is_not_invented() {
        let _lock = lock();
        let _env = EnvGuard::set(None);
        let root = tempfile::tempdir().unwrap();
        let agent = write_agent(root.path());
        let text = written(root.path(), &agent).await;
        assert!(text.is_empty(), "{text}");
        assert!(!text.contains("Library/Caches/aster/sandbox"));
    }

    #[tokio::test]
    async fn existing_sandbox_dir_wins_over_the_sibling() {
        let _lock = lock();
        let root = tempfile::tempdir().unwrap();
        let explicit = root.path().join("kept-explicit");
        let _env = EnvGuard::set(Some(explicit.as_os_str()));
        let bin = root.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let agent = write_agent(&bin);
        write_companions(&bin.join("sandbox"));
        assert_eq!(
            written(root.path(), &agent).await,
            explicit.to_string_lossy()
        );
    }
}

#[cfg(all(test, target_os = "macos", target_arch = "aarch64"))]
#[path = "sandbox_session_e2e.rs"]
mod sandbox_session_e2e;
