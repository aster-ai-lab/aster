//! Local agent plane: agents, sessions, and a3s-code turns behind an a3s-boot app on
//! loopback. It stands in for the Electron main process's agent services in the Tauri shell.

mod acp;
pub(crate) mod aix_episodes;

pub(crate) async fn search_admitted_wiki(
    wiki: &std::path::Path,
    query: &str,
) -> std::result::Result<serde_json::Value, String> {
    acp::search_admitted_wiki(wiki, query).await
}
mod catalog;
mod cloud_runtime;
mod controllers;
mod document_read;
mod domain;
mod edge_box;
mod edge_runtime;
mod error;
mod events;
mod execution_observation;
#[cfg(test)]
mod fake_acp;
mod harness_assemble;
mod launcher;
mod plugin_selection;
#[cfg(test)]
#[path = "reference_tasks_live_tests.rs"]
mod reference_tasks_live_tests;
mod review_prompt;
mod review_text;
mod session_archive;
#[cfg(test)]
#[path = "session_archive_operation_tests.rs"]
mod session_archive_operation_tests;
#[cfg(test)]
#[path = "session_archive_tests.rs"]
mod session_archive_tests;
pub(crate) use review_text::ReviewTextSource;

#[cfg(test)]
pub(crate) fn review_model_prompt_for_tests(human: &str) -> String {
    let skill = crate::review_plugin::published_review_skill_text();
    review_prompt::review_model_prompt(human, None, skill.as_deref())
}
mod forms;
#[cfg(test)]
#[path = "knowledge_compile_session_tests.rs"]
mod knowledge_compile_session_tests;
mod mail_mcp;
pub(crate) mod presenton_mcp;
mod question;
mod schedule;
mod schedule_mcp;
mod session_query;
#[cfg(test)]
#[path = "session_query_tests.rs"]
mod session_query_tests;
mod store;
mod stream;
mod tasks;
mod tobot_memory;
mod turns;
mod work_mcp;
mod workbench_artifacts;
mod workbench_proof;
pub(crate) use workbench_artifacts::ArtifactStore;
pub(crate) use workbench_proof::{
    ArtifactOrigin, ArtifactOriginKind, ArtifactProcessorStatus, Proof as ArtifactProof,
};
mod workbench_reviewer;

pub use domain::{PrivateMemory, is_toboss_room_session, private_memory, toboss_room_parts};
pub use tobot_memory::{
    memory_directives, merge_memory_turn, merge_room_memory_turn, room_memory_directives,
    room_retain, room_turn_ask,
};

use std::io;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;

use a3s_boot::{
    AxumAdapter, BootApplication, ControllerDefinition, CorsOptions, ExecutionContext, HttpMethod,
    Module, ModuleRef, ProviderDefinition, Result,
};
use axum::body::Body;
use axum::http::{HeaderValue, Request};
use axum::middleware::{self, Next};
use axum::response::Response;

use catalog::{AgentCatalog, PinCatalog, SessionCatalog};
use controllers::{
    AgentsController, ApprovalsController, EventsController, PinsController, SessionsController,
    TasksController,
};
use forms::FormsController;
use mail_mcp::{DisabledMail, HttpMailReader, MailMcpController, MailMcpEndpoint, MailReader};
use presenton_mcp::PresentonMcpController;
use schedule_mcp::{ScheduleMcpEndpoint, ScheduleTaskMcpController};
use store::PlaneStore;
use tasks::TaskService;
use turns::{EventHub, TurnService};
use work_mcp::{DisabledWork, HttpWorkReader, WorkMcpController, WorkReader};

pub type CredentialLookup =
    Arc<dyn Fn(&str) -> Option<crate::providers::ProviderCredentials> + Send + Sync>;

pub struct PlaneConfig {
    /// `None` keeps state in memory (tests).
    pub root: Option<PathBuf>,
    /// Provider id → endpoint and key, read fresh for every a3s-code launch.
    pub credentials: CredentialLookup,
    /// Overrides `launcher::find_binary`.
    pub binary: Option<PathBuf>,
    pub(crate) workspace_documents: Option<Arc<crate::workspace_document::WorkspaceDocuments>>,
    /// Loopback MCP the scheduled-task skill calls. Absent in tests that do not serve it.
    pub schedule_mcp: Option<ScheduleMcpEndpoint>,
    /// Loopback MCP ToBot uses to read the signed-in member's mailbox.
    pub mail_mcp: Option<MailMcpEndpoint>,
    /// Empty in harnesses, which keep the shared `tobot` session. A tenant slug
    /// resolves ToBot to the signed-in member and refuses the shared file.
    pub slug: String,
    /// Personal-bot wiki directory. `None` reads the saved workspace outside tests.
    pub personal_wiki: Option<PersonalWikiLookup>,
    /// Signed-in Cloud agent-runtime mount. `None` refuses a cloud turn.
    /// The host does not spawn the package instead.
    pub cloud_agent_runtime: Option<cloud_runtime::CloudAgentRuntime>,
}

pub type PersonalWikiLookup = Arc<dyn Fn() -> Option<PathBuf> + Send + Sync>;

impl PlaneConfig {
    fn for_tenant(slug: &str) -> Self {
        let slug = slug.to_owned();
        let credential_slug = slug.clone();
        Self {
            root: crate::providers::tenant_data_dir(&slug).map(|dir| dir.join("agent-plane")),
            credentials: Arc::new(move |provider| {
                let token =
                    crate::member_session::MemberSessionStore::open(&credential_slug).token();
                crate::providers::ProviderStore::open(&credential_slug)
                    .execution_credentials(provider, token.as_deref())
            }),
            binary: None,
            workspace_documents: None,
            schedule_mcp: None,
            mail_mcp: None,
            slug,
            personal_wiki: None,
            cloud_agent_runtime: Some(cloud_runtime::production_endpoint()),
        }
    }

    /// Wiki directory the personal bot searches. A test lookup wins.
    /// Production reads `app.workspace.path` for this tenant.
    pub fn resident_wiki(&self) -> Option<PathBuf> {
        if let Some(resolve) = &self.personal_wiki {
            return resolve();
        }
        #[cfg(test)]
        {
            let _ = &self.slug;
            return None;
        }
        #[cfg(not(test))]
        {
            if self.slug.is_empty() {
                return None;
            }
            let home = std::env::var_os("HOME").map(PathBuf::from)?;
            let stored = crate::local_assets::stored_workspace_preference(&self.slug);
            crate::local_assets::app_wiki_directory(&stored, &home)
        }
    }
}

struct AgentPlaneModule {
    config: Arc<PlaneConfig>,
}

