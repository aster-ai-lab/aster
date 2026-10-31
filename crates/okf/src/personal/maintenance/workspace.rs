//! Workspace inputs adapt the original maintenance job; the host still owns source admission.

use super::*;
use crate::WorkspaceDocumentProjection;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WorkspaceBinding {
    pub(super) source_root: PathBuf,
    pub(super) revision: String,
}

pub struct WorkspaceKnowledgeSource {
    path: String,
    bytes: Vec<u8>,
}

impl WorkspaceKnowledgeSource {
    /// Explicit original-line labels distinguish source coordinates from reader offsets.
    pub fn text(path: &str, bytes: &[u8]) -> io::Result<Self> {
        let text = std::str::from_utf8(bytes).map_err(io::Error::other)?;
        if bytes.len() > MAX_TEXT_BYTES || text.trim().is_empty() || text.contains('\0') {
            return Err(refused("knowledge.invalid_workspace_source"));
        }
        let body = text
            .split_inclusive('\n')
            .enumerate()
            .map(|(index, line)| format!("L{}: {line}", index + 1))
            .collect::<String>();
        Self::new(
            path,
            bytes,
            &format!(
                "source_start_line: 1\nsource_end_line: {}\nsource_line_format: original_line_labels_v1\n",
                text.lines().count()
            ),
            &body,
        )
    }

    pub fn document(
        path: &str,
        bytes: &[u8],
        projection: &WorkspaceDocumentProjection,
    ) -> io::Result<Self> {
        if projection.source_generation() != format!("sha256:{}", content_generation(bytes)) {
            return Err(refused("knowledge.workspace_projection_mismatch"));
        }
        let body = serde_json::to_string_pretty(projection).map_err(io::Error::other)?;
        Self::new(path, bytes, "", &body)
    }

    fn new(path: &str, source: &[u8], coordinates: &str, body: &str) -> io::Result<Self> {
        if path.is_empty()
            || path.len() > 4096
            || path.contains(['\\', ':'])
            || path.chars().any(char::is_control)
            || path
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
            || source.len() > MAX_PACKAGE_BYTES
        {
            return Err(refused("knowledge.invalid_workspace_source"));
        }
        let generation = format!("sha256:{}", content_generation(source));
        let content = format!(
            "---\ntype: Source\ntitle: {}\nresource: {}\nsource_generation: {}\n{coordinates}---\n\n{body}\n",
            crate::yaml_quote(path),
            crate::yaml_quote(&format!("workspace:{path}")),
            crate::yaml_quote(&generation),
        );
        if content.len() > MAX_TEXT_BYTES {
            return Err(refused("knowledge.limit_exceeded"));
        }
        Ok(Self {
            path: path.into(),
            bytes: content.into_bytes(),
        })
    }
}

/// Only bounded projections are copied into staging/history. Live sources stay outside the owner.
pub struct WorkspaceKnowledgeInput {
    binding: WorkspaceBinding,
    output: PathBuf,
    generation: String,
    files: BTreeMap<String, Vec<u8>>,
}

impl WorkspaceKnowledgeInput {
    pub fn new(
        source_root: &Path,
        binding_revision: &str,
        source_snapshot_generation: &str,
        sources: Vec<WorkspaceKnowledgeSource>,
    ) -> io::Result<Self> {
        if !source_root.is_absolute()
            || !valid_revision(binding_revision)
            || !valid_generation(source_snapshot_generation)
            || sources.len() > MAX_FILES
        {
            return Err(refused("knowledge.invalid_workspace_input"));
        }
        let source_root = source_root.canonicalize()?;
        if !source_root.is_dir() || source_root.parent().is_none() {
            return Err(refused("knowledge.invalid_workspace_input"));
        }
        let binding = WorkspaceBinding {
            source_root,
            revision: binding_revision.into(),
        };
        let output = workspace_knowledge_output_root(&binding.source_root, &binding.revision)?;
        let mut files = BTreeMap::new();
        let mut total = 0usize;
        for source in sources {
            total += source.bytes.len();
            let key = format!("_sources/{}.md", content_generation(source.path.as_bytes()));
            if total > MAX_PACKAGE_BYTES || files.insert(key, source.bytes).is_some() {
                return Err(refused("knowledge.invalid_workspace_input"));
            }
        }
        let identity = serde_json::to_vec(&(
            "workspace-knowledge-input-v1",
            &binding,
            source_snapshot_generation,
            files
                .iter()
                .map(|(path, bytes)| (path, content_generation(bytes)))
                .collect::<Vec<_>>(),
        ))
        .map_err(io::Error::other)?;
        Ok(Self {
            binding,
            output,
            generation: format!("sha256:{}", content_generation(&identity)),
            files,
        })
    }

