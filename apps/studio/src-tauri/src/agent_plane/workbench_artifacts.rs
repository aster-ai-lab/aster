//! Immutable byte snapshots for one member-local agent plane.

use base64::Engine;
use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

use serde_json::Value;

use super::domain::{new_id, now_iso};
use super::workbench_proof::{
    ArtifactEvidence, ArtifactOrigin, ArtifactStatus, ArtifactVersion, Policy, ProducerStatus,
    Proof, ToolEvidence, ToolState, digest,
};

const MAX_BYTES: u64 = 20 * 1024 * 1024;

pub(crate) struct ArtifactStore {
    root: Option<PathBuf>,
    memory: Mutex<HashMap<(String, String), Vec<u8>>>,
}

impl ArtifactStore {
    pub fn new(root: Option<PathBuf>) -> Self {
        Self {
            root: root.map(|path| path.join("proof-artifacts")),
            memory: Mutex::new(HashMap::new()),
        }
    }

    pub fn capture(
        &self,
        workspace: &Path,
        session: &str,
        run: &str,
        parts: &[Value],
        prior: &[Proof],
        complete: bool,
    ) -> Result<Vec<ArtifactVersion>, String> {
        let mut created = Vec::new();
        let result: Result<Vec<ArtifactVersion>, String> = (|| {
            let root =
                fs::canonicalize(workspace).map_err(|_| "artifact workspace is unavailable")?;
            let mut versions = Vec::new();
            let mut candidates = HashMap::<String, (&Value, &str)>::new();
            for part in parts {
                if part["state"] != "output-available" {
                    continue;
                }
                // ACP's typed edit kind is authority; display titles and prose are not.
                if part
                    .pointer("/callProviderMetadata/cherry/toolKind")
                    .and_then(Value::as_str)
                    != Some("edit")
                {
                    continue;
                }
                let Some(call) = part.get("toolCallId").and_then(Value::as_str) else {
                    continue;
                };
                for path in part
                    .get("files")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                {
                    candidates.insert(path.to_owned(), (part, call));
                }
                if let Some(path) = part
                    .pointer("/input/file_path")
                    .or_else(|| part.pointer("/input/path"))
                    .and_then(Value::as_str)
                {
                    candidates.insert(path.to_owned(), (part, call));
                }
            }
            let mut candidates: Vec<_> = candidates.into_iter().collect();
            if candidates.len() > 32 {
                return Err("artifact count exceeds the snapshot limit".into());
            }
            candidates.sort_by(|a, b| a.0.cmp(&b.0));
            let mut seen = HashSet::new();
            for (raw_path, (part, call)) in candidates {
                let path = Path::new(&raw_path);
                let path = fs::canonicalize(if path.is_absolute() {
                    path.to_path_buf()
                } else {
                    root.join(path)
                })
                .map_err(|_| "a written artifact is unavailable")?;
                if !path.starts_with(&root) {
                    return Err("artifact path is outside the authorized workspace".into());
                }
                if !seen.insert(path.clone()) {
                    continue;
                }
                let mut file = fs::File::open(&path).map_err(|_| "artifact cannot be opened")?;
                let handle = same_file::Handle::from_file(
                    file.try_clone()
                        .map_err(|_| "artifact identity is unavailable")?,
                )
                .map_err(|_| "artifact identity is unavailable")?;
                let metadata = file
                    .metadata()
                    .map_err(|_| "artifact metadata is unavailable")?;
                if !metadata.is_file() || metadata.len() > MAX_BYTES {
                    return Err("artifact is not a supported bounded file".into());
                }
                let mut bytes = Vec::new();
                Read::by_ref(&mut file)
                    .take(MAX_BYTES + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|_| "artifact bytes are unavailable")?;
                let after = file
                    .metadata()
                    .map_err(|_| "artifact changed while snapshotting")?;
                if bytes.len() as u64 != metadata.len()
                    || bytes.len() as u64 > MAX_BYTES
                    || after
                        .modified()
                        .map_err(|_| "artifact timestamp is unavailable")?
                        != metadata
                            .modified()
                            .map_err(|_| "artifact timestamp is unavailable")?
                    || after.len() != metadata.len()
                    || same_file::Handle::from_path(&path).ok().as_ref() != Some(&handle)
                    || fs::canonicalize(workspace).ok().as_ref() != Some(&root)
                    || fs::canonicalize(&path).ok().as_ref() != Some(&path)
                {
                    return Err("artifact changed while snapshotting".into());
                }
                let content_digest = digest(&bytes);
                let path = path
                    .to_str()
                    .ok_or("artifact path is not valid UTF-8")?
                    .to_owned();
                let ordinal = prior
                    .iter()
                    .filter(|proof| proof.session_id == session)
                    .flat_map(|proof| &proof.artifacts)
                    .filter(|artifact| artifact.path == path)
                    .map(|artifact| artifact.ordinal)
                    .max()
                    .unwrap_or(0)
                    .checked_add(1)
                    .ok_or("artifact version limit reached")?;
                let id = new_id();
                self.save(session, &id, &bytes)?;
                created.push(id.clone());
                let input = part.get("input").map(Value::to_string).unwrap_or_default();
                let environment=serde_json::json!({"host":"studio","hostVersion":env!("CARGO_PKG_VERSION"),"os":std::env::consts::OS,"architecture":std::env::consts::ARCH,"transport":"a3s-code-acp"}).to_string();
                let workflow = serde_json::to_vec(
                    &parts
                        .iter()
                        .filter(|part| part.get("toolCallId").is_some())
                        .collect::<Vec<_>>(),
                )
                .map_err(|_| "artifact execution evidence is unavailable")?;
                let mut version = ArtifactVersion {
                    origin: None,
                    id,
                    path: path.clone(),
                    name: Path::new(&path)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .ok_or("artifact name is unavailable")?
                        .into(),
                    ordinal,
                    bytes: bytes.len() as u64,
                    media_type: media_type(&path).into(),
                    content_digest,
                    receipt_digest: String::new(),
                    execution_id: run.into(),
                    tool_call_id: call.into(),
                    input_digests: vec![digest(input.as_bytes())],
                    workflow_digest: digest(&workflow),
                    code_digest: digest(input.as_bytes()),
                    environment_digest: digest(environment.as_bytes()),
                    environment,
                    tool_name: part
                        .get("toolName")
                        .and_then(Value::as_str)
                        .unwrap_or("edit")
                        .into(),
                    created_at: now_iso(),
                    status: if complete {
                        ArtifactStatus::Traced
                    } else {
                        ArtifactStatus::Incomplete
                    },
                    error: (!complete).then(|| "producer execution did not complete".into()),
                };
                let binding =
                    serde_json::to_vec(&("a3s.studio.artifact-lineage.v1", session, &version))
                        .map_err(|_| "artifact receipt failed")?;
                version.receipt_digest = digest(&binding);
                versions.push(version);
            }
            Ok(versions)
        })();
        if result.is_err() {
            for id in created {
                self.discard_unpublished(session, &id);
            }
        }
        result
    }

