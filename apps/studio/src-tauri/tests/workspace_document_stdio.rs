#![cfg(test)]

use std::io::{BufRead, Read, Write};
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

#[path = "../src/agent_plane/document_read.rs"]
mod document_read;

#[path = "../src/host_family.rs"]
mod host_family;
#[path = "../src/local_documents.rs"]
mod local_documents;
#[path = "../src/workspace_document.rs"]
mod workspace_document;
#[path = "../src/workspace_file.rs"]
mod workspace_file;
#[path = "../src/workspace_snapshot.rs"]
mod workspace_snapshot;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DiskAddress {
    root: String,
    path: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    id: u64,
    request: Option<workspace_document::DocumentRequest>,
    bearer: Option<String>,
    disk: Option<DiskAddress>,
    harness: Option<HarnessRequest>,
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum HarnessRequest {
    Bind { root: String, session: String },
    Read { params: Value },
    ReleaseRun { params: Value },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let disk = Arc::new(workspace_snapshot::WorkspaceSnapshots::default());
    let documents = Arc::new(workspace_document::WorkspaceDocuments::new(Arc::clone(
        &disk,
    )));
    let mut reader: Option<(document_read::DocumentReader, String)> = None;
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    loop {
        let mut line = String::new();
        let size = Read::by_ref(&mut input)
            .take(workspace_file::RAW_LIMIT + 1)
            .read_line(&mut line)?;
        if size == 0 {
            return Ok(());
        }
        if size as u64 > workspace_file::RAW_LIMIT || !line.ends_with('\n') {
            return Err("probe input exceeds limit".into());
        }
        let request: Request = serde_json::from_str(&line)?;
        let result: Result<Value, u16> = match (request.request, request.disk, request.harness) {
            (Some(command), None, None) => (match request.bearer.as_deref() {
                Some(bearer) => documents.request_scoped(bearer, command),
                None => documents.request(command),
            })
            .map_err(|error| error.status())
            .and_then(|text| serde_json::from_str(&text).map_err(|_| 500u16)),
            (None, Some(address), None) => disk
                .read(&address.root, &address.path)
                .map(|snapshot| json!({"bytes":snapshot.bytes,"revision":snapshot.revision})),
            (None, None, Some(HarnessRequest::Bind { root, session })) => {
                document_read::DocumentReader::new(documents.clone(), std::path::Path::new(&root))
                    .map(|host| {
                        reader = Some((host, session));
                        json!({"bound":true})
                    })
                    .map_err(|error| error.status())
            }
            (None, None, Some(HarnessRequest::Read { params })) => {
                reader.as_ref().ok_or(404u16).and_then(|(host, session)| {
                    host.request(session, params)
                        .map_err(|error| error.status())
                })
            }
            (None, None, Some(HarnessRequest::ReleaseRun { params })) => {
                reader.as_ref().ok_or(404u16).and_then(|(host, session)| {
                    host.release_run(session, params)
                        .map_err(|error| error.status())
                })
            }
            _ => Err(400),
        };
        let response = match result {
            Ok(value) => json!({"id":request.id,"status":200,"value":value}),
            Err(status) => json!({"id":request.id,"status":status}),
        };
        serde_json::to_writer(&mut output, &response)?;
        output.write_all(b"\n")?;
        output.flush()?;
    }
}
