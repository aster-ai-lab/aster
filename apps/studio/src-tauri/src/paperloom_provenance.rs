//! Durable, member-bound receipts for verified workspace copies, not processor execution attestations.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use base64::Engine;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::agent_plane::{
    ArtifactOrigin, ArtifactOriginKind, ArtifactProcessorStatus, ArtifactProof, ArtifactStore,
};
use crate::workspace_snapshot::WorkspaceSnapshots;

use super::{Response, hex_digest, json_response, processor_revision, proxy};

type Result<T> = std::result::Result<T, (u16, &'static str)>;
const MAX_RECORD: u64 = 256 * 1024;
const MAX_HISTORY: usize = 128;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Registration {
    root: String,
    source_path: String,
    source_revision: String,
    source_digest: String,
    saved_path: String,
    job_id: String,
    artifact_key: String,
    attempt: u32,
    checksum: String,
}

pub(crate) fn route(
    state: &Path,
    member: &str,
    package_digest: &str,
    method: &str,
    parts: &[&str],
    query: &str,
    body: &[u8],
) -> Option<Response> {
    if parts.first() != Some(&"provenance") {
        return None;
    }
    let result = (|| {
        if uuid::Uuid::parse_str(member).is_err() {
            return Err((404, "not_found"));
        }
        match (method, parts) {
            ("POST", ["provenance"]) if query.is_empty() => {
                let request: Registration =
                    serde_json::from_slice(body).map_err(|_| (400, "invalid_request"))?;
                register(state, member, package_digest, request)
            }
            ("GET", ["provenance"]) => lookup_source(state, member, query),
            ("GET", ["provenance", id] | ["provenance", id, "history" | "snapshot"]) => {
                if uuid::Uuid::parse_str(id).is_err() {
                    return Err((404, "not_found"));
                }
                let pairs: Vec<_> = url::form_urlencoded::parse(query.as_bytes()).collect();
                let exact = |key| -> Result<String> {
                    let rows: Vec<_> = pairs.iter().filter(|(name, _)| name == key).collect();
                    if rows.len() != 1 {
                        return Err((400, "invalid_request"));
                    }
                    Ok(rows[0].1.to_string())
                };
                if pairs.len() != 2 {
                    return Err((400, "invalid_request"));
                }
                let root = canonical_root(&exact("root")?)?;
                let path = exact("path")?;
                validate_saved_path(&root, &path)?;
                let dir = ledger(state, member, &root, &path)?;
                let proof = read_proof(&dir.join(format!("{id}.json")))?;
                check_scope(&proof, member, &root, &path)?;
                if parts.last() == Some(&"history") {
                    let history = history(&dir)?;
                    let store = ArtifactStore::new(Some(dir));
                    for item in &history {
                        check_scope(item, member, &root, &path)?;
                        store
                            .verify_receipt(&item.session_id, &item.artifacts[0])
                            .map_err(|_| (409, "provenance_invalid"))?;
                    }
                    return Ok(json!(history));
                }
                let artifact = proof.artifacts.first().ok_or((404, "not_found"))?;
                let bytes = ArtifactStore::new(Some(dir))
                    .read(&proof.session_id, artifact)
                    .map_err(|_| (409, "artifact_changed"))?;
                if parts.last() == Some(&"snapshot") {
                    Ok(
                        json!({ "id":artifact.id, "name":artifact.name, "mediaType":artifact.media_type,
                        "contentDigest":artifact.content_digest,
                        "data":base64::engine::general_purpose::STANDARD.encode(bytes) }),
                    )
                } else {
                    Ok(json!(proof))
                }
            }
            _ => Err((404, "not_found")),
        }
    })();
    Some(match result {
        Ok(value) => json_response(200, json!({"code":0,"data":value})),
        Err((status, error)) => json_response(status, json!({"error":error})),
    })
}

fn register(
    state: &Path,
    member: &str,
    package_digest: &str,
    mut request: Registration,
) -> Result<Value> {
    if hex64(&request.checksum) {
        request.checksum = format!("sha256:{}", request.checksum);
    }
    if request.attempt == 0
        || !hex64(&request.source_digest)
        || !sha256(&request.checksum)
        || !hex64(&request.source_revision)
        || !sha256(package_digest)
        || !super::public_route(
            "GET",
            &["jobs", &request.job_id, "artifacts", &request.artifact_key],
        )
    {
        return Err((400, "invalid_request"));
    }
    let root = canonical_root(&request.root)?;
    let source = canonical_file(&root, &request.source_path)?;
    let saved = canonical_file(&root, &request.saved_path)?;
    if source == saved {
        return Err((409, "source_overwrite"));
    }
    verify_source(
        &root,
        &source,
        &request.source_revision,
        &request.source_digest,
    )?;
    let revision = processor_revision();
    let job = service_json(state, member, &["jobs", &request.job_id])?;
    if job["job_id"] != request.job_id || !job["request_payload"]["source"].is_object() {
        return Err((409, "artifact_source_mismatch"));
    }
    let job_source = job["request_payload"]["source"].clone();
    let manifest = service_json(
        state,
        member,
        &["jobs", &request.job_id, "artifacts-manifest"],
    )?;
    if manifest["job_id"] != request.job_id {
        return Err((409, "artifact_source_mismatch"));
    }
    let source_item = source_manifest_item(&manifest, &request)?;
    // Upload IDs are opaque. Bind this job to the workspace through actual source bytes.
    let service_source = service_bytes(
        state,
        member,
        &["jobs", &request.job_id, "artifacts", "source_pdf"],
    )?;
    if service_source.len() > super::UPLOAD_LIMIT
        || !service_source.starts_with(b"%PDF-")
        || digest(&service_source) != format!("sha256:{}", request.source_digest)
        || source_item["size_bytes"].as_u64() != Some(service_source.len() as u64)
    {
        return Err((409, "artifact_source_mismatch"));
    }
    let matching: Vec<_> = manifest["items"]
        .as_array()
        .ok_or((502, "invalid_manifest"))?
        .iter()
        .filter(|item| item["artifact_key"] == request.artifact_key)
        .collect();
    if matching.len() != 1 {
        return Err((409, "artifact_source_mismatch"));
    }
    let item = matching[0];
    let name = Path::new(&saved)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or((400, "invalid_request"))?;
    if item["ready"] != true
        || item["attempt"].as_u64() != Some(u64::from(request.attempt))
        || normalized_checksum(&item["checksum"]).as_deref() != Some(request.checksum.as_str())
        || item["file_name"] != name
    {
        return Err((409, "artifact_changed"));
    }
    let bytes = service_bytes(
        state,
        member,
        &["jobs", &request.job_id, "artifacts", &request.artifact_key],
    )?;
    if bytes.len() > super::UPLOAD_LIMIT {
        return Err((413, "artifact_too_large"));
    }
    if bytes.is_empty() || digest(&bytes) != request.checksum {
        return Err((409, "artifact_changed"));
    }
    let after = service_json(
        state,
        member,
        &["jobs", &request.job_id, "artifacts-manifest"],
    )?;
    let after_items: Vec<_> = after["items"]
        .as_array()
        .ok_or((502, "invalid_manifest"))?
        .iter()
        .filter(|item| item["artifact_key"] == request.artifact_key)
        .collect();
    if after["job_id"] != request.job_id || after_items.len() != 1 || after_items[0] != item {
        return Err((409, "artifact_changed"));
    }
    if source_manifest_item(&after, &request)? != source_item {
        return Err((409, "artifact_source_mismatch"));
    }
    let job = service_json(state, member, &["jobs", &request.job_id])?;
    if job["job_id"] != request.job_id || job["request_payload"]["source"] != job_source {
        return Err((409, "artifact_source_mismatch"));
    }
    let job_status =
        ArtifactProcessorStatus::parse(job["status"].as_str().ok_or((502, "invalid_job"))?)
            .map_err(|_| (502, "invalid_job"))?;
    let disk = WorkspaceSnapshots::default()
        .read(&root, &saved)
        .map_err(|_| (404, "saved_file_unavailable"))?;
    if disk.bytes != bytes {
        return Err((409, "saved_file_changed"));
    }
    verify_source(
        &root,
        &source,
        &request.source_revision,
        &request.source_digest,
    )?;
    let origin = ArtifactOrigin {
        kind: ArtifactOriginKind::PaperloomService,
        member_id: member.into(),
        workspace_root: root.clone(),
        source_path: source,
        source_revision: request.source_revision,
        source_digest: format!("sha256:{}", request.source_digest),
        saved_revision: disk.revision,
        job_id: request.job_id,
        job_status,
        attempt: request.attempt,
        artifact_key: request.artifact_key,
        manifest_digest: digest(&serde_json::to_vec(item).map_err(|_| (502, "invalid_manifest"))?),
        connection_revision: revision,
        package_digest: package_digest.into(),
    };
    let dir = ledger(state, member, &root, &saved)?;
    let lock = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join(".lock"))
        .map_err(|_| (503, "provenance_unavailable"))?;
    lock.lock().map_err(|_| (503, "provenance_unavailable"))?;
    let prior = history(&dir)?;
    let store = ArtifactStore::new(Some(dir.clone()));
    for proof in &prior {
        check_scope(proof, member, &root, &saved)?;
        if let Some(version) = proof.artifacts.first()
            && version.origin.as_ref() == Some(&origin)
            && version.content_digest == request.checksum
        {
            store
                .read(&proof.session_id, version)
                .map_err(|_| (409, "artifact_changed"))?;
            return Ok(json!(proof));
        }
    }
    if prior.len() >= MAX_HISTORY {
        return Err((413, "provenance_history_limit"));
    }
    let scope = format!(
        "paperloom-{}-{}",
        member,
        &hex_digest(root.as_bytes())[..16]
    );
    let environment = json!({"host":"studio", "hostVersion":env!("CARGO_PKG_VERSION"),
        "os":std::env::consts::OS, "architecture":std::env::consts::ARCH,
        "transport":"in-process", "processorOrigin":"",
        "authority":"verified-workspace-copy; text-layer extraction is host-reported"})
    .to_string();
    let proof = store
        .record_verified_copy(
            &scope,
            &saved,
            &request.checksum,
            origin,
            &digest(&serde_json::to_vec(&job).map_err(|_| (502, "invalid_job"))?),
            &environment,
            &prior,
        )
        .map_err(|_| (409, "saved_file_changed"))?;
    let publish = (|| {
        let bytes = serde_json::to_vec(&proof).map_err(|_| (503, "provenance_unavailable"))?;
        if bytes.len() as u64 > MAX_RECORD {
            return Err((413, "provenance_record_limit"));
        }
        let mut file =
            tempfile::NamedTempFile::new_in(&dir).map_err(|_| (503, "provenance_unavailable"))?;
        file.write_all(&bytes)
            .and_then(|_| file.as_file().sync_all())
            .map_err(|_| (503, "provenance_unavailable"))?;
        file.persist_noclobber(dir.join(format!("{}.json", proof.message_id)))
            .map_err(|_| (503, "provenance_unavailable"))?;
        Ok(())
    })();
    if let Err(error) = publish {
        for version in &proof.artifacts {
            store.discard_unpublished(&scope, &version.id);
        }
        return Err(error);
    }
    Ok(json!(proof))
}