    pub fn output_root(&self) -> &Path {
        &self.output
    }
    pub fn binding_revision(&self) -> &str {
        &self.binding.revision
    }
    pub fn generation(&self) -> &str {
        &self.generation
    }
    pub(super) fn files(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.files
    }
}

fn valid_revision(value: &str) -> bool {
    !value.is_empty() && value.len() <= 4096 && !value.chars().any(char::is_control)
}

/// Reuse the original job's verified conversion, including after host restart.
pub fn retained_workspace_document_projection(
    source_root: &Path,
    revision: &str,
    path: &str,
    source: &[u8],
    parser: &str,
) -> io::Result<Option<WorkspaceDocumentProjection>> {
    let output = workspace_knowledge_output_root(source_root, revision)?;
    let Some(_read) = try_workspace_knowledge_read(&output)? else {
        return Ok(None);
    };
    let ledger = load_ledger(&output)?;
    validate_binding(&output, revision, &ledger)?;
    let job = match &ledger.active {
        Some(job) => job.clone(),
        None => match ledger.activity.first() {
            Some(activity) => load_job(&output, &ledger, &activity.id)?,
            None => return Ok(None),
        },
    };
    let key = format!("_sources/{}.md", content_generation(path.as_bytes()));
    let Some(generation) = job.source_generations.get(&key) else {
        return Ok(None);
    };
    let bytes = read_bounded(
        &output
            .join(".wiki-history")
            .join(&job.id)
            .join("sources")
            .join(&key),
        MAX_TEXT_BYTES,
    )?;
    if generation != &content_generation(&bytes) {
        return Err(conflict("knowledge.history_changed"));
    }
    let text = std::str::from_utf8(&bytes).map_err(io::Error::other)?;
    let parts = split_note(text);
    let Some(header) = parts.inner else {
        return Ok(None);
    };
    if scalar(header, "resource") != Some(format!("workspace:{path}"))
        || scalar(header, "source_generation")
            != Some(format!("sha256:{}", content_generation(source)))
    {
        return Ok(None);
    }
    let projection: WorkspaceDocumentProjection = match serde_json::from_str(parts.body.trim()) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };
    Ok(projection.matches(source, parser).then_some(projection))
}

