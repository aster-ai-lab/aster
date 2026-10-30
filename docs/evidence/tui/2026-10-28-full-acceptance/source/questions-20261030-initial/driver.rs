#![cfg(test)]

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde_json::{json, Value};

pub(super) struct LiveTui {
    _root: tempfile::TempDir,
    pub workspace: PathBuf,
    pub home: PathBuf,
    config: PathBuf,
    config_before: Vec<u8>,
    editable_config: Option<PathBuf>,
    fixture_env: Vec<(String, String)>,
    binary: PathBuf,
    companion: PathBuf,
    acp_trace: Option<PathBuf>,
    pub editor: Option<String>,
    pub submit_keys: Vec<u8>,
    session: Option<String>,
    secrets: Vec<String>,
    child: Option<Box<dyn Child + Send + Sync>>,
    master: Option<Box<dyn MasterPty + Send>>,
    writer: Option<Box<dyn Write + Send>>,
    rx: Option<Receiver<Vec<u8>>>,
    parser: vt100::Parser,
    raw: Vec<u8>,
    cpr_through: usize,
    case: String,
    evidence: Vec<Value>,
    finished: bool,
}

impl LiveTui {
    pub fn new(case: &str) -> Self {
        let config = PathBuf::from(
            std::env::var_os("A3S_CONFIG_FILE")
                .expect("A3S_CONFIG_FILE is required; no skipped live observations"),
        );
        let config = config.canonicalize().expect("explicit host ACL exists");
        let parsed = a3s_code_core::CodeConfig::from_file(&config)
            .unwrap_or_else(|_| panic!("explicit host ACL could not be parsed"));
        assert_eq!(parsed.default_model.as_deref(), Some("kimi/k3.1-pro"));
        let mut secrets = Vec::new();
        for provider in &parsed.providers {
            secrets.extend(provider.api_key.iter().cloned());
            secrets.extend(provider.base_url.iter().cloned());
            secrets.extend(provider.headers.values().cloned());
            for model in &provider.models {
                secrets.extend(model.api_key.iter().cloned());
                secrets.extend(model.base_url.iter().cloned());
                secrets.extend(model.headers.values().cloned());
            }
        }
        let root = if cfg!(windows) {
            let base =
                PathBuf::from(std::env::var_os("SystemRoot").expect("SystemRoot")).join("Temp");
            tempfile::Builder::new()
                .prefix("aster-kimi-")
                .tempdir_in(base)
                .unwrap()
        } else {
            tempfile::Builder::new()
                .prefix("aster-kimi-")
                .tempdir()
                .unwrap()
        };
        let home = root.path().join("home");
        let workspace = root.path().join("workspace");
        for dir in [
            &workspace,
            &home.join("grok"),
            &home.join("AppData/Local"),
            &home.join("AppData/Roaming"),
        ] {
            std::fs::create_dir_all(dir).unwrap();
        }
        let companion = PathBuf::from(
            std::env::var_os("A3S_CODE_ACP_BIN")
                .unwrap_or_else(|| env!("CARGO_BIN_EXE_a3s-code-acp").into()),
        );
        let binary = std::env::var_os("A3S_CODE_TUI_BIN")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                companion.with_file_name(format!("aster{}", std::env::consts::EXE_SUFFIX))
            });
        assert!(
            binary.is_file() && companion.is_file(),
            "build the source pair first"
        );
        let build_id = |path: &Path| {
            let out = Command::new(path).arg("--build-id").output().unwrap();
            assert!(out.status.success());
            String::from_utf8(out.stdout).unwrap().trim().to_string()
        };
        let tui_id = build_id(&binary);
        assert_eq!(
            tui_id,
            build_id(&companion),
            "TUI and ACP source identities differ"
        );
        let mut value = Self {
            config_before: std::fs::read(&config).unwrap(),
            editable_config: None,
            fixture_env: Vec::new(),
            _root: root,
            workspace,
            home,
            config,
            binary,
            companion,
            acp_trace: None,
            editor: None,
            submit_keys: b"\r".to_vec(),
            session: None,
            secrets,
            child: None,
            master: None,
            writer: None,
            rx: None,
            parser: vt100::Parser::new(40, 120, 2000),
            raw: Vec::new(),
            cpr_through: 0,
            case: case.into(),
            evidence: Vec::new(),
            finished: false,
        };
        value.record("source-pair", &tui_id);
        value
    }

    pub fn launch(&mut self, args: &[&str]) {
        self.launch_inner(args, true);
    }

    pub fn launch_default(&mut self, args: &[&str]) {
        self.launch_inner(args, false);
    }

    pub fn set_env(&mut self, name: &str, value: &str) {
        self.fixture_env.push((name.into(), value.into()));
    }

    fn launch_inner(&mut self, args: &[&str], explicit_normal: bool) {
        let cwd = self.workspace.clone();
        self.launch_inner_at(args, explicit_normal, &cwd, &cwd);
    }

    pub fn launch_relative(&mut self, relative: &Path) {
        let process_cwd = self.workspace.parent().unwrap().to_path_buf();
        self.launch_inner_at(&[], true, relative, &process_cwd);
    }

    fn launch_inner_at(
        &mut self,
        args: &[&str],
        explicit_normal: bool,
        cli_cwd: &Path,
        process_cwd: &Path,
    ) {
        assert!(self.child.is_none());
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 40,
                cols: 120,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        let mut command = CommandBuilder::new(&self.binary);
        command.args(["--cwd", cli_cwd.to_str().unwrap()]);
        if explicit_normal && !args.contains(&"--plan") {
            command.arg("--no-plan");
        }
        command.args(args);
        command.cwd(process_cwd);
        command.env_clear();
        // Carry only OS execution dependencies, never ambient provider/account state.
        for key in [
            "PATH",
            "SystemRoot",
            "WINDIR",
            "COMSPEC",
            "PATHEXT",
            "ProgramFiles",
            "ProgramFiles(x86)",
            "ProgramData",
            "TEMP",
            "TMP",
        ] {
            if let Some(value) = std::env::var_os(key) {
                command.env(key, value);
            }
        }
        command.env("HOME", &self.home);
        command.env("USERPROFILE", &self.home);
        command.env("APPDATA", self.home.join("AppData/Roaming"));
        command.env("LOCALAPPDATA", self.home.join("AppData/Local"));
        command.env("GROK_HOME", self.home.join("grok"));
        command.env(
            "A3S_CONFIG",
            self.editable_config.as_ref().unwrap_or(&self.config),
        );
        for (name, value) in &self.fixture_env {
            command.env(name, value);
        }
        command.env("A3S_DISABLE_CC_SWITCH", "1");
        command.env("A3S_DISABLE_GROK_ACCOUNT", "1");
        command.env("A3S_DISABLE_CURSOR_ACCOUNT", "1");
        command.env("TERM", "xterm-256color");
        if let Some(editor) = &self.editor {
            command.env("EDITOR", editor);
            command.env("VISUAL", editor);
        }
        self.child = Some(pair.slave.spawn_command(command).unwrap());
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().unwrap();
        self.writer = Some(pair.master.take_writer().unwrap());
        self.master = Some(pair.master);
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let mut buffer = [0_u8; 8192];
            while let Ok(n) = reader.read(&mut buffer) {
                if n == 0 || tx.send(buffer[..n].to_vec()).is_err() {
                    break;
                }
            }
        });
        self.rx = Some(rx);
        self.parser = vt100::Parser::new(40, 120, 2000);
        self.raw.clear();
        self.cpr_through = 0;
        self.session = None;
    }

    pub fn use_editable_config(&mut self) -> PathBuf {
        use a3s_acl::{Block, Value as AclValue};
        let parsed = a3s_code_core::CodeConfig::from_file(&self.config).unwrap();
        let (provider, model) = parsed.default_model_config().unwrap();
        let mut provider_json = serde_json::to_value(provider).unwrap();
        let attrs = provider_json.as_object_mut().unwrap();
        attrs.remove("name");
        attrs.remove("models");
        let mut model_json = serde_json::to_value(model).unwrap();
        model_json.as_object_mut().unwrap().remove("id");
        let attributes = |value: &Value| {
            value
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.clone(), fixture_acl_value(v, &self.secrets)))
                .collect()
        };
        let mut document = a3s_acl::parse_acl("default_model = \"kimi/k3.1-pro\"\n").unwrap();
        document.blocks.push(Block {
            name: "providers".into(),
            labels: vec![provider.name.clone()],
            attributes: attributes(&provider_json),
            blocks: vec![Block {
                name: "models".into(),
                labels: vec![model.id.clone()],
                blocks: vec![],
                attributes: attributes(&model_json),
            }],
        });
        document.blocks.push(Block {
            name: "llm_api_timeout_ms".into(),
            labels: vec![],
            blocks: vec![],
            attributes: [("llm_api_timeout_ms".into(), AclValue::Number(120_000.0))].into(),
        });
        let text = a3s_acl::generate_acl(&document);
        for secret in self.secrets.iter().filter(|s| !s.is_empty()) {
            assert!(
                !text.contains(secret),
                "fixture must never contain a resolved secret"
            );
        }
        let path = self.home.join("editable-config.acl");
        std::fs::write(&path, text).unwrap();
        self.fixture_env = self
            .secrets
            .iter()
            .enumerate()
            .map(|(i, value)| (format!("ASTER_TUI_CONFIG_SECRET_{i}"), value.clone()))
            .collect();
        self.editable_config = Some(path.clone());
        path
    }

    pub fn bytes(&mut self, bytes: &[u8]) {
        let writer = self.writer.as_mut().unwrap();
        writer.write_all(bytes).unwrap();
        writer.flush().unwrap();
    }

    pub fn screen(&mut self) -> String {
        while let Ok(bytes) = self.rx.as_ref().unwrap().try_recv() {
            self.parser.process(&bytes);
            self.raw.extend_from_slice(&bytes);
        }
        if self.raw[self.cpr_through..]
            .windows(4)
            .any(|w| w == b"\x1b[6n")
        {
            self.cpr_through = self.raw.len();
            self.bytes(b"\x1b[12;1R");
        }
        let screen = self.parser.screen().contents();
        self.redact(&screen)
    }

    pub fn redact(&self, text: &str) -> String {
        let mut out = text.to_string();
        for secret in self.secrets.iter().filter(|secret| secret.len() >= 6) {
            out = out.replace(secret, "[redacted]");
        }
        out
    }

    pub fn physical_screen(&mut self) -> String {
        self.screen();
        let screen = self.parser.screen();
        let (_, cols) = screen.size();
        self.redact(&screen.rows(0, cols).collect::<Vec<_>>().join("\n"))
    }

    pub fn pump(&mut self, millis: u64) -> String {
        let deadline = Instant::now() + Duration::from_millis(millis);
        while Instant::now() < deadline {
            self.screen();
            std::thread::sleep(Duration::from_millis(25));
        }
        self.screen()
    }

    pub fn wait_for(&mut self, needle: &str, seconds: u64) -> String {
        self.wait_for_inner(needle, seconds, true)
    }

    pub fn wait_for_unanswered(&mut self, needle: &str, seconds: u64) -> String {
        self.wait_for_inner(needle, seconds, false)
    }

    fn wait_for_inner(&mut self, needle: &str, seconds: u64, answer_trust: bool) -> String {
        let deadline = Instant::now() + Duration::from_secs(seconds);
        let mut last = String::new();
        while Instant::now() < deadline {
            last = self.screen();
            if answer_trust && last.contains("Do you trust the contents of this directory") {
                self.bytes(b"y");
                self.pump(300);
                continue;
            }
            if last.contains(needle) {
                return last;
            }
            let status = self.child.as_mut().unwrap().try_wait().unwrap();
            assert!(status.is_none(), "TUI exited {status:?}\n{last}");
            std::thread::sleep(Duration::from_millis(40));
        }
        panic!("timeout waiting for {needle:?}\n{last}");
    }

    pub fn dismiss(&mut self) {
        self.bytes(b"\x1b");
        self.pump(700);
        self.focus_composer();
    }

    fn focus_composer(&mut self) {
        if cfg!(unix) {
            // Wait for rendered input before clearing it: an async focus change
            // can otherwise leave the focus key behind or coalesce Enter into a paste.
            const MARKER: &str = "TUI_INPUT_BARRIER";
            self.bytes(b" ");
            self.pump(100);
            self.bytes(b"\x1b[200~TUI_INPUT_BARRIER\x1b[201~");
            self.wait_for(MARKER, 8);
            self.bytes(b"\x05\x15");
            let deadline = Instant::now() + Duration::from_secs(8);
            while self.pump(80).contains(MARKER) {
                assert!(Instant::now() < deadline, "composer input did not clear");
            }
        } else {
            self.bytes(b"i");
            self.pump(100);
            self.bytes(b"\x15");
            self.pump(100);
        }
    }

    pub fn command(&mut self, text: &str) {
        self.focus_composer();
        self.bytes(b"\x1b[200~");
        // ConPTY exposes bare LF as Ctrl+Enter, not a text newline. Send the
        // native Enter encoding; the pager coalesces the paste back to LF.
        let paste = if cfg!(windows) {
            assert!(!text.contains('\r'), "logical live prompts use LF");
            text.replace('\n', "\r")
        } else {
            text.to_owned()
        };
        self.bytes(paste.as_bytes());
        self.bytes(b"\x1b[201~");
        if cfg!(windows) && !text.starts_with('/') {
            // Native console events lack a paste boundary. End preserves the
            // draft and keeps the submit key out of the final text-key batch.
            self.bytes(b"\x1b[F");
        }
        let prefix: String = text
            .lines()
            .next()
            .unwrap_or_default()
            .chars()
            .take(48)
            .collect();
        if text.starts_with('/') {
            // A previous refusal can contain the next command as a substring
            // (/workflows followed by /workflow). Wait for the actual draft.
            let deadline = Instant::now() + Duration::from_secs(8);
            loop {
                self.pump(80);
                let screen = self.physical_screen();
                let ready = screen.lines().any(|row| {
                    let row = row.trim_start();
                    let boxed = row
                        .strip_prefix("│ > ")
                        .or_else(|| row.strip_prefix("│ ❯ "))
                        .is_some_and(|draft| {
                            draft.trim_end().trim_end_matches('│').trim_end() == text
                        });
                    let minimal = row
                        .strip_prefix("> ")
                        .or_else(|| row.strip_prefix("❯ "))
                        .is_some_and(|draft| draft.trim_end() == text);
                    boxed || minimal
                });
                if ready {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "command did not reach the actual composer\n{screen}"
                );
            }
        } else {
            self.wait_for(&prefix, 8);
        }
        self.bytes(&self.submit_keys.clone());
    }

    pub fn literal_command(&mut self, text: &str) {
        self.focus_composer();
        self.bytes(b"\x1b[200~");
        self.bytes(text.as_bytes());
        self.bytes(b"\x1b[201~");
        self.wait_for(text, 8);
        self.bytes(b"\x1b");
        let armed = self.pump(700);
        assert!(
            armed.contains(text),
            "literal command changed before submission\n{armed}"
        );
        self.bytes(b"\r");
        self.pump(300);
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.master
            .as_ref()
            .unwrap()
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        self.parser.set_size(rows, cols);
        self.pump(600);
    }

    pub fn mouse_modes(&self) -> usize {
        [
            b"\x1b[?1000h".as_slice(),
            b"\x1b[?1000l",
            b"\x1b[?1002h",
            b"\x1b[?1002l",
            b"\x1b[?1006h",
            b"\x1b[?1006l",
        ]
        .iter()
        .map(|code| self.raw.windows(code.len()).filter(|w| w == code).count())
        .sum()
    }

    pub fn preferences(&self) -> String {
        [
            self.home.join("grok/config.toml"),
            self.home.join(".grok/config.toml"),
        ]
        .iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .collect::<Vec<_>>()
        .join("\n")
    }

    pub fn wait_preference(&mut self, text: &str) {
        let deadline = Instant::now() + Duration::from_secs(8);
        while !self.preferences().contains(text) {
            self.pump(100);
            assert!(
                Instant::now() < deadline,
                "preference was not persisted: {text}"
            );
        }
    }

    pub fn wait_fact(&mut self, kind: &str, before: usize) -> Value {
        let deadline = Instant::now() + Duration::from_secs(90);
        loop {
            self.pump(80);
            if let Some(fact) = self
                .facts()
                .into_iter()
                .skip(before)
                .find(|f| f["type"] == kind)
            {
                return fact;
            }
            assert!(
                Instant::now() < deadline,
                "missing fact {kind}\n{}",
                self.screen()
            );
        }
    }

    pub fn session_id(&mut self) -> String {
        self.command("/session-info");
        let deadline = Instant::now() + Duration::from_secs(30);
        let info = loop {
            let info = self.pump(100);
            if info.contains("Session ID") {
                break info;
            }
            assert!(
                Instant::now() < deadline,
                "timeout waiting for bound Session ID\n{info}"
            );
            let status = self.child.as_mut().unwrap().try_wait().unwrap();
            assert!(status.is_none(), "TUI exited {status:?}\n{info}");
            if info.contains("No active session") {
                self.command("/session-info");
            }
        };
        let mut rows = info.lines();
        while let Some(row) = rows.next() {
            if !row.contains("Session ID") {
                continue;
            }
            for line in [row, rows.next().unwrap_or_default()] {
                for token in line.split_whitespace() {
                    if uuid::Uuid::parse_str(token).is_ok() {
                        self.session = Some(token.to_string());
                        return token.to_string();
                    }
                }
            }
        }
        panic!("session-info omitted session id\n{info}");
    }

    pub fn files(&self, dir: &Path, extension: &str) -> Vec<PathBuf> {
        fn walk(dir: &Path, extension: &str, depth: usize, out: &mut Vec<PathBuf>) {
            if depth > 10 {
                return;
            }
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        walk(&path, extension, depth + 1, out);
                    } else if path.extension().and_then(|v| v.to_str()) == Some(extension) {
                        out.push(path);
                    }
                }
            }
        }
        let mut paths = Vec::new();
        walk(dir, extension, 0, &mut paths);
        paths
    }

    pub fn facts(&self) -> Vec<Value> {
        let mut out = Vec::new();
        for path in self.files(&self.workspace.join(".a3s"), "jsonl") {
            if self
                .session
                .as_deref()
                .is_some_and(|id| path.file_stem().and_then(|p| p.to_str()) != Some(id))
            {
                continue;
            }
            if let Ok(text) = std::fs::read_to_string(path) {
                out.extend(
                    text.lines()
                        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                        .filter(|f| f.get("type").is_some() && f.get("seq").is_some()),
                );
            }
        }
        out
    }

    pub fn trace_acp(&mut self, proxy: &Path) {
        let dir = self._root.path().join("acp-proxy");
        std::fs::create_dir(&dir).unwrap();
        let suffix = std::env::consts::EXE_SUFFIX;
        let binary = dir.join(format!("aster{suffix}"));
        let companion = dir.join(format!("a3s-code-acp{suffix}"));
        std::fs::copy(&self.binary, &binary).unwrap();
        std::fs::copy(proxy, &companion).unwrap();
        let trace = self._root.path().join("acp-traffic.jsonl");
        let target = self.companion.to_string_lossy().into_owned();
        self.set_env("A3S_CODE_ACP_PROXY_TARGET", &target);
        self.set_env("A3S_CODE_ACP_TRACE", trace.to_str().unwrap());
        self.binary = binary;
        self.acp_trace = Some(trace);
    }

    pub fn acp_session_info(&self, id: &str, turn: u64) -> Value {
        let trace = self.acp_trace.as_ref().expect("ACP trace is enabled");
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let raw = std::fs::read_to_string(trace).unwrap_or_default();
            if let Some(info) = raw
                .lines()
                .rev()
                .filter_map(|line| {
                    let entry = serde_json::from_str::<Value>(line).ok()?;
                    let info = entry["message"]["result"]["result"].clone();
                    (info["sessionId"].as_str() == Some(id)
                        && info["turnIndex"].as_u64() == Some(turn))
                    .then_some(info)
                })
                .next()
            {
                return info;
            }
            assert!(
                Instant::now() < deadline,
                "missing session info response for {id}, turn {turn}\n{}",
                self.redact(&raw)
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    pub fn assistant_text(&self, facts: &[Value]) -> String {
        facts
            .iter()
            .filter(|f| f["type"] == "model.turn")
            .filter_map(|f| f["payload"]["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn turn(&mut self, prompt: &str, permission: Option<&[u8]>) -> Vec<Value> {
        if self.session.is_none() {
            self.session_id();
            self.dismiss();
        }
        let before = self.facts().len();
        self.command(prompt);
        self.settle_turn(before, prompt, permission)
    }

    pub fn settle_turn(
        &mut self,
        before: usize,
        prompt: &str,
        permission: Option<&[u8]>,
    ) -> Vec<Value> {
        let deadline = Instant::now() + Duration::from_secs(420);
        let mut answered = false;
        loop {
            let screen = self.pump(80);
            assert!(
                !screen.contains("Turn failed"),
                "live turn failed\n{screen}"
            );
            if !screen.contains("Yes, proceed") {
                answered = false;
            }
            if screen.contains("Yes, proceed") && !answered {
                if let Some(answer) = permission {
                    self.bytes(answer);
                    answered = true;
                    self.record("permission-answer", "answered a real permission card");
                } else {
                    panic!("unexpected permission card\n{screen}");
                }
            }
            let facts = self.facts();
            let delta = facts.get(before..).unwrap_or_default();
            if delta.iter().any(|f| f["type"] == "turn.complete")
                && delta.iter().any(|f| {
                    f["type"] == "user.message" && f["payload"]["text"].as_str() == Some(prompt)
                })
            {
                let observed = delta.to_vec();
                let submitted = observed
                    .iter()
                    .filter(|f| f["type"] == "user.message")
                    .filter_map(|f| f["payload"]["text"].as_str())
                    .collect::<Vec<_>>();
                assert_eq!(
                    submitted,
                    vec![prompt],
                    "submitted prompt bytes differ from the fixed test input"
                );
                let idle_deadline = Instant::now() + Duration::from_secs(15);
                let screen = loop {
                    let screen = self.pump(80);
                    if !screen.contains("[stop]") && !screen.contains("Ctrl+c:cancel") {
                        break screen;
                    }
                    assert!(
                        Instant::now() < idle_deadline,
                        "TUI did not become idle after the terminal fact\n{screen}"
                    );
                };
                self.record(
                    "settled-turn",
                    &format!(
                        "facts={} model_turns={} prompt_tokens={}",
                        observed.len(),
                        observed
                            .iter()
                            .filter(|f| f["type"] == "model.turn")
                            .count(),
                        observed
                            .iter()
                            .filter_map(|f| f["payload"]["prompt_tokens"].as_u64())
                            .sum::<u64>()
                    ),
                );
                return observed;
            }
            assert!(
                Instant::now() < deadline,
                "live turn did not settle\n{screen}"
            );
            assert!(
                !screen.contains("Connection failed") && !screen.contains("Request failed"),
                "provider failure\n{screen}"
            );
        }
    }

    pub fn record(&mut self, name: &str, detail: &str) {
        eprintln!(
            "ORACLE case={} name={name} result=pass detail={detail}",
            self.case
        );
        let screen = self.rx.as_ref().map(|_| ()).map(|_| self.screen());
        self.evidence
            .push(json!({"name":name,"detail":detail,"screen":screen}));
    }

    pub fn quit(&mut self) {
        self.command("/quit");
        self.wait_exit(15);
        self.record("quit", "exit 0");
    }

    pub fn wait_exit(&mut self, seconds: u64) {
        let deadline = Instant::now() + Duration::from_secs(seconds);
        loop {
            self.pump(60);
            if let Some(status) = self.child.as_mut().unwrap().try_wait().unwrap() {
                assert!(status.success(), "exit status {status:?}");
                break;
            }
            assert!(Instant::now() < deadline, "TUI did not exit");
        }
        self.child = None;
        self.master = None;
        self.writer = None;
        self.rx = None;
    }

    pub fn finish(&mut self) {
        assert!(
            std::fs::read(&self.config).unwrap() == self.config_before,
            "operator ACL changed"
        );
        self.finished = true;
    }

    pub fn checkpoint_session(&mut self, label: &str) {
        let facts = self.evidence_facts();
        self.evidence
            .push(json!({"checkpoint":label,"session":self.session,"facts":facts}));
    }

    pub fn permission_answer_count(&self) -> usize {
        self.evidence
            .iter()
            .filter(|entry| entry["name"] == "permission-answer")
            .count()
    }

    fn evidence_facts(&self) -> Vec<Value> {
        self.facts().iter().filter_map(|fact| {
            let p = &fact["payload"];
            let payload = match fact["type"].as_str()? {
                "tool.prepared" => json!({"identity":p["prepared"]["identity"], "tool":p["prepared"]["tool_name"]}),
                "tool.result" => {
                    let r = &p["receipt"];
                    json!({"identity":r["identity"],"prepared_digest":r["prepared_digest"],"outcome":r["outcome"],"effects":r["effects"],"output":r["output"],"scheduler_output_matches_receipt":p["output"]==r["output"]})
                }
                "model.turn" => json!({"prompt_tokens":p["prompt_tokens"],"has_text":p["text"].as_str().is_some_and(|v| !v.is_empty()),"tool":p["call"]["name"],"call":{"id":p["call"]["id"],"name":p["call"]["name"],"args":p["call"]["args"]},"text":p["text"]}),
                "turn.complete" | "a3s.goal.created" | "a3s.goal.paused" | "a3s.goal.resumed" | "a3s.goal.cancelled" | "compaction.done" => p.clone(),
                _ => return None,
            };
            Some(json!({"seq":fact["seq"],"type":fact["type"],"payload":payload}))
        }).collect()
    }
}

fn fixture_acl_value(value: &Value, secrets: &[String]) -> a3s_acl::Value {
    use a3s_acl::Value as A;
    match value {
        Value::Null => A::Null,
        Value::Bool(v) => A::Bool(*v),
        Value::Number(v) => A::Number(v.as_f64().unwrap()),
        Value::String(v) => match secrets.iter().position(|s| !s.is_empty() && s == v) {
            Some(i) => A::Call(
                "env".into(),
                vec![A::String(format!("ASTER_TUI_CONFIG_SECRET_{i}"))],
            ),
            None => A::String(v.clone()),
        },
        Value::Array(v) => A::List(v.iter().map(|v| fixture_acl_value(v, secrets)).collect()),
        Value::Object(v) => A::Object(
            v.iter()
                .map(|(k, v)| (k.clone(), fixture_acl_value(v, secrets)))
                .collect(),
        ),
    }
}

impl Drop for LiveTui {
    fn drop(&mut self) {
        let screen = if self.rx.is_some() {
            self.screen()
        } else {
            String::new()
        };
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(dir) = std::env::var_os("A3S_TUI_LIVE_EVIDENCE_DIR") {
            let dir = PathBuf::from(dir);
            std::fs::create_dir_all(&dir).unwrap();
            let facts = self.evidence_facts();
            let raw_start = self.raw.len().saturating_sub(64 * 1_024);
            let raw_tail = (!self.finished)
                .then(|| String::from_utf8_lossy(&self.raw[raw_start..]).into_owned());
            let report = json!({"case":self.case,"model":"kimi/k3.1-pro",
                "assertions_finished":self.finished,"panicked":std::thread::panicking(),
                "oracles":self.evidence,"last_screen":screen,"facts":facts,
                "raw_terminal_tail":raw_tail,
                "config_unchanged":std::fs::read(&self.config).ok().as_deref()==Some(self.config_before.as_slice())});
            let report = self.redact(&serde_json::to_string_pretty(&report).unwrap());
            std::fs::write(dir.join(format!("{}.json", self.case)), report).unwrap();
        }
    }
}
