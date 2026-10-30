//! Retrieval over the session's saved wiki pages.
//!
//! Search reads the current workspace publication through its original a3s-vec index and owner read lock.
//! It does not wait for an in-flight build, and it does not change the permission policy.

use std::sync::{Arc, Mutex};

use a3s_code_core::tools::{Tool, ToolCapabilities, ToolContext, ToolOutput};
use a3s_code_core::workspace::{WikiIndex, WikiSearchPhase, WikiSearchResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const TOOL_NAME: &str = "search_wiki";

pub struct SearchWikiTool {
    index: Option<Arc<WikiIndex>>,
    denies: Arc<Mutex<Vec<String>>>,
    host: Option<HostSearch>,
}

struct HostSearch {
    bridge: Arc<crate::document_read::DocumentBridge>,
    session: String,
    workspace: crate::plugin_workspace::WorkspaceBinding,
}

#[derive(Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Scope {
    Current,
    All,
}
impl Scope {
    fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::All => "all",
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchInput {
    query: String,
    scope: Option<Scope>,
    cursor: Option<String>,
}

impl SearchWikiTool {
    pub fn new(index: Option<Arc<WikiIndex>>, denies: Arc<Mutex<Vec<String>>>) -> Self {
        if let Some(index) = &index {
            publish_index(index);
        }
        Self {
            index,
            denies,
            host: None,
        }
    }

    pub(crate) fn with_host(
        mut self,
        bridge: Arc<crate::document_read::DocumentBridge>,
        session: &str,
        workspace: &std::path::Path,
    ) -> anyhow::Result<Self> {
        self.host = Some(HostSearch {
            bridge,
            session: session.into(),
            workspace: crate::plugin_workspace::WorkspaceBinding::new(workspace)?,
        });
        Ok(self)
    }
}

/// One process keeps the index `search_wiki` already searches.
/// A second index would publish a different generation.
type PublishedIndexes = Mutex<Vec<(std::path::PathBuf, std::sync::Weak<WikiIndex>)>>;

fn indexes() -> &'static PublishedIndexes {
    static INDEXES: std::sync::OnceLock<PublishedIndexes> = std::sync::OnceLock::new();
    INDEXES.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn publish_index(index: &Arc<WikiIndex>) {
    let path = std::fs::canonicalize(index.root()).unwrap_or_else(|_| index.root());
    let mut guard = indexes()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    guard.retain(|(registered, weak)| weak.upgrade().is_some() && registered != &path);
    guard.push((path, Arc::downgrade(index)));
}

fn registered(wiki: &std::path::Path) -> Option<Arc<WikiIndex>> {
    indexes()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .iter()
        .find_map(|(path, index)| (path == wiki).then(|| index.upgrade()).flatten())
}

pub(crate) fn shared_index(root: &std::path::Path) -> anyhow::Result<Arc<WikiIndex>> {
    let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let mut indexes = indexes().lock().unwrap_or_else(|p| p.into_inner());
    if let Some(index) = indexes
        .iter()
        .find_map(|(path, index)| (path == &root).then(|| index.upgrade()).flatten())
    {
        return Ok(index);
    }
    let index = Arc::new(WikiIndex::start(root.clone())?);
    indexes.retain(|(_, index)| index.upgrade().is_some());
    indexes.push((root, Arc::downgrade(&index)));
    Ok(index)
}

/// The trusted Studio broker admits cold saved publications; the model supplies no filesystem roots.
pub(crate) async fn search_admitted_body(wiki: &std::path::Path, query: &str) -> Value {
    if !matches!(a3s_okf::try_workspace_knowledge_read(wiki), Ok(Some(_))) {
        return json!({"phase":"not-ready","hits":[]});
    }
    let Ok(index) = shared_index(wiki) else {
        return json!({"phase":"not-ready","hits":[]});
    };
    // shortcut: retain at most 32 cold collections per ACP process; revisit for profiles needing a larger hot set.
    static COLD: std::sync::OnceLock<Mutex<std::collections::VecDeque<Arc<WikiIndex>>>> =
        std::sync::OnceLock::new();
    {
        let mut cold = COLD
            .get_or_init(Default::default)
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        cold.retain(|entry| entry.root() != index.root());
        cold.push_back(Arc::clone(&index));
        while cold.len() > 32 {
            cold.pop_front();
        }
    }
    if let Ok(Ok(report)) = tokio::time::timeout(
        std::time::Duration::from_millis(500),
        index.reconcile_now(None),
    )
    .await
    {
        let _ = tokio::time::timeout(
            std::time::Duration::from_millis(500),
            index.wait_published(report.source_revision),
        )
        .await;
    }
    search_published_body(wiki, query)
}

/// Search the index registered for `wiki`. Missing and in-flight generations are not-ready.
pub fn search_published(wiki: &std::path::Path, query: &str) -> WikiSearchResult {
    let Ok(Some(_guard)) = a3s_okf::try_workspace_knowledge_read(wiki) else {
        return idle_search();
    };
    let query = query.trim();
    if query.is_empty() || query.contains('\0') || query.chars().count() > 2_000 {
        return idle_search();
    }
    let wanted = std::fs::canonicalize(wiki).unwrap_or_else(|_| wiki.to_path_buf());
    let index = registered(&wanted);
    match index {
        Some(index) => bound_hits(index.search(query).unwrap_or_else(|_| idle_search())),
        None => idle_search(),
    }
}

pub fn search_published_body(wiki: &std::path::Path, query: &str) -> Value {
    let result = search_published(wiki, query);
    let phase = match result.phase {
        WikiSearchPhase::NotReady => "not-ready",
        WikiSearchPhase::Published => "published",
    };
    let hits = if result.phase == WikiSearchPhase::NotReady {
        Vec::new()
    } else {
        result
            .hits
            .iter()
            .map(|hit| json!({ "name": hit.name, "path": hit.path, "passage": hit.passage }))
            .collect()
    };
    json!({
        "phase": phase,
        "source_revision": result.source_revision,
        "hits": hits,
    })
}

fn idle_search() -> WikiSearchResult {
    WikiSearchResult {
        phase: WikiSearchPhase::NotReady,
        lexical_engine: "a3s_vec_fts_v1",
        source_revision: 0,
        catalog_revision: 0,
        hits: Vec::new(),
    }
}

#[async_trait]
impl Tool for SearchWikiTool {
    fn name(&self) -> &str {
        TOOL_NAME
    }

