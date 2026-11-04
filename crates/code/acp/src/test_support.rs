#![cfg(test)]

use std::io::Write;
use std::sync::{Arc, Mutex};

pub(crate) struct PhaseTrace {
    bytes: Arc<Mutex<Vec<u8>>>,
    _guard: tracing::subscriber::DefaultGuard,
}

#[derive(Clone)]
struct TraceWriter(Arc<Mutex<Vec<u8>>>);

impl Write for TraceWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().write(bytes)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl PhaseTrace {
    // These transport tests use Tokio's current-thread runtime, so the scoped
    // subscriber also observes the real Harness tool tasks across awaits.
    pub(crate) fn new() -> Self {
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let writer = TraceWriter(Arc::clone(&bytes));
        let subscriber = tracing_subscriber::fmt()
            .with_env_filter("aster_acp::retrieval_phase=debug")
            .with_ansi(false)
            .without_time()
            .with_writer(move || writer.clone())
            .finish();
        Self {
            bytes,
            _guard: tracing::subscriber::set_default(subscriber),
        }
    }

    pub(crate) fn assert_host_search(
        &self,
        method: &str,
        session: &str,
        outcomes: &[bool],
        forbidden: &[&str],
    ) {
        let text = String::from_utf8(self.bytes.lock().unwrap().clone()).unwrap();
        if let Some(directory) = std::env::var_os("ASTER_RETRIEVAL_EVIDENCE_DIR") {
            let file = match method {
                "aster/wiki/search-workspaces" => "wiki-host-rpc.log",
                "aster/memory/search-workspaces" => "memory-host-rpc.log",
                _ => panic!("unsupported phase fixture method: {method}"),
            };
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(std::path::PathBuf::from(directory).join(file), &text).unwrap();
        }
        for payload in forbidden {
            assert!(!text.contains(payload), "phase log exposed a payload");
        }
        let samples = text
            .lines()
            .filter(|line| line.contains("phase=\"host_workspace_search\""))
            .map(|line| {
                line.split_whitespace()
                    .filter_map(|field| field.split_once('='))
                    .map(|(key, value)| (key, value.trim_matches('"')))
                    .collect::<std::collections::BTreeMap<_, _>>()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            samples.len(),
            outcomes.len(),
            "actual RPC phase samples: {text}"
        );
        let mut scopes = std::collections::BTreeSet::new();
        for (sample, success) in samples.into_iter().zip(outcomes) {
            assert_eq!(sample["method"], method);
            assert_eq!(sample["session_id"], session);
            let elapsed = sample["elapsed_ms"].parse::<f64>().unwrap();
            assert!(elapsed.is_finite() && elapsed >= 0.0);
            assert_eq!(sample["success"], success.to_string());
            if *success {
                assert!((1..=65536).contains(&sample["response_bytes"].parse::<usize>().unwrap()));
            } else {
                assert!(!sample.contains_key("response_bytes"));
            }
            assert!(sample["scope_id"].contains("/turn/"));
            let run = uuid::Uuid::parse_str(sample["run_instance_id"]).unwrap();
            assert!(scopes.insert((run, sample["scope_id"])));
        }
    }
}

pub(crate) fn successful_tool_json(text: &str) -> serde_json::Value {
    let body = text
        .strip_prefix("Tool exit code: 0\n")
        .unwrap_or_else(|| panic!("expected an actual successful tool result: {text}"));
    serde_json::from_str(body).expect("successful native tool payload is JSON")
}

pub(crate) fn assert_failed_tool_text(text: &str) {
    let body = text
        .strip_prefix("Tool exit code: 1\n")
        .unwrap_or_else(|| panic!("expected an actual failed tool result: {text}"));
    assert!(
        serde_json::from_str::<serde_json::Value>(body).is_err(),
        "refused document content must not enter the model as accepted JSON: {body}"
    );
}

#[cfg(windows)]
pub(crate) fn native_executable_fixture(
    root: &std::path::Path,
    name: &str,
    source: &str,
) -> std::path::PathBuf {
    let input = root.join(format!("{name}.rs"));
    let output = root.join(format!("{name}.exe"));
    std::fs::write(&input, source).unwrap();
    let result = std::process::Command::new("rustc")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["--edition=2021", "--crate-name", name])
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "native fixture compilation failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    output
}