/// Replay an already published explanation only over the same newly admitted source bytes.
/// Callers must independently authorize both bindings; neither input grants access to the other.
pub fn reuse_workspace_knowledge_maintenance(
    input: &WorkspaceKnowledgeInput,
    previous: &WorkspaceKnowledgeInput,
) -> io::Result<bool> {
    if input.binding.source_root != previous.binding.source_root
        || input.files != previous.files
        || input.output == previous.output
    {
        return Ok(false);
    }
    let saved = match with_workspace(
        &previous.output,
        &previous.binding.revision,
        |root, ledger| {
            let view = plan_with_inputs(root, ledger, Some(previous))?;
            if view.active.is_some()
                || view.pages == 0
                || view
                    .sources
                    .iter()
                    .any(|source| source.status != SourceStatus::Current)
            {
                return Ok(None);
            }
            let mut pages = Vec::new();
            for (path, bytes) in snapshot(root)?
                .into_iter()
                .filter(|(path, _)| page_path(path))
            {
                let references =
                    aster_use_core::okf_document_references(&path, &bytes, bundle_limits())
                        .map_err(io::Error::other)?;
                let reference = references
                    .last()
                    .ok_or_else(|| refused("knowledge.invalid_citation"))?;
                let citation = citation_at(root, ledger, previous, &path, &reference.target)?;
                let relative = citation
                    .snapshot_path
                    .strip_prefix(root.join(".wiki-history"))
                    .map_err(|_| refused("knowledge.invalid_citation"))?;
                let id = relative
                    .components()
                    .next()
                    .and_then(|part| part.as_os_str().to_str())
                    .ok_or_else(|| refused("knowledge.invalid_citation"))?;
                let job = load_job(root, ledger, id)?;
                let change = job
                    .changes
                    .iter()
                    .find(|change| change.path == path)
                    .ok_or_else(|| refused("knowledge.change_missing"))?;
                for key in &change.sources {
                    let evidence = read_bounded(
                        &root
                            .join(".wiki-history")
                            .join(id)
                            .join("sources")
                            .join(key),
                        MAX_TEXT_BYTES,
                    )?;
                    if previous.files.get(key) != Some(&evidence)
                        || job.source_generations.get(key) != Some(&content_generation(&evidence))
                    {
                        return Ok(None);
                    }
                }
                let footer = cited_body_at(
                    b"",
                    &change.sources,
                    &path,
                    &format!(".wiki-history/{id}/sources/"),
                )?;
                let text = std::str::from_utf8(&bytes).map_err(io::Error::other)?;
                let raw = text
                    .strip_suffix(&footer)
                    .ok_or_else(|| refused("knowledge.invalid_citation"))?;
                if content_generation(raw.as_bytes()) != change.candidate_generation {
                    return Err(conflict("knowledge.candidate_changed"));
                }
                pages.push((change.clone(), raw.as_bytes().to_vec()));
            }
            Ok(Some((pages, ledger.categories.clone())))
        },
    ) {
        Ok(saved) => saved,
        // A stale or invalid donor must not block an independently authorized compilation.
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound
                    | io::ErrorKind::InvalidData
                    | io::ErrorKind::AlreadyExists
                    | io::ErrorKind::WouldBlock
                    | io::ErrorKind::PermissionDenied
            ) =>
        {
            return Ok(false)
        }
        Err(error) => return Err(error),
    };
    let Some((pages, categories)) = saved else {
        return Ok(false);
    };
    ensure_workspace_owner(input)?;
    with_workspace(&input.output, &input.binding.revision, |root, ledger| {
        let view = plan_with_inputs(root, ledger, Some(input))?;
        if view.active.is_some() || view.pages != 0 || !ledger.activity.is_empty() {
            return Ok(false);
        }
        let job = begin(root, ledger, &[], MaintenanceOperation::Ingest, Some(input))?;
        let result = (|| {
            let staging = stage_path(root, &job)?;
            let mut proposals = Vec::new();
            for (change, bytes) in &pages {
                write_owned(&staging.join(&change.path), bytes)?;
                proposals.push(serde_json::json!({
                    "path":change.path,"title":change.title,"kind":change.kind,
                    "change":"create","reason":"Reused the same current source evidence",
                    "requires_review":false,"sources":change.sources
                }));
            }
            write_owned(&staging.join(MANIFEST), &serde_json::to_vec(&serde_json::json!({
                "sources":job.sources,"pages":proposals,"categories":categories,
                "summary":"Reused a source-qualified workspace encyclopedia without a model request"
            })).map_err(io::Error::other)?)?;
            finish(root, ledger, &job.id, Some(input))?;
            Ok(true)
        })();
        if result.is_err() {
            fail_job(root, ledger, &job.id)?;
        }
        result
    })
}

/// A namespace alone grants no access to another binding's pages or job.
pub fn workspace_knowledge_output_root(source_root: &Path, revision: &str) -> io::Result<PathBuf> {
    if !source_root.is_absolute() || !valid_revision(revision) {
        return Err(refused("knowledge.invalid_workspace_input"));
    }
    let source_root = source_root.canonicalize()?;
    if source_root.parent().is_none() || !source_root.is_dir() {
        return Err(refused("knowledge.invalid_workspace_input"));
    }
    let output = source_root
        .join("wiki/workspace")
        .join(content_generation(revision.as_bytes()));
    if output.to_str().is_none_or(|path| path.len() > 4096) {
        return Err(refused("knowledge.invalid_workspace_input"));
    }
    Ok(output)
}