impl Module for AgentPlaneModule {
    fn name(&self) -> &'static str {
        "agent-plane"
    }

    fn providers(&self) -> Result<Vec<ProviderDefinition>> {
        let config = Arc::clone(&self.config);
        let mail_config = Arc::clone(&self.config);
        let work_config = Arc::clone(&self.config);
        let presenton_config = Arc::clone(&self.config);
        Ok(vec![
            ProviderDefinition::singleton(PlaneStore::open(config.root.clone())),
            ProviderDefinition::singleton(EventHub::new()),
            ProviderDefinition::factory::<AgentCatalog, _>(|module_ref: &ModuleRef| {
                Ok(AgentCatalog::new(module_ref.get::<PlaneStore>()?))
            }),
            ProviderDefinition::factory::<SessionCatalog, _>(|module_ref: &ModuleRef| {
                Ok(SessionCatalog::new(module_ref.get::<PlaneStore>()?))
            }),
            ProviderDefinition::factory::<PinCatalog, _>(|module_ref: &ModuleRef| {
                Ok(PinCatalog::new(module_ref.get::<PlaneStore>()?))
            }),
            ProviderDefinition::factory::<TurnService, _>(move |module_ref: &ModuleRef| {
                Ok(TurnService::new(
                    module_ref.get::<PlaneStore>()?,
                    module_ref.get::<AgentCatalog>()?,
                    module_ref.get::<SessionCatalog>()?,
                    Arc::clone(&config),
                    module_ref.get::<EventHub>()?,
                ))
            }),
            ProviderDefinition::factory::<TaskService, _>(|module_ref: &ModuleRef| {
                Ok(TaskService::new(
                    module_ref.get::<PlaneStore>()?,
                    module_ref.get::<AgentCatalog>()?,
                    module_ref.get::<SessionCatalog>()?,
                    module_ref.get::<TurnService>()?,
                    module_ref.get::<EventHub>()?,
                ))
            }),
            ProviderDefinition::factory::<AgentsController, _>(|module_ref: &ModuleRef| {
                Ok(AgentsController {
                    agents: module_ref.get::<AgentCatalog>()?,
                    tasks: module_ref.get::<TaskService>()?,
                })
            }),
            ProviderDefinition::factory::<SessionsController, _>(|module_ref: &ModuleRef| {
                Ok(SessionsController {
                    sessions: module_ref.get::<SessionCatalog>()?,
                    turns: module_ref.get::<TurnService>()?,
                    hub: module_ref.get::<EventHub>()?,
                })
            }),
            ProviderDefinition::factory::<PinsController, _>(|module_ref: &ModuleRef| {
                Ok(PinsController {
                    pins: module_ref.get::<PinCatalog>()?,
                })
            }),
            ProviderDefinition::factory::<ApprovalsController, _>(|module_ref: &ModuleRef| {
                Ok(ApprovalsController {
                    turns: module_ref.get::<TurnService>()?,
                })
            }),
            ProviderDefinition::factory::<FormsController, _>(|module_ref: &ModuleRef| {
                Ok(FormsController {
                    turns: module_ref.get::<TurnService>()?,
                })
            }),
            ProviderDefinition::factory::<EventsController, _>(|module_ref: &ModuleRef| {
                Ok(EventsController {
                    hub: module_ref.get::<EventHub>()?,
                })
            }),
            ProviderDefinition::factory::<TasksController, _>(|module_ref: &ModuleRef| {
                let tasks = module_ref.get::<TaskService>()?;
                tasks.arm();
                Ok(TasksController { tasks })
            }),
            ProviderDefinition::factory::<ScheduleTaskMcpController, _>(
                |module_ref: &ModuleRef| {
                    Ok(ScheduleTaskMcpController {
                        tasks: module_ref.get::<TaskService>()?,
                        agents: module_ref.get::<AgentCatalog>()?,
                    })
                },
            ),
            ProviderDefinition::factory::<MailMcpController, _>(move |_module_ref: &ModuleRef| {
                let reader: Arc<dyn MailReader> = match mail_config.mail_mcp.as_ref() {
                    Some(endpoint) => Arc::new(HttpMailReader {
                        origin: endpoint.origin.clone(),
                        slug: endpoint.slug.clone(),
                    }),
                    None => Arc::new(DisabledMail),
                };
                Ok(MailMcpController { reader })
            }),
            ProviderDefinition::factory::<WorkMcpController, _>(move |_module_ref: &ModuleRef| {
                let reader: Arc<dyn WorkReader> = match work_config.mail_mcp.as_ref() {
                    Some(endpoint) => Arc::new(HttpWorkReader {
                        origin: endpoint.origin.clone(),
                        slug: endpoint.slug.clone(),
                    }),
                    None => Arc::new(DisabledWork),
                };
                Ok(WorkMcpController { reader })
            }),
            ProviderDefinition::factory::<PresentonMcpController, _>(
                move |_module_ref: &ModuleRef| {
                    let caps = presenton_config
                        .root
                        .as_ref()
                        .and_then(|root| root.parent().map(|tenant| tenant.join("capabilities")));
                    Ok(PresentonMcpController::new(caps))
                },
            ),
        ])
    }

    fn controllers(&self, module_ref: &ModuleRef) -> Result<Vec<ControllerDefinition>> {
        Ok(vec![
            module_ref.get::<AgentsController>()?.controller()?,
            module_ref.get::<SessionsController>()?.controller()?,
            module_ref.get::<PinsController>()?.controller()?,
            module_ref.get::<ApprovalsController>()?.controller()?,
            module_ref.get::<FormsController>()?.controller()?,
            module_ref.get::<EventsController>()?.controller()?,
            module_ref.get::<TasksController>()?.controller()?,
            module_ref
                .get::<ScheduleTaskMcpController>()?
                .controller()?,
            module_ref.get::<MailMcpController>()?.controller()?,
            module_ref.get::<WorkMcpController>()?.controller()?,
            module_ref.get::<PresentonMcpController>()?.controller()?,
        ])
    }
}

/// The shell's bearer token, or `?token=` for EventSource, which cannot set headers.
fn authorized(context: &ExecutionContext, token: &str) -> bool {
    if context.method == HttpMethod::Options {
        return true;
    }
    let presented = context
        .request
        .header("authorization")
        .and_then(|value| value.strip_prefix("Bearer "))
        .or_else(|| context.request.query_param("token"));
    presented.is_some_and(|presented| crate::authority::tokens_match(presented, token))
}

fn application(config: PlaneConfig, token: String) -> Result<BootApplication> {
    let cors = CorsOptions::new()
        .allow_any_origin()
        .allow_any_header()
        .allow_methods([
            HttpMethod::Get,
            HttpMethod::Post,
            HttpMethod::Put,
            HttpMethod::Patch,
            HttpMethod::Delete,
        ]);
    BootApplication::builder()
        .import(AgentPlaneModule {
            config: Arc::new(config),
        })
        .use_global_cors(cors)
        .use_global_guard(move |context: ExecutionContext| {
            let allowed = authorized(&context, &token);
            async move { Ok(allowed) }
        })
        .build()
}

