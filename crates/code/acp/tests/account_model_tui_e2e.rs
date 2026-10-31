//! PTY end-to-end for model selection: the shipped TUI, the shipped ACP
//! agent, and a fixture login for the Codex account import.
//! No account endpoint is dialed — these tests cover the picker catalog,
//! filtering, and switching, not turns. The wire paths are proven by the
//! importer unit tests and `codex_login_roundtrip`.

#![cfg(test)]

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, CommandBuilder, PtySize};

/// A JWT the parser accepts (header.payload.sig), exp far in the future.
const FUTURE_JWT: &str =
    "eyJhbGciOiJub25lIn0.eyJlbWFpbCI6InVzZXJAZXhhbXBsZS50ZXN0IiwiZXhwIjo0MTAyNDQ0ODAwfQ.sig";
const EXPIRED_JWT: &str =
    "eyJhbGciOiJub25lIn0.eyJlbWFpbCI6InVzZXJAZXhhbXBsZS50ZXN0IiwiZXhwIjo5NDY2ODQ4MDB9.sig";
const CODEX_EMAIL: &str = "user@example.test";

struct Fixture {
    _root: tempfile::TempDir,
    workspace: PathBuf,
    config: PathBuf,
    env: Vec<(String, String)>,
}

fn write_codex_home(root: &Path, access_token: &str) -> PathBuf {
    let home = root.join("codex-home");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        home.join("auth.json"),
        serde_json::json!({
            "auth_mode": "chatgpt",
            "tokens": {
                "id_token": FUTURE_JWT,
                "access_token": access_token,
                "refresh_token": "refresh-secret",
                "account_id": "codex-acc-1"
            }
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(
        home.join("models_cache.json"),
        serde_json::json!({
            "models": [
                {"slug": "gpt-6.1-sol", "display_name": "GPT-6.1-Sol", "context_window": 500000, "visibility": "list", "supported_in_api": true},
                {"slug": "hidden-one", "display_name": "Hidden One", "context_window": 272000, "visibility": "hide", "supported_in_api": true},
                {"slug": "gpt-6-luna", "display_name": "GPT-6-Luna", "context_window": 272000, "visibility": "list", "supported_in_api": true},
                {"slug": "not-in-api", "display_name": "Not In API", "context_window": 272000, "visibility": "list", "supported_in_api": false}
            ]
        })
        .to_string(),
    )
    .unwrap();
    home
}

/// A bootable ACL: the discard-port mock provider carries the default so no
/// imported account has to be dialed to start or switch.
fn prepare(import_accounts: &str) -> Fixture {
    let root = tempfile::tempdir().expect("temp");
    let home = root.path().join("home");
    let workspace = root.path().join("workspace");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&workspace).unwrap();
    let config = home.join("config.acl");
    std::fs::write(
        &config,
        format!(
            r#"
providers "mock" {{
  api_key = "mock-key"
  base_url = "http://127.0.0.1:9/v1"
  models "m1" {{ }}
}}
default_model = "mock/m1"
import_accounts = [{import_accounts}]
"#
        ),
    )
    .unwrap();
    let aster_home = root.path().join("aster-home");
    std::fs::create_dir_all(&aster_home).unwrap();
    // env_clear() strips the launcher's environment; keep a usable PATH for
    // child tools the TUI may shell out to.
    let mut path = "/usr/bin:/bin".to_string();
    if let Some(parent) = std::env::var_os("PATH") {
        path.push(':');
        path.push_str(&parent.to_string_lossy());
    }
    let env = vec![
        (
            "ASTER_ACP_AGENT_BIN".to_string(),
            env!("CARGO_BIN_EXE_aster-acp").to_string(),
        ),
        ("ASTER_HOME".to_string(), aster_home.display().to_string()),
        ("ASTER_CONFIG".to_string(), config.display().to_string()),
        ("ASTER_DISABLE_CC_SWITCH".to_string(), "1".to_string()),
        ("ASTER_DISABLE_GROK_ACCOUNT".to_string(), "1".to_string()),
        ("ASTER_DISABLE_CURSOR_ACCOUNT".to_string(), "1".to_string()),
        ("ASTER_DISABLE_CODEX_ACCOUNT".to_string(), "1".to_string()),
        ("HOME".to_string(), home.display().to_string()),
        (
            "TMPDIR".to_string(),
            std::env::temp_dir().display().to_string(),
        ),
        ("TERM".to_string(), "xterm-256color".to_string()),
        ("PATH".to_string(), path),
    ];
    Fixture {
        _root: root,
        workspace,
        config,
        env,
    }
}

fn with_env(fixture: &Fixture, extra: &[(&str, &str)]) -> Vec<(String, String)> {
    let mut env = fixture.env.clone();
    for (key, value) in extra {
        if let Some(entry) = env.iter_mut().find(|(name, _)| name == key) {
            entry.1 = value.to_string();
        } else {
            env.push((key.to_string(), value.to_string()));
        }
    }
    env
}

fn as_refs(env: &[(String, String)]) -> Vec<(&str, &str)> {
    env.iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect()
}

fn redact(screen: &str) -> String {
    screen
        .replace(FUTURE_JWT, "[jwt]")
        .replace(EXPIRED_JWT, "[jwt]")
        .replace(CODEX_EMAIL, "[redacted]")
}

fn assert_screen_has_no_secret(screen: &str) {
    for secret in [FUTURE_JWT, EXPIRED_JWT, CODEX_EMAIL] {
        assert!(
            !screen.contains(secret),
            "screen showed a fixture credential\n{}",
            redact(screen)
        );
    }
}

fn tui_binary() -> PathBuf {
    let binary = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../target/debug/aster");
    assert!(
        binary.is_file(),
        "build aster before this test: {}",
        binary.display()
    );
    binary
}

struct PtySession {
    parser: vt100::Parser,
    rx: std::sync::mpsc::Receiver<Vec<u8>>,
    writer: Box<dyn Write>,
    child: Box<dyn portable_pty::Child>,
    raw: Vec<u8>,
    cpr_replied_through: usize,
}

static OPENED: AtomicUsize = AtomicUsize::new(0);

fn open_pty(binary: &Path, cwd: &Path, env: &[(&str, &str)]) -> PtySession {
    let rows = 40;
    let cols = 120;
    let system = native_pty_system();
    let pair = system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("pty");
    let mut command = CommandBuilder::new(binary);
    command.env_clear();
    command.arg("--cwd");
    command.arg(cwd);
    for (key, value) in env {
        command.env(key, value);
    }
    command.cwd(cwd);
    let child = pair.slave.spawn_command(command).expect("spawn tui");
    let mut reader = pair.master.try_clone_reader().expect("reader");
    let writer = pair.master.take_writer().expect("writer");
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = [0_u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    });
    OPENED.fetch_add(1, Ordering::SeqCst);
    PtySession {
        parser: vt100::Parser::new(rows, cols, 1000),
        rx,
        writer,
        child,
        raw: Vec::new(),
        cpr_replied_through: 0,
    }
}

