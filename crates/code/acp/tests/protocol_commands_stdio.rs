#![cfg(test)]
#![cfg(unix)]

//! ACP-CMD1: the actual ACP binary announces its slash commands with a
//! standard `available_commands_update` after the session response, and an
//! extension method it does not implement is a JSON-RPC method-not-found
//! error, not a silent `null` success.

#[path = "support/binaries.rs"]
mod binaries;

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout};

const USER_ACL: &str = r#"
default_model = "mock/test"
providers "mock" {
  api_key = "not-a-model-credential"
  base_url = "http://127.0.0.1:9/v1"
  models "test" {}
}
"#;

#[tokio::test]
async fn deleting_a_session_removes_its_core_snapshot_and_refuses_reload() {
    use aster_core::store::SessionStore;
    let mut agent = Agent::launch();
    let id = uuid::Uuid::new_v4().to_string();
    persist_empty_snapshot(agent._root.path(), &agent.workspace, &id).await;
    agent.initialize().await;
    let deleted = agent
        .rpc(json!({"jsonrpc":"2.0","id":81,
        "method":"_aster/session/delete","params":{"sessionId":id,"cwd":agent.workspace}}))
        .await;
    assert_eq!(deleted["result"]["success"], true, "{deleted}");
    let store = aster_core::store::FileSessionStore::new(
        agent._root.path().join("home/.aster/session-store"),
    )
    .await
    .unwrap();
    assert!(store.load_snapshot(&id).await.unwrap().is_none());
    let loaded = agent
        .rpc(json!({"jsonrpc":"2.0","id":82,"method":"session/load",
        "params":{"sessionId":id,"cwd":agent.workspace,"mcpServers":[]}}))
        .await;
    assert!(
        loaded.get("error").is_some(),
        "deleted snapshot was restored: {loaded}"
    );
}

#[tokio::test]
async fn global_models_list_matches_the_session_catalog_without_a_model_request() {
    let mut agent = Agent::launch();
    agent.initialize().await;
    let listed = agent
        .rpc(json!({"jsonrpc":"2.0","id":91,"method":"_aster/models/list","params":{}}))
        .await;
    assert!(listed.get("error").is_none(), "{listed}");
    let catalog = &listed["result"]["result"];
    assert_eq!(catalog["currentModelId"], "mock/test");
    assert_eq!(catalog["availableModels"].as_array().unwrap().len(), 1);
    assert_eq!(catalog["availableModels"][0]["modelId"], "mock/test");
    let opened = agent
        .rpc(json!({"jsonrpc":"2.0","id":92,"method":"session/new",
            "params":{"cwd":agent.workspace,"mcpServers":[]}}))
        .await;
    assert_eq!(*catalog, opened["result"]["models"]);
    let unknown = agent
        .rpc(json!({"jsonrpc":"2.0","id":93,"method":"_aster/models/list/extra","params":{}}))
        .await;
    assert_eq!(unknown["error"]["code"], -32601, "{unknown}");
}