    fn description(&self) -> &str {
        if self.host.is_some() {
            "Decide whether saved evidence is useful for the current task; retrieval is optional, not required for every answer. Search published OKF encyclopedia pages across this Studio profile's task/project workspaces and personal materials library (default scope=all), or scope=current for this task only. The host resolves saved bindings; callers cannot supply roots. Results identify their task, project or personal library and page. Follow nextCursor with cursor for further collections. Pending collections are not a no-match: report incomplete coverage and continue with available evidence, marking unsupported facts as unconfirmed. Compilation runs separately; do not sleep, busy-wait or repeatedly poll for publication within this task. Legacy personal wiki/local, raw source packets, history and unpublished candidates are excluded. Results describe saved knowledge; check original files for current facts."
        } else {
            "Search the current workspace's published OKF encyclopedia. Personal wiki/local, other workspaces, source packets, history and unpublished candidates are excluded. Use returned page paths and passages as evidence. not-ready means no readable generation is available; no-match means this workspace has no relevant page."
        }
    }

    fn parameters(&self) -> Value {
        json!({"type":"object","properties":{"query":{"type":"string","minLength":1,"maxLength":2000,"description":"Relevance query over published workspace encyclopedia pages."},"scope":{"type":"string","enum":["current","all"],"default":if self.host.is_some(){"all"}else{"current"}},"cursor":{"type":"string","maxLength":256,"description":"Exact nextCursor returned by the previous cross-workspace search."}},"required":["query"],"additionalProperties":false})
    }

    fn capabilities(&self, _: &Value) -> ToolCapabilities {
        ToolCapabilities {
            read_only: true,
            idempotent: true,
            ..ToolCapabilities::conservative()
        }
    }