impl Drop for PtySession {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

impl PtySession {
    fn drain(&mut self) {
        while let Ok(bytes) = self.rx.try_recv() {
            self.raw.extend_from_slice(&bytes);
            self.parser.process(&bytes);
        }
        let query = b"\x1b[6n";
        if let Some(at) = self
            .raw
            .windows(query.len())
            .rposition(|window| window == query)
        {
            if at >= self.cpr_replied_through {
                self.cpr_replied_through = self.raw.len();
                let _ = self.writer.write_all(b"\x1b[12;1R");
                let _ = self.writer.flush();
            }
        }
    }

    fn screen(&mut self) -> String {
        self.drain();
        self.parser.screen().contents()
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        if let Err(error) = self.writer.write_all(bytes) {
            panic!("pty write failed: {error}\n{}", redact(&self.screen()));
        }
        self.writer.flush().ok();
    }

    fn type_text(&mut self, text: &str) {
        self.write_bytes(text.as_bytes());
    }

    fn enter(&mut self) {
        self.write_bytes(b"\r");
    }

    fn escape(&mut self) {
        self.write_bytes(b"\x1b");
    }

    fn clear_line(&mut self) {
        self.write_bytes(b"\x15");
    }

    fn wait_for(&mut self, needle: &str, timeout: Duration) -> String {
        let deadline = Instant::now() + timeout;
        let mut last = String::new();
        while Instant::now() < deadline {
            last = self.screen();
            if last.contains(needle) {
                return last;
            }
            std::thread::sleep(Duration::from_millis(40));
        }
        panic!("timed out waiting for {needle:?}\n{}", redact(&last));
    }

    fn quit(&mut self) {
        self.clear_line();
        self.escape();
        std::thread::sleep(Duration::from_millis(450));
        self.clear_line();
        self.type_text("/quit");
        self.escape();
        std::thread::sleep(Duration::from_millis(450));
        self.enter();
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            self.drain();
            match self.child.try_wait() {
                Ok(Some(_)) | Err(_) => return,
                Ok(None) => std::thread::sleep(Duration::from_millis(40)),
            }
        }
    }
}

fn wait_ready(pty: &mut PtySession, needle: &str, timeout: Duration) -> String {
    let deadline = Instant::now() + timeout;
    let mut last = String::new();
    while Instant::now() < deadline {
        last = pty.screen();
        if last.contains("Do you trust the contents of this directory") {
            pty.type_text("y");
            std::thread::sleep(Duration::from_millis(200));
            continue;
        }
        if last.contains(needle) {
            return last;
        }
        std::thread::sleep(Duration::from_millis(40));
    }
    panic!("timed out waiting for {needle:?}\n{}", redact(&last));
}