#[tokio::test]
async fn wiki_search_admits_two_cold_publications_through_the_actual_acp_process() {
    let mut agent = Agent::launch();
    let mut publications = Vec::new();
    for name in ["task-a", "task-b"] {
        let root = agent.workspace.join(name);
        std::fs::create_dir(&root).unwrap();
        let bytes = format!("crossstdioevidence identifies {name}.\n");
        std::fs::write(root.join("source.md"), &bytes).unwrap();
        let input = aster_okf::WorkspaceKnowledgeInput::new(
            &root,
            name,
            &format!("sha256:{}", "b".repeat(64)),
            vec![aster_okf::WorkspaceKnowledgeSource::text("source.md", bytes.as_bytes()).unwrap()],
        )
        .unwrap();
        let job = aster_okf::begin_workspace_knowledge_maintenance(&input)
            .unwrap()
            .unwrap();
        let stage = Path::new(&job.staging);
        std::fs::write(
            stage.join("concepts/overview.md"),
            format!("---\ntype: Synthesis\ntitle: {name}\n---\n{bytes}"),
        )
        .unwrap();
        std::fs::write(stage.join(".wiki-changes.json"), json!({"sources":job.sources,"summary":"Stdio fixture","categories":{},"pages":[{"path":"concepts/overview.md","title":name,"kind":"synthesis","change":"create","reason":"Admitted source","requires_review":false,"sources":job.sources}]}).to_string()).unwrap();
        aster_okf::finish_workspace_knowledge_maintenance(&input, &job.id).unwrap();
        publications.push((name, input.output_root().to_owned()));
    }
    agent.initialize().await;
    let started = std::time::Instant::now();
    for (index, (name, wiki)) in publications.iter().enumerate() {
        let cold = agent.rpc(json!({"jsonrpc":"2.0","id":10+index*3,"method":"_aster/wiki/search","params":{"wiki":wiki,"query":"crossstdioevidence"}})).await;
        assert_eq!(cold["result"]["phase"], "not-ready");
        let found = agent.rpc(json!({"jsonrpc":"2.0","id":11+index*3,"method":"_aster/wiki/search","params":{"wiki":wiki,"query":"crossstdioevidence","admit":true}})).await;
        assert_eq!(found["result"]["phase"], "published", "{found}");
        assert_eq!(found["result"]["hits"].as_array().unwrap().len(), 1);
        assert_eq!(found["result"]["hits"][0]["name"], *name);
        let warm = agent.rpc(json!({"jsonrpc":"2.0","id":12+index*3,"method":"_aster/wiki/search","params":{"wiki":wiki,"query":"crossstdioevidence","admit":true}})).await;
        assert_eq!(
            warm["result"]["source_revision"],
            found["result"]["source_revision"]
        );
        assert_eq!(warm["result"]["hits"], found["result"]["hits"]);
    }
    eprintln!(
        "ORACLE cold-plus-warm-two-wikis-ms={}",
        started.elapsed().as_millis()
    );
    agent.child.kill().await.unwrap();
}