    /// Records the host's verified copy operation; it does not attest an external processor's execution.
    pub fn record_verified_copy(
        &self,
        scope: &str,
        path: &str,
        expected_digest: &str,
        origin: ArtifactOrigin,
        workflow_digest: &str,
        environment: &str,
        prior: &[Proof],
    ) -> Result<Proof, String> {
        let snapshot = crate::workspace_snapshot::WorkspaceSnapshots::default()
            .read(&origin.workspace_root, path)
            .map_err(|_| "saved artifact is unavailable")?;
        if snapshot.bytes.len() as u64 > MAX_BYTES
            || digest(&snapshot.bytes) != expected_digest
            || snapshot.revision != origin.saved_revision
        {
            return Err("saved artifact changed".into());
        }
        let id = new_id();
        let input = serde_json::to_string(&origin).map_err(|_| "artifact source is invalid")?;
        let ordinal = prior
            .iter()
            .flat_map(|proof| &proof.artifacts)
            .filter(|version| version.path == path)
            .map(|version| version.ordinal)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or("artifact version limit reached")?;
        let mut version = ArtifactVersion {
            origin: Some(origin.clone()),
            id: id.clone(),
            path: path.into(),
            name: Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or("artifact name is unavailable")?
                .into(),
            ordinal,
            bytes: snapshot.bytes.len() as u64,
            media_type: media_type(path).into(),
            content_digest: expected_digest.into(),
            receipt_digest: String::new(),
            input_digests: vec![origin.source_digest.clone()],
            workflow_digest: workflow_digest.into(),
            code_digest: digest(input.as_bytes()),
            environment_digest: digest(environment.as_bytes()),
            environment: environment.into(),
            execution_id: origin.job_id.clone(),
            tool_call_id: id.clone(),
            tool_name: "paperloom.verify-saved-copy".into(),
            created_at: now_iso(),
            status: ArtifactStatus::Traced,
            error: None,
        };
        let binding = serde_json::to_vec(&("a3s.studio.artifact-lineage.v1", scope, &version))
            .map_err(|_| "artifact receipt failed")?;
        version.receipt_digest = digest(&binding);
        let mut proof = Proof::new(
            scope,
            &id,
            &origin.job_id,
            "unreported",
            "",
            Policy {
                review_mode: false,
                artifact_trace: true,
            },
        );
        proof.producer_status = ProducerStatus::Completed;
        proof.tools.push(ToolEvidence {
            id,
            name: version.tool_name.clone(),
            state: ToolState::Success,
            input,
            output: serde_json::json!({ "path":path, "contentDigest":expected_digest }).to_string(),
        });
        self.save(scope, &version.id, &snapshot.bytes)?;
        proof.artifacts.push(version);
        Ok(proof)
    }

    pub(crate) fn discard_unpublished(&self, session: &str, id: &str) {
        if let Some(root) = &self.root {
            if let Ok(path) = owned_path(root, session, id) {
                if fs::remove_file(path).is_err() {
                    eprintln!("studio: unpublished artifact snapshot cleanup failed");
                }
            }
        } else if let Ok(mut memory) = self.memory.lock() {
            memory.remove(&(session.to_owned(), id.to_owned()));
        }
    }