fn lookup_source(state: &Path, member: &str, query: &str) -> Result<Value> {
    let pairs: Vec<_> = url::form_urlencoded::parse(query.as_bytes()).collect();
    let exact = |key| -> Result<String> {
        let rows: Vec<_> = pairs.iter().filter(|(name, _)| name == key).collect();
        if rows.len() != 1 {
            return Err((400, "invalid_request"));
        }
        Ok(rows[0].1.to_string())
    };
    if pairs.len() != 4 {
        return Err((400, "invalid_request"));
    }
    let root = canonical_root(&exact("root")?)?;
    let path = canonical_file(&root, &exact("sourcePath")?)?;
    let revision = exact("sourceRevision")?;
    let source_digest = exact("sourceDigest")?;
    if !hex64(&revision) || !hex64(&source_digest) {
        return Err((400, "invalid_request"));
    }
    verify_source(&root, &path, &revision, &source_digest)?;
    let base = state
        .canonicalize()
        .map_err(|_| (503, "provenance_unavailable"))?
        .join("paperloom-provenance")
        .join(member)
        .join(hex_digest(root.as_bytes()));
    if !base
        .try_exists()
        .map_err(|_| (503, "provenance_unavailable"))?
    {
        return Ok(json!([]));
    }
    // Validate each private parent before reading the workspace ledger.
    let mut current = state
        .canonicalize()
        .map_err(|_| (503, "provenance_unavailable"))?;
    for segment in ["paperloom-provenance", member, &hex_digest(root.as_bytes())] {
        current = current.join(segment);
        if !fs::symlink_metadata(&current).is_ok_and(|metadata| metadata.is_dir())
            || current.canonicalize().ok().as_ref() != Some(&current)
        {
            return Err((503, "provenance_unavailable"));
        }
    }
    let mut result = Vec::new();
    let mut inspected = 0usize;
    let mut directories = 0usize;
    for entry in fs::read_dir(base).map_err(|_| (503, "provenance_unavailable"))? {
        directories += 1;
        if directories > 512 {
            return Err((413, "provenance_history_limit"));
        }
        let entry = entry.map_err(|_| (503, "provenance_unavailable"))?;
        if !entry
            .file_type()
            .map_err(|_| (503, "provenance_unavailable"))?
            .is_dir()
        {
            return Err((409, "provenance_invalid"));
        }
        let store = ArtifactStore::new(Some(entry.path()));
        for proof in history(&entry.path())? {
            inspected += 1;
            if inspected > 512 {
                return Err((413, "provenance_history_limit"));
            }
            let [artifact] = proof.artifacts.as_slice() else {
                return Err((409, "provenance_invalid"));
            };
            check_scope(&proof, member, &root, &artifact.path)?;
            store
                .verify_receipt(&proof.session_id, artifact)
                .map_err(|_| (409, "provenance_invalid"))?;
            if artifact.origin.as_ref().is_some_and(|origin| {
                origin.source_path == path
                    && origin.source_revision == revision
                    && origin.source_digest == format!("sha256:{source_digest}")
            }) {
                result.push(proof);
                if result.len() > MAX_HISTORY {
                    return Err((413, "provenance_history_limit"));
                }
            }
        }
    }
    Ok(json!(result))
}