#[tokio::test]
async fn workspace_search_reads_the_automatic_index_through_the_actual_acp_process() {
    let mut agent = Agent::launch();
    let source = agent.workspace.join("evidence.rs");
    std::fs::write(&source, "pub fn stdio_wiki_evidence_marker() {}\n").unwrap();
    std::fs::write(agent.workspace.join(".env"), "private_qxv_stdio=secret\n").unwrap();
    let (session, _) = agent.open_session().await;
    agent.wait_for_workspace_index().await;
    let mode = agent
        .rpc(json!({"jsonrpc":"2.0","id":3,"method":"session/set_mode",
        "params":{"sessionId":session,"modeId":"auto"}}))
        .await;
    assert!(mode.get("result").is_some(), "{mode}");
    let workspace = agent.workspace.clone();
    let params = json!({"schema":"aster.acp.workspace-search.v1","sessionId":session,
        "operationId":"stdio-search","cwd":workspace,"query":"stdio_wiki_evidence_marker"});
    let found = agent
        .rpc(json!({"jsonrpc":"2.0","id":4,"method":"_aster/workspace/search/v1","params":params}))
        .await;
    assert!(
        found["result"]["output"]
            .as_str()
            .unwrap()
            .contains("stdio_wiki_evidence_marker"),
        "{found}"
    );
    assert_eq!(found["result"]["sessionId"], session);
    assert_eq!(found["result"]["operationId"], "stdio-search");
    assert!(found["result"]["metadata"]["results"]
        .as_array()
        .unwrap()
        .iter()
        .all(|hit| hit["digest_verified"] == true));
    let foreign = tempfile::tempdir().unwrap();
    let mut invalid = params.clone();
    invalid["cwd"] = json!(foreign.path());
    let refused = agent
        .rpc(json!({"jsonrpc":"2.0","id":5,"method":"_aster/workspace/search/v1","params":invalid}))
        .await;
    assert_eq!(refused["error"]["code"], -32002);
    std::fs::write(&source, "pub fn stdio_updated_evidence_marker() {}\n").unwrap();
    let stale = agent
        .rpc(json!({"jsonrpc":"2.0","id":6,"method":"_aster/workspace/search/v1","params":params}))
        .await;
    assert!(!stale["result"]["output"]
        .as_str()
        .unwrap()
        .contains("stdio_wiki_evidence_marker"));
    let mut updated = params.clone();
    updated["query"] = json!("stdio_updated_evidence_marker");
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut id = 10;
        loop {
            let found = agent.rpc(json!({"jsonrpc":"2.0","id":id,"method":"_aster/workspace/search/v1","params":updated})).await;
            if found["result"]["output"].as_str().unwrap().contains("stdio_updated_evidence_marker") { break; }
            id += 1;
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await.unwrap();
    let mut secret = params.clone();
    secret["query"] = json!("private_qxv_stdio");
    let secret = agent
        .rpc(
            json!({"jsonrpc":"2.0","id":10000,"method":"_aster/workspace/search/v1","params":secret}),
        )
        .await;
    assert!(!secret["result"]["output"]
        .as_str()
        .unwrap()
        .contains("private_qxv_stdio"));
    let review = agent.rpc(json!({"jsonrpc":"2.0","id":10001,"method":"session/new",
        "params":{"cwd":workspace,"mcpServers":[],"_meta":{"asterHost":{"sourceReview":true,"assets":{"wiki":workspace}}}}})).await;
    let review_id = review["result"]["sessionId"].as_str().unwrap();
    let mut restricted = params;
    restricted["sessionId"] = json!(review_id);
    let denied = agent.rpc(json!({"jsonrpc":"2.0","id":10002,"method":"_aster/workspace/search/v1","params":restricted})).await;
    assert_eq!(denied["error"]["code"], -32602, "{denied}");
    assert!(denied["error"]["data"]
        .as_str()
        .unwrap()
        .contains("Permission denied"));
    assert_eq!(
        std::fs::read_to_string(&source).unwrap(),
        "pub fn stdio_updated_evidence_marker() {}\n"
    );
    assert_eq!(
        std::fs::read_to_string(agent.workspace.join(".env")).unwrap(),
        "private_qxv_stdio=secret\n"
    );
    agent.stop().await;
}

#[tokio::test]
async fn workspace_hybrid_search_uses_offline_vectors_and_refreshes_current_sources() {
    for files in [2, 5_000] {
        let acl = format!("{USER_ACL}\nworkspace_retrieval {{ enabled = true local_cpu {{}} }}\n");
        let mut agent = Agent::launch_config(tempfile::tempdir().unwrap(), &acl);
        let source = agent.workspace.join("configuration.rs");
        std::fs::write(
            &source,
            "// Read application configuration settings from disk.\npub fn load_settings() {}\n",
        )
        .unwrap();
        std::fs::write(
            agent.workspace.join("weather.md"),
            "# Forecast\nThe weather is sunny today.\n",
        )
        .unwrap();
        std::fs::write(
            agent.workspace.join(".env"),
            "private_vector_qxz=excluded\n",
        )
        .unwrap();
        for path in [
            "meta/aster-session.json",
            "child-task/meta/aster-session.json",
        ] {
            let locator = agent.workspace.join(path);
            std::fs::create_dir_all(locator.parent().unwrap()).unwrap();
            std::fs::write(
                locator,
                r#"{"id":"private_session_locator_qxz","runtime":"aster"}"#,
            )
            .unwrap();
        }
        for index in 0..files - 2 {
            std::fs::write(agent.workspace.join(format!("inventory_{index:04}.md")), format!("# Warehouse {index}\nThis module calculates inventory quantities and transforms stock records.\n")).unwrap();
        }
        let opened = std::time::Instant::now();
        let (session, _) = agent.open_session().await;
        let open_elapsed = opened.elapsed();
        let mode = agent
            .rpc(json!({"jsonrpc":"2.0","id":3,"method":"session/set_mode",
            "params":{"sessionId":session,"modeId":"auto"}}))
            .await;
        assert!(mode.get("result").is_some(), "{mode}");
        agent.wait_for_workspace_index().await;
        let params = json!({"schema":"aster.acp.workspace-search.v1","sessionId":session,
            "operationId":"offline-vector-search","cwd":agent.workspace,"query":"parse application configuration"});
        let mut id = 10;
        let found = tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                id += 1;
                let found = agent.rpc(json!({"jsonrpc":"2.0","id":id,"method":"_aster/workspace/search/v1","params":params})).await;
                assert!(found.get("error").is_none(), "{found}");
                if found["result"]["metadata"]["semantic_status"]["phase"] == "ready" { break found; }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }).await.expect("the local vector generation must become ready");
        let ready_elapsed = opened.elapsed();
        let metadata = &found["result"]["metadata"];
        assert_eq!(metadata["semantic_status"]["model"]["provider"], "light");
        assert_eq!(metadata["semantic_status"]["indexedFiles"], files);
        assert!(
            metadata["channels"]
                .as_array()
                .unwrap()
                .iter()
                .any(|channel| {
                    channel["channel"] == "semantic"
                        && channel["candidateCount"].as_u64().unwrap_or(0) > 0
                }),
            "{found}"
        );
        assert!(
            metadata["results"].as_array().unwrap().iter().any(|hit| {
                hit["path"] == "configuration.rs"
                    && hit["digest_verified"] == true
                    && hit["channels"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|rank| rank["channel"] == "semantic")
            }),
            "{found}"
        );
        let refresh_started = std::time::Instant::now();
        std::fs::write(&source, "pub fn replaced_configuration_marker() {}\n").unwrap();
        id += 1;
        let current = agent
            .rpc(json!({"jsonrpc":"2.0","id":id,"method":"_aster/workspace/search/v1","params":params}))
            .await;
        assert!(!current["result"]["output"]
            .as_str()
            .unwrap()
            .contains("load_settings"));
        let mut updated = params.clone();
        updated["query"] = json!("replaced_configuration_marker");
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                id += 1;
                let found = agent.rpc(json!({"jsonrpc":"2.0","id":id,"method":"_aster/workspace/search/v1","params":updated})).await;
                if found["result"]["output"].as_str().unwrap().contains("replaced_configuration_marker")
                    && found["result"]["metadata"]["semantic_status"]["phase"] == "ready" { break; }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }).await.unwrap();
        let refresh_elapsed = refresh_started.elapsed();
        for query in ["private_vector_qxz", "private_session_locator_qxz"] {
            let mut secret = params.clone();
            secret["query"] = json!(query);
            id += 1;
            let secret = agent
                .rpc(json!({"jsonrpc":"2.0","id":id,"method":"_aster/workspace/search/v1","params":secret}))
                .await;
            assert!(!secret.to_string().contains(query), "{secret}");
        }
        eprintln!(
            "ORACLE offline-vector-session-open-ms={} indexed-files={} first-ready-search-ms={} changed-source-ready-ms={} current-source-verified=true",
            open_elapsed.as_millis(),
            files,
            ready_elapsed.as_millis(),
            refresh_elapsed.as_millis()
        );
        agent.stop().await;
    }
}