    fn save(&self, session: &str, id: &str, bytes: &[u8]) -> Result<(), String> {
        if let Some(root) = &self.root {
            let path = owned_path(root, session, id)?;
            let parent = path.parent().ok_or("artifact snapshot path is invalid")?;
            fs::create_dir_all(parent).map_err(|_| "artifact snapshot directory is unavailable")?;
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options
                .open(&path)
                .map_err(|_| "artifact snapshot cannot be created")?;
            if file.write_all(bytes).and_then(|_| file.sync_all()).is_err() {
                drop(file);
                if fs::remove_file(path).is_err() {
                    eprintln!("studio: incomplete artifact snapshot cleanup failed");
                }
                return Err("artifact snapshot could not be saved".into());
            }
        } else {
            self.memory
                .lock()
                .map_err(|_| "artifact store is unavailable")?
                .insert((session.into(), id.into()), bytes.to_vec());
        }
        Ok(())
    }

    pub fn verify_receipt(&self, session: &str, version: &ArtifactVersion) -> Result<(), String> {
        let mut binding = version.clone();
        binding.receipt_digest.clear();
        let binding = serde_json::to_vec(&("a3s.studio.artifact-lineage.v1", session, &binding))
            .map_err(|_| "artifact receipt is invalid")?;
        if digest(&binding) != version.receipt_digest {
            return Err("artifact receipt identity mismatch".into());
        }
        Ok(())
    }

    pub fn read(&self, session: &str, version: &ArtifactVersion) -> Result<Vec<u8>, String> {
        self.verify_receipt(session, version)?;
        let bytes = if let Some(root) = &self.root {
            let mut file = fs::File::open(owned_path(root, session, &version.id)?)
                .map_err(|_| "artifact snapshot not found")?;
            let mut bytes = Vec::new();
            Read::by_ref(&mut file)
                .take(MAX_BYTES + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "artifact snapshot not found")?;
            if bytes.len() as u64 > MAX_BYTES {
                return Err("artifact snapshot is too large".into());
            }
            bytes
        } else {
            self.memory
                .lock()
                .map_err(|_| "artifact store is unavailable")?
                .get(&(session.into(), version.id.clone()))
                .cloned()
                .ok_or("artifact snapshot not found")?
        };
        if digest(&bytes) != version.content_digest || bytes.len() as u64 != version.bytes {
            return Err("artifact snapshot digest mismatch".into());
        }
        Ok(bytes)
    }

    pub fn evidence(
        &self,
        session: &str,
        versions: &[ArtifactVersion],
    ) -> Result<Vec<ArtifactEvidence>, String> {
        if versions
            .iter()
            .try_fold(0u64, |sum, version| sum.checked_add(version.bytes))
            .is_none_or(|total| total > MAX_BYTES)
        {
            return Err("artifact evidence exceeds the independent review limit".into());
        }
        let mut result = Vec::new();
        for version in versions {
            let bytes = self.read(session, version)?;
            let (text, image_url) =
                if matches!(version.media_type.as_str(), "image/png" | "image/jpeg") {
                    (
                        None,
                        Some(format!(
                            "data:{};base64,{}",
                            version.media_type,
                            base64::engine::general_purpose::STANDARD.encode(bytes)
                        )),
                    )
                } else if Path::new(&version.path)
                    .extension()
                    .and_then(|extension| extension.to_str())
                    == Some("docx")
                {
                    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
                        .map_err(|_| "artifact document is invalid")?;
                    let xml = archive
                        .by_name("word/document.xml")
                        .map_err(|_| "artifact document text is unavailable")?;
                    if xml.size() > 128 * 1024 {
                        return Err("artifact document exceeds review evidence limit".into());
                    }
                    let mut body = String::new();
                    xml.take(128 * 1024 + 1)
                        .read_to_string(&mut body)
                        .map_err(|_| "artifact document text is invalid")?;
                    (
                        Some(super::review_prompt::paragraphs_from_document_xml(&body)),
                        None,
                    )
                } else {
                    (
                        Some(String::from_utf8(bytes).map_err(|_| {
                            "this artifact format needs a supported independent document reviewer"
                        })?),
                        None,
                    )
                };
            result.push(ArtifactEvidence {
                id: version.id.clone(),
                text,
                image_url,
            });
        }
        Ok(result)
    }
}

fn owned_path(root: &Path, session: &str, id: &str) -> Result<PathBuf, String> {
    let segment = |value: &str| {
        let mut c = Path::new(value).components();
        matches!(c.next(), Some(Component::Normal(_))) && c.next().is_none()
    };
    if !segment(session) || uuid::Uuid::parse_str(id).is_err() {
        return Err("artifact identity is invalid".into());
    }
    Ok(root.join(session).join(id))
}

fn media_type(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("svg") => "image/svg+xml",
        Some("pdf") => "application/pdf",
        Some("json") => "application/json",
        Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        Some("md" | "txt" | "rs" | "ts" | "tsx" | "js" | "py" | "csv" | "html" | "css" | "acl") => {
            "text/plain"
        }
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
#[path = "workbench_artifacts_tests.rs"]
mod tests;