    async fn execute(&self, args: &Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        if !ctx.workspace_services.capabilities().read {
            return Ok(ToolOutput::error("denied"));
        }
        let denies = self
            .denies
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone();
        if wiki_read_denied(&denies) {
            return Ok(ToolOutput::error("denied"));
        }
        let Ok(input) = serde_json::from_value::<SearchInput>(args.clone()) else {
            return Ok(ToolOutput::error("invalid query"));
        };
        let query = input.query.trim();
        if query.is_empty() || query.contains('\0') || query.chars().count() > 2000 {
            return Ok(ToolOutput::error("invalid query"));
        }
        let scope = input.scope.unwrap_or(if self.host.is_some() {
            Scope::All
        } else {
            Scope::Current
        });
        if input.cursor.as_ref().is_some_and(|cursor| {
            cursor.is_empty() || cursor.len() > 256 || cursor.chars().any(char::is_control)
        }) || (scope == Scope::Current && input.cursor.is_some())
        {
            return Ok(ToolOutput::error("invalid Wiki cursor"));
        }
        if let Some(host) = &self.host {
            host.workspace.validate(&ctx.workspace)?;
            let value = host
                .bridge
                .search_workspace_wikis(
                    ctx.workspace_operation(a3s_code_core::capability::WorkspaceOperation::Read)?,
                    &host.session,
                    query,
                    scope.as_str(),
                    input.cursor.as_deref(),
                )
                .await?;
            host.workspace.validate(&ctx.workspace)?;
            if wiki_read_denied(
                &self
                    .denies
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner()),
            ) {
                return Ok(ToolOutput::error("denied"));
            }
            validate_host_reply(&value, &host.session, scope, query)?;
            return Ok(ToolOutput::success(value.to_string()).with_metadata(json!({"phase":value["phase"],"scope":scope.as_str(),"nextCursor":value["nextCursor"],"pending":value["pending"],"lexical_engine":"a3s_vec_fts_v1","wiki_result":value})));
        }
        if scope == Scope::All {
            return Ok(ToolOutput::error(
                "Cross-workspace Wiki search requires an admitted Studio host",
            ));
        }
        let Some(index) = &self.index else {
            return Ok(render(idle_search()));
        };
        let root = index.root();
        let workspace = ctx.workspace.canonicalize().ok();
        if root
            .parent()
            .and_then(std::path::Path::parent)
            .and_then(std::path::Path::parent)
            != workspace.as_deref()
        {
            return Ok(ToolOutput::error(
                "workspace encyclopedia is not bound to this workspace",
            ));
        }
        let index = Arc::clone(index);
        let record_root = root.clone();
        let query = query.to_owned();
        let result = tokio::task::spawn_blocking(move || {
            let Ok(Some(_guard)) = a3s_okf::try_workspace_knowledge_read(&root) else {
                return Ok(idle_search());
            };
            index.search(query)
        })
        .await
        .map_err(|error| anyhow::anyhow!("wiki search task failed: {error}"))?;
        match result {
            Ok(result) => {
                let result = bound_hits(result);
                note_retrieved_pages(&record_root, &result);
                Ok(render(result))
            }
            Err(error) => Ok(ToolOutput::error(error.to_string())),
        }
    }
}