#[tokio::test]
async fn workspace_hybrid_search_uses_the_published_aster_vec_index() {
    let mut agent = Agent::launch();
    std::fs::write(
        agent.workspace.join("evidence.rs"),
        "pub fn indexed_hybrid_marker() {}\n",
    )
    .unwrap();
    let (session, _) = agent.open_session().await;
    agent.wait_for_workspace_index().await;
    let mode = agent
        .rpc(json!({"jsonrpc":"2.0","id":3,"method":"session/set_mode",
        "params":{"sessionId":session,"modeId":"auto"}}))
        .await;
    assert!(mode.get("result").is_some(), "{mode}");
    let workspace = agent.workspace.clone();
    let found = agent
        .rpc(
            json!({"jsonrpc":"2.0","id":4,"method":"_aster/workspace/search/v1",
        "params":{"schema":"aster.acp.workspace-search.v1","sessionId":session,
            "operationId":"persistent-hybrid","cwd":workspace,"query":"indexed_hybrid_marker"}}),
        )
        .await;
    assert!(
        found["result"]["output"]
            .as_str()
            .unwrap()
            .contains("indexed_hybrid_marker"),
        "{found}"
    );
    assert_eq!(
        found["result"]["metadata"]["lexical_engine"],
        "aster_vec_fts_v1"
    );
    assert_eq!(
        found["result"]["metadata"]["index_kind"],
        "persistent_aster_vec_fts"
    );
    agent.stop().await;
}

struct Agent {
    _root: tempfile::TempDir,
    workspace: PathBuf,
    child: Child,
    input: ChildStdin,
    output: Lines<BufReader<ChildStdout>>,
    /// Every line read so far, in arrival order.
    seen: Vec<Value>,
}

impl Agent {
    fn launch() -> Self {
        Self::launch_with_root(tempfile::tempdir().unwrap())
    }

    fn launch_with_root(root: tempfile::TempDir) -> Self {
        Self::launch_config(root, USER_ACL)
    }