/// Binds `127.0.0.1` on `listen_port` and serves for the process lifetime.
/// Port 0 asks the system for an ephemeral port. A side Studio instance uses that.
pub fn start(
    slug: &str,
    token: &str,
    documents: Arc<crate::workspace_document::WorkspaceDocuments>,
    listen_port: u16,
) -> io::Result<String> {
    let listener = crate::authority::bind_loopback_preferring(listen_port, "studio agent plane")?;
    listener.set_nonblocking(true)?;
    let base_url = format!("http://{}", listener.local_addr()?);
    let mut config = PlaneConfig::for_tenant(slug);
    config.workspace_documents = Some(documents);
    config.schedule_mcp = Some(ScheduleMcpEndpoint {
        base_url: base_url.clone(),
        token: token.to_owned(),
    });
    config.mail_mcp = Some(MailMcpEndpoint {
        base_url: base_url.clone(),
        token: token.to_owned(),
        slug: slug.to_owned(),
        origin: crate::org_login::default_origin(),
    });
    config.cloud_agent_runtime = Some(cloud_runtime::production_endpoint());
    let app = application(config, token.to_owned()).map_err(io::Error::other)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("studio-agent-plane")
        .enable_all()
        .build()?;
    std::thread::Builder::new()
        .name("studio-agent-plane".to_owned())
        .spawn(move || {
            runtime.block_on(async move {
                if let Err(error) = serve(app, listener).await {
                    eprintln!("studio: agent plane stopped: {error}");
                }
            });
        })?;
    Ok(base_url)
}

async fn serve(app: BootApplication, listener: TcpListener) -> io::Result<()> {
    app.bootstrap().await.map_err(io::Error::other)?;
    let router = app
        .into_adapter(&AxumAdapter::new())
        .map_err(io::Error::other)?
        // a3s-boot adds CORS on success responses only. A bare error is opaque to
        // WKWebView, which reports the failed turn as "Load failed".
        .layer(middleware::from_fn(cors_on_errors));
    let listener = tokio::net::TcpListener::from_std(listener)?;
    axum::serve(listener, router).await
}