pub(super) fn valid_generation(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

fn empty_unowned_output(root: &Path) -> io::Result<()> {
    if root.canonicalize()? != root || fs::symlink_metadata(root)?.file_type().is_symlink() {
        return Err(refused("knowledge.symlink_refused"));
    }
    if fs::read_dir(root)?.any(|entry| {
        entry.is_err() || entry.is_ok_and(|entry| entry.file_name() != ".wiki-maintenance.lock")
    }) {
        return Err(refused("knowledge.workspace_output_unowned"));
    }
    Ok(())
}

pub(super) fn with_workspace<T>(
    root: &Path,
    revision: &str,
    action: impl FnOnce(&Path, &mut Ledger) -> io::Result<T>,
) -> io::Result<T> {
    validate_output(root, revision)?;
    // Presence is checked before opening the shared lock, so arbitrary unowned directories are not adopted.
    read_bounded(&root.join(STATE), MAX_REPORT_BYTES)?;
    with_locked(root, |root, ledger| {
        validate_binding(root, revision, ledger)?;
        publication::recover(root, ledger)?;
        action(root, ledger)
    })
}

fn validate_output(root: &Path, revision: &str) -> io::Result<()> {
    if !valid_revision(revision)
        || !root.is_absolute()
        || root
            .file_name()
            .is_none_or(|name| name != content_generation(revision.as_bytes()).as_str())
        || root
            .parent()
            .and_then(Path::file_name)
            .is_none_or(|name| name != "workspace")
        || root
            .parent()
            .and_then(Path::parent)
            .and_then(Path::file_name)
            .is_none_or(|name| name != "wiki")
    {
        return Err(refused("knowledge.invalid_workspace_output"));
    }
    Ok(())
}

fn validate_binding(root: &Path, revision: &str, ledger: &Ledger) -> io::Result<()> {
    let binding = ledger
        .workspace_binding
        .as_ref()
        .ok_or_else(|| refused("knowledge.invalid_workspace_output"))?;
    if binding
        .source_root
        .join("wiki/workspace")
        .join(content_generation(revision.as_bytes()))
        != root
        || binding.revision != revision
    {
        return Err(conflict("knowledge.workspace_scope_changed"));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceKnowledgeStatus {
    NotGenerated,
    Generating,
    Review,
    Failed,
    Published,
    Busy,
    Unavailable,
}

#[derive(Clone, Debug, Serialize)]
pub struct WorkspaceKnowledgeSummary {
    pub status: WorkspaceKnowledgeStatus,
    pub updated_ms: Option<u64>,
    pub pages: usize,
    pub sources: usize,
}

// Count metadata under the original bounded page root, never source bodies or history.
fn page_count(root: &Path) -> io::Result<usize> {
    let concepts = root.join("concepts");
    match fs::symlink_metadata(&concepts) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error),
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(refused("knowledge.symlink_refused"))
        }
        Ok(_) => {}
    }
    let mut pending = vec![(concepts, 0)];
    let mut visited = 0;
    let mut pages = 0;
    while let Some((directory, depth)) = pending.pop() {
        if depth > MAX_DEPTH {
            return Err(refused("knowledge.depth_exceeded"));
        }
        let metadata = fs::symlink_metadata(&directory)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(refused("knowledge.symlink_refused"));
        }
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            visited += 1;
            if visited > MAX_FILES * 2 {
                return Err(refused("knowledge.file_limit_exceeded"));
            }
            let kind = entry.file_type()?;
            if kind.is_symlink() {
                return Err(refused("knowledge.symlink_refused"));
            }
            let path = entry.path();
            if kind.is_dir() {
                pending.push((path, depth + 1));
            } else if kind.is_file()
                && page_path(
                    &path
                        .strip_prefix(root)
                        .map_err(io::Error::other)?
                        .to_string_lossy(),
                )
            {
                pages += 1;
                if pages > MAX_FILES {
                    return Err(refused("knowledge.file_limit_exceeded"));
                }
            }
        }
    }
    Ok(pages)
}