/// Open the live `/model` picker and hand back the first rendered page.
/// Open the live `/model` picker and step Right across the provider tabs
/// until the wanted provider's rows render. The picker opens on the
/// current provider's tab, so an imported account needs its tab selected
/// before its rows — and filter — exist.
fn open_model_picker(pty: &mut PtySession, needle: &str) -> String {
    pty.escape();
    pty.clear_line();
    // Bracketed paste, as a human composer does.
    pty.write_bytes(b"\x1b[200~/model \x1b[201~");
    std::thread::sleep(Duration::from_millis(700));
    for _ in 0..10 {
        let screen = pty.screen();
        if screen.contains(needle) {
            return screen;
        }
        pty.write_bytes(b"\x1b[C");
        std::thread::sleep(Duration::from_millis(300));
    }
    panic!(
        "no provider tab showed {needle:?}\n{}",
        redact(&pty.screen())
    );
}

/// Type a filter, Enter, then absorb a possible effort phase with one more
/// Enter. Returns once the footer carries `footer_needle`.
fn switch_model(pty: &mut PtySession, filter: &str, footer_needle: &str) {
    pty.type_text(filter);
    std::thread::sleep(Duration::from_millis(250));
    let narrowed = pty.screen();
    assert!(
        narrowed.to_lowercase().contains(&filter.to_lowercase()),
        "filter {filter:?} did not take\n{}",
        redact(&narrowed)
    );
    assert_screen_has_no_secret(&narrowed);
    // Enter 1 selects the row into the composer command, Enter 2 settles
    // the effort phase when the picker asks, Enter 3 submits the command.
    pty.enter();
    std::thread::sleep(Duration::from_millis(400));
    pty.enter();
    std::thread::sleep(Duration::from_millis(400));
    pty.enter();
    let settled = pty.wait_for(footer_needle, Duration::from_secs(15));
    assert!(
        !settled.contains("Couldn't switch model"),
        "model switch failed\n{}",
        redact(&settled)
    );
    assert_screen_has_no_secret(&settled);
}

fn assert_config_clean(fixture: &Fixture) {
    let acl = std::fs::read_to_string(&fixture.config).unwrap_or_default();
    for secret in [FUTURE_JWT, EXPIRED_JWT, CODEX_EMAIL] {
        assert!(!acl.contains(secret), "acl contained a fixture credential");
    }
}

#[test]
fn codex_models_list_filter_and_switch() {
    let fixture = prepare("\"codex\"");
    let codex_home = write_codex_home(fixture._root.path(), FUTURE_JWT);
    let env = with_env(
        &fixture,
        &[
            ("ASTERX_HOME", codex_home.to_str().unwrap()),
            ("ASTER_DISABLE_CODEX_ACCOUNT", "0"),
        ],
    );
    let refs = as_refs(&env);
    let mut pty = open_pty(&tui_binary(), &fixture.workspace, &refs);
    wait_ready(&mut pty, "mock/m1", Duration::from_secs(45));

    let picker = open_model_picker(&mut pty, "Logged-in Codex account");
    assert!(
        picker.contains("codex · GPT-6.1-Sol") && picker.contains("codex · GPT-6-Luna"),
        "picker missed the visible codex models\n{}",
        redact(&picker)
    );
    assert!(
        !picker.contains("Hidden One") && !picker.contains("Not In API"),
        "hidden or api-excluded codex models leaked into the picker\n{}",
        redact(&picker)
    );
    assert_screen_has_no_secret(&picker);

    switch_model(&mut pty, "luna", "codex · GPT-6-Luna");
    // Switch back within the same provider: filtering and Enter again.
    open_model_picker(&mut pty, "Logged-in Codex account");
    switch_model(&mut pty, "6.1", "codex · GPT-6.1-Sol");

    pty.quit();
    assert_config_clean(&fixture);
}

#[test]
fn an_expired_codex_token_stays_out_of_the_picker() {
    let fixture = prepare("\"codex\"");
    let codex_home = write_codex_home(fixture._root.path(), EXPIRED_JWT);
    let env = with_env(
        &fixture,
        &[
            ("ASTERX_HOME", codex_home.to_str().unwrap()),
            ("ASTER_DISABLE_CODEX_ACCOUNT", "0"),
        ],
    );
    let refs = as_refs(&env);
    let mut pty = open_pty(&tui_binary(), &fixture.workspace, &refs);
    wait_ready(&mut pty, "mock/m1", Duration::from_secs(45));

    open_model_picker(&mut pty, "From Aster ACL provider mock");
    std::thread::sleep(Duration::from_millis(600));
    let settled = pty.screen();
    assert!(
        !settled.contains("codex"),
        "an expired codex login reached the picker\n{}",
        redact(&settled)
    );
    assert_screen_has_no_secret(&settled);
    pty.quit();
    assert_config_clean(&fixture);
}