async fn cors_on_errors(request: Request<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    if response
        .headers()
        .get("access-control-allow-origin")
        .is_none()
    {
        response
            .headers_mut()
            .insert("access-control-allow-origin", HeaderValue::from_static("*"));
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    pub(super) const TOKEN: &str =
        "abababababababababababababababababababababababababababababababab";

    #[test]
    fn tenant_plane_points_cloud_runtime_at_the_member_org_mount() {
        let config = PlaneConfig::for_tenant("acme");
        let Some(endpoint) = config.cloud_agent_runtime else {
            panic!("signed-in cloud runtime");
        };
        let origin = crate::org_login::default_origin();
        let origin = origin.trim_end_matches('/');
        assert_eq!(endpoint.base, format!("{origin}/v1/api/org/cloud"));
        assert!(!endpoint.base.contains("/v1/api/admin/"));
        assert!(endpoint.bearer.is_none());
        assert!(
            endpoint.base.starts_with("http://127.0.0.1:")
                || endpoint.base.starts_with("http://localhost:")
        );
    }

    pub(super) fn request(
        base: &str,
        method: &str,
        path: &str,
        token: Option<&str>,
        body: Option<&str>,
    ) -> (u16, String) {
        use std::io::{Read, Write};
        let address = base.trim_start_matches("http://");
        let mut stream = std::net::TcpStream::connect(address).unwrap();
        let body = body.unwrap_or("");
        let auth = token
            .map(|token| format!("Authorization: Bearer {token}\r\n"))
            .unwrap_or_default();
        let head = format!(
            "{method} {path} HTTP/1.1\r\nHost: {address}\r\n{auth}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(head.as_bytes()).unwrap();
        let mut raw = String::new();
        stream.read_to_string(&mut raw).unwrap();
        let status = raw[9..12].parse().unwrap();
        let payload = raw.split("\r\n\r\n").nth(1).unwrap_or("").to_owned();
        (status, payload)
    }

    pub(super) fn start_in_memory() -> String {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let app = application(
            PlaneConfig {
                root: None,
                credentials: Arc::new(|_| None),
                binary: None,
                workspace_documents: None,
                schedule_mcp: None,
                mail_mcp: None,
                slug: String::new(),
                personal_wiki: None,
                cloud_agent_runtime: None,
            },
            TOKEN.into(),
        )
        .unwrap();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        std::thread::spawn(move || runtime.block_on(serve(app, listener)));
        base
    }

    #[test]
    fn serves_agents_and_sessions_behind_the_token() {
        let base = start_in_memory();
        let (status, _) = request(&base, "GET", "/api/agents", None, None);
        assert_eq!(status, 403);

        let (status, body) = request(
            &base,
            "POST",
            "/api/agents",
            Some(TOKEN),
            Some(r#"{"type":"a3s-code","name":"Helper","model":"openai::gpt-4o"}"#),
        );
        assert_eq!(status, 201, "{body}");
        let agent: serde_json::Value = serde_json::from_str(&body).unwrap();
        let agent_id = agent["id"].as_str().unwrap();

        let (status, body) = request(&base, "GET", "/api/agents?limit=500", Some(TOKEN), None);
        assert_eq!(status, 200);
        assert!(body.contains(agent_id));

        let (status, body) = request(
            &base,
            "POST",
            "/api/agent-sessions/reuse-or-create",
            Some(TOKEN),
            Some(&format!(
                r#"{{"agentId":"{agent_id}","workspace":{{"type":"system"}}}}"#
            )),
        );
        assert_eq!(status, 200, "{body}");
        let created: serde_json::Value = serde_json::from_str(&body).unwrap();
        let session_id = created["session"]["id"].as_str().unwrap();

        let (status, body) = request(
            &base,
            "GET",
            "/api/agent-sessions/latest",
            Some(TOKEN),
            None,
        );
        assert_eq!(status, 200);
        assert!(body.contains(session_id));

        let (status, body) = request(
            &base,
            "GET",
            &format!("/api/agent-sessions/{session_id}/messages?limit=50"),
            Some(TOKEN),
            None,
        );
        assert_eq!(status, 200);
        assert_eq!(body, r#"{"items":[]}"#);

        let (status, _) = request(
            &base,
            "GET",
            "/api/agent-sessions/missing",
            Some(TOKEN),
            None,
        );
        assert_eq!(status, 404);

        let (status, body) = request(
            &base,
            "GET",
            &format!("/api/agent-sessions/{session_id}/turn"),
            Some(TOKEN),
            None,
        );
        assert_eq!(status, 200);
        assert_eq!(body, r#"{"status":"not-found"}"#);

        let (status, body) = request(
            &base,
            "POST",
            "/api/agent-sessions/archive",
            Some(TOKEN),
            Some(&format!(r#"{{"sessionIds":["{session_id}"]}}"#)),
        );
        assert_eq!(status, 200, "{body}");
        assert!(body.contains(session_id));

        let (status, body) = request(
            &base,
            "GET",
            "/api/agent-sessions/latest",
            Some(TOKEN),
            None,
        );
        assert_eq!(status, 200);
        assert!(!body.contains(session_id));
    }

    #[test]
    fn schedule_task_skill_creates_a_task() {
        let base = start_in_memory();
        let (status, body) = request(
            &base,
            "POST",
            "/api/agents",
            Some(TOKEN),
            Some(r#"{"type":"a3s-code","name":"Helper","model":"openai::gpt-4o"}"#),
        );
        assert_eq!(status, 201, "{body}");

        let (status, _) = request(&base, "POST", "/mcp/schedule-task", None, Some("{}"));
        assert_eq!(status, 403);

        let (status, body) = request(
            &base,
            "POST",
            "/mcp/schedule-task",
            Some(TOKEN),
            Some(
                r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"create_scheduled_task","arguments":{"name":"Morning brief","prompt":"Summarize the inbox","cron":"0 9 * * *"}}}"#,
            ),
        );
        assert_eq!(status, 200, "{body}");
        let created: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(created["id"], 1);
        assert_eq!(created["result"]["isError"], false, "{body}");
        let text = created["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("Morning brief"), "{text}");
        assert!(text.contains("0 9 * * *"), "{text}");
        assert!(
            text.contains("\"type\":\"system\"") || text.contains("\"type\": \"system\""),
            "{text}"
        );

        let (status, listed) =
            request(&base, "GET", "/api/agent-tasks?limit=50", Some(TOKEN), None);
        assert_eq!(status, 200, "{listed}");
        assert!(listed.contains("Morning brief"), "{listed}");

        let listed_tool = mcp(
            &base,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"list_scheduled_tasks","arguments":{}}}"#,
        );
        let listed_text = listed_tool["result"]["content"][0]["text"]
            .as_str()
            .unwrap();
        assert!(listed_text.contains("Morning brief"), "{listed_text}");
    }

    #[test]
    fn schedule_task_skill_lists_its_tools_and_stores_the_chosen_context() {
        let base = start_in_memory();
        let _ = request(
            &base,
            "POST",
            "/api/agents",
            Some(TOKEN),
            Some(r#"{"type":"a3s-code","name":"Helper","model":"openai::gpt-4o"}"#),
        );
        let handshake = mcp(
            &base,
            r#"{"jsonrpc":"2.0","id":"boot","method":"initialize","params":{"protocolVersion":"2025-03-26"}}"#,
        );
        assert_eq!(handshake["id"], "boot");
        assert_eq!(handshake["result"]["serverInfo"]["name"], "schedule-task");
        assert_eq!(handshake["result"]["protocolVersion"], "2025-03-26");

        let tools = mcp(&base, r#"{"jsonrpc":"2.0","id":3,"method":"tools/list"}"#);
        let names: Vec<&str> = tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect();
        assert_eq!(names, vec!["create_scheduled_task", "list_scheduled_tasks"]);

        let created = mcp(
            &base,
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"create_scheduled_task","arguments":{"name":"Brief","prompt":"Read the notes","cron":"30 10 * * 1-5","model":"openai::gpt-4o","skills":["pdf"],"files":[{"path":"/tmp/a b.md"}]}}}"#,
        );
        assert_eq!(created["result"]["isError"], false, "{created}");
        let text = created["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("gpt-4o"), "{text}");
        assert!(text.contains("pdf"), "{text}");
        assert!(text.contains("/tmp/a b.md"), "{text}");
        assert!(text.contains("a b.md"), "{text}");
    }

    #[test]
    fn schedule_task_skill_rejects_bad_input_and_an_ambiguous_owner() {
        let base = start_in_memory();
        request(
            &base,
            "POST",
            "/api/agents",
            Some(TOKEN),
            Some(r#"{"type":"a3s-code","name":"Helper","model":"openai::gpt-4o"}"#),
        );
        let bad_cron = mcp(
            &base,
            r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"create_scheduled_task","arguments":{"name":"Nope","prompt":"Do it","cron":"not cron"}}}"#,
        );
        assert_eq!(bad_cron["result"]["isError"], true, "{bad_cron}");
        assert!(
            bad_cron["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("invalid trigger"),
            "{bad_cron}"
        );

        let relative = mcp(
            &base,
            r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"create_scheduled_task","arguments":{"name":"Files","prompt":"Read","cron":"0 9 * * *","files":[{"path":"notes.md"}]}}}"#,
        );
        assert_eq!(relative["result"]["isError"], true, "{relative}");

        let (status, listed) =
            request(&base, "GET", "/api/agent-tasks?limit=50", Some(TOKEN), None);
        assert_eq!(status, 200, "{listed}");
        assert!(!listed.contains("Nope"), "{listed}");
        assert!(!listed.contains("Files"), "{listed}");

        let second = request(
            &base,
            "POST",
            "/api/agents",
            Some(TOKEN),
            Some(r#"{"type":"a3s-code","name":"Other","model":"openai::gpt-4o"}"#),
        );
        assert_eq!(second.0, 201, "{}", second.1);
        let other_id = serde_json::from_str::<serde_json::Value>(&second.1).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let ambiguous = mcp(
            &base,
            r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"create_scheduled_task","arguments":{"name":"Shared","prompt":"Do it","cron":"0 9 * * *"}}}"#,
        );
        assert_eq!(ambiguous["result"]["isError"], true, "{ambiguous}");
        assert!(
            ambiguous["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("agentId"),
            "{ambiguous}"
        );

        let owned = mcp(
            &base,
            &format!(
                r#"{{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{{"name":"create_scheduled_task","arguments":{{"agentId":"{other_id}","name":"Owned","prompt":"Do it","cron":"0 9 * * *"}}}}}}"#
            ),
        );
        assert_eq!(owned["result"]["isError"], false, "{owned}");
        assert!(
            owned["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains(&other_id)
        );

        let missing = mcp(
            &base,
            r#"{"jsonrpc":"2.0","id":9,"method":"tools/missing"}"#,
        );
        assert_eq!(missing["error"]["code"], -32601);

        let (status, quiet) = request(
            &base,
            "POST",
            "/mcp/schedule-task",
            Some(TOKEN),
            Some(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#),
        );
        assert_eq!(status, 200, "{quiet}");
        assert!(quiet.trim().is_empty() || quiet.trim() == "null", "{quiet}");
    }

    fn mcp(base: &str, body: &str) -> serde_json::Value {
        let (status, raw) = request(base, "POST", "/mcp/schedule-task", Some(TOKEN), Some(body));
        assert_eq!(status, 200, "{raw}");
        serde_json::from_str(&raw).unwrap()
    }

    fn start_on_disk(root: std::path::PathBuf) -> String {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let app = application(
            PlaneConfig {
                root: Some(root),
                credentials: Arc::new(|_| None),
                binary: None,
                workspace_documents: None,
                schedule_mcp: None,
                mail_mcp: None,
                slug: String::new(),
                personal_wiki: None,
                cloud_agent_runtime: None,
            },
            TOKEN.into(),
        )
        .unwrap();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        std::thread::spawn(move || runtime.block_on(serve(app, listener)));
        base
    }

    fn json_at(
        base: &str,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> (u16, serde_json::Value) {
        let (status, payload) = request(base, method, path, Some(TOKEN), body);
        let value = if payload.trim().is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_str(&payload).unwrap_or_else(|error| panic!("{error}: {payload}"))
        };
        (status, value)
    }

    /// Scheduled-task lifecycle against the loopback plane the workbench uses.
    ///
    /// Cases:
    /// 1. unauthenticated list is refused
    /// 2. an unknown agent and an unparsed cron are refused
    /// 3. daily, weekday, custom-weekday, one-shot, and missed tasks are created
    /// 4. the list and the on-disk file contain those rows
    /// 5. pause, resume, and rename round-trip
    /// 6. a manual run with an unresolved workspace is recorded as a failed log
    #[test]
    fn serves_scheduled_tasks_end_to_end() {
        use chrono::{Datelike, Timelike};

        let root = std::env::temp_dir().join(format!("studio-task-e2e-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let base = start_on_disk(root.clone());

        let (status, _) = request(&base, "GET", "/api/agent-tasks?limit=50", None, None);
        assert_eq!(status, 403);

        let (status, agent) = json_at(
            &base,
            "POST",
            "/api/agents",
            Some(r#"{"type":"a3s-code","name":"ToWork 工作助手","model":"openai::gpt-4o"}"#),
        );
        assert_eq!(status, 201, "{agent}");
        let agent_id = agent["id"].as_str().unwrap();

        let (status, denied) = json_at(
            &base,
            "POST",
            "/api/agent-tasks",
            Some(&format!(
                r#"{{"agentId":"missing","name":"Nope","prompt":"Go","trigger":{{"kind":"cron","expr":"0 9 * * *"}},"workspace":{{"type":"system"}}}}"#
            )),
        );
        assert_eq!(status, 404, "{denied}");

        let (status, invalid) = json_at(
            &base,
            "POST",
            "/api/agent-tasks",
            Some(&format!(
                r#"{{"agentId":"{agent_id}","name":"Broken","prompt":"Go","trigger":{{"kind":"cron","expr":"not cron"}},"workspace":{{"type":"system"}}}}"#
            )),
        );
        assert_eq!(status, 400, "{invalid}");

        let create = |name: &str, prompt: &str, trigger: &str, workspace: &str| {
            json_at(
                &base,
                "POST",
                "/api/agent-tasks",
                Some(&format!(
                    r#"{{"agentId":"{agent_id}","name":"{name}","prompt":"{prompt}","trigger":{trigger},"workspace":{workspace},"timeoutMinutes":15,"reuseSession":false}}"#
                )),
            )
        };

        let (status, daily) = create(
            "【测试】每日早报",
            "汇总昨夜邮件",
            r#"{"kind":"cron","expr":"0 9 * * *"}"#,
            r#"{"type":"system"}"#,
        );
        assert_eq!(status, 201, "{daily}");
        assert_eq!(daily["status"], "active");
        assert!(daily["nextRun"].as_str().is_some());

        let (status, weekdays) = create(
            "【测试】工作日巡检",
            "检查队列",
            r#"{"kind":"cron","expr":"30 10 * * 1-5"}"#,
            r#"{"type":"system"}"#,
        );
        assert_eq!(status, 201, "{weekdays}");
        let weekday_next =
            chrono::DateTime::parse_from_rfc3339(weekdays["nextRun"].as_str().unwrap()).unwrap();
        let weekday = weekday_next.weekday().num_days_from_sunday();
        assert!((1..=5).contains(&weekday), "{weekday_next}");
        assert_eq!(weekday_next.hour(), 10);
        assert_eq!(weekday_next.minute(), 30);

        let (status, custom) = create(
            "【测试】自定义工作日",
            "每个工作日每隔 2 小时看一眼",
            r#"{"kind":"cron","expr":"0 */2 * * 1,2,3,4,5"}"#,
            r#"{"type":"system"}"#,
        );
        assert_eq!(status, 201, "{custom}");
        assert_eq!(custom["trigger"]["expr"], "0 */2 * * 1,2,3,4,5");
        let custom_next =
            chrono::DateTime::parse_from_rfc3339(custom["nextRun"].as_str().unwrap()).unwrap();
        let custom_day = custom_next.weekday().num_days_from_sunday();
        assert!((1..=5).contains(&custom_day), "{custom_next}");
        assert_eq!(custom_next.minute(), 0);
        assert_eq!(custom_next.hour() % 2, 0);

        let once_at = chrono::DateTime::parse_from_rfc3339("2026-12-01T01:00:00.000Z")
            .unwrap()
            .timestamp_millis();
        let (status, once) = create(
            "【测试】一次性提醒",
            "年底前回顾一次",
            &format!(r#"{{"kind":"once","at":{once_at}}}"#),
            r#"{"type":"system"}"#,
        );
        assert_eq!(status, 201, "{once}");
        assert_eq!(once["status"], "active");

        let missed_at = (chrono::Utc::now() - chrono::Duration::hours(48)).timestamp_millis();
        let (status, missed) = create(
            "【测试】已错过的提醒",
            "这班已经过了",
            &format!(r#"{{"kind":"once","at":{missed_at}}}"#),
            r#"{"type":"system"}"#,
        );
        assert_eq!(status, 201, "{missed}");
        assert_eq!(missed["status"], "missed");

        let custom_id = custom["id"].as_str().unwrap();
        let (status, paused) = json_at(
            &base,
            "POST",
            &format!("/api/agent-tasks/{custom_id}/pause"),
            Some(&format!(r#"{{"agentId":"{agent_id}"}}"#)),
        );
        assert_eq!(status, 200, "{paused}");
        assert_eq!(paused["status"], "paused");
        assert_eq!(paused["enabled"], false);

        let (status, resumed) = json_at(
            &base,
            "POST",
            &format!("/api/agent-tasks/{custom_id}/resume"),
            Some(&format!(r#"{{"agentId":"{agent_id}"}}"#)),
        );
        assert_eq!(status, 200, "{resumed}");
        assert_eq!(resumed["status"], "active");

        let (status, paused) = json_at(
            &base,
            "POST",
            &format!("/api/agent-tasks/{custom_id}/pause"),
            Some(&format!(r#"{{"agentId":"{agent_id}"}}"#)),
        );
        assert_eq!(status, 200, "{paused}");

        let daily_id = daily["id"].as_str().unwrap();
        let (status, renamed) = json_at(
            &base,
            "PATCH",
            &format!("/api/agent-tasks/{daily_id}"),
            Some(&format!(
                r#"{{"agentId":"{agent_id}","name":"【测试】每日早报（已改名）"}}"#
            )),
        );
        assert_eq!(status, 200, "{renamed}");
        assert_eq!(renamed["name"], "【测试】每日早报（已改名）");
        assert_eq!(renamed["prompt"], "汇总昨夜邮件");

        let (status, listed) = json_at(&base, "GET", "/api/agent-tasks?limit=500", None);
        assert_eq!(status, 200, "{listed}");
        assert_eq!(listed["total"], 5);
        let names: Vec<&str> = listed["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"【测试】每日早报（已改名）"));
        assert!(names.contains(&"【测试】自定义工作日"));

        let (status, unresolved) = create(
            "【测试】目录没准备好",
            "这一次应该记成失败",
            r#"{"kind":"cron","expr":"0 9 * * *"}"#,
            r#"{"type":"user","workspaceId":"missing-workspace"}"#,
        );
        assert_eq!(status, 201, "{unresolved}");
        let unresolved_id = unresolved["id"].as_str().unwrap();
        let (status, ran) = json_at(
            &base,
            "POST",
            &format!("/api/agent-tasks/{unresolved_id}/run"),
            Some(&format!(r#"{{"agentId":"{agent_id}"}}"#)),
        );
        assert_eq!(status, 400, "{ran}");
        let (status, logs) = json_at(
            &base,
            "GET",
            &format!("/api/agents/{agent_id}/tasks/{unresolved_id}/logs?limit=50"),
            None,
        );
        assert_eq!(status, 200, "{logs}");
        assert_eq!(logs["items"][0]["status"], "failed");
        assert!(
            logs["items"][0]["error"]
                .as_str()
                .unwrap()
                .contains("workspace was not resolved")
        );

        let (status, _) = json_at(
            &base,
            "DELETE",
            &format!("/api/agent-tasks/{unresolved_id}?agentId={agent_id}"),
            None,
        );
        assert_eq!(status, 200);

        let (status, missing) = json_at(
            &base,
            "GET",
            &format!("/api/agent-tasks/{unresolved_id}"),
            None,
        );
        assert_eq!(status, 404, "{missing}");

        let stored: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(root.join("tasks.json")).unwrap())
                .unwrap();
        let stored_names: Vec<&str> = stored
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["name"].as_str().unwrap())
            .collect();
        assert_eq!(stored_names.len(), 5);
        assert!(
            stored_names
                .iter()
                .any(|name| name.contains("自定义工作日"))
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The hub subscribes when this socket is accepted. Connect and read the
    /// hello comment before `POST /turns`, or the turn's events are gone.
    struct LiveSse {
        stream: std::net::TcpStream,
        raw: Vec<u8>,
        cursor: usize,
    }

    impl LiveSse {
        fn open(base: &str) -> Self {
            use std::io::Write;
            let address = base.trim_start_matches("http://");
            let mut stream = std::net::TcpStream::connect(address).unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(15)))
                .unwrap();
            let head = format!(
                "GET /api/agent-events HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {TOKEN}\r\nAccept: text/event-stream\r\nConnection: keep-alive\r\n\r\n"
            );
            stream.write_all(head.as_bytes()).unwrap();
            Self {
                stream,
                raw: Vec::new(),
                cursor: 0,
            }
        }

        fn decoded(&self) -> String {
            let Some(split) = self.raw.windows(4).position(|window| window == b"\r\n\r\n") else {
                return String::new();
            };
            let headers = String::from_utf8_lossy(&self.raw[..split]).to_ascii_lowercase();
            let body = &self.raw[split + 4..];
            if headers.contains("transfer-encoding: chunked") {
                decode_chunks(body)
            } else {
                String::from_utf8_lossy(body).into_owned()
            }
        }

        fn read_some(&mut self) {
            use std::io::Read;
            let mut buf = [0u8; 8192];
            match self.stream.read(&mut buf) {
                Ok(0) => panic!("sse closed\n{}", String::from_utf8_lossy(&self.raw)),
                Ok(n) => self.raw.extend_from_slice(&buf[..n]),
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        || error.kind() == std::io::ErrorKind::TimedOut
                        || error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(error) => panic!("{error}\n{}", String::from_utf8_lossy(&self.raw)),
            }
        }

        fn wait_until(&mut self, ready: impl Fn(&str) -> bool) -> String {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
            loop {
                let text = self.decoded();
                if ready(&text) {
                    return text;
                }
                if std::time::Instant::now() >= deadline {
                    panic!("sse timed out\n{}", String::from_utf8_lossy(&self.raw));
                }
                self.read_some();
            }
        }

        /// Events from the previous cursor through the next `channel` frame.
        fn read_channel(&mut self, channel: &str) -> Vec<serde_json::Value> {
            let cursor = self.cursor;
            let needle = format!("\"channel\":\"{channel}\"");
            let error_needle = "\"channel\":\"ai.stream.error\"";
            let text = self.wait_until(|text| {
                let Some(rest) = text.get(cursor..) else {
                    return false;
                };
                let ready = |mark: &str| {
                    rest.find(mark)
                        .is_some_and(|at| rest[at..].contains("\n\n"))
                };
                ready(&needle) || (channel != "ai.stream.error" && ready(error_needle))
            });
            let rest = &text[cursor..];
            if channel != "ai.stream.error" {
                if let Some(at) = rest.find(error_needle) {
                    let done_at = rest.find(&needle);
                    if done_at.is_none_or(|done| at < done) && rest[at..].contains("\n\n") {
                        let end = rest[at..].find("\n\n").unwrap() + at;
                        panic!("turn failed: {}", &rest[..end]);
                    }
                }
            }
            let at = rest.find(&needle).unwrap();
            let end = rest[at..].find("\n\n").unwrap() + at + 2;
            let slice = rest[..end].to_owned();
            self.cursor = cursor + end;
            sse_events(&slice)
        }
    }

    fn decode_chunks(mut body: &[u8]) -> String {
        let mut out = Vec::new();
        loop {
            let Some(line_end) = body.windows(2).position(|window| window == b"\r\n") else {
                break;
            };
            let line = std::str::from_utf8(&body[..line_end]).unwrap_or("");
            let size_hex = line.split(';').next().unwrap_or("").trim();
            if size_hex.is_empty() {
                break;
            }
            let Ok(size) = usize::from_str_radix(size_hex, 16) else {
                break;
            };
            let rest = &body[line_end + 2..];
            if rest.len() < size + 2 {
                break;
            }
            out.extend_from_slice(&rest[..size]);
            body = &rest[size + 2..];
            if size == 0 {
                break;
            }
        }
        String::from_utf8_lossy(&out).into_owned()
    }

    fn sse_events(text: &str) -> Vec<serde_json::Value> {
        text.split("\n\n")
            .filter_map(|block| block.lines().find_map(|line| line.strip_prefix("data: ")))
            .map(|data| {
                serde_json::from_str(data).unwrap_or_else(|error| panic!("{error}: {data}"))
            })
            .collect()
    }

    pub(super) struct TestPlane {
        url: String,
        stop: Option<tokio::sync::oneshot::Sender<()>>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl std::ops::Deref for TestPlane {
        type Target = str;
        fn deref(&self) -> &str {
            &self.url
        }
    }

    impl Drop for TestPlane {
        fn drop(&mut self) {
            if let Some(stop) = self.stop.take() {
                let _ = stop.send(());
            }
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    pub(super) fn start_with_binary(
        root: std::path::PathBuf,
        binary: std::path::PathBuf,
    ) -> TestPlane {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let app = application(
            PlaneConfig {
                root: Some(root),
                credentials: Arc::new(|_| {
                    Some(crate::providers::ProviderCredentials {
                        base_url: "https://example.test/v1".into(),
                        api_key: "sk-test".into(),
                    })
                }),
                binary: Some(binary),
                workspace_documents: None,
                schedule_mcp: None,
                mail_mcp: None,
                slug: String::new(),
                personal_wiki: None,
                cloud_agent_runtime: None,
            },
            TOKEN.into(),
        )
        .unwrap();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let turns = app.get::<TurnService>().unwrap();
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let thread = std::thread::spawn(move || {
            runtime.block_on(async {
                tokio::select! { _ = serve(app, listener) => {}, _ = stopped => {} }
                turns.close_test_runtimes().await;
            })
        });
        TestPlane {
            url: base,
            stop: Some(stop),
            thread: Some(thread),
        }
    }

    fn json_body(value: serde_json::Value) -> String {
        value.to_string()
    }

    fn part_kinds(parts: &[serde_json::Value]) -> Vec<&str> {
        parts
            .iter()
            .map(|part| part["type"].as_str().unwrap_or(""))
            .collect()
    }

    fn assistant_turn(base: &str, session_id: &str) -> (Vec<serde_json::Value>, serde_json::Value) {
        let (status, body) = request(
            base,
            "GET",
            &format!("/api/agent-sessions/{session_id}/messages?limit=50"),
            Some(TOKEN),
            None,
        );
        assert_eq!(status, 200, "{body}");
        let page: serde_json::Value = serde_json::from_str(&body).unwrap();
        let assistant = page["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["role"] == "assistant")
            .unwrap_or_else(|| panic!("no assistant in {body}"));
        assert_eq!(assistant["status"], "success");
        let parts = assistant["data"]["parts"].as_array().unwrap().clone();
        (parts, page)
    }

    /// The plane's JSON escapes non-ASCII and newlines. The transcript keeps the
    /// characters the assertions already read, so the saved record is readable.
    fn readable_json(value: &serde_json::Value) -> String {
        let raw = serde_json::to_string_pretty(value).unwrap();
        let mut out = String::new();
        let mut rest = raw.as_str();
        while let Some(index) = rest.find('\\') {
            out.push_str(&rest[..index]);
            let escaped = &rest[index..];
            if let Some(hex) = escaped.strip_prefix("\\u") {
                if hex.len() >= 4 {
                    if let Ok(code) = u32::from_str_radix(&hex[..4], 16) {
                        if let Some(decoded) = char::from_u32(code) {
                            out.push(decoded);
                            rest = &hex[4..];
                            continue;
                        }
                    }
                }
            }
            let next = escaped.chars().nth(1);
            match next {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('\\') => out.push('\\'),
                Some('"') => out.push('"'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
            rest = &escaped[next.map(|ch| ch.len_utf8() + 1).unwrap_or(1)..];
        }
        out.push_str(rest);
        out
    }

    fn save_transcript(name: &str, value: &serde_json::Value) {
        let Ok(dir) = std::env::var("A3S_TRANSCRIPT_DIR") else {
            return;
        };
        if dir.is_empty() {
            return;
        }
        let path = std::path::Path::new(&dir).join(name);
        let text = readable_json(value);
        std::fs::write(&path, text).unwrap_or_else(|error| {
            panic!("could not write {}: {error}", path.display());
        });
    }

    fn chunk_index(events: &[serde_json::Value], kind: &str, field: &str, value: &str) -> usize {
        events
            .iter()
            .position(|event| {
                event["channel"] == "ai.stream.chunk"
                    && event["payload"]["chunk"]["type"] == kind
                    && (field.is_empty() || event["payload"]["chunk"][field] == value)
            })
            .unwrap_or_else(|| panic!("missing {kind} {value} in {events:?}"))
    }

    #[test]
    fn http_turn_finishes_prose_tool_and_written_file() {
        if std::process::Command::new("node")
            .arg("--version")
            .output()
            .is_err()
        {
            eprintln!("skipping: node is not installed");
            return;
        }
        let dir = std::env::temp_dir().join(format!("a3s-plane-http-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let binary = dir.join("fake-acp");
        std::fs::write(&binary, super::fake_acp::SCRIPT).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let base = start_with_binary(dir.join("state"), binary);

        let (status, _) = request(&base, "POST", "/api/agents", None, Some("{}"));
        assert_eq!(status, 403);

        let (status, body) = request(
            &base,
            "POST",
            "/api/agents",
            Some(TOKEN),
            Some(r#"{"type":"a3s-code","name":"Helper","model":"openai::gpt-4o"}"#),
        );
        assert_eq!(status, 201, "{body}");
        let agent: serde_json::Value = serde_json::from_str(&body).unwrap();
        let agent_id = agent["id"].as_str().unwrap();

        let mut sse = LiveSse::open(&base);
        sse.wait_until(|text| text.contains(": connected"));

        for _ in 0..2 {
            let (status, body) = request(
                &base,
                "POST",
                "/api/agent-sessions/reuse-or-create",
                Some(TOKEN),
                Some(&format!(
                    r#"{{"agentId":"{agent_id}","workspace":{{"type":"system"}}}}"#
                )),
            );
            assert_eq!(status, 200, "{body}");
            let created: serde_json::Value = serde_json::from_str(&body).unwrap();
            let session_id = created["session"]["id"].as_str().unwrap().to_owned();

            let write_body = json_body(serde_json::json!({
                "userMessageParts": [{ "type": "text", "text": "please write-file the notes" }]
            }));
            let (status, body) = request(
                &base,
                "POST",
                &format!("/api/agent-sessions/{session_id}/turns"),
                Some(TOKEN),
                Some(&write_body),
            );
            assert_eq!(status, 200, "{body}");
            assert!(body.contains("\"mode\":\"started\""), "{body}");

            let seen = sse.read_channel("ai.stream.done");
            let prose = chunk_index(&seen, "text-delta", "delta", "先写文件。");
            let tool = chunk_index(&seen, "tool-output-available", "", "");
            let closing = chunk_index(&seen, "text-delta", "delta", "写好了。");
            assert!(prose < tool && tool < closing, "{seen:?}");
            let done = seen.last().unwrap();
            assert_eq!(done["channel"], "ai.stream.done");
            assert_eq!(done["payload"]["status"], "success");

            let (parts, _) = assistant_turn(&base, &session_id);
            assert_eq!(
                part_kinds(&parts),
                ["step-start", "reasoning", "text", "dynamic-tool", "text"]
            );
            assert_eq!(parts[1]["text"], "thinking");
            assert_eq!(parts[2]["text"], "先写文件。");
            assert_eq!(parts[3]["toolName"], "edit");
            assert_eq!(parts[3]["state"], "output-available");
            assert_eq!(
                parts[3]["files"],
                serde_json::json!(["/workspace/notes.md"])
            );
            assert_eq!(parts[4]["text"], "写好了。");

            let fail_body = json_body(serde_json::json!({
                "userMessageParts": [{ "type": "text", "text": "please fail-file the notes" }]
            }));
            let (status, body) = request(
                &base,
                "POST",
                &format!("/api/agent-sessions/{session_id}/turns"),
                Some(TOKEN),
                Some(&fail_body),
            );
            assert_eq!(status, 200, "{body}");
            let failed = sse.read_channel("ai.stream.done");
            if failed
                .iter()
                .any(|event| event["channel"] == "ai.stream.error")
            {
                panic!("turn failed: {failed:?}");
            }
            let error_at = chunk_index(&failed, "tool-output-error", "errorText", "disk full");
            assert!(
                failed
                    .iter()
                    .all(|event| event["payload"]["chunk"]["type"] != "tool-output-available"),
                "{failed:?}"
            );
            assert!(
                chunk_index(&failed, "text-delta", "delta", "写失败了。") < error_at,
                "{failed:?}"
            );
            assert_eq!(failed.last().unwrap()["payload"]["status"], "success");

            let (parts, page) = assistant_turn(&base, &session_id);
            assert_eq!(
                part_kinds(&parts),
                ["step-start", "reasoning", "text", "dynamic-tool"]
            );
            assert_eq!(parts[2]["text"], "写失败了。");
            assert_eq!(parts[3]["toolName"], "edit");
            assert_eq!(parts[3]["state"], "output-error");
            assert_eq!(parts[3]["errorText"], "disk full");
            assert!(parts[3].get("files").is_none());
            let record = readable_json(&page);
            assert!(record.contains("先写文件。"), "{record}");
            assert!(record.contains("/workspace/notes.md"), "{record}");
            assert!(record.contains("写好了。"), "{record}");
            assert!(record.contains("disk full"), "{record}");
            assert!(record.contains("output-error"), "{record}");
            save_transcript("transcript-write.txt", &page);
        }
    }

    #[test]
    fn http_turn_finishes_a_render_prompt() {
        if std::process::Command::new("node")
            .arg("--version")
            .output()
            .is_err()
        {
            eprintln!("skipping: node is not installed");
            return;
        }
        let dir =
            std::env::temp_dir().join(format!("a3s-plane-http-render-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let binary = dir.join("fake-acp");
        std::fs::write(&binary, super::fake_acp::SCRIPT).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let base = start_with_binary(dir.join("state"), binary);

        let (status, body) = request(
            &base,
            "POST",
            "/api/agents",
            Some(TOKEN),
            Some(r#"{"type":"a3s-code","name":"Helper","model":"openai::gpt-4o"}"#),
        );
        assert_eq!(status, 201, "{body}");
        let agent: serde_json::Value = serde_json::from_str(&body).unwrap();
        let agent_id = agent["id"].as_str().unwrap();

        let mut sse = LiveSse::open(&base);
        sse.wait_until(|text| text.contains(": connected"));

        let (status, body) = request(
            &base,
            "POST",
            "/api/agent-sessions/reuse-or-create",
            Some(TOKEN),
            Some(&format!(
                r#"{{"agentId":"{agent_id}","workspace":{{"type":"system"}}}}"#
            )),
        );
        assert_eq!(status, 200, "{body}");
        let created: serde_json::Value = serde_json::from_str(&body).unwrap();
        let session_id = created["session"]["id"].as_str().unwrap();

        // The scripted peer treats any prompt containing "ask" as a permission
        // request, and the word "task" contains that substring.
        let render_body = json_body(serde_json::json!({
            "userMessageParts": [{ "type": "text", "text": "please render the notes" }]
        }));
        let (status, body) = request(
            &base,
            "POST",
            &format!("/api/agent-sessions/{session_id}/turns"),
            Some(TOKEN),
            Some(&render_body),
        );
        assert_eq!(status, 200, "{body}");
        assert!(body.contains("\"mode\":\"started\""), "{body}");

        let seen = sse.read_channel("ai.stream.done");
        let prose = chunk_index(&seen, "text-delta", "delta", "先看这段。");
        let tool = chunk_index(&seen, "tool-output-available", "", "");
        let ts_fence = chunk_index(
            &seen,
            "text-delta",
            "delta",
            "\n\n```ts\nconst answer = 1\n```\n\n",
        );
        assert!(prose < tool && tool < ts_fence, "{seen:?}");
        assert_eq!(seen.last().unwrap()["channel"], "ai.stream.done");
        assert_eq!(seen.last().unwrap()["payload"]["status"], "success");

        let (parts, _) = assistant_turn(&base, session_id);
        assert_eq!(
            part_kinds(&parts),
            ["step-start", "reasoning", "text", "dynamic-tool", "text"]
        );
        assert_eq!(parts[2]["text"], "先看这段。");
        assert_eq!(parts[3]["toolName"], "edit");
        assert_eq!(parts[3]["state"], "output-available");
        assert_eq!(
            parts[3]["files"],
            serde_json::json!(["/workspace/notes.md"])
        );
        let fences = parts[4]["text"].as_str().unwrap();
        let ts_at = fences.find("```ts\nconst answer = 1\n```").unwrap();
        let diagram_at = fences.find("```mermaid\ngraph TD\nA-->B\n```").unwrap();
        let open_at = fences.find("```mermaid\ngraph TD\nC-->D").unwrap();
        assert!(ts_at < diagram_at && diagram_at < open_at, "{fences}");
        assert!(fences.ends_with("C-->D"), "{fences}");
        let record = readable_json(&serde_json::Value::Array(parts.clone()));
        assert!(record.contains("先看这段。"), "{record}");
        assert!(record.contains("/workspace/notes.md"), "{record}");
        assert!(record.contains("const answer = 1"), "{record}");
        assert!(record.contains("A-->B"), "{record}");
        assert!(record.contains("C-->D"), "{record}");
        let open = record.rfind("```mermaid").expect("open mermaid fence");
        assert!(
            !record[open + 3..].contains("```"),
            "trailing fence must stay open\n{record}"
        );
        save_transcript("transcript-render.txt", &serde_json::Value::Array(parts));
    }
}

#[cfg(test)]
#[path = "forms_http_tests.rs"]
mod forms_http_tests;
