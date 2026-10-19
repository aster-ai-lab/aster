//! Member-local paper jobs. Pages with a text layer are read here.
//! shortcut: a textless page stays in the page count and is omitted from markdown. The job fails only when every trimmed page is empty. Bitmap pages are not rasterized and are not posted to the OS gateway OCR route.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use chrono::Utc;
use lopdf::Document;
use serde_json::{Value, json};

use super::{Response, UPLOAD_LIMIT, hex_digest, json_response, public_route};

const MAX_PAGES: usize = 2000;

pub(crate) fn handle(
    root: &Path,
    member: &str,
    method: &str,
    parts: &[&str],
    query: &str,
    body: &[u8],
) -> Response {
    match route(root, member, method, parts, query, body) {
        Ok(response) => response,
        Err(error) if error.kind() == io::ErrorKind::InvalidInput => {
            json_response(400, json!({"error":"invalid_request"}))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            json_response(404, json!({"error":"not_found"}))
        }
        Err(_) => json_response(503, json!({"error":"processor_unavailable"})),
    }
}

pub(crate) fn install_fixture(
    root: &Path,
    member: &str,
    job: &Value,
    manifest: &Value,
    files: &[(&str, &[u8])],
) -> io::Result<()> {
    let job_id = job["job_id"].as_str().ok_or_else(invalid)?;
    if !public_route("GET", &["jobs", job_id]) {
        return Err(invalid());
    }
    let dir = jobs(root, member).join(job_id);
    fs::create_dir_all(&dir)?;
    write_atomic(&dir.join("job.json"), job.to_string().as_bytes())?;
    write_atomic(&dir.join("manifest.json"), manifest.to_string().as_bytes())?;
    let artifacts = dir.join("artifacts");
    if artifacts.exists() {
        fs::remove_dir_all(&artifacts)?;
    }
    fs::create_dir_all(&artifacts)?;
    for (key, bytes) in files {
        if !public_route("GET", &["jobs", job_id, "artifacts", key]) {
            return Err(invalid());
        }
        write_atomic(&artifacts.join(key), bytes)?;
    }
    Ok(())
}

fn route(
    root: &Path,
    member: &str,
    method: &str,
    parts: &[&str],
    query: &str,
    body: &[u8],
) -> io::Result<Response> {
    let upload = method == "POST" && parts == ["uploads"];
    if uuid::Uuid::parse_str(member).is_err()
        || (!upload && !public_route(method, parts))
        || query.len() > 4096
        || query.contains(['\r', '\n', '#'])
    {
        return Err(invalid());
    }
    let _lock = lock(root, member)?;
    if upload {
        return store_upload(root, member, query, body);
    }
    match (method, parts) {
        ("GET", ["health"]) => Ok(envelope(json!({"status":"ok"}))),
        ("GET", ["documents"]) => list_documents(root, member),
        ("GET", ["documents", id]) => {
            Ok(envelope(public_document(&read_document(root, member, id)?)))
        }
        ("PATCH", ["documents", id]) => patch_document(root, member, id, body),
        ("GET", ["documents", id, "source.pdf"]) => read_pdf(root, member, id),
        ("GET", ["documents", id, "jobs"]) => list_jobs(root, member, id),
        ("POST", ["documents", id, "ocr"]) => start_ocr(root, member, id, body),
        ("POST", ["documents", _, "translate"] | ["jobs"]) => Err(invalid()),
        ("GET", ["jobs", id]) => Ok(envelope(read_job(root, member, id)?)),
        ("GET", ["jobs", id, "artifacts-manifest"]) => {
            Ok(envelope(read_manifest(root, member, id)?))
        }
        ("GET", ["jobs", id, "markdown"]) => read_markdown(root, member, id, query),
        ("GET", ["jobs", id, "artifacts", key]) => read_artifact(root, member, id, key),
        ("POST", ["jobs", id, "cancel"]) => set_status(root, member, id, "canceled"),
        ("POST", ["jobs", id, "resume"]) => resume(root, member, id),
        ("POST", ["ai", "ask"]) => ask(root, member, body),
        ("GET", ["search"]) => search(root, member, query),
        _ => Err(invalid()),
    }
}