/// Reads only the existing ledger and bounded page metadata; never admits sources, recovers or enables a Wiki.
pub fn workspace_knowledge_summary(
    root: &Path,
    revision: &str,
) -> io::Result<WorkspaceKnowledgeSummary> {
    validate_output(root, revision)?;
    let Some(_read) = try_workspace_knowledge_read(root)? else {
        return Ok(WorkspaceKnowledgeSummary {
            status: WorkspaceKnowledgeStatus::NotGenerated,
            updated_ms: None,
            pages: 0,
            sources: 0,
        });
    };
    if root.canonicalize()? != root {
        return Err(refused("knowledge.invalid_root"));
    }
    let ledger = load_ledger(root)?;
    validate_binding(root, revision, &ledger)?;
    let updated_ms = match fs::symlink_metadata(root.join("index.md")) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => Some(
            u64::try_from(
                metadata
                    .modified()?
                    .duration_since(UNIX_EPOCH)
                    .map_err(io::Error::other)?
                    .as_millis(),
            )
            .map_err(io::Error::other)?,
        ),
        Ok(_) => return Err(refused("knowledge.invalid_publication")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let status = match ledger.active {
        Some(job) if job.error.is_some() => WorkspaceKnowledgeStatus::Failed,
        Some(job) if job.ready => WorkspaceKnowledgeStatus::Review,
        Some(_) => WorkspaceKnowledgeStatus::Generating,
        None if updated_ms.is_some() => WorkspaceKnowledgeStatus::Published,
        None => WorkspaceKnowledgeStatus::NotGenerated,
    };
    Ok(WorkspaceKnowledgeSummary {
        status,
        updated_ms,
        pages: page_count(root)?,
        sources: ledger.sources.len(),
    })
}

pub fn workspace_knowledge_maintenance_plan(
    input: &WorkspaceKnowledgeInput,
) -> io::Result<KnowledgePlan> {
    match fs::symlink_metadata(&input.output) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return plan_files(&input.files, &Ledger::default())
        }
        Err(error) => return Err(error),
        Ok(_) => {}
    }
    if !input.output.join(STATE).exists() {
        empty_unowned_output(&input.output)?;
        return plan_files(&input.files, &Ledger::default());
    }
    with_workspace(&input.output, &input.binding.revision, |root, ledger| {
        plan_with_inputs(root, ledger, Some(input))
    })
}

/// Discover an existing owner without rescanning sources or enabling an untouched workspace.
pub fn workspace_knowledge_maintenance_enabled(root: &Path, revision: &str) -> io::Result<bool> {
    match with_workspace(root, revision, |_, _| Ok(true)) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        result => result,
    }
}

fn ensure_workspace_owner(input: &WorkspaceKnowledgeInput) -> io::Result<()> {
    create_owned_directory(&input.output)?;
    if !input.output.join(STATE).exists() {
        empty_unowned_output(&input.output)?;
        with_locked(&input.output, |root, ledger| {
            if ledger.workspace_binding.is_some()
                || ledger.active.is_some()
                || !ledger.sources.is_empty()
            {
                return Err(refused("knowledge.workspace_output_unowned"));
            }
            ledger.workspace_binding = Some(input.binding.clone());
            save(root, ledger)
        })?;
    }
    Ok(())
}

pub fn begin_workspace_knowledge_maintenance(
    input: &WorkspaceKnowledgeInput,
) -> io::Result<Option<KnowledgeJob>> {
    ensure_workspace_owner(input)?;
    with_workspace(&input.output, &input.binding.revision, |root, ledger| {
        if ledger
            .active
            .as_ref()
            .is_some_and(|job| job.workspace_generation.as_deref() != Some(input.generation()))
        {
            return Err(conflict("knowledge.workspace_source_changed"));
        }
        let view = plan_with_inputs(root, ledger, Some(input))?;
        if view.active.is_none()
            && view
                .sources
                .iter()
                .all(|source| source.status == SourceStatus::Current)
        {
            return Ok(None);
        }
        begin(root, ledger, &[], MaintenanceOperation::Ingest, Some(input)).map(Some)
    })
}

/// The host must supply its newly admitted packet immediately before settlement.
pub fn finish_workspace_knowledge_maintenance(
    input: &WorkspaceKnowledgeInput,
    id: &str,
) -> io::Result<KnowledgePlan> {
    with_workspace(&input.output, &input.binding.revision, |root, ledger| {
        if ledger
            .active
            .as_ref()
            .is_none_or(|job| job.workspace_generation.as_deref() != Some(input.generation()))
        {
            return Err(conflict("knowledge.workspace_source_changed"));
        }
        finish(root, ledger, id, Some(input))
    })
}

pub fn workspace_knowledge_compile_workspace(
    root: &Path,
    id: &str,
    revision: &str,
) -> io::Result<PathBuf> {
    with_workspace(root, revision, |root, ledger| {
        let job = ledger
            .active
            .as_ref()
            .filter(|job| {
                job.id == id
                    && !job.ready
                    && job.error.is_none()
                    && job
                        .workspace_generation
                        .as_deref()
                        .is_some_and(valid_generation)
            })
            .ok_or_else(|| refused("knowledge.job_not_running"))?;
        stage_path(root, job)
    })
}