    fn launch_config(root: tempfile::TempDir, config: &str) -> Self {
        let home = root.path().join("home");
        let state = root.path().join("state");
        let workspace = root.path().join("workspace");
        for dir in [home.join(".aster"), state.clone(), workspace.clone()] {
            std::fs::create_dir_all(dir).unwrap();
        }
        std::fs::write(home.join(".aster/config.acl"), config).unwrap();
        let mut child = tokio::process::Command::new(binaries::acp_binary())
            .current_dir(&workspace)
            .args(["--cwd", "."])
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", &home)
            .env("ASTER_STATE_HOME", &state)
            .env("ASTER_DISABLE_CC_SWITCH", "1")
            .env("ASTER_DISABLE_GROK_ACCOUNT", "1")
            .env("ASTER_DISABLE_CURSOR_ACCOUNT", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap()).lines();
        Self {
            workspace: std::fs::canonicalize(&workspace).unwrap(),
            _root: root,
            child,
            input,
            output,
            seen: Vec::new(),
        }
    }

    async fn send(&mut self, message: Value) {
        self.input
            .write_all(format!("{message}\n").as_bytes())
            .await
            .unwrap();
        self.input.flush().await.unwrap();
    }

    /// Read until a line satisfies `done`; return its index in `seen`.
    async fn read_until(&mut self, what: &str, done: impl Fn(&Value) -> bool) -> usize {
        self.read_until_from(what, 0, done).await
    }