fn store_upload(root: &Path, member: &str, query: &str, body: &[u8]) -> io::Result<Response> {
    let name = query_value(query, "name")
        .filter(|name| {
            name.len() <= 240
                && !name.contains(['\r', '\n', '/', '\\', '"'])
                && name.to_lowercase().ends_with(".pdf")
        })
        .ok_or_else(invalid)?;
    let pages = pdf_pages(body)?;
    let id = hex_digest(body);
    let now = Utc::now().to_rfc3339();
    let document = json!({
        "document_id": id,
        "title": name.trim_end_matches(".pdf").trim_end_matches(".PDF"),
        "source_filename": name,
        "page_count": pages.len(),
        "bytes": body.len(),
        "active_job_id": Value::Null,
        "reading_status": "unread",
        "added_at": now,
        "updated_at": now,
        "tags": [],
        "pages": pages,
    });
    let dir = docs(root, member);
    fs::create_dir_all(&dir)?;
    write_atomic(
        &dir.join(format!("{id}.json")),
        document.to_string().as_bytes(),
    )?;
    write_atomic(&dir.join(&id), body)?;
    Ok(envelope(
        json!({"document_id": id, "page_count": pages.len()}),
    ))
}

fn list_documents(root: &Path, member: &str) -> io::Result<Response> {
    let dir = docs(root, member);
    let mut documents = Vec::new();
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            documents.push(public_document(&read_json(&path)?));
        }
    }
    documents.sort_by(|left, right| {
        left["document_id"]
            .as_str()
            .cmp(&right["document_id"].as_str())
    });
    let total = documents.len();
    Ok(envelope(json!({"documents": documents, "total": total})))
}

fn patch_document(root: &Path, member: &str, id: &str, body: &[u8]) -> io::Result<Response> {
    let mut document = read_document(root, member, id)?;
    let value: Value = serde_json::from_slice(body).map_err(|_| invalid())?;
    let status = value["reading_status"].as_str().ok_or_else(invalid)?;
    if !matches!(status, "unread" | "reading" | "done")
        || value.as_object().is_none_or(|o| o.len() != 1)
    {
        return Err(invalid());
    }
    document["reading_status"] = json!(status);
    document["updated_at"] = json!(Utc::now().to_rfc3339());
    write_document(root, member, &document)?;
    Ok(envelope(public_document(&document)))
}

fn read_pdf(root: &Path, member: &str, id: &str) -> io::Result<Response> {
    let document = read_document(root, member, id)?;
    let bytes = fs::read(docs(root, member).join(id))?;
    if hex_digest(&bytes) != document["document_id"].as_str().unwrap_or("") {
        return Err(invalid());
    }
    Ok((200, "OK", "application/pdf", bytes))
}

fn list_jobs(root: &Path, member: &str, id: &str) -> io::Result<Response> {
    let _ = read_document(root, member, id)?;
    let mut items = Vec::new();
    let dir = jobs(root, member);
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let path = entry?.path().join("job.json");
            if !path.is_file() {
                continue;
            }
            let job = read_json(&path)?;
            if job["document_id"] == id {
                items.push(public_job(&job));
            }
        }
    }
    items.sort_by(|left, right| {
        left["created_at"]
            .as_str()
            .cmp(&right["created_at"].as_str())
    });
    Ok(envelope(json!({"items": items})))
}

fn start_ocr(root: &Path, member: &str, id: &str, body: &[u8]) -> io::Result<Response> {
    let mut document = read_document(root, member, id)?;
    let request: Value = serde_json::from_slice(body).map_err(|_| invalid())?;
    if request["workflow"] != "ocr"
        || request["ocr"]["provider"] != "a3s_gateway"
        || request["ocr"].as_object().is_none_or(|ocr| ocr.len() != 1)
    {
        return Err(invalid());
    }
    let pages = document["pages"].as_array().cloned().unwrap_or_default();
    let text: Vec<_> = pages
        .iter()
        .filter_map(|page| page.as_str())
        .filter(|page| !page.trim().is_empty())
        .map(str::trim)
        .collect();
    let now = Utc::now().to_rfc3339();
    let job_id = format!("job-{}", uuid::Uuid::now_v7().simple());
    let failed = text.is_empty();
    let markdown = text.join("\n\n");
    let markdown_bytes = markdown.as_bytes();
    let source = fs::read(docs(root, member).join(id))?;
    let source_name = document["source_filename"].as_str().unwrap_or("source.pdf");
    let mut artifacts = vec![artifact(
        "source_pdf",
        source_name,
        "application/pdf",
        1,
        &source,
    )];
    if !failed {
        artifacts.push(artifact(
            "markdown_raw",
            "paper.md",
            "text/markdown",
            1,
            markdown_bytes,
        ));
    }
    let job = json!({
        "job_id": job_id,
        "document_id": id,
        "workflow": "ocr",
        "status": if failed { "failed" } else { "succeeded" },
        "attempt": 1,
        "display_name": document["title"],
        "stages": {
            "ocr": {"state": if failed { "failed" } else { "completed" }, "progress": {"current": pages.len(), "total": pages.len(), "unit": "page"}},
            "translation": {"state": "skipped", "progress": {}},
            "render": {"state": "skipped", "progress": {}}
        },
        "output_pdf_ready": false,
        "markdown_ready": !failed,
        "bundle_ready": false,
        "created_at": now,
        "updated_at": now,
        "request_payload": {
            "workflow": "ocr",
            "source": {"upload_id": id},
            "ocr": {"provider": "a3s_gateway"}
        }
    });
    let manifest = json!({"job_id": job_id, "items": artifacts});
    let mut files: Vec<(&str, &[u8])> = vec![("source_pdf", source.as_slice())];
    if !failed {
        files.push(("markdown_raw", markdown_bytes));
    }
    install_fixture(root, member, &job, &manifest, &files)?;
    document["active_job_id"] = json!(job_id);
    document["updated_at"] = json!(now);
    write_document(root, member, &document)?;
    Ok(envelope(json!({"job_id": job_id, "status": job["status"]})))
}

