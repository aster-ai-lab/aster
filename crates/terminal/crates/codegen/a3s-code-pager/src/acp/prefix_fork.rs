//! Fork the current, completed transcript using Core's optimistic log boundary.
use std::path::Path;

use a3s_acp_lib::{AcpAgentTx, acp_send};
use agent_client_protocol as acp;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::json;

const PAGE_SCHEMA: &str = "a3s.acp.session-transcript-page.v1";
const FORK_SCHEMA: &str = "a3s.acp.session-prefix-fork.v1";

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
struct Cut {
    seq: u64,
    digest: String,
}

#[derive(Deserialize, Serialize)]
struct Cursor {
    session_id: String,
    after_entry_id: String,
    source_cut: Cut,
}

#[derive(Deserialize)]
struct Entry {
    entry_id: String,
    kind: String,
    fact_seq: u64,
}

#[derive(Deserialize)]
struct Page {
    session_id: String,
    entries: Vec<Entry>,
    next_cursor: Option<Cursor>,
    source_cut: Cut,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageResponse {
    schema: String,
    session_id: String,
    operation_id: String,
    page: Page,
}

pub(crate) async fn fork(
    tx: &AcpAgentTx,
    parent: &str,
    cwd: &Path,
    target: Option<&str>,
    source_load: Option<acp::LoadSessionRequest>,
) -> Result<String> {
    let target = target
        .map(str::to_owned)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    ensure!(
        !target.trim().is_empty() && target != parent,
        "fork requires a different child session id"
    );
    if let Some(request) = source_load {
        ensure!(
            request.session_id.0.as_ref() == parent && request.cwd == cwd,
            "fork source load identity mismatch"
        );
        let _: acp::LoadSessionResponse = acp_send(request, tx).await?;
    }
    let operation = uuid::Uuid::new_v4().to_string();
    let mut cursor = None;
    let mut cut = None;
    let mut last: Option<Entry> = None;
    loop {
        let response = request(
            tx,
            "a3s/session/transcript/page/v1",
            json!({
                "schema": PAGE_SCHEMA, "sessionId": parent, "operationId": operation,
                "cwd": cwd, "cursor": cursor, "limit": 256,
            }),
        )
        .await?;
        let response: PageResponse = serde_json::from_value(response)?;
        ensure!(
            response.schema == PAGE_SCHEMA
                && response.session_id == parent
                && response.operation_id == operation
                && response.page.session_id == parent,
            "transcript response identity mismatch"
        );
        let page = response.page;
        ensure!(
            cut.as_ref().is_none_or(|cut| *cut == page.source_cut),
            "transcript changed during fork"
        );
        cut = Some(page.source_cut.clone());
        ensure!(
            page.entries.len() <= 256,
            "transcript page exceeds requested limit"
        );
        let empty = page.entries.is_empty();
        for entry in page.entries {
            ensure!(
                !entry.entry_id.is_empty()
                    && last
                        .as_ref()
                        .is_none_or(|last| entry.fact_seq > last.fact_seq),
                "transcript page did not advance"
            );
            last = Some(entry);
        }
        match page.next_cursor {
            Some(next) => {
                ensure!(
                    !empty
                        && next.session_id == parent
                        && next.source_cut == page.source_cut
                        && last
                            .as_ref()
                            .is_some_and(|last| next.after_entry_id == last.entry_id),
                    "transcript cursor identity mismatch"
                );
                cursor = Some(next);
            }
            None => break,
        }
    }
    let boundary = last
        .filter(|entry| entry.kind == "turn_complete")
        .ok_or_else(|| {
            anyhow::anyhow!("Cannot fork: session has no completed transcript boundary")
        })?;
    let response = request(
        tx,
        "a3s/session/fork/prefix/v1",
        json!({
            "schema": FORK_SCHEMA, "sessionId": parent, "operationId": operation,
            "request": {"boundary_entry_id": boundary.entry_id, "expected_cut": cut,
                "target_session_id": target, "expected_workspace": cwd},
        }),
    )
    .await?;
    ensure!(
        response["schema"] == FORK_SCHEMA
            && response["operationId"] == operation
            && response["childId"] == target
            && response["sourceCut"] == serde_json::to_value(cut)?
            && response["lineage"]["sourceSessionId"] == parent
            && response["lineage"]["childSessionId"] == target
            && response["lineage"]["boundaryEntryId"] == boundary.entry_id,
        "fork response identity mismatch"
    );
    Ok(target)
}

async fn request(
    tx: &AcpAgentTx,
    method: &'static str,
    params: serde_json::Value,
) -> Result<serde_json::Value> {
    let response = acp_send(
        acp::ExtRequest::new(method, serde_json::value::to_raw_value(&params)?.into()),
        tx,
    )
    .await?;
    Ok(serde_json::from_str(response.0.get())?)
}

#[cfg(test)]
#[path = "prefix_fork_tests.rs"]
mod tests;