    async fn read_until_from(
        &mut self,
        what: &str,
        from: usize,
        done: impl Fn(&Value) -> bool,
    ) -> usize {
        if let Some(index) = (from..self.seen.len()).find(|index| done(&self.seen[*index])) {
            return index;
        }
        tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                let line = self
                    .output
                    .next_line()
                    .await
                    .unwrap()
                    .expect("ACP stdout ended");
                let value: Value = serde_json::from_str(&line).unwrap();
                let matched = done(&value);
                self.seen.push(value);
                if matched {
                    return self.seen.len() - 1;
                }
            }
        })
        .await
        .unwrap_or_else(|_| panic!("no {what} within 20 s; saw {:?}", self.seen))
    }

    async fn rpc(&mut self, request: Value) -> Value {
        let id = request["id"].clone();
        self.send(request).await;
        let index = self.read_until("response", |value| value["id"] == id).await;
        self.seen[index].clone()
    }

    async fn initialize(&mut self) {
        let initialized = self
            .rpc(json!({"jsonrpc":"2.0","id":1,"method":"initialize",
                        "params":{"protocolVersion":1,"clientCapabilities":{}}}))
            .await;
        assert!(initialized.get("result").is_some(), "{initialized}");
        let global_commands = initialized["result"]["_meta"]["availableCommands"]
            .as_array()
            .expect("initialize global command metadata");
        assert_eq!(command_names(global_commands), ["goal", "memory", "tasks"]);
    }

    async fn open_session(&mut self) -> (String, usize) {
        let started = std::time::Instant::now();
        self.initialize().await;
        let initialized = started.elapsed();
        let workspace = self.workspace.clone();
        let opened = self
            .rpc(json!({"jsonrpc":"2.0","id":2,"method":"session/new",
                        "params":{"cwd":workspace,"mcpServers":[]}}))
            .await;
        eprintln!(
            "ORACLE initialize-ms={} session-open-ms={}",
            initialized.as_millis(),
            started.elapsed().saturating_sub(initialized).as_millis()
        );
        let session_id = opened["result"]["sessionId"]
            .as_str()
            .unwrap_or_else(|| panic!("{opened}"))
            .to_owned();
        let response_at = self
            .seen
            .iter()
            .rposition(|value| value["id"] == 2)
            .unwrap();
        (session_id, response_at)
    }

    async fn wait_for_workspace_index(&self) {
        tokio::time::timeout(Duration::from_secs(15), async {
            while !self.workspace.join(".aster/index/CURRENT").is_file() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("automatic publication before the first search");
    }

    async fn restart(mut self) -> Self {
        self.child.kill().await.unwrap();
        self.child.wait().await.unwrap();
        Self::launch_with_root(self._root)
    }

    async fn stop(mut self) {
        self.child.kill().await.unwrap();
        self.child.wait().await.unwrap();
    }
}

fn is_commands_update(value: &Value, session_id: &str) -> bool {
    value["method"] == "session/update"
        && value["params"]["sessionId"] == session_id
        && value["params"]["update"]["sessionUpdate"] == "available_commands_update"
}

fn command_names(commands: &[Value]) -> Vec<&str> {
    commands
        .iter()
        .map(|command| command["name"].as_str().expect("command name"))
        .collect()
}

async fn persist_empty_snapshot(root: &Path, workspace: &Path, session_id: &str) {
    let store = std::sync::Arc::new(
        aster_core::store::FileSessionStore::new(root.join("home/.aster/session-store"))
            .await
            .unwrap(),
    );
    let core = aster_core::Agent::new(USER_ACL).await.unwrap();
    let session = core
        .session_async(
            workspace.display().to_string(),
            Some(
                aster_core::SessionOptions::new()
                    .with_session_id(session_id)
                    .with_session_store(store),
            ),
        )
        .await
        .unwrap();
    session.save().await.unwrap();
    session.close().await;
    core.close().await;
}

fn assert_current_session_catalog(commands: &[Value]) {
    let names = command_names(commands);
    for expected in [
        "goal", "help", "compact", "memory", "model", "tasks", "tools",
    ] {
        assert!(
            names.contains(&expected),
            "missing /{expected} in {names:?}"
        );
    }
    let unique = names.iter().collect::<std::collections::HashSet<_>>();
    assert_eq!(unique.len(), names.len(), "duplicate names in {names:?}");

    let goal = commands
        .iter()
        .find(|command| command["name"] == "goal")
        .expect("goal command");
    assert_eq!(
        goal["description"],
        "Set, pause, resume, or clear a durable goal"
    );
    assert_eq!(
        goal["input"]["hint"],
        "<objective> | status | pause | resume | clear"
    );
    let model = commands
        .iter()
        .find(|command| command["name"] == "model")
        .expect("Core model command");
    assert_eq!(model["description"], "Show or switch the current model");
    assert_eq!(model["input"]["hint"], "/model [provider/model]");
}

#[tokio::test]
async fn a_new_session_receives_the_available_commands_after_its_response() {
    let mut agent = Agent::launch();
    let (session_id, response_at) = agent.open_session().await;
    let update_at = agent
        .read_until("available_commands_update", |value| {
            is_commands_update(value, &session_id)
        })
        .await;
    assert!(
        update_at > response_at,
        "the update must follow the session/new response"
    );
    let commands = agent.seen[update_at]["params"]["update"]["availableCommands"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_current_session_catalog(&commands);
    agent.stop().await;
}

#[tokio::test]
async fn a_loaded_session_receives_the_current_commands_after_its_response() {
    let mut agent = Agent::launch();
    let (session_id, _) = agent.open_session().await;
    agent
        .read_until("new-session available_commands_update", |value| {
            is_commands_update(value, &session_id)
        })
        .await;

    let mut agent = agent.restart().await;
    persist_empty_snapshot(agent._root.path(), &agent.workspace, &session_id).await;
    agent.initialize().await;
    let search_from = agent.seen.len();
    let workspace = agent.workspace.clone();
    let loaded = agent
        .rpc(json!({"jsonrpc":"2.0","id":3,"method":"session/load",
                    "params":{"sessionId":session_id,"cwd":workspace,"mcpServers":[]}}))
        .await;
    assert!(loaded.get("result").is_some(), "{loaded}");
    let response_at = agent
        .seen
        .iter()
        .rposition(|value| value["id"] == 3)
        .unwrap();
    let update_at = agent
        .read_until_from(
            "load-session available_commands_update",
            search_from,
            |value| is_commands_update(value, &session_id),
        )
        .await;
    assert!(
        update_at > response_at,
        "the update must follow the session/load response"
    );
    let commands = agent.seen[update_at]["params"]["update"]["availableCommands"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_current_session_catalog(&commands);
    agent.stop().await;
}

#[tokio::test]
async fn an_unknown_extension_method_is_method_not_found() {
    let mut agent = Agent::launch();
    let (session_id, _) = agent.open_session().await;
    let unknown = agent
        .rpc(
            json!({"jsonrpc":"2.0","id":3,"method":"_aster/no-such-method",
                    "params":{"sessionId": session_id}}),
        )
        .await;
    assert_eq!(unknown["error"]["code"], -32601, "{unknown}");
    assert!(unknown.get("result").is_none(), "{unknown}");

    let known = agent
        .rpc(
            json!({"jsonrpc":"2.0","id":4,"method":"_aster/session/info",
                    "params":{"sessionId": session_id}}),
        )
        .await;
    assert!(known.get("error").is_none(), "{known}");
    agent.stop().await;
}

#[tokio::test]
async fn terminal_inspection_commands_use_current_memory_and_task_contracts_without_a_model() {
    use aster_core::durable_memory_vec::{MemoryRecord, VecMemoryStore};
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("memory");
    let store = VecMemoryStore::open(&directory).unwrap();
    store
        .put(&MemoryRecord {
            id: "stdio-memory".into(),
            content: "Preserve the pinned Rust toolchain".into(),
            tags: vec![],
            importance: 0.7,
            memory_type: "\"procedural\"".into(),
            timestamp: "2026-10-24T00:00:00Z".into(),
            meta: "{}".into(),
        })
        .unwrap();
    drop(store);
    let config = format!(
        "{USER_ACL}\nmemory_dir = {}\n",
        serde_json::to_string(&directory).unwrap()
    );
    let mut agent = Agent::launch_config(root, &config);
    let (session, _) = agent.open_session().await;
    let second_workspace = agent.workspace.parent().unwrap().join("second-workspace");
    std::fs::create_dir(&second_workspace).unwrap();
    let second_workspace = second_workspace.canonicalize().unwrap();
    let opened = agent
        .rpc(json!({"jsonrpc":"2.0","id":3,"method":"session/new",
        "params":{"cwd":second_workspace,"mcpServers":[]}}))
        .await;
    let second_session = opened["result"]["sessionId"].as_str().unwrap().to_owned();
    let mut id = 20;
    let mut revision = String::new();
    for (target, command, expected, refused) in [
        (
            &session,
            "/memory list Rust".into(),
            "Stored memories: 1",
            false,
        ),
        (
            &session,
            "/memory read stdio-memory".into(),
            "Preserve the pinned Rust toolchain",
            false,
        ),
        (
            &session,
            format!("/memory delete stdio-memory {}", "0".repeat(64)),
            "conflict",
            true,
        ),
        (
            &session,
            "/memory list Rust".into(),
            "Stored memories: 1",
            false,
        ),
        (
            &second_session,
            "/memory read stdio-memory".into(),
            "Preserve the pinned Rust toolchain",
            false,
        ),
        (&session, "/tasks".into(), "No background tasks.", false),
        (
            &session,
            "/tasks stop missing-task".into(),
            "does not belong to this session",
            true,
        ),
    ] {
        id += 1;
        let from = agent.seen.len();
        let response = agent
            .rpc(json!({"jsonrpc":"2.0","id":id,"method":"session/prompt",
            "params":{"sessionId":target,"prompt":[{"type":"text","text":command}]}}))
            .await;
        if refused {
            assert_eq!(response["error"]["code"], -32602, "{response}");
            assert!(
                response["error"]["data"]
                    .as_str()
                    .unwrap()
                    .contains(expected),
                "{response}"
            );
        } else {
            assert_eq!(response["result"]["stopReason"], "end_turn", "{response}");
            let text: String = agent.seen[from..]
                .iter()
                .filter(|message| {
                    message["params"]["sessionId"] == *target
                        && message["params"]["update"]["sessionUpdate"] == "agent_message_chunk"
                })
                .filter_map(|message| message["params"]["update"]["content"]["text"].as_str())
                .collect();
            assert!(text.contains(expected), "{command}: {text}");
            if command == "/memory read stdio-memory" {
                revision = text
                    .lines()
                    .find_map(|line| line.strip_prefix("Revision: "))
                    .unwrap()
                    .into();
            }
        }
    }
    assert_eq!(revision.len(), 64);
    for (id, binding) in [(38, None), (39, Some(session.as_str()))] {
        let refused = agent
            .rpc(
                json!({"jsonrpc":"2.0","id":id,"method":"_aster/memory/manage",
            "params":{"workspace_root":second_workspace,"session_id":binding,
            "request":{"operation":"list","query":"","memory_type":null,"cursor":null}}}),
            )
            .await;
        assert_eq!(refused["error"]["code"], -32002, "{refused}");
    }
    let deleted = agent.rpc(json!({"jsonrpc":"2.0","id":40,"method":"session/prompt",
        "params":{"sessionId":session,"prompt":[{"type":"text","text":format!("/memory delete stdio-memory {revision}")}]}})).await;
    assert_eq!(deleted["result"]["stopReason"], "end_turn", "{deleted}");
    let page = agent.rpc(json!({"jsonrpc":"2.0","id":41,"method":"_aster/memory/manage",
        "params":{"workspace_root":agent.workspace,"request":{"operation":"list","query":"","memory_type":null,"cursor":null}}})).await;
    assert_eq!(page["result"]["outcome"]["Ok"]["total"], 0, "{page}");
    agent.child.kill().await.unwrap();
    agent.child.wait().await.unwrap();
    assert!(
        directory.is_dir(),
        "reopening must inspect the retained store"
    );
    let store = VecMemoryStore::open(&directory).unwrap();
    assert!(store.recall("Rust", 10).unwrap().is_empty());
}

#[tokio::test]
async fn tooling_response_and_update_publish_the_real_versioned_session_projection() {
    let mut agent = Agent::launch();
    let (session_id, response_at) = agent.open_session().await;
    let update_at = agent
        .read_until("session tooling update", |value| {
            value["method"] == "_aster/session/tooling/update"
                && value["params"]["sessionId"] == session_id
        })
        .await;
    assert!(update_at > response_at);
    let update = agent.seen[update_at]["params"].clone();
    assert_eq!(update["schema"], "aster.acp.session-tooling.v1");
    assert!(update["tools"].as_array().unwrap().len() > 5);

    let workspace = agent.workspace.clone();
    let response = agent
        .rpc(
            json!({"jsonrpc":"2.0","id":3,"method":"_aster/session/tooling/v1",
            "params":{
                "schema":"aster.acp.session-tooling.v1",
                "sessionId":session_id,
                "operationId":"stdio-tooling",
                "cwd":workspace,
            }}),
        )
        .await;
    assert!(response.get("error").is_none(), "{response}");
    let result = &response["result"];
    assert_eq!(result["operationId"], "stdio-tooling");
    assert_eq!(result["tools"], update["tools"]);
    assert_eq!(result["capabilityStamp"], update["capabilityStamp"]);
    assert!(result["capabilities"].is_array());
    assert!(result["unavailable"].is_array());
    for duplicate_catalog in ["models", "modes", "commands"] {
        assert!(result.get(duplicate_catalog).is_none());
    }
    for tool in result["tools"].as_array().unwrap() {
        assert!(tool["name"].as_str().is_some_and(|name| !name.is_empty()));
        assert!(tool["description"].is_string());
        assert!(tool["parameters"].is_object());
    }
    agent.stop().await;
}

#[tokio::test]
async fn load_without_a_snapshot_never_creates_a_same_id_empty_session() {
    let mut agent = Agent::launch();
    agent.initialize().await;
    let log = agent.workspace.join(".aster/effect-log");
    std::fs::create_dir_all(&log).unwrap();
    std::fs::write(
        log.join("snapshot-missing.jsonl"),
        "{\"seq\":1,\"type\":\"user.message\",\"key\":\"user:1\",\"payload\":{\"text\":\"legacy\"}}\n",
    )
    .unwrap();
    let workspace = agent.workspace.clone();
    let loaded = agent
        .rpc(json!({"jsonrpc":"2.0","id":2,"method":"session/load",
                    "params":{"sessionId":"snapshot-missing","cwd":workspace,"mcpServers":[]}}))
        .await;
    assert_eq!(loaded["error"]["code"], -32603, "{loaded}");
    assert!(loaded["error"]["data"]
        .as_str()
        .is_some_and(|reason| reason.contains("snapshot store is missing")));

    let page = agent
        .rpc(
            json!({"jsonrpc":"2.0","id":3,"method":"_aster/session/transcript/page/v1",
            "params":{
                "schema":"aster.acp.session-transcript-page.v1",
                "sessionId":"snapshot-missing",
                "operationId":"must-not-exist",
                "cwd":workspace,
                "limit":10,
                "cursor":null,
            }}),
        )
        .await;
    assert!(page.get("result").is_none(), "{page}");
    assert!(page["error"]["data"]
        .as_str()
        .is_some_and(|reason| reason.contains("unknown session id")));
    assert!(!agent.workspace.join(".aster/sessions").exists());
    agent.stop().await;
}