fn set_status(root: &Path, member: &str, id: &str, status: &str) -> io::Result<Response> {
    let mut job = read_job(root, member, id)?;
    if !matches!(
        job["status"].as_str(),
        Some("queued" | "running" | "canceled")
    ) {
        return Err(invalid());
    }
    job["status"] = json!(status);
    job["updated_at"] = json!(Utc::now().to_rfc3339());
    write_json(&job_dir(root, member, id)?.join("job.json"), &job)?;
    Ok(envelope(json!({"accepted": true})))
}

fn resume(root: &Path, member: &str, id: &str) -> io::Result<Response> {
    let job = read_job(root, member, id)?;
    if job["status"] != "canceled"
        || job["request_payload"]["ocr"]["provider"] != "a3s_gateway"
        || job["request_payload"]["workflow"] != "ocr"
    {
        return Err(invalid());
    }
    let document_id = job["document_id"].as_str().ok_or_else(invalid)?;
    start_ocr(
        root,
        member,
        document_id,
        json!({"workflow":"ocr","ocr":{"provider":"a3s_gateway"}})
            .to_string()
            .as_bytes(),
    )
}

fn read_markdown(root: &Path, member: &str, id: &str, query: &str) -> io::Result<Response> {
    let job = read_job(root, member, id)?;
    if job["markdown_ready"] != true {
        return Err(io::Error::from(io::ErrorKind::NotFound));
    }
    let bytes = read_artifact_bytes(root, member, id, "markdown_raw")?;
    if query_value(query, "raw").as_deref() == Some("true") {
        return Ok((200, "OK", "text/plain; charset=utf-8", bytes));
    }
    let text = String::from_utf8(bytes).map_err(|_| invalid())?;
    Ok(envelope(json!({"markdown": text})))
}

