//! Shipped `aster` session connect. One test covers the ACP child, the missing
//! companion, the ACL model, leader forwarding, auto-update, and `agent serve`.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

const ACL_MODEL: &str = "fixture/from-acl";
const TOML_MODEL: &str = "toml/not-this";
const SESSION_TIMEOUT: Duration = Duration::from_secs(180);
const SERVE_TIMEOUT: Duration = Duration::from_secs(90);

fn shipped_aster() -> &'static str {
    env!("CARGO_BIN_EXE_aster")
}

fn temp_root(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!(
        "aster-one-agent-{label}-{}-{nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("temp");
    path
}

fn write_preferences(home: &Path) {
    let dir = home.join(".a3s");
    fs::create_dir_all(&dir).expect("a3s home");
    fs::write(
        dir.join("config.acl"),
        r#"
default_model = "fixture/from-acl"

providers "fixture" {
  api_key = "fixture-key"
  base_url = "http://127.0.0.1:9/v1/"
  models "from-acl" { name = "From ACL" }
}
"#,
    )
    .expect("acl");
    fs::write(
        dir.join("config.toml"),
        r#"
[ui]
theme = "dark"
vim_mode = true

[models]
default = "toml/not-this"
"#,
    )
    .expect("toml");
}

fn child_env(command: &mut portable_pty::CommandBuilder, home: &Path, workspace: &Path) {
    command.env_clear();
    command.env("HOME", home);
    command.env("PATH", "/usr/bin:/bin:/usr/local/bin");
    command.env("TERM", "xterm-256color");
    command.env("A3S_DISABLE_CC_SWITCH", "1");
    command.env("A3S_DISABLE_GROK_ACCOUNT", "1");
    command.env("A3S_DISABLE_CURSOR_ACCOUNT", "1");
    command.cwd(workspace);
    if let Some(tmpdir) = std::env::var_os("TMPDIR") {
        command.env("TMPDIR", tmpdir);
    }
}

fn shell_env(command: &mut Command, home: &Path) {
    command.env_clear();
    command.env("HOME", home);
    command.env("PATH", "/usr/bin:/bin:/usr/local/bin");
    command.env("TERM", "xterm-256color");
    command.env("A3S_DISABLE_CC_SWITCH", "1");
    command.env("A3S_DISABLE_GROK_ACCOUNT", "1");
    command.env("A3S_DISABLE_CURSOR_ACCOUNT", "1");
    if let Some(tmpdir) = std::env::var_os("TMPDIR") {
        command.env("TMPDIR", tmpdir);
    }
}

struct PtySession {
    child: Box<dyn portable_pty::Child + Send + Sync>,
    writer: Box<dyn Write + Send>,
    rx: std::sync::mpsc::Receiver<Vec<u8>>,
    raw: Vec<u8>,
    cpr_replied_through: usize,
    trust_answered: bool,
}

impl Drop for PtySession {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

impl PtySession {
    fn spawn(program: &Path, args: &[&str], home: &Path, workspace: &Path) -> Self {
        let system = portable_pty::native_pty_system();
        let pair = system
            .openpty(portable_pty::PtySize {
                rows: 40,
                cols: 120,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("pty");
        let mut command = portable_pty::CommandBuilder::new(program);
        for arg in args {
            command.arg(*arg);
        }
        child_env(&mut command, home, workspace);
        let child = pair.slave.spawn_command(command).expect("spawn aster");
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().expect("pty reader");
        let writer = pair.master.take_writer().expect("pty writer");
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
        Self {
            child,
            writer,
            rx,
            raw: Vec::new(),
            cpr_replied_through: 0,
            trust_answered: false,
        }
    }

    fn pump(&mut self) {
        while let Ok(bytes) = self.rx.try_recv() {
            self.raw.extend_from_slice(&bytes);
        }
        let query = b"\x1b[6n";
        if let Some(at) = self
            .raw
            .windows(query.len())
            .rposition(|window| window == query)
            && at >= self.cpr_replied_through
        {
            self.cpr_replied_through = self.raw.len();
            let _ = self.writer.write_all(b"\x1b[12;1R");
            let _ = self.writer.flush();
        }
        if !self.trust_answered {
            let text = String::from_utf8_lossy(&self.raw);
            if text.contains("Do you trust the contents of this directory") {
                let _ = self.writer.write_all(b"y");
                let _ = self.writer.flush();
                self.trust_answered = true;
            }
        }
    }

    fn exited(&mut self) -> Option<u32> {
        self.child
            .try_wait()
            .ok()
            .flatten()
            .map(|status| status.exit_code())
    }
}

fn acp_log(home: &Path) -> String {
    let dir = home.join(".a3s").join("logs");
    let Ok(entries) = fs::read_dir(&dir) else {
        return String::new();
    };
    let mut text = String::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("a3s-code-acp.") && name.ends_with(".log") {
            if let Ok(body) = fs::read_to_string(entry.path()) {
                text.push_str(&body);
            }
        }
    }
    text
}

fn session_line(log: &str) -> Option<String> {
    log.lines()
        .find(|line| line.contains("acp session ready"))
        .map(str::to_string)
}

fn tail(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut chars = text.chars();
    let count = chars.clone().count();
    if count > 1500 {
        chars.nth(count - 1500);
    }
    chars.collect()
}

fn assert_acp_session(home: &Path, leader: bool) {
    let root = temp_root(if leader { "leader" } else { "default" });
    let workspace = root.join("workspace");
    fs::create_dir_all(&workspace).expect("workspace");
    write_preferences(home);
    let mut args = vec!["--cwd", workspace.to_str().expect("cwd utf-8"), "--trust"];
    if leader {
        args.push("--leader");
    }
    let mut pty = PtySession::spawn(Path::new(shipped_aster()), &args, home, &workspace);
    let deadline = Instant::now() + SESSION_TIMEOUT;
    let line = loop {
        pty.pump();
        if let Some(line) = session_line(&acp_log(home)) {
            break line;
        }
        if let Some(code) = pty.exited() {
            panic!(
                "aster exited {code} before the ACP session\n{}",
                tail(&pty.raw)
            );
        }
        if Instant::now() >= deadline {
            panic!(
                "timed out waiting for acp session ready\nlog-hints:\n{}\nterminal:\n{}",
                log_hints(&acp_log(home)),
                tail(&pty.raw)
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(
        line.contains(ACL_MODEL),
        "prompt model must come from the ACL: {line}"
    );
    assert!(
        !line.contains(TOML_MODEL),
        "TOML theme and key settings must not select the model: {line}"
    );
    assert!(
        line.contains("command_tool=\"bash\"") || line.contains("command_tool=bash"),
        "command tool must be bash: {line}"
    );
    assert!(
        !line.contains("run_terminal_command"),
        "leader and default sessions use the ACP bash tool: {line}"
    );
    let shell_log = home.join(".a3s").join("shell-agent.log");
    assert!(
        !shell_log.exists(),
        "the interactive session constructed the in-process shell agent"
    );
    let forward = home.join(".a3s").join("leader-forward.log");
    if leader {
        let body = fs::read_to_string(&forward).unwrap_or_default();
        assert!(
            body.contains("leader connection forwarded to ACP child"),
            "leader connection did not forward: {body}"
        );
    } else {
        assert!(
            !forward.exists(),
            "the default session wrote a leader forward marker"
        );
    }
    println!(
        "CAPTURE {} spawned the ACP companion command_tool=bash model={ACL_MODEL} and did not construct the in-process shell agent",
        if leader { "leader" } else { "default" }
    );
    if !leader {
        println!(
            "CAPTURE prompt model from ACL {ACL_MODEL}; TOML theme and vim_mode did not select {TOML_MODEL}"
        );
    }
}

fn log_hints(log: &str) -> String {
    log.lines()
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            (lower.contains("session") || lower.contains("model") || lower.contains("error"))
                && !lower.contains("api_key")
                && !lower.contains("fixture-key")
        })
        .take(20)
        .collect::<Vec<_>>()
        .join("\n")
}

fn isolated_aster(dir: &Path) -> PathBuf {
    fs::create_dir_all(dir).expect("pair dir");
    let dest = dir.join("aster");
    let cloned = Command::new("cp")
        .arg("-c")
        .arg(shipped_aster())
        .arg(&dest)
        .status()
        .expect("cp");
    if !cloned.success() {
        match fs::hard_link(shipped_aster(), &dest) {
            Ok(()) => {}
            Err(_) => {
                fs::copy(shipped_aster(), &dest).expect("copy aster");
            }
        }
    }
    assert!(
        !dir.join("a3s-code-acp").exists(),
        "the missing-companion directory must not contain the ACP sibling"
    );
    dest
}

fn assert_missing_companion() {
    let root = temp_root("missing");
    let home = root.join("home");
    let workspace = root.join("workspace");
    fs::create_dir_all(&home).expect("home");
    fs::create_dir_all(&workspace).expect("workspace");
    let aster = isolated_aster(&root.join("bin"));
    let mut pty = PtySession::spawn(
        &aster,
        &["--cwd", workspace.to_str().expect("utf-8")],
        &home,
        &workspace,
    );
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        pty.pump();
        let text = String::from_utf8_lossy(&pty.raw);
        if text.contains("a3s-code-acp") && text.contains("no agent backend") {
            println!(
                "CAPTURE missing companion failed closed and named a3s-code-acp while A3S_ACP_AGENT_BIN was unset"
            );
            return;
        }
        if let Some(code) = pty.exited() {
            assert!(
                text.contains("a3s-code-acp"),
                "startup failure must name the missing companion (exit {code}): {}",
                tail(&pty.raw)
            );
            println!(
                "CAPTURE missing companion failed closed and named a3s-code-acp while A3S_ACP_AGENT_BIN was unset"
            );
            return;
        }
        if Instant::now() >= deadline {
            panic!("missing companion did not fail closed\n{}", tail(&pty.raw));
        }
        std::thread::sleep(Duration::from_millis(40));
    }
}

fn assert_auto_update_off() {
    let root = temp_root("update");
    let home = root.join("home");
    fs::create_dir_all(&home).expect("home");
    let mut command = Command::new(shipped_aster());
    command.args(["update", "--check", "--json"]);
    shell_env(&mut command, &home);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let output = command.output().expect("update check");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "update --check failed\n{stdout}\n{stderr}"
    );
    assert!(
        stdout.contains("\"autoUpdate\":false"),
        "background auto-update stayed on: {stdout}"
    );
    assert!(
        stdout.contains("\"updateAvailable\":false"),
        "update check contacted a release or reported an update: {stdout}"
    );
    println!("CAPTURE auto-update off autoUpdate=false updateAvailable=false");
}

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    listener.local_addr().expect("addr").port()
}

fn assert_serve_constructs_shell_agent() {
    let root = temp_root("serve");
    let home = root.join("home");
    fs::create_dir_all(&home).expect("home");
    let port = free_port();
    let bind = format!("127.0.0.1:{port}");
    let mut command = Command::new(shipped_aster());
    command.args(["agent", "serve", "--bind", &bind, "--secret", "hermetic"]);
    shell_env(&mut command, &home);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().expect("spawn serve");
    let stdout = child.stdout.take().expect("stdout");
    let stderr = child.stderr.take().expect("stderr");
    let stdout_thread = std::thread::spawn(move || read_pipe(stdout));
    let stderr_thread = std::thread::spawn(move || read_pipe(stderr));
    let marker = home.join(".a3s").join("shell-agent.log");
    let deadline = Instant::now() + SERVE_TIMEOUT;
    let mut upgraded = false;
    let mut socket = None;
    let result = loop {
        if marker.is_file()
            && fs::read_to_string(&marker)
                .unwrap_or_default()
                .contains("constructed in-process shell agent")
        {
            break Ok(());
        }
        if !upgraded
            && std::net::TcpStream::connect_timeout(
                &format!("127.0.0.1:{port}").parse().expect("addr"),
                Duration::from_millis(200),
            )
            .is_ok()
        {
            match websocket_upgrade(port) {
                Ok(stream) => {
                    socket = Some(stream);
                    upgraded = true;
                }
                Err(error) => {
                    if Instant::now() >= deadline {
                        break Err(error);
                    }
                }
            }
        }
        if let Some(status) = child.try_wait().expect("serve wait") {
            break Err(format!("serve exited {}", status.code().unwrap_or(1)));
        }
        if Instant::now() >= deadline {
            break Err("timed out waiting for the in-process shell agent".to_string());
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let _ = child.kill();
    let _ = child.wait();
    drop(socket);
    let serve_stdout = stdout_thread.join().unwrap_or_default();
    let serve_stderr = stderr_thread.join().unwrap_or_default();
    if let Err(error) = result {
        panic!(
            "serve did not construct the in-process shell agent: {error}\n{serve_stdout}\n{serve_stderr}"
        );
    }
    println!("CAPTURE serve constructed in-process shell agent");
}

fn read_pipe(mut pipe: impl Read) -> String {
    let mut buf = Vec::new();
    let _ = pipe.read_to_end(&mut buf);
    String::from_utf8_lossy(&buf).into_owned()
}

fn websocket_upgrade(port: u16) -> Result<std::net::TcpStream, String> {
    let mut stream = std::net::TcpStream::connect_timeout(
        &format!("127.0.0.1:{port}").parse().expect("addr"),
        Duration::from_secs(2),
    )
    .map_err(|error| error.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| error.to_string())?;
    let request = format!(
        "GET /ws HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\nAuthorization: Bearer hermetic\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|error| error.to_string())?;
    let mut buf = [0_u8; 2048];
    let n = stream.read(&mut buf).map_err(|error| error.to_string())?;
    let text = String::from_utf8_lossy(&buf[..n]);
    if text.contains("101") {
        Ok(stream)
    } else {
        Err(format!("websocket upgrade rejected: {text}"))
    }
}

#[test]
fn shipped_tui_one_agent_connects_acp_and_keeps_serve_in_process() {
    assert_auto_update_off();
    assert_missing_companion();
    let default_home = temp_root("default-home");
    assert_acp_session(&default_home, false);
    let leader_home = temp_root("leader-home");
    assert_acp_session(&leader_home, true);
    assert_serve_constructs_shell_agent();
}