pub fn workspace_knowledge_maintenance_job(
    input: &WorkspaceKnowledgeInput,
    id: &str,
) -> io::Result<KnowledgeJob> {
    with_workspace(&input.output, &input.binding.revision, |root, ledger| {
        load_job(root, ledger, id)
    })
}

pub fn abandon_workspace_knowledge_maintenance(
    input: &WorkspaceKnowledgeInput,
    id: &str,
) -> io::Result<KnowledgePlan> {
    with_workspace(&input.output, &input.binding.revision, |root, ledger| {
        close(root, ledger, id, ActivityStatus::Discarded, Some(input))
    })
}

/// Revoke candidate authority without rereading live sources when the host retires a binding.
pub fn retire_workspace_knowledge(root: &Path, revision: &str) -> io::Result<()> {
    match with_workspace(root, revision, |root, ledger| {
        if let Some(id) = ledger.active.as_ref().map(|job| job.id.clone()) {
            close_job(root, ledger, &id, ActivityStatus::Discarded, true)?;
        }
        Ok(())
    }) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

pub fn cancel_workspace_knowledge_job(root: &Path, revision: &str, id: &str) -> io::Result<()> {
    with_workspace(root, revision, |root, ledger| {
        close_job(root, ledger, id, ActivityStatus::Cancelled, false)
    })
}

pub fn fail_workspace_knowledge_job(root: &Path, revision: &str, id: &str) -> io::Result<()> {
    with_workspace(root, revision, |root, ledger| fail_job(root, ledger, id))
}

pub fn abandon_workspace_knowledge_job(root: &Path, revision: &str, id: &str) -> io::Result<()> {
    with_workspace(root, revision, |root, ledger| {
        close_job(root, ledger, id, ActivityStatus::Discarded, false)
    })
}

pub fn fail_workspace_knowledge_maintenance(
    input: &WorkspaceKnowledgeInput,
    id: &str,
) -> io::Result<KnowledgePlan> {
    with_workspace(&input.output, &input.binding.revision, |root, ledger| {
        fail(root, ledger, id, Some(input))
    })
}

pub fn cancel_workspace_knowledge_maintenance(
    input: &WorkspaceKnowledgeInput,
    id: &str,
) -> io::Result<KnowledgePlan> {
    with_workspace(&input.output, &input.binding.revision, |root, ledger| {
        close(root, ledger, id, ActivityStatus::Cancelled, Some(input))
    })
}

pub fn workspace_knowledge_change(
    input: &WorkspaceKnowledgeInput,
    id: &str,
    path: &str,
) -> io::Result<KnowledgeChangeDetail> {
    with_workspace(&input.output, &input.binding.revision, |root, ledger| {
        change_detail(root, ledger, id, path)
    })
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceKnowledgeCitation {
    pub page: String,
    pub page_generation: String,
    pub source_path: String,
    pub source_generation: String,
    pub snapshot_path: PathBuf,
    pub current: bool,
}

/// Resolve a declared citation against the published page's original job and frozen evidence.
pub fn workspace_knowledge_citation(
    input: &WorkspaceKnowledgeInput,
    page: &str,
    reference: &str,
) -> io::Result<WorkspaceKnowledgeCitation> {
    if !page_path(page) || reference.is_empty() || reference.len() > 8192 {
        return Err(refused("knowledge.invalid_citation"));
    }
    with_workspace(&input.output, &input.binding.revision, |root, ledger| {
        citation_at(root, ledger, input, page, reference)
    })
}

fn citation_at(
    root: &Path,
    ledger: &Ledger,
    input: &WorkspaceKnowledgeInput,
    page: &str,
    reference: &str,
) -> io::Result<WorkspaceKnowledgeCitation> {
    let bytes = read_bounded(&root.join(page), MAX_TEXT_BYTES)?;
    let references = aster_use_core::okf_document_references(page, &bytes, bundle_limits())
        .map_err(io::Error::other)?;
    let target = references
        .iter()
        .find(|item| item.target == reference)
        .and_then(|item| item.resolved_path.as_deref())
        .ok_or_else(|| refused("knowledge.invalid_citation"))?;
    // The owner appends its evidence footer last; page bytes must still match that job.
    let (id, _) = references
        .last()
        .and_then(|item| item.resolved_path.as_deref())
        .and_then(|path| path.strip_prefix(".wiki-history/"))
        .and_then(|path| path.split_once("/sources/"))
        .ok_or_else(|| refused("knowledge.invalid_citation"))?;
    let job = load_job(root, ledger, id)?;
    let generation = content_generation(&bytes);
    let change = job
        .changes
        .iter()
        .find(|change| {
            change.path == page
                && change.decision == ChangeDecision::Published
                && change.generation == generation
        })
        .filter(|_| job.workspace_generation.is_some())
        .ok_or_else(|| conflict("knowledge.published_page_changed"))?;
    let key = change
        .sources
        .iter()
        .find(|key| target == key.as_str() || target == format!(".wiki-history/{id}/sources/{key}"))
        .ok_or_else(|| refused("knowledge.invalid_citation"))?;
    let snapshot_path = root
        .join(".wiki-history")
        .join(id)
        .join("sources")
        .join(key);
    let evidence = read_bounded(&snapshot_path, MAX_TEXT_BYTES)?;
    if job.source_generations.get(key) != Some(&content_generation(&evidence)) {
        return Err(conflict("knowledge.history_changed"));
    }
    let text = std::str::from_utf8(&evidence).map_err(io::Error::other)?;
    let header = split_note(text)
        .inner
        .ok_or_else(|| refused("knowledge.invalid_citation"))?;
    let source_path = scalar(header, "resource")
        .and_then(|value| value.strip_prefix("workspace:").map(str::to_owned))
        .filter(|path| *key == format!("_sources/{}.md", content_generation(path.as_bytes())))
        .ok_or_else(|| refused("knowledge.invalid_citation"))?;
    let source_generation = scalar(header, "source_generation")
        .filter(|value| valid_generation(value))
        .ok_or_else(|| refused("knowledge.invalid_citation"))?;
    Ok(WorkspaceKnowledgeCitation {
        page: page.into(),
        page_generation: format!("sha256:{generation}"),
        source_path,
        source_generation,
        snapshot_path,
        current: input.files.get(key) == Some(&evidence),
    })
}

pub struct WorkspaceKnowledgePage {
    pub body: String,
    pub generation: String,
    pub sources: Vec<WorkspaceKnowledgeCitation>,
}

/// Read a published page only when every owner-retained source is current.
pub fn workspace_knowledge_page(
    input: &WorkspaceKnowledgeInput,
    page: &str,
) -> io::Result<WorkspaceKnowledgePage> {
    if !page_path(page) {
        return Err(refused("knowledge.invalid_page"));
    }
    let (bytes, references) = with_workspace(&input.output, &input.binding.revision, |root, _| {
        let bytes = read_bounded(&root.join(page), MAX_TEXT_BYTES)?;
        let references = aster_use_core::okf_document_references(page, &bytes, bundle_limits())
            .map_err(io::Error::other)?
            .into_iter()
            .filter(|reference| {
                reference
                    .resolved_path
                    .as_deref()
                    .is_some_and(|path| path.starts_with(".wiki-history/"))
            })
            .map(|reference| reference.target)
            .collect::<BTreeSet<_>>();
        Ok((bytes, references))
    })?;
    if references.is_empty() {
        return Err(refused("knowledge.invalid_citation"));
    }
    let generation = format!("sha256:{}", content_generation(&bytes));
    let mut sources = Vec::new();
    for reference in references {
        let source = workspace_knowledge_citation(input, page, &reference)?;
        if source.page_generation != generation || !source.current {
            return Err(conflict("knowledge.workspace_source_changed"));
        }
        sources.push(source);
    }
    Ok(WorkspaceKnowledgePage {
        body: String::from_utf8(bytes).map_err(io::Error::other)?,
        generation,
        sources,
    })
}

pub fn review_workspace_knowledge_change(
    input: &WorkspaceKnowledgeInput,
    id: &str,
    path: &str,
    generation: &str,
    accept: bool,
) -> io::Result<KnowledgePlan> {
    with_workspace(&input.output, &input.binding.revision, |root, ledger| {
        if ledger.active.as_ref().is_none_or(|job| {
            job.id != id || job.workspace_generation.as_deref() != Some(input.generation())
        }) {
            return Err(conflict("knowledge.workspace_source_changed"));
        }
        review(root, ledger, id, path, generation, accept, Some(input))
    })
}

#[cfg(test)]
mod tests;