fn ask(root: &Path, member: &str, body: &[u8]) -> io::Result<Response> {
    let mut value: Value = serde_json::from_slice(body).map_err(|_| invalid())?;
    let object = value.as_object_mut().ok_or_else(invalid)?;
    object.remove("confirm_document_operation");
    let document_id = object
        .get("document_id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .ok_or_else(invalid)?
        .to_owned();
    let question = object
        .get("question")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .ok_or_else(invalid)?
        .to_owned();
    let document = read_document(root, member, &document_id)?;
    let pages = document["pages"].as_array().cloned().unwrap_or_default();
    let mut citations = Vec::new();
    for (index, page) in pages.iter().enumerate() {
        let snippet = page.as_str().unwrap_or("").trim();
        if snippet.is_empty() || !snippet.contains(&question) && !question.contains(snippet) {
            continue;
        }
        citations.push(json!({
            "ref": citations.len() + 1,
            "document_id": document_id,
            "job_id": document["active_job_id"].as_str().unwrap_or(""),
            "page_idx": index,
            "block_id": format!("p{}", index + 1),
            "snippet": snippet
        }));
    }
    let answer = if citations.is_empty() {
        "正文中没有这句话。".to_owned()
    } else {
        citations
            .iter()
            .filter_map(|item| item["snippet"].as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    };
    Ok(envelope(json!({
        "answer": answer,
        "citations": citations,
        "persisted": false
    })))
}

fn search(root: &Path, member: &str, query: &str) -> io::Result<Response> {
    let q = query_value(query, "q").ok_or_else(invalid)?;
    let document_id = query_value(query, "document_id").ok_or_else(invalid)?;
    let document = read_document(root, member, &document_id)?;
    let mut hits = Vec::new();
    for (index, page) in document["pages"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let snippet = page.as_str().unwrap_or("");
        if !snippet.contains(&q) {
            continue;
        }
        hits.push(json!({
            "document_id": document_id,
            "job_id": document["active_job_id"].as_str().unwrap_or(""),
            "page_idx": index,
            "block_id": format!("p{}", index + 1),
            "source_snippet": snippet,
            "translated_snippet": ""
        }));
        if hits.len() == 50 {
            break;
        }
    }
    Ok(envelope(json!({"hits": hits})))
}

fn pdf_pages(bytes: &[u8]) -> io::Result<Vec<String>> {
    if bytes.is_empty() || bytes.len() > UPLOAD_LIMIT || !bytes.starts_with(b"%PDF-") {
        return Err(invalid());
    }
    let document = Document::load_mem(bytes).map_err(|_| invalid())?;
    if document.is_encrypted() {
        return Err(invalid());
    }
    let mut numbers: Vec<u32> = document.get_pages().keys().copied().collect();
    numbers.sort_unstable();
    if numbers.is_empty() || numbers.len() > MAX_PAGES {
        return Err(invalid());
    }
    Ok(numbers
        .into_iter()
        .map(|number| {
            document
                .extract_text(&[number])
                .unwrap_or_default()
                .trim()
                .to_owned()
        })
        .collect())
}

fn artifact(key: &str, name: &str, content_type: &str, attempt: u64, bytes: &[u8]) -> Value {
    json!({
        "artifact_key": key,
        "artifact_kind": "file",
        "file_name": name,
        "content_type": content_type,
        "checksum": format!("sha256:{}", hex_digest(bytes)),
        "ready": true,
        "attempt": attempt,
        "size_bytes": bytes.len()
    })
}

fn public_document(document: &Value) -> Value {
    let mut public = document.clone();
    if let Some(object) = public.as_object_mut() {
        object.remove("pages");
    }
    public
}

fn public_job(job: &Value) -> Value {
    job.clone()
}

fn read_document(root: &Path, member: &str, id: &str) -> io::Result<Value> {
    if !public_route("GET", &["documents", id]) {
        return Err(invalid());
    }
    read_json(&docs(root, member).join(format!("{id}.json")))
}

fn write_document(root: &Path, member: &str, document: &Value) -> io::Result<()> {
    let id = document["document_id"].as_str().ok_or_else(invalid)?;
    write_json(&docs(root, member).join(format!("{id}.json")), document)
}

fn read_job(root: &Path, member: &str, id: &str) -> io::Result<Value> {
    read_json(&job_dir(root, member, id)?.join("job.json"))
}

fn read_manifest(root: &Path, member: &str, id: &str) -> io::Result<Value> {
    read_json(&job_dir(root, member, id)?.join("manifest.json"))
}

fn read_artifact(root: &Path, member: &str, id: &str, key: &str) -> io::Result<Response> {
    Ok((
        200,
        "OK",
        "application/octet-stream",
        read_artifact_bytes(root, member, id, key)?,
    ))
}

fn read_artifact_bytes(root: &Path, member: &str, id: &str, key: &str) -> io::Result<Vec<u8>> {
    let path = job_dir(root, member, id)?.join("artifacts").join(key);
    if !path.is_file() {
        return Err(io::Error::from(io::ErrorKind::NotFound));
    }
    Ok(fs::read(path)?)
}

fn job_dir(root: &Path, member: &str, id: &str) -> io::Result<PathBuf> {
    if !public_route("GET", &["jobs", id]) {
        return Err(invalid());
    }
    let dir = jobs(root, member).join(id);
    if !dir.join("job.json").is_file() {
        return Err(io::Error::from(io::ErrorKind::NotFound));
    }
    Ok(dir)
}

fn docs(root: &Path, member: &str) -> PathBuf {
    root.join("paperloom-native").join(member).join("docs")
}

fn jobs(root: &Path, member: &str) -> PathBuf {
    root.join("paperloom-native").join(member).join("jobs")
}

fn lock(root: &Path, member: &str) -> io::Result<File> {
    let dir = root.join("paperloom-native");
    fs::create_dir_all(&dir)?;
    let file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join(format!("{member}.lock")))?;
    file.lock()?;
    Ok(file)
}

fn read_json(path: &Path) -> io::Result<Value> {
    if !path.is_file() {
        return Err(io::Error::from(io::ErrorKind::NotFound));
    }
    let bytes = fs::read(path)?;
    serde_json::from_slice(&bytes).map_err(|_| invalid())
}

fn write_json(path: &Path, value: &Value) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    write_atomic(path, value.to_string().as_bytes())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path.parent().ok_or_else(invalid)?;
    fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(())
}

fn query_value(query: &str, key: &str) -> Option<String> {
    url::form_urlencoded::parse(query.as_bytes())
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.into_owned())
        .filter(|value| !value.is_empty())
}

fn envelope(data: Value) -> Response {
    json_response(200, json!({"code": 0, "data": data}))
}

fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, "invalid paper request")
}
