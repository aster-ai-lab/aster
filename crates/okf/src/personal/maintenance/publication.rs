//! A workspace generation commits through the existing ledger and history under its original lock.

use super::*;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Publication {
    job_id: String,
    state_generation: String,
    files: BTreeMap<String, PublicationFile>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PublicationFile {
    before: Option<String>,
    after: String,
}

/// Keeps directory/file readers outside an in-progress workspace publication.
#[derive(Debug)]
pub struct WorkspaceKnowledgeRead {
    _lock: File,
}

/// Ordinary directories and personal vaults keep their existing read behavior.
/// A workspace reader recovers an interrupted publication before admitting any bytes.
pub fn workspace_knowledge_read(root: &Path) -> io::Result<Option<WorkspaceKnowledgeRead>> {
    read(root, true)
}

/// Search refuses an interrupted generation without recovering on its query thread.
pub fn try_workspace_knowledge_read(root: &Path) -> io::Result<Option<WorkspaceKnowledgeRead>> {
    read(root, false)
}

fn read(root: &Path, recover: bool) -> io::Result<Option<WorkspaceKnowledgeRead>> {
    let path = match root.canonicalize() {
        Ok(root) => root,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let Some(canonical) = path.ancestors().take(MAX_DEPTH + 8).find(|candidate| {
        candidate
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|name| name == "workspace")
            && candidate
                .parent()
                .and_then(Path::parent)
                .and_then(Path::file_name)
                .is_some_and(|name| name == "wiki")
            && candidate.join(STATE).exists()
    }) else {
        return Ok(None);
    };
    for _ in 0..2 {
        let lock_path = canonical.join(".wiki-maintenance.lock");
        if fs::symlink_metadata(&lock_path)?.file_type().is_symlink() {
            return Err(refused("knowledge.symlink_refused"));
        }
        let lock = OpenOptions::new().read(true).write(true).open(lock_path)?;
        lock.try_lock_shared()
            .map_err(|_| io::Error::new(io::ErrorKind::WouldBlock, "knowledge.busy"))?;
        let ledger = load_ledger(&canonical)?;
        let binding = ledger
            .workspace_binding
            .as_ref()
            .ok_or_else(|| refused("knowledge.invalid_workspace_output"))?;
        if binding
            .source_root
            .join("wiki/workspace")
            .join(content_generation(binding.revision.as_bytes()))
            != canonical
        {
            return Err(refused("knowledge.invalid_workspace_output"));
        }
        if ledger.publication.is_none() {
            return Ok(Some(WorkspaceKnowledgeRead { _lock: lock }));
        }
        if !recover {
            return Err(io::Error::new(io::ErrorKind::WouldBlock, "knowledge.busy"));
        }
        let revision = binding.revision.clone();
        drop(lock);
        workspace::with_workspace(&canonical, &revision, |_, _| Ok(()))?;
    }
    Err(io::Error::new(io::ErrorKind::WouldBlock, "knowledge.busy"))
}

fn history(root: &Path, id: &str) -> io::Result<PathBuf> {
    job_directory(root, id)?;
    let path = root.join(".wiki-history").join(id);
    if path.canonicalize()? != path {
        return Err(refused("knowledge.symlink_refused"));
    }
    Ok(path)
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(super) fn recover(root: &Path, ledger: &mut Ledger) -> io::Result<()> {
    let Some(publication) = &ledger.publication else {
        return Ok(());
    };
    if ledger.workspace_binding.is_none()
        || publication.files.len() > MAX_FILES + 2
        || !valid_digest(&publication.state_generation)
        || publication.files.iter().any(|(path, item)| {
            (!page_path(path) && path != "index.md" && path != "log.md")
                || !valid_digest(&item.after)
                || item
                    .before
                    .as_ref()
                    .is_some_and(|value| !valid_digest(value))
        })
    {
        return Err(refused("knowledge.invalid_publication"));
    }
    let history = history(root, &publication.job_id)?;
    let state = history.join("publication").join(STATE);
    if state.canonicalize()? != state {
        return Err(refused("knowledge.symlink_refused"));
    }
    let baseline = read_bounded(&state, MAX_REPORT_BYTES)?;
    if content_generation(&baseline) != publication.state_generation {
        return Err(conflict("knowledge.history_changed"));
    }
    let mut restored: Ledger = serde_json::from_slice(&baseline).map_err(io::Error::other)?;
    if restored.version != 1
        || restored.publication.is_some()
        || restored.workspace_binding != ledger.workspace_binding
        || !valid_categories(&restored.categories)
        || restored.sources.keys().any(|path| !source_path(path))
        || restored
            .active
            .as_ref()
            .is_none_or(|job| job.id != publication.job_id)
    {
        return Err(refused("knowledge.invalid_publication"));
    }
    if let Some(job) = &restored.active {
        validate_job(root, job)?;
    }
    let mut originals = BTreeMap::new();
    let mut external_edit = false;
    for (path, entry) in &publication.files {
        let current = optional_bytes(&root.join(path))?.map(|bytes| content_generation(&bytes));
        let changed = current != entry.before && current.as_ref() != Some(&entry.after);
        let bytes = if let Some(generation) = &entry.before {
            let saved = history.join("before").join(path);
            if saved.canonicalize()? != saved {
                return Err(refused("knowledge.symlink_refused"));
            }
            let bytes = read_bounded(&saved, MAX_TEXT_BYTES)?;
            if &content_generation(&bytes) != generation {
                return Err(conflict("knowledge.history_changed"));
            }
            Some(bytes)
        } else {
            None
        };
        if changed {
            external_edit = true;
        } else {
            originals.insert(path.clone(), bytes);
        }
    }
    for (path, bytes) in originals {
        if let Some(bytes) = bytes {
            write_owned(&root.join(path), &bytes)?;
        } else if root.join(&path).exists() {
            fs::remove_file(root.join(&path))?;
            #[cfg(unix)]
            if let Some(parent) = root.join(&path).parent() {
                File::open(parent)?.sync_all()?;
            }
        }
    }
    if let Some(job) = &mut restored.active {
        job.error = Some(
            if external_edit {
                "knowledge.publication_conflict"
            } else {
                "knowledge.publication_interrupted"
            }
            .into(),
        );
    }
    save(root, &restored)?;
    *ledger = restored;
    Ok(())
}

pub(super) fn publish(
    root: &Path,
    ledger: &mut Ledger,
    input: &WorkspaceKnowledgeInput,
) -> io::Result<KnowledgePlan> {
    publish_with(root, ledger, input, |_| Ok(()))
}

pub(super) fn publish_with(
    root: &Path,
    ledger: &mut Ledger,
    input: &WorkspaceKnowledgeInput,
    mut after_write: impl FnMut(&str) -> io::Result<()>,
) -> io::Result<KnowledgePlan> {
    let mut job = ledger
        .active
        .clone()
        .ok_or_else(|| refused("knowledge.job_missing"))?;
    if job
        .changes
        .iter()
        .any(|change| change.decision == ChangeDecision::Pending)
    {
        save(root, ledger)?;
        return plan_with_inputs(root, ledger, Some(input));
    }
    let mut projected = snapshot(root)?;
    let mut outputs = BTreeMap::new();
    for change in job
        .changes
        .iter()
        .filter(|change| change.decision == ChangeDecision::Approved)
    {
        validate_dependencies_with_inputs(root, &job, change, Some(input))?;
        let body = candidate(root, &job, change)?.into_bytes();
        projected.insert(change.path.clone(), body.clone());
        outputs.insert(change.path.clone(), body);
    }
    if outputs.is_empty() {
        complete(root, ledger)?;
        return plan_with_inputs(root, ledger, Some(input));
    }
    let listed = projected
        .iter()
        .filter(|(path, _)| page_path(path))
        .map(|(path, bytes)| {
            let text = std::str::from_utf8(bytes).map_err(io::Error::other)?;
            let title = split_note(text)
                .inner
                .and_then(|inner| scalar(inner, "title"))
                .unwrap_or_else(|| path.clone());
            Ok((title, path.clone()))
        })
        .collect::<io::Result<Vec<_>>>()?;
    let index = render_index(&listed, &[], &[]).into_bytes();
    projected.insert("index.md".into(), index.clone());
    aster_use_core::inspect_okf_bundle(
        aster_use_core::OkfFormatVersion::V0_2,
        bundle_limits(),
        projected
            .into_iter()
            .filter(|(path, _)| page_path(path) || path == "index.md")
            .map(|(path, bytes)| aster_use_core::OkfBundleFile::new(path, bytes)),
    )
    .map_err(io::Error::other)?;
    outputs.insert("index.md".into(), index);
    let mut published_job = job.clone();
    for change in &mut published_job.changes {
        if change.decision == ChangeDecision::Approved {
            change.decision = ChangeDecision::Published;
        }
    }
    outputs.insert("log.md".into(), completion_log(root, &published_job)?);
    let history = root.join(".wiki-history").join(&job.id);
    create_owned_directory(&history.join("publication"))?;
    let baseline = encode(ledger)?;
    write_owned(&history.join("publication").join(STATE), &baseline)?;
    let mut files = BTreeMap::new();
    for path in outputs.keys() {
        let before = optional_bytes(&root.join(path))?;
        if let Some(bytes) = &before {
            write_owned(&history.join("before").join(path), bytes)?;
        }
        files.insert(
            path.clone(),
            PublicationFile {
                before: before.as_ref().map(|bytes| content_generation(bytes)),
                after: content_generation(&outputs[path]),
            },
        );
    }
    ledger.publication = Some(Publication {
        job_id: job.id.clone(),
        state_generation: content_generation(&baseline),
        files,
    });
    save(root, ledger)?;
    let result = (|| {
        for (path, bytes) in &outputs {
            write_owned(&root.join(path), bytes)?;
            after_write(path)?;
        }
        for change in &mut job.changes {
            if change.decision == ChangeDecision::Approved {
                change.decision = ChangeDecision::Published;
            }
        }
        ledger.active = Some(job.clone());
        complete(root, ledger)?;
        ledger.publication = None;
        save(root, ledger)
    })();
    if let Err(error) = result {
        if ledger.publication.is_none() {
            *ledger = load_ledger(root)?;
        }
        recover(root, ledger).map_err(|rollback| {
            io::Error::other(format!("{error}; publication recovery failed: {rollback}"))
        })?;
        return Err(error);
    }
    if let Ok(staging) = stage_path(root, &job) {
        let _ = fs::remove_dir_all(staging);
    }
    plan_with_inputs(root, ledger, Some(input))
}