fn validate_host_reply(
    value: &Value,
    session: &str,
    scope: Scope,
    query: &str,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        value["sessionId"] == session
            && value["scope"] == scope.as_str()
            && value["query"] == query,
        "workspace Wiki response binding mismatch"
    );
    anyhow::ensure!(
        matches!(value["phase"].as_str(), Some("published" | "not-ready")),
        "invalid Wiki response phase"
    );
    for key in ["searched", "pending"] {
        anyhow::ensure!(
            value[key].as_u64().is_some_and(|n| n <= 32),
            "invalid Wiki coverage"
        );
    }
    anyhow::ensure!(
        value["complete"].is_boolean(),
        "missing Wiki coverage state"
    );
    anyhow::ensure!(
        value["nextCursor"].is_null()
            || value["nextCursor"].as_str().is_some_and(|s| !s.is_empty()
                && s.len() <= 256
                && !s.chars().any(char::is_control)),
        "invalid Wiki continuation"
    );
    let hits = value["hits"]
        .as_array()
        .filter(|hits| hits.len() <= 12)
        .ok_or_else(|| anyhow::anyhow!("invalid Wiki hits"))?;
    anyhow::ensure!(
        value["phase"] != "not-ready" || hits.is_empty(),
        "unpublished Wiki hits refused"
    );
    for hit in hits {
        for key in ["sourceRoot", "outputRoot", "path", "name", "passage"] {
            anyhow::ensure!(
                hit[key]
                    .as_str()
                    .is_some_and(|s| !s.is_empty() && s.len() <= 8192 && !s.contains('\0')),
                "invalid Wiki attribution or evidence"
            );
        }
        if hit["ownerKind"] == "library" {
            anyhow::ensure!(
                hit["libraryId"]
                    .as_str()
                    .is_some_and(|id| !id.is_empty() && id.len() <= 256 && !id.contains('\0'))
                    && hit["ownerId"] == hit["libraryId"],
                "invalid personal library attribution"
            );
        } else {
            anyhow::ensure!(
                hit["ownerKind"].is_null()
                    || matches!(hit["ownerKind"].as_str(), Some("task" | "project")),
                "unknown Wiki owner kind"
            );
            for key in ["taskId", "taskName", "workspaceId", "workspaceName"] {
                anyhow::ensure!(
                    hit[key].as_str().is_some_and(|value| !value.is_empty()
                        && value.len() <= 8192
                        && !value.contains('\0')),
                    "invalid workspace Wiki attribution"
                );
            }
        }
        let path = hit["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("invalid Wiki path"))?;
        anyhow::ensure!(
            path.starts_with("concepts/")
                && path.ends_with(".md")
                && !path.contains(['\\', ':'])
                && path
                    .split('/')
                    .all(|part| !part.is_empty() && part != ".." && part != "."),
            "non-page Wiki result refused"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod host_tests;

fn wiki_read_denied(denies: &[String]) -> bool {
    denies.iter().any(|rule| {
        let rule = rule.trim();
        rule == "open_local_asset" || rule == TOOL_NAME
    })
}

fn bound_hits(mut result: WikiSearchResult) -> WikiSearchResult {
    let mut chars = 0usize;
    let mut hits = Vec::new();
    for hit in result.hits {
        if hits.len() == 12 {
            break;
        }
        let next = hit.passage.chars().count();
        if chars + next > 24_000 {
            break;
        }
        chars += next;
        hits.push(hit);
    }
    result.hits = hits;
    result
}

/// The documents list reads `{workspace}/.a3s-agent-retrievals`, one canonical path per retrieval.
fn note_retrieved_pages(wiki_root: &std::path::Path, result: &WikiSearchResult) {
    if !matches!(result.phase, WikiSearchPhase::Published) {
        return;
    }
    let Some(workspace) = wiki_root.ancestors().nth(3) else {
        return;
    };
    let mut seen = std::collections::HashSet::new();
    let mut lines = String::new();
    for hit in &result.hits {
        let relative = hit.path.trim_matches('/');
        if relative.is_empty() || relative == "." || relative.contains('\0') {
            continue;
        }
        let Ok(canon) = std::fs::canonicalize(wiki_root.join(relative)) else {
            continue;
        };
        if !canon.starts_with(workspace) || !canon.is_file() || !seen.insert(canon.clone()) {
            continue;
        }
        lines.push_str(&canon.to_string_lossy());
        lines.push('\n');
    }
    if lines.is_empty() {
        return;
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(workspace.join(".a3s-agent-retrievals"))
    {
        use std::io::Write;
        let _ = file.write_all(lines.as_bytes());
    }
}

fn render(result: WikiSearchResult) -> ToolOutput {
    let names: Vec<&str> = result.hits.iter().map(|hit| hit.name.as_str()).collect();
    let phase = match result.phase {
        WikiSearchPhase::NotReady => "not-ready",
        WikiSearchPhase::Published => "published",
    };
    let structured =
        (result.phase == WikiSearchPhase::Published && !result.hits.is_empty()).then(|| {
            json!({"hits": result.hits.iter().map(|hit|
        json!({"name":hit.name,"path":hit.path,"passage":hit.passage})
    ).collect::<Vec<_>>()})
        });
    let content = match (result.phase, structured.as_ref()) {
        (WikiSearchPhase::NotReady, _) => "not-ready".to_owned(),
        (WikiSearchPhase::Published, None) => "no-match".to_owned(),
        (WikiSearchPhase::Published, Some(structured)) => structured.to_string(),
    };
    ToolOutput::success(content).with_metadata(json!({
        "phase": phase,
        "lexical_engine": result.lexical_engine,
        "source_revision": result.source_revision,
        "catalog_revision": result.catalog_revision,
        "names": names,
        "wiki_result": structured,
    }))
}