fn verify_source(root: &str, path: &str, revision: &str, source_digest: &str) -> Result<()> {
    let snapshot = WorkspaceSnapshots::default()
        .read(root, path)
        .map_err(|_| (404, "workspace_source_failed"))?;
    if snapshot.bytes.len() > super::UPLOAD_LIMIT || !snapshot.bytes.starts_with(b"%PDF-") {
        return Err((400, "invalid_pdf"));
    }
    if snapshot.revision != revision || hex_digest(&snapshot.bytes) != source_digest {
        return Err((409, "source_changed"));
    }
    Ok(())
}

fn canonical_root(raw: &str) -> Result<String> {
    crate::local_documents::canonical_workspace(raw)
        .map_err(|_| (404, "workspace_source_failed"))?
        .to_str()
        .map(str::to_owned)
        .ok_or((400, "invalid_request"))
}
fn canonical_file(root: &str, raw: &str) -> Result<String> {
    if raw.len() > 4096
        || raw.contains('\0')
        || !Path::new(raw).is_absolute()
        || Path::new(raw)
            .components()
            .any(|part| part == std::path::Component::ParentDir)
    {
        return Err((404, "not_found"));
    }
    let path = fs::canonicalize(raw).map_err(|_| (404, "workspace_source_failed"))?;
    if !path.starts_with(root) {
        return Err((404, "workspace_source_failed"));
    }
    path.to_str()
        .map(str::to_owned)
        .ok_or((400, "invalid_request"))
}
fn validate_saved_path(root: &str, raw: &str) -> Result<()> {
    let path = Path::new(raw);
    if raw.len() > 4096
        || raw.contains('\0')
        || !path.is_absolute()
        || !path.starts_with(root)
        || path
            .components()
            .any(|part| part == std::path::Component::ParentDir)
    {
        return Err((404, "not_found"));
    }
    Ok(())
}
fn ledger(state: &Path, member: &str, root: &str, path: &str) -> Result<PathBuf> {
    let mut parent = state
        .canonicalize()
        .map_err(|_| (503, "provenance_unavailable"))?;
    for segment in [
        "paperloom-provenance",
        member,
        &hex_digest(root.as_bytes()),
        &hex_digest(path.as_bytes()),
    ] {
        let next = parent.join(segment);
        match fs::create_dir(&next) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err((503, "provenance_unavailable")),
        }
        if !fs::symlink_metadata(&next).is_ok_and(|meta| meta.is_dir())
            || next.canonicalize().ok().as_ref() != Some(&next)
        {
            return Err((503, "provenance_unavailable"));
        }
        parent = next;
    }
    Ok(parent)
}
fn history(dir: &Path) -> Result<Vec<ArtifactProof>> {
    let mut proofs = Vec::new();
    for entry in fs::read_dir(dir).map_err(|_| (503, "provenance_unavailable"))? {
        let entry = entry.map_err(|_| (503, "provenance_unavailable"))?;
        if entry
            .path()
            .extension()
            .is_none_or(|extension| extension != "json")
        {
            continue;
        }
        if proofs.len() >= MAX_HISTORY {
            return Err((413, "provenance_history_limit"));
        }
        proofs.push(read_proof(&entry.path())?);
    }
    Ok(proofs)
}
fn read_proof(path: &Path) -> Result<ArtifactProof> {
    let meta = fs::symlink_metadata(path).map_err(|_| (404, "not_found"))?;
    if !meta.is_file() || meta.len() > MAX_RECORD {
        return Err((409, "provenance_invalid"));
    }
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|file| file.take(MAX_RECORD + 1).read_to_end(&mut bytes))
        .map_err(|_| (503, "provenance_unavailable"))?;
    if bytes.len() as u64 > MAX_RECORD {
        return Err((409, "provenance_invalid"));
    }
    serde_json::from_slice(&bytes).map_err(|_| (409, "provenance_invalid"))
}
fn check_scope(proof: &ArtifactProof, member: &str, root: &str, path: &str) -> Result<()> {
    let [version] = proof.artifacts.as_slice() else {
        return Err((409, "provenance_invalid"));
    };
    let source = version.origin.as_ref().ok_or((409, "provenance_invalid"))?;
    if source.member_id != member
        || source.workspace_root != root
        || version.path != path
        || proof.message_id != version.id
        || source.kind != ArtifactOriginKind::PaperloomService
    {
        return Err((404, "not_found"));
    }
    Ok(())
}
fn source_manifest_item<'a>(manifest: &'a Value, request: &Registration) -> Result<&'a Value> {
    let mut items = manifest["items"]
        .as_array()
        .ok_or((502, "invalid_manifest"))?
        .iter()
        .filter(|item| item["artifact_key"] == "source_pdf");
    let item = items.next().ok_or((409, "artifact_source_mismatch"))?;
    if items.next().is_some()
        || item["ready"] != true
        || item["artifact_kind"] != "file"
        || item["content_type"] != "application/pdf"
        || item["attempt"].as_u64() != Some(u64::from(request.attempt))
        || !item["size_bytes"]
            .as_u64()
            .is_some_and(|bytes| bytes > 0 && bytes <= super::UPLOAD_LIMIT as u64)
        || normalized_checksum(&item["checksum"]).as_deref()
            != Some(format!("sha256:{}", request.source_digest).as_str())
    {
        return Err((409, "artifact_source_mismatch"));
    }
    Ok(item)
}

fn normalized_checksum(value: &Value) -> Option<String> {
    let checksum = value.as_str()?;
    if hex64(checksum) {
        Some(format!("sha256:{checksum}"))
    } else if sha256(checksum) {
        Some(checksum.to_owned())
    } else {
        None
    }
}

fn service_json(state: &Path, member: &str, parts: &[&str]) -> Result<Value> {
    let bytes = service_bytes(state, member, parts)?;
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|_| (502, "invalid_processor_response"))?;
    if value["code"] != 0 {
        return Err((502, "invalid_processor_response"));
    }
    value
        .get("data")
        .cloned()
        .ok_or((502, "invalid_processor_response"))
}
fn service_bytes(state: &Path, member: &str, parts: &[&str]) -> Result<Vec<u8>> {
    let response = proxy(state, member, "GET", parts, "", b"");
    if response.0 != 200 {
        return Err((502, "processor_unavailable"));
    }
    Ok(response.3)
}
fn hex64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
fn sha256(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(hex64)
}
fn digest(bytes: &[u8]) -> String {
    format!("sha256:{}", hex_digest(bytes))
}

#[cfg(test)]
#[path = "paperloom_provenance_tests.rs"]
mod tests;
