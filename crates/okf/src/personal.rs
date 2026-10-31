//! Personal note bundle. The directory itself is the OKF v0.2 root.
//! Handwritten notes stay on their current paths. Concept pages land in `concepts/`.
//! A model does not write the prose.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use aster_use_core::{inspect_okf_bundle, OkfBundleFile, OkfBundleLimits, OkfFormatVersion};

use crate::{
    compile_concept, concept_file_name, content_generation, extract_document, published_generation,
    published_parser, render_index, source_title, write_atomic, yaml_quote, CompileOutcome,
    ConceptRequest, ExtractPolicy, Extracted, SkipReason, TextPassage, WorkspaceReport, BODY_CAP,
    MAX_DEPTH, MAX_PACKAGE_BYTES, MAX_TEXT_BYTES, OKF_INDEX, PARSER_GENERATION,
};

const CONCEPTS_DIR: &str = "concepts";

mod maintenance;
pub use maintenance::{
    abandon_knowledge_maintenance, begin_knowledge_maintenance, cancel_knowledge_maintenance,
    fail_knowledge_maintenance, fail_workspace_knowledge_job, finish_knowledge_maintenance,
    knowledge_change, knowledge_compile_workspace, knowledge_maintenance_job,
    knowledge_maintenance_plan, knowledge_page_path, review_knowledge_change,
    try_workspace_knowledge_read, with_knowledge_source_mutation, workspace_knowledge_read,
    KnowledgeChangeDetail, KnowledgeJob, KnowledgePlan, KnowledgeSourceMutationBlocked,
    MaintenanceOperation, WorkspaceKnowledgeRead,
};
pub use maintenance::{
    abandon_workspace_knowledge_job, abandon_workspace_knowledge_maintenance,
    begin_workspace_knowledge_maintenance, cancel_workspace_knowledge_job,
    cancel_workspace_knowledge_maintenance, fail_workspace_knowledge_maintenance,
    finish_workspace_knowledge_maintenance, retained_workspace_document_projection,
    retire_workspace_knowledge, reuse_workspace_knowledge_maintenance,
    review_workspace_knowledge_change, workspace_knowledge_change, workspace_knowledge_citation,
    workspace_knowledge_compile_workspace, workspace_knowledge_maintenance_enabled,
    workspace_knowledge_maintenance_job, workspace_knowledge_maintenance_plan,
    workspace_knowledge_output_root, workspace_knowledge_page, workspace_knowledge_summary,
    WorkspaceKnowledgeCitation, WorkspaceKnowledgeInput, WorkspaceKnowledgePage,
    WorkspaceKnowledgeSource, WorkspaceKnowledgeStatus, WorkspaceKnowledgeSummary,
};

/// `wiki/local` is the only directory the personal compile command accepts.
pub fn is_personal_vault(root: &Path) -> bool {
    let Some(name) = root.file_name().and_then(|item| item.to_str()) else {
        return false;
    };
    let Some(parent) = root
        .parent()
        .and_then(|item| item.file_name())
        .and_then(|item| item.to_str())
    else {
        return false;
    };
    name == "local" && parent == "wiki"
}

/// Compile one note directory into an OKF bundle in place.
///
/// Each note is written before its callback. A note that fails inspection is not written
/// and does not notify; notes already written stay. The index is replaced only after
/// every note has been accepted.
pub fn publish_note_bundle(root: &Path) -> io::Result<WorkspaceReport> {
    publish_note_bundle_with(root, &mut |_, _| {})
}

/// Same publish as [`publish_note_bundle`]. `on_file` runs once per note after that note's
/// bytes are on disk, and before later notes are planned.
pub fn publish_note_bundle_with(
    root: &Path,
    on_file: &mut dyn FnMut(&str, &CompileOutcome),
) -> io::Result<WorkspaceReport> {
    if !root.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "note bundle missing",
        ));
    }
    let files = collect_files(root)?;
    let planned = plan_bundle(&files, Some(root), on_file)?;
    // Inspect what the planner produced, not every file on disk. A note skipped for being
    // empty, oversized, or too deep keeps its raw bytes and must not be read as a concept:
    // one empty note would otherwise reject the whole bundle. A note the planner marked
    // failed keeps its raw bytes in `notes`, so a malformed one still rejects the publish.
    let mut snapshot = planned.notes.clone();
    for (relative, bytes) in &planned.concepts {
        snapshot.insert(relative.clone(), bytes.clone());
    }
    snapshot.insert(OKF_INDEX.to_string(), planned.index.clone());
    let bundle_files = snapshot
        .iter()
        .map(|(path, content)| OkfBundleFile::new(path.clone(), content.clone()))
        .collect::<Vec<_>>();
    // Notes and concept pages are already on disk. A bundle-level rejection still leaves
    // them there; only the index and unrelated stale concepts wait for this check.
    inspect_okf_bundle(OkfFormatVersion::V0_2, bundle_limits(), bundle_files)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    remove_stale_concepts(root, &planned.keep)?;
    write_if_changed(
        root,
        OKF_INDEX,
        &planned.index,
        files.get(OKF_INDEX).map(Vec::as_slice),
    )?;
    Ok(planned.report)
}

/// Replace `index.md` from the notes and concept pages already on disk.
///
/// Concept pages and note bodies stay byte-for-byte. A rejected inspection leaves the
/// previous index. This does not publish prose.
pub fn accept_personal_bundle(root: &Path) -> io::Result<WorkspaceReport> {
    if !is_personal_vault(root) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "personal knowledge compile only accepts wiki/local",
        ));
    }
    if !root.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "note bundle missing",
        ));
    }
    let files = collect_files(root)?;
    let projected = project_accept(&files);
    let mut snapshot = BTreeMap::new();
    snapshot.insert(OKF_INDEX.to_string(), projected.index.clone());
    // Notes stay out of this check. Many have no frontmatter, and stamping them would change bytes.
    for (path, bytes) in &files {
        if is_direct_concept(path) {
            snapshot.insert(path.clone(), bytes.clone());
        }
    }
    let bundle_files = snapshot
        .iter()
        .map(|(path, content)| OkfBundleFile::new(path.clone(), content.clone()))
        .collect::<Vec<_>>();
    inspect_okf_bundle(OkfFormatVersion::V0_2, bundle_limits(), bundle_files)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    write_if_changed(
        root,
        OKF_INDEX,
        &projected.index,
        files.get(OKF_INDEX).map(Vec::as_slice),
    )?;
    Ok(projected.report)
}

/// Relative note paths that still need a concept page.
///
/// The note file is not rewritten. Generation is the hash of the stamped note in memory.
/// A concept page counts as current only when its frontmatter `source_generation` matches
/// that hash and its type is Document. A page without that field is pending.
pub fn plan_personal_compile(root: &Path) -> io::Result<Vec<String>> {
    require_vault(root)?;
    let files = collect_files(root)?;
    let mut pending = Vec::new();
    for work in compile_candidates(&files) {
        let existing = files
            .get(&work.concept_relative)
            .and_then(|bytes| std::str::from_utf8(bytes).ok());
        if existing.is_some_and(|existing| agent_current(existing, &work.generation)) {
            continue;
        }
        pending.push(work.relative);
    }
    Ok(pending)
}

/// Copy notes and concept pages the agent must not change.
///
/// The copy lives outside the vault. Concept pages for `pending` are left unpinned
/// so the agent may rewrite them. A later [`release_personal_compile`] puts the
/// pinned bytes back.
pub fn pin_personal_compile(root: &Path, pending: &[String]) -> io::Result<()> {
    require_vault(root)?;
    let files = collect_files(root)?;
    let open_concepts = pending_concepts(pending);
    let pin = pin_dir(root)?;
    if pin.exists() {
        fs::remove_dir_all(&pin)?;
    }
    fs::create_dir_all(&pin)?;
    for (relative, bytes) in &files {
        if !safe_relative(relative) {
            continue;
        }
        let pinned =
            is_note(relative) || (is_direct_concept(relative) && !open_concepts.contains(relative));
        if !pinned {
            continue;
        }
        let dest = pin.join(relative);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&dest, bytes)?;
        if is_note(relative) {
            let modified = fs::metadata(bundle_path(root, relative))?.modified()?;
            fs::File::options()
                .write(true)
                .open(&dest)?
                .set_modified(modified)?;
        }
    }
    Ok(())
}

/// Restore pinned notes and concept pages, then delete the pin.
///
/// Missing pin is a no-op. Concept pages for the pending notes are not in the pin.
/// A note whose modification time is later than the pinned copy is left as the user
/// wrote it. Concept pages outside the plan are still restored.
pub fn release_personal_compile(root: &Path) -> io::Result<()> {
    require_vault(root)?;
    let pin = pin_dir(root)?;
    if !pin.is_dir() {
        return Ok(());
    }
    restore_tree(root, &pin, &pin)?;
    fs::remove_dir_all(pin)?;
    Ok(())
}

/// Record `source_generation` on concept pages the agent wrote for `pending`.
///
/// The prose after the frontmatter stays byte-for-byte. Notes are not written.
/// A concept page without `type: Document` is left alone.
pub fn remember_personal_generations(root: &Path, pending: &[String]) -> io::Result<()> {
    require_vault(root)?;
    let files = collect_files(root)?;
    for relative in pending {
        if !is_note(relative) || relative.split('/').any(|segment| segment == "..") {
            continue;
        }
        let Some(raw) = files.get(relative) else {
            continue;
        };
        let Ok(text) = std::str::from_utf8(raw) else {
            continue;
        };
        let file_name = relative.rsplit('/').next().unwrap_or(relative.as_str());
        let Some(stamped_note) = stamp_note(text, &source_title(file_name)) else {
            continue;
        };
        let generation = content_generation(stamped_note.as_bytes());
        let concept_relative = format!("{CONCEPTS_DIR}/{}", concept_file_name(relative));
        let Some(bytes) = files.get(&concept_relative) else {
            continue;
        };
        let Ok(existing) = std::str::from_utf8(bytes) else {
            continue;
        };
        if !concept_type_is_document(existing) {
            continue;
        }
        if published_generation(existing) == Some(generation.as_str()) {
            continue;
        }
        let Some(stamped) = stamp_source_generation(existing, &generation) else {
            continue;
        };
        write_if_changed(root, &concept_relative, stamped.as_bytes(), Some(bytes))?;
    }
    Ok(())
}

fn require_vault(root: &Path) -> io::Result<()> {
    if !is_personal_vault(root) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "personal knowledge compile only accepts wiki/local",
        ));
    }
    if !root.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "note bundle missing",
        ));
    }
    Ok(())
}

struct NoteWork {
    relative: String,
    generation: String,
    concept_relative: String,
}

/// Notes accept would count as having substance. Generation is not written onto the note.
fn compile_candidates(files: &BTreeMap<String, Vec<u8>>) -> Vec<NoteWork> {
    let cap = bundle_limits().max_concepts as usize;
    let mut used_names = BTreeSet::new();
    let mut work = Vec::new();
    let note_paths = files
        .keys()
        .filter(|path| is_note(path))
        .cloned()
        .collect::<Vec<_>>();
    for (index, relative) in note_paths.into_iter().enumerate() {
        let Some(raw) = files.get(&relative) else {
            continue;
        };
        let Ok(text) = std::str::from_utf8(raw) else {
            continue;
        };
        if split_note(text).unterminated {
            continue;
        }
        if dir_depth(&relative) > MAX_DEPTH || raw.len() > MAX_TEXT_BYTES || index >= cap {
            continue;
        }
        if note_body(text).trim().is_empty() {
            continue;
        }
        let concept_name = concept_file_name(&relative);
        if !used_names.insert(concept_name.clone()) {
            continue;
        }
        let file_name = relative.rsplit('/').next().unwrap_or(relative.as_str());
        let Some(stamped) = stamp_note(text, &source_title(file_name)) else {
            continue;
        };
        work.push(NoteWork {
            relative,
            generation: content_generation(stamped.as_bytes()),
            concept_relative: format!("{CONCEPTS_DIR}/{concept_name}"),
        });
    }
    work
}

fn agent_current(existing: &str, generation: &str) -> bool {
    published_generation(existing) == Some(generation) && concept_type_is_document(existing)
}

fn concept_type_is_document(markdown: &str) -> bool {
    let parts = split_note(markdown);
    if parts.unterminated {
        return false;
    }
    parts
        .inner
        .and_then(|inner| scalar(inner, "type"))
        .as_deref()
        == Some("Document")
}

fn pending_concepts(pending: &[String]) -> BTreeSet<String> {
    pending
        .iter()
        .filter(|relative| safe_relative(relative) && is_note(relative))
        .map(|relative| format!("{CONCEPTS_DIR}/{}", concept_file_name(relative)))
        .collect()
}

fn safe_relative(relative: &str) -> bool {
    !relative.is_empty()
        && !relative.starts_with('/')
        && !relative
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
}

fn pin_dir(root: &Path) -> io::Result<PathBuf> {
    let canonical = root.canonicalize()?;
    let digest = content_generation(canonical.to_string_lossy().as_bytes());
    Ok(std::env::temp_dir().join("aster-okf-pin").join(digest))
}

fn note_written_after_pin(vault: &Path, relative: &str, pinned: &fs::Metadata) -> io::Result<bool> {
    let Ok(pinned_modified) = pinned.modified() else {
        return Ok(false);
    };
    let Ok(live) = fs::metadata(bundle_path(vault, relative)) else {
        return Ok(false);
    };
    let Ok(live_modified) = live.modified() else {
        return Ok(false);
    };
    Ok(live_modified > pinned_modified)
}

fn restore_tree(vault: &Path, pin_root: &Path, directory: &Path) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            restore_tree(vault, pin_root, &path)?;
            continue;
        }
        if !metadata.is_file() {
            continue;
        }
        let Ok(relative) = path.strip_prefix(pin_root) else {
            continue;
        };
        let relative = relative.to_string_lossy().replace('\\', "/");
        if !safe_relative(&relative) {
            continue;
        }
        if is_note(&relative) && note_written_after_pin(vault, &relative, &metadata)? {
            continue;
        }
        let bytes = fs::read(&path)?;
        let current = fs::read(bundle_path(vault, &relative)).ok();
        write_if_changed(vault, &relative, &bytes, current.as_deref())?;
    }
    Ok(())
}

/// Insert or replace `source_generation` in the frontmatter. The body slice stays intact.
fn stamp_source_generation(markdown: &str, generation: &str) -> Option<String> {
    let parts = split_note(markdown);
    if parts.unterminated {
        return None;
    }
    let inner = parts.inner?;
    if scalar(inner, "type").as_deref() != Some("Document") {
        return None;
    }
    let quoted = yaml_quote(generation);
    let mut next = String::new();
    let mut replaced = false;
    for line in inner.split_inclusive('\n') {
        let trimmed = trim_line(line).trim();
        if trimmed.starts_with("source_generation:") {
            if !replaced {
                let newline = if line.ends_with("\r\n") {
                    "\r\n"
                } else if line.ends_with('\n') {
                    "\n"
                } else {
                    ""
                };
                next.push_str("source_generation: ");
                next.push_str(&quoted);
                next.push_str(newline);
                replaced = true;
            }
            continue;
        }
        next.push_str(line);
    }
    if !replaced {
        if !next.is_empty() && !next.ends_with('\n') {
            next.push('\n');
        }
        next.push_str("source_generation: ");
        next.push_str(&quoted);
        next.push('\n');
    }
    Some(format!("---\n{next}---\n{}", parts.body))
}

struct AcceptedIndex {
    index: Vec<u8>,
    report: WorkspaceReport,
}

fn project_accept(files: &BTreeMap<String, Vec<u8>>) -> AcceptedIndex {
    let cap = bundle_limits().max_concepts as usize;
    let mut listed = Vec::new();
    let mut skipped = Vec::new();
    let mut failed = Vec::new();
    let mut outcomes = Vec::new();
    let mut used_names = BTreeSet::new();
    let note_paths = files
        .keys()
        .filter(|path| is_note(path))
        .cloned()
        .collect::<Vec<_>>();

    for (index, relative) in note_paths.iter().enumerate() {
        let raw = &files[relative];
        let file_name = relative.rsplit('/').next().unwrap_or(relative.as_str());
        let Ok(text) = std::str::from_utf8(raw) else {
            failed.push((relative.clone(), "encoding".to_string()));
            outcomes.push((relative.clone(), CompileOutcome::Failed));
            continue;
        };
        if split_note(text).unterminated {
            failed.push((relative.clone(), "frontmatter".to_string()));
            outcomes.push((relative.clone(), CompileOutcome::Failed));
            continue;
        }
        if dir_depth(relative) > MAX_DEPTH {
            skipped.push((relative.clone(), SkipReason::OverDepth));
            outcomes.push((
                relative.clone(),
                CompileOutcome::Skipped(SkipReason::OverDepth),
            ));
            continue;
        }
        if raw.len() > MAX_TEXT_BYTES {
            skipped.push((relative.clone(), SkipReason::OverSize));
            outcomes.push((
                relative.clone(),
                CompileOutcome::Skipped(SkipReason::OverSize),
            ));
            continue;
        }
        if index >= cap {
            skipped.push((relative.clone(), SkipReason::OverCount));
            outcomes.push((
                relative.clone(),
                CompileOutcome::Skipped(SkipReason::OverCount),
            ));
            continue;
        }
        if note_body(text).trim().is_empty() {
            skipped.push((relative.clone(), SkipReason::Empty));
            outcomes.push((relative.clone(), CompileOutcome::Skipped(SkipReason::Empty)));
            continue;
        }
        let concept_name = concept_file_name(relative);
        let concept_relative = format!("{CONCEPTS_DIR}/{concept_name}");
        if !used_names.insert(concept_name) {
            failed.push((relative.clone(), "concept name already used".to_string()));
            outcomes.push((relative.clone(), CompileOutcome::Failed));
            continue;
        }
        if files.contains_key(&concept_relative) {
            listed.push((note_title(text, file_name), concept_relative));
            outcomes.push((relative.clone(), CompileOutcome::Unchanged));
        } else {
            failed.push((relative.clone(), "missing concept".to_string()));
            outcomes.push((relative.clone(), CompileOutcome::Failed));
        }
    }

    AcceptedIndex {
        index: render_index(&listed, &skipped, &failed).into_bytes(),
        report: WorkspaceReport {
            compiled: 0,
            unchanged: listed.len(),
            skipped: skipped.len(),
            failed: failed.len(),
            outcomes,
            failure_reasons: failed,
        },
    }
}

struct PlannedBundle {
    notes: BTreeMap<String, Vec<u8>>,
    concepts: BTreeMap<String, Vec<u8>>,
    keep: BTreeSet<String>,
    index: Vec<u8>,
    report: WorkspaceReport,
}

fn plan_bundle(
    files: &BTreeMap<String, Vec<u8>>,
    root: Option<&Path>,
    on_file: &mut dyn FnMut(&str, &CompileOutcome),
) -> io::Result<PlannedBundle> {
    plan_notes(files, bundle_limits().max_concepts as usize, root, on_file)
}

fn plan_notes(
    files: &BTreeMap<String, Vec<u8>>,
    note_cap: usize,
    root: Option<&Path>,
    on_file: &mut dyn FnMut(&str, &CompileOutcome),
) -> io::Result<PlannedBundle> {
    let mut notes = BTreeMap::new();
    let mut concepts = BTreeMap::new();
    let mut keep = BTreeSet::new();
    let mut listed = Vec::new();
    let mut skipped = Vec::new();
    let mut failed = Vec::new();
    let mut outcomes = Vec::new();
    let mut compiled = 0;
    let mut unchanged = 0;
    let mut used_names = BTreeSet::new();
    let note_paths = files
        .keys()
        .filter(|path| is_note(path))
        .cloned()
        .collect::<Vec<_>>();

    for (index, relative) in note_paths.iter().enumerate() {
        let raw = &files[relative];
        let file_name = relative.rsplit('/').next().unwrap_or(relative.as_str());
        let Ok(text) = std::str::from_utf8(raw) else {
            notes.insert(relative.clone(), raw.clone());
            failed.push((relative.clone(), "encoding".to_string()));
            record(
                root,
                files,
                &notes,
                &concepts,
                &mut outcomes,
                on_file,
                relative,
                CompileOutcome::Failed,
            )?;
            continue;
        };
        let stamped = match stamp_note(text, &source_title(file_name)) {
            Some(stamped) => stamped,
            None => {
                notes.insert(relative.clone(), raw.clone());
                failed.push((relative.clone(), "frontmatter".to_string()));
                record(
                    root,
                    files,
                    &notes,
                    &concepts,
                    &mut outcomes,
                    on_file,
                    relative,
                    CompileOutcome::Failed,
                )?;
                continue;
            }
        };
        let title = note_title(&stamped, file_name);
        let body = note_body(&stamped).to_string();
        let generation = content_generation(stamped.as_bytes());
        notes.insert(relative.clone(), stamped.into_bytes());

        if index >= note_cap {
            skip(
                root,
                files,
                &notes,
                &concepts,
                &mut skipped,
                &mut outcomes,
                on_file,
                relative,
                SkipReason::OverCount,
            )?;
            continue;
        }
        if dir_depth(relative) > MAX_DEPTH {
            skip(
                root,
                files,
                &notes,
                &concepts,
                &mut skipped,
                &mut outcomes,
                on_file,
                relative,
                SkipReason::OverDepth,
            )?;
            continue;
        }
        if raw.len() > MAX_TEXT_BYTES {
            skip(
                root,
                files,
                &notes,
                &concepts,
                &mut skipped,
                &mut outcomes,
                on_file,
                relative,
                SkipReason::OverSize,
            )?;
            continue;
        }
        let concept_name = concept_file_name(relative);
        let concept_relative = format!("{CONCEPTS_DIR}/{concept_name}");
        if !used_names.insert(concept_name) {
            failed.push((relative.clone(), "concept name already used".to_string()));
            record(
                root,
                files,
                &notes,
                &concepts,
                &mut outcomes,
                on_file,
                relative,
                CompileOutcome::Failed,
            )?;
            continue;
        }
        if let Some(existing) = files
            .get(&concept_relative)
            .and_then(|bytes| String::from_utf8(bytes.clone()).ok())
            .filter(|existing| reusable(existing, &generation))
        {
            concepts.insert(concept_relative.clone(), existing.into_bytes());
            keep.insert(concept_relative.clone());
            listed.push((title, concept_relative));
            record(
                root,
                files,
                &notes,
                &concepts,
                &mut outcomes,
                on_file,
                relative,
                CompileOutcome::Unchanged,
            )?;
            unchanged += 1;
            continue;
        }
        match compile_body(relative, &title, &generation, &body) {
            BodyOutcome::Page(page) => {
                concepts.insert(concept_relative.clone(), page.into_bytes());
                keep.insert(concept_relative.clone());
                listed.push((title, concept_relative));
                record(
                    root,
                    files,
                    &notes,
                    &concepts,
                    &mut outcomes,
                    on_file,
                    relative,
                    CompileOutcome::Compiled,
                )?;
                compiled += 1;
            }
            BodyOutcome::Skip(reason) => skip(
                root,
                files,
                &notes,
                &concepts,
                &mut skipped,
                &mut outcomes,
                on_file,
                relative,
                reason,
            )?,
            BodyOutcome::Fail(message) => {
                if let Some(existing) = files.get(&concept_relative) {
                    concepts.insert(concept_relative.clone(), existing.clone());
                    keep.insert(concept_relative.clone());
                    listed.push((title, concept_relative));
                }
                failed.push((relative.clone(), message));
                record(
                    root,
                    files,
                    &notes,
                    &concepts,
                    &mut outcomes,
                    on_file,
                    relative,
                    CompileOutcome::Failed,
                )?;
            }
        }
    }

    Ok(PlannedBundle {
        notes,
        concepts,
        keep,
        index: render_index(&listed, &skipped, &failed).into_bytes(),
        report: WorkspaceReport {
            compiled,
            unchanged,
            skipped: skipped.len(),
            failed: failed.len(),
            outcomes,
            failure_reasons: failed,
        },
    })
}

fn record(
    root: Option<&Path>,
    files: &BTreeMap<String, Vec<u8>>,
    notes: &BTreeMap<String, Vec<u8>>,
    concepts: &BTreeMap<String, Vec<u8>>,
    outcomes: &mut Vec<(String, CompileOutcome)>,
    on_file: &mut dyn FnMut(&str, &CompileOutcome),
    relative: &str,
    outcome: CompileOutcome,
) -> io::Result<()> {
    if let Some(root) = root {
        persist_note(root, files, notes, concepts, relative)?;
    }
    on_file(relative, &outcome);
    outcomes.push((relative.to_string(), outcome));
    Ok(())
}

fn skip(
    root: Option<&Path>,
    files: &BTreeMap<String, Vec<u8>>,
    notes: &BTreeMap<String, Vec<u8>>,
    concepts: &BTreeMap<String, Vec<u8>>,
    skipped: &mut Vec<(String, SkipReason)>,
    outcomes: &mut Vec<(String, CompileOutcome)>,
    on_file: &mut dyn FnMut(&str, &CompileOutcome),
    relative: &str,
    reason: SkipReason,
) -> io::Result<()> {
    skipped.push((relative.to_string(), reason));
    record(
        root,
        files,
        notes,
        concepts,
        outcomes,
        on_file,
        relative,
        CompileOutcome::Skipped(reason),
    )
}

/// Write this note, and its concept page when this outcome keeps one, before the caller notifies.
fn persist_note(
    root: &Path,
    files: &BTreeMap<String, Vec<u8>>,
    notes: &BTreeMap<String, Vec<u8>>,
    concepts: &BTreeMap<String, Vec<u8>>,
    relative: &str,
) -> io::Result<()> {
    let note_bytes = notes
        .get(relative)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "planned note missing"))?;
    let concept_relative = format!("{CONCEPTS_DIR}/{}", concept_file_name(relative));
    let concept_bytes = concepts.get(&concept_relative);
    let mut bundle = Vec::with_capacity(2);
    bundle.push(OkfBundleFile::new(relative.to_string(), note_bytes.clone()));
    if let Some(bytes) = concept_bytes {
        bundle.push(OkfBundleFile::new(concept_relative.clone(), bytes.clone()));
    }
    inspect_okf_bundle(OkfFormatVersion::V0_2, bundle_limits(), bundle)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    write_if_changed(
        root,
        relative,
        note_bytes,
        files.get(relative).map(Vec::as_slice),
    )?;
    if let Some(bytes) = concept_bytes {
        write_if_changed(
            root,
            &concept_relative,
            bytes,
            files.get(&concept_relative).map(Vec::as_slice),
        )?;
    } else if files.contains_key(&concept_relative) {
        let path = bundle_path(root, &concept_relative);
        if path.is_file() {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

enum BodyOutcome {
    Page(String),
    Skip(SkipReason),
    Fail(String),
}

fn compile_body(relative: &str, title: &str, generation: &str, body: &str) -> BodyOutcome {
    let policy = ExtractPolicy {
        recognize: None,
        max_text_bytes: MAX_TEXT_BYTES,
        max_package_bytes: MAX_PACKAGE_BYTES,
        max_output: BODY_CAP,
        deadline: Instant::now() + Duration::from_secs(5),
    };
    let extracted = extract_document(relative, body.as_bytes(), &policy);
    let Extracted::Parsed { blocks, .. } = extracted else {
        return match extracted {
            Extracted::Unparsed(reason) => BodyOutcome::Skip(reason),
            Extracted::Failed(message) => BodyOutcome::Fail(message),
            Extracted::Parsed { .. } => unreachable!(),
        };
    };
    let owned = crate::passage_bodies(&blocks);
    let passages = owned
        .iter()
        .map(|(heading, body)| TextPassage { heading, body })
        .collect::<Vec<_>>();
    // Concept files live in concepts/, so the note link climbs one directory.
    // A same-directory link would resolve onto the concept page itself.
    let resource = format!("../{relative}");
    match compile_concept(&ConceptRequest {
        title,
        file_name: title,
        content_type: "text/markdown",
        resource: &resource,
        source_generation: generation,
        passages: &passages,
    }) {
        Some(page) => BodyOutcome::Page(page.markdown),
        None => BodyOutcome::Skip(SkipReason::Empty),
    }
}

fn reusable(existing: &str, generation: &str) -> bool {
    published_generation(existing) == Some(generation)
        && published_parser(existing) == Some(PARSER_GENERATION)
        && existing.contains("type: Document")
}

fn bundle_limits() -> OkfBundleLimits {
    OkfBundleLimits {
        max_files: 4_096,
        max_concepts: 4_096,
        max_expanded_bytes: 512 * 1024 * 1024,
        max_document_bytes: 8 * 1024 * 1024,
        max_links_per_document: 16_384,
    }
}

fn is_note(relative: &str) -> bool {
    if !relative.ends_with(".md") || relative.starts_with(&format!("{CONCEPTS_DIR}/")) {
        return false;
    }
    let name = relative.rsplit('/').next().unwrap_or(relative);
    name != OKF_INDEX && name != "log.md"
}

fn is_direct_concept(relative: &str) -> bool {
    let Some(name) = relative.strip_prefix(&format!("{CONCEPTS_DIR}/")) else {
        return false;
    };
    !name.is_empty() && !name.contains('/') && name.ends_with(".md")
}

fn dir_depth(relative: &str) -> usize {
    relative.matches('/').count()
}

fn stamp_note(original: &str, fallback_title: &str) -> Option<String> {
    let parts = split_note(original);
    if parts.unterminated {
        return None;
    }
    let Some(inner) = parts.inner else {
        return Some(format!(
            "---\ntype: Note\ntitle: {}\n---\n{original}",
            yaml_quote(fallback_title)
        ));
    };
    if scalar(inner, "type").is_some() {
        return Some(original.to_string());
    }
    let mut next = String::from("type: Note\n");
    if scalar(inner, "title").is_none() {
        next.push_str("title: ");
        next.push_str(&yaml_quote(fallback_title));
        next.push('\n');
    }
    next.push_str(inner);
    Some(format!("---\n{next}---\n{}", parts.body))
}

fn note_title(text: &str, file_name: &str) -> String {
    if let Some(inner) = split_note(text).inner {
        if let Some(title) = scalar(inner, "title") {
            return title.chars().take(200).collect();
        }
    }
    source_title(file_name)
}

fn note_body<'a>(text: &'a str) -> &'a str {
    match split_note(text).inner {
        Some(_) => split_note(text).body,
        None => text,
    }
}

struct NoteParts<'a> {
    inner: Option<&'a str>,
    body: &'a str,
    unterminated: bool,
}

fn split_note(text: &str) -> NoteParts<'_> {
    let mut lines = text.split_inclusive('\n');
    let Some(first) = lines.next() else {
        return NoteParts {
            inner: None,
            body: text,
            unterminated: false,
        };
    };
    if trim_line(first) != "---" {
        return NoteParts {
            inner: None,
            body: text,
            unterminated: false,
        };
    }
    let start = first.len();
    let mut offset = start;
    for line in lines {
        if trim_line(line) == "---" {
            let body_start = offset + line.len();
            return NoteParts {
                inner: Some(&text[start..offset]),
                body: &text[body_start..],
                unterminated: false,
            };
        }
        offset += line.len();
    }
    NoteParts {
        inner: None,
        body: text,
        unterminated: true,
    }
}

fn scalar(inner: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}:");
    for line in inner.lines() {
        let Some(rest) = line.trim().strip_prefix(&prefix) else {
            continue;
        };
        let value = unquote_scalar(rest.trim());
        if !value.is_empty() {
            return Some(value);
        }
    }
    None
}

fn unquote_scalar(value: &str) -> String {
    if let Some(inner) = value
        .strip_prefix('"')
        .and_then(|item| item.strip_suffix('"'))
    {
        return inner.replace("\\\\", "\\").replace("\\\"", "\"");
    }
    if let Some(inner) = value
        .strip_prefix('\'')
        .and_then(|item| item.strip_suffix('\''))
    {
        return inner.to_string();
    }
    value.to_string()
}

fn trim_line(line: &str) -> &str {
    line.trim_end_matches(['\r', '\n'])
}

fn collect_files(root: &Path) -> io::Result<BTreeMap<String, Vec<u8>>> {
    let mut files = BTreeMap::new();
    walk_files(root, root, &mut files)?;
    Ok(files)
}

fn walk_files(
    root: &Path,
    directory: &Path,
    out: &mut BTreeMap<String, Vec<u8>>,
) -> io::Result<()> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(directory)? {
        entries.push(entry?);
    }
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            walk_files(root, &path, out)?;
            continue;
        }
        if !metadata.is_file() {
            continue;
        }
        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };
        let relative = relative.to_string_lossy().replace('\\', "/");
        out.insert(relative, fs::read(&path)?);
    }
    Ok(())
}

fn write_if_changed(
    root: &Path,
    relative: &str,
    bytes: &[u8],
    previous: Option<&[u8]>,
) -> io::Result<()> {
    if previous == Some(bytes) {
        return Ok(());
    }
    let path = bundle_path(root, relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    write_atomic(&path, bytes)
}

fn bundle_path(root: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(root.to_path_buf(), |path, segment| path.join(segment))
}

fn remove_stale_concepts(root: &Path, keep: &BTreeSet<String>) -> io::Result<()> {
    let directory = root.join(CONCEPTS_DIR);
    if !directory.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(&directory)? {
        let entry = entry?;
        let path = entry.path();
        if fs::symlink_metadata(&path)?.file_type().is_symlink() || !path.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.ends_with(".md") {
            continue;
        }
        let relative = format!("{CONCEPTS_DIR}/{name}");
        if !keep.contains(&relative) {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use aster_use_core::{inspect_okf_bundle, OkfBundleFile, OkfDiagnosticCode, OkfFormatVersion};

    use super::*;
    use crate::published_generation;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!("aster-okf-personal-{nanos}-{n}"));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn body_after_frontmatter(text: &str) -> &str {
        let parts = split_note(text);
        assert!(parts.inner.is_some(), "frontmatter missing");
        assert!(!parts.unterminated);
        parts.body
    }

    fn inspect_tree(root: &Path) {
        let files = collect_files(root).unwrap();
        let bundle = files
            .iter()
            .map(|(path, content)| OkfBundleFile::new(path.clone(), content.clone()))
            .collect::<Vec<_>>();
        inspect_okf_bundle(OkfFormatVersion::V0_2, bundle_limits(), bundle)
            .unwrap_or_else(|error| panic!("{error}"));
    }

    #[test]
    fn a_note_becomes_a_document_inside_the_same_bundle() {
        let scratch = Scratch::new();
        let root = scratch.path();
        let original = "句子仍在。\n\n[[乙]]\n";
        fs::write(root.join("甲.md"), original).unwrap();
        fs::create_dir(root.join("目录")).unwrap();
        fs::write(root.join("目录").join("乙.md"), "嵌套句子。\n").unwrap();
        let titled = "---\ntitle: 自定义\n---\n正文不变\n";
        fs::write(root.join("已有.md"), titled).unwrap();
        let typed = "---\ntype: Notebook\ntitle: 手册\n---\n手册正文\n";
        fs::write(root.join("手册.md"), typed).unwrap();
        fs::write(root.join("空.md"), "\n").unwrap();

        let report = publish_note_bundle(root).unwrap();
        assert_eq!(report.compiled, 4);
        assert_eq!(report.unchanged, 0);
        assert_eq!(report.failed, 0);
        assert!(report.outcomes.iter().any(|(name, outcome)| {
            name == "空.md" && *outcome == CompileOutcome::Skipped(SkipReason::Empty)
        }));

        let stamped = fs::read_to_string(root.join("甲.md")).unwrap();
        assert_eq!(body_after_frontmatter(&stamped), original);
        assert!(stamped.contains("type: Note"));
        assert!(stamped.contains("[[乙]]"));
        let generation = content_generation(stamped.as_bytes());

        let concept = fs::read_to_string(root.join("concepts").join("甲.md")).unwrap();
        assert!(concept.contains("type: Document"));
        assert!(concept.contains("resource: \"../甲.md\""));
        assert!(concept.contains(&format!("source_generation: \"{generation}\"")));
        assert_eq!(crate::published_parser(&concept), Some(PARSER_GENERATION));
        assert!(concept.contains("by: aster-okf-compiler/0.2"));
        assert!(concept.contains("句子仍在。"));
        assert!(concept.contains("[[乙]]"));
        assert!(!concept.contains("type: Note"));
        assert!(concept.contains("](../甲.md)"));

        let nested = fs::read_to_string(root.join("concepts").join("目录__乙.md")).unwrap();
        assert!(nested.contains("resource: \"../目录/乙.md\""));
        assert!(nested.contains("嵌套句子。"));

        let kept_title = fs::read_to_string(root.join("已有.md")).unwrap();
        assert!(kept_title.starts_with("---\ntype: Note\ntitle: 自定义\n---\n"));
        assert_eq!(body_after_frontmatter(&kept_title), "正文不变\n");
        assert_eq!(kept_title.matches("正文不变").count(), 1);

        assert_eq!(fs::read_to_string(root.join("手册.md")).unwrap(), typed);
        let notebook = fs::read_to_string(root.join("concepts").join("手册.md")).unwrap();
        assert!(notebook.contains("手册正文"));
        assert!(!notebook.contains("type: Notebook"));

        assert!(!root.join("concepts").join("空.md").exists());
        let index = fs::read_to_string(root.join("index.md")).unwrap();
        let head = index.split("\n---\n").next().unwrap();
        assert_eq!(head, "---\nokf_version: \"0.2\"");
        assert!(!head.contains("type:"));
        assert!(index.contains("](/concepts/甲.md)"));
        assert!(index.contains("`空.md` — empty"));
        assert!(!root.join("wiki").exists());
        inspect_tree(root);
    }

    #[test]
    fn an_unchanged_note_keeps_its_concept_bytes() {
        let scratch = Scratch::new();
        let root = scratch.path();
        fs::write(root.join("甲.md"), "句子仍在。\n").unwrap();
        let first = publish_note_bundle(root).unwrap();
        assert_eq!(first.compiled, 1);
        let concept = root.join("concepts").join("甲.md");
        let mut bytes = fs::read(&concept).unwrap();
        bytes.extend_from_slice(b"\n<!-- keep -->\n");
        fs::write(&concept, &bytes).unwrap();

        let second = publish_note_bundle(root).unwrap();
        assert_eq!(second.compiled, 0);
        assert_eq!(second.unchanged, 1);
        assert!(second
            .outcomes
            .iter()
            .any(|(name, outcome)| { name == "甲.md" && *outcome == CompileOutcome::Unchanged }));
        assert_eq!(fs::read(&concept).unwrap(), bytes);

        let mut note = fs::read_to_string(root.join("甲.md")).unwrap();
        note.push_str("新句子。\n");
        fs::write(root.join("甲.md"), note).unwrap();
        let third = publish_note_bundle(root).unwrap();
        assert_eq!(third.compiled, 1);
        assert_eq!(third.unchanged, 0);
        let rewritten = fs::read_to_string(&concept).unwrap();
        assert!(rewritten.contains("新句子。"));
        assert!(!rewritten.contains("<!-- keep -->"));
    }

    #[test]
    fn index_log_and_concepts_are_not_compiled_as_notes() {
        let scratch = Scratch::new();
        let root = scratch.path();
        fs::write(root.join("甲.md"), "句子仍在。\n").unwrap();
        fs::write(root.join("log.md"), "保持原文\n").unwrap();
        fs::write(root.join("index.md"), "旧索引\n").unwrap();
        fs::create_dir(root.join("concepts")).unwrap();
        fs::write(root.join("concepts").join("手工.md"), "手工正文\n").unwrap();

        let report = publish_note_bundle(root).unwrap();
        assert!(report.outcomes.iter().all(|(name, _)| name == "甲.md"));
        assert_eq!(
            fs::read_to_string(root.join("log.md")).unwrap(),
            "保持原文\n"
        );
        assert!(!root.join("concepts").join("手工.md").exists());
        assert!(root.join("concepts").join("甲.md").is_file());
        let index = fs::read_to_string(root.join("index.md")).unwrap();
        assert!(index.starts_with("---\nokf_version: \"0.2\"\n---\n"));
        assert_ne!(index, "旧索引\n");
        inspect_tree(root);
    }

    #[test]
    fn a_rejected_bundle_keeps_the_previous_bytes() {
        let scratch = Scratch::new();
        let root = scratch.path();
        let good = "应该保持未加类型\n";
        let bad = "---\ntype: Note\n[unterminated\n---\n正文\n";
        fs::write(root.join("好.md"), good).unwrap();
        fs::write(root.join("坏.md"), bad).unwrap();
        fs::write(root.join("index.md"), "旧索引\n").unwrap();
        fs::create_dir(root.join("concepts")).unwrap();
        fs::write(root.join("concepts").join("旧.md"), "旧概念\n").unwrap();

        let error = publish_note_bundle(root).unwrap_err();
        assert!(error.to_string().contains("坏.md"), "{}", error);
        assert_eq!(fs::read_to_string(root.join("好.md")).unwrap(), good);
        assert_eq!(fs::read_to_string(root.join("坏.md")).unwrap(), bad);
        assert_eq!(
            fs::read_to_string(root.join("index.md")).unwrap(),
            "旧索引\n"
        );
        assert_eq!(
            fs::read_to_string(root.join("concepts").join("旧.md")).unwrap(),
            "旧概念\n"
        );
    }

    #[test]
    fn an_empty_directory_is_left_untouched() {
        let scratch = Scratch::new();
        let error = publish_note_bundle(scratch.path()).unwrap_err();
        assert!(error.to_string().contains("concept"), "{error}");
        assert!(fs::read_dir(scratch.path()).unwrap().next().is_none());
    }

    #[test]
    fn notes_past_the_vault_cap_are_skipped_without_losing_their_body() {
        let mut files = BTreeMap::new();
        files.insert("先.md".into(), "先写的句子\n".as_bytes().to_vec());
        files.insert("后.md".into(), "后写的句子\n".as_bytes().to_vec());
        let planned = plan_notes(&files, 1, None, &mut |_, _| {}).unwrap();
        assert!(planned
            .report
            .outcomes
            .iter()
            .any(|(name, outcome)| { name == "先.md" && *outcome == CompileOutcome::Compiled }));
        assert!(planned.report.outcomes.iter().any(|(name, outcome)| {
            name == "后.md" && *outcome == CompileOutcome::Skipped(SkipReason::OverCount)
        }));
        assert!(planned.concepts.keys().all(|path| path.ends_with("先.md")));
        let kept = String::from_utf8(planned.notes.get("后.md").unwrap().clone()).unwrap();
        assert!(kept.contains("type: Note"));
        assert!(kept.contains("后写的句子"));
    }

    #[test]
    fn each_note_is_reported_as_it_is_planned() {
        let mut files = BTreeMap::new();
        files.insert("先.md".into(), "先写的句子\n".as_bytes().to_vec());
        files.insert("后.md".into(), "后写的句子\n".as_bytes().to_vec());
        let mut seen = Vec::new();
        let planned = plan_notes(&files, 1, None, &mut |path, outcome| {
            seen.push((path.to_string(), outcome.clone()));
        })
        .unwrap();
        assert_eq!(seen, planned.report.outcomes);
        assert_eq!(seen.len(), 2);
    }

    #[test]
    fn a_note_is_on_disk_before_its_row_and_before_later_notes() {
        let scratch = Scratch::new();
        let root = scratch.path();
        fs::write(root.join("先.md"), "先写的句子\n").unwrap();
        fs::write(root.join("后.md"), "后写的句子\n").unwrap();
        fs::write(root.join("空.md"), "\n").unwrap();
        let mut saw_first = false;
        let report = publish_note_bundle_with(root, &mut |path, outcome| {
            if path == "先.md" {
                assert_eq!(*outcome, CompileOutcome::Compiled);
                let stamped = fs::read_to_string(root.join("先.md")).unwrap();
                assert!(stamped.contains("type: Note"));
                assert!(stamped.contains("先写的句子"));
                let concept = fs::read_to_string(root.join("concepts").join("先.md")).unwrap();
                assert!(concept.contains("先写的句子"));
                assert_eq!(
                    fs::read_to_string(root.join("后.md")).unwrap(),
                    "后写的句子\n"
                );
                assert!(!root.join("concepts").join("后.md").exists());
                assert_eq!(fs::read_to_string(root.join("空.md")).unwrap(), "\n");
                saw_first = true;
            }
            if path == "后.md" {
                assert!(saw_first);
                assert!(fs::read_to_string(root.join("后.md"))
                    .unwrap()
                    .contains("type: Note"));
                assert!(root.join("concepts").join("后.md").is_file());
                assert_eq!(fs::read_to_string(root.join("空.md")).unwrap(), "\n");
            }
            if path == "空.md" {
                assert_eq!(*outcome, CompileOutcome::Skipped(SkipReason::Empty));
                assert!(fs::read_to_string(root.join("空.md"))
                    .unwrap()
                    .contains("type: Note"));
                assert!(!root.join("concepts").join("空.md").exists());
            }
        })
        .unwrap();
        assert!(saw_first);
        assert_eq!(report.compiled, 2);
        assert_eq!(report.skipped, 1);
        inspect_tree(root);
    }

    #[test]
    fn a_later_invalid_note_leaves_earlier_notes_on_disk() {
        let scratch = Scratch::new();
        let root = scratch.path();
        let good = "应该写下去\n";
        let bad = "---\ntype: Note\n[unterminated\n---\n正文\n";
        fs::write(root.join("a.md"), good).unwrap();
        fs::write(root.join("z.md"), bad).unwrap();
        let mut saw_good = false;
        let error = publish_note_bundle_with(root, &mut |path, outcome| {
            assert_eq!(path, "a.md");
            assert_eq!(*outcome, CompileOutcome::Compiled);
            let stamped = fs::read_to_string(root.join("a.md")).unwrap();
            assert!(stamped.contains("type: Note"));
            assert!(stamped.contains(good));
            assert!(root.join("concepts").join("a.md").is_file());
            assert_eq!(fs::read_to_string(root.join("z.md")).unwrap(), bad);
            assert!(!root.join("concepts").join("z.md").exists());
            saw_good = true;
        })
        .unwrap_err();
        assert!(saw_good);
        assert!(error.to_string().contains("z.md"), "{error}");
        assert!(fs::read_to_string(root.join("a.md"))
            .unwrap()
            .contains("type: Note"));
        assert_eq!(fs::read_to_string(root.join("z.md")).unwrap(), bad);
        assert!(!root.join("concepts").join("z.md").exists());
    }

    #[test]
    fn a_note_after_the_old_forty_eight_still_compiles() {
        let scratch = Scratch::new();
        let root = scratch.path();
        for index in 0..49 {
            fs::write(root.join(format!("p{index:02}.md")), "有内容的段落\n").unwrap();
        }
        let report = publish_note_bundle(root).unwrap();
        assert!(report
            .outcomes
            .iter()
            .all(|(_, outcome)| { *outcome == CompileOutcome::Compiled }));
        assert!(root.join("concepts").join("p48.md").is_file());
        inspect_tree(root);
    }

    #[test]
    fn notes_outside_the_depth_or_size_cap_are_skipped() {
        let scratch = Scratch::new();
        let root = scratch.path();
        fs::write(root.join("浅.md"), "浅的句子\n").unwrap();
        let deep = root.join("a").join("b").join("c").join("d").join("e");
        fs::create_dir_all(&deep).unwrap();
        fs::write(deep.join("深.md"), "太深的句子\n").unwrap();
        fs::write(root.join("大.md"), vec![b'a'; MAX_TEXT_BYTES + 1]).unwrap();

        let report = publish_note_bundle(root).unwrap();
        assert!(report
            .outcomes
            .iter()
            .any(|(name, outcome)| { name == "浅.md" && *outcome == CompileOutcome::Compiled }));
        assert!(report.outcomes.iter().any(|(name, outcome)| {
            name == "a/b/c/d/e/深.md" && *outcome == CompileOutcome::Skipped(SkipReason::OverDepth)
        }));
        assert!(report.outcomes.iter().any(|(name, outcome)| {
            name == "大.md" && *outcome == CompileOutcome::Skipped(SkipReason::OverSize)
        }));
        assert!(root.join("concepts").join("浅.md").is_file());
        assert!(!root.join("concepts").join("a__b__c__d__e__深.md").exists());
        assert!(!root.join("concepts").join("大.md").exists());
        assert!(fs::read_to_string(deep.join("深.md"))
            .unwrap()
            .contains("type: Note"));
        let large = fs::read(root.join("大.md")).unwrap();
        let large_text = String::from_utf8(large).unwrap();
        assert!(large_text.contains("type: Note"));
        assert!(large_text.ends_with(&"a".repeat(32)));
        inspect_tree(root);
    }

    #[test]
    fn a_dangling_link_does_not_reject_the_bundle() {
        let scratch = Scratch::new();
        let root = scratch.path();
        fs::write(root.join("甲.md"), "见 [缺失](缺失.md)。\n").unwrap();
        let report = publish_note_bundle(root).unwrap();
        assert_eq!(report.failed, 0);
        assert_eq!(report.compiled, 1);
        let files = collect_files(root).unwrap();
        let bundle = files
            .iter()
            .map(|(path, content)| OkfBundleFile::new(path.clone(), content.clone()))
            .collect::<Vec<_>>();
        let inspection =
            inspect_okf_bundle(OkfFormatVersion::V0_2, bundle_limits(), bundle).unwrap();
        assert!(inspection
            .diagnostics
            .iter()
            .any(|item| item.code == OkfDiagnosticCode::DanglingLink && item.target == "缺失.md"));
    }

    #[test]
    fn only_a_wiki_local_directory_is_a_personal_vault() {
        assert!(is_personal_vault(Path::new("/workspace/wiki/local")));
        assert!(is_personal_vault(Path::new("wiki/local")));
        assert!(!is_personal_vault(Path::new("/workspace/wiki/local/extra")));
        assert!(!is_personal_vault(Path::new("/workspace/notes/local")));
        assert!(!is_personal_vault(Path::new("local")));
        assert!(!is_personal_vault(Path::new("/workspace/Wiki/local")));
    }

    fn personal_vault(scratch: &Scratch) -> PathBuf {
        let root = scratch.path().join("wiki").join("local");
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn accept_keeps_concept_bytes_and_note_body_and_skips_an_empty_note() {
        let scratch = Scratch::new();
        let root = personal_vault(&scratch);
        let note = "见句子。\n";
        let empty = "\n";
        let missing = "另一篇。\n";
        let concept =
            "---\ntype: Document\ntitle: \"甲\"\nresource: \"../甲.md\"\n---\n\n见句子。\n";
        fs::write(root.join("甲.md"), note).unwrap();
        fs::write(root.join("空.md"), empty).unwrap();
        fs::write(root.join("缺.md"), missing).unwrap();
        fs::create_dir(root.join("concepts")).unwrap();
        fs::write(root.join("concepts").join("甲.md"), concept).unwrap();

        let report = accept_personal_bundle(&root).unwrap();
        assert_eq!(fs::read(root.join("甲.md")).unwrap(), note.as_bytes());
        assert_eq!(fs::read(root.join("空.md")).unwrap(), empty.as_bytes());
        assert_eq!(fs::read(root.join("缺.md")).unwrap(), missing.as_bytes());
        assert_eq!(
            fs::read(root.join("concepts").join("甲.md")).unwrap(),
            concept.as_bytes()
        );
        assert!(!note.contains("type:"));
        assert_eq!(report.compiled, 0);
        assert_eq!(report.unchanged, 1);
        assert_eq!(report.skipped, 1);
        assert_eq!(report.failed, 1);
        assert!(report.outcomes.iter().any(|(name, outcome)| {
            name == "空.md" && *outcome == CompileOutcome::Skipped(SkipReason::Empty)
        }));
        let index = fs::read_to_string(root.join("index.md")).unwrap();
        assert!(index.contains("](/concepts/甲.md)"));
        assert!(index.contains("`空.md` — empty"));
        assert!(index.contains("`缺.md` — missing concept"));
        assert!(!index.contains("](/concepts/缺.md)"));
        assert!(report
            .failure_reasons
            .iter()
            .any(|(path, reason)| { path == "缺.md" && reason == "missing concept" }));
        assert!(index.contains("\n## Skipped\n"));
        assert!(index.contains("\n## Failed\n"));
        println!("concept bytes unchanged");
        println!("note body unchanged");
        println!("empty note skipped in the written index");
    }

    #[test]
    fn accept_leaves_the_previous_index_when_inspection_fails() {
        let scratch = Scratch::new();
        let root = personal_vault(&scratch);
        let note = "见句子。\n";
        let concept = "没有前言。\n";
        let previous = "---\nokf_version: \"0.2\"\n---\n\nprevious\n";
        fs::write(root.join("甲.md"), note).unwrap();
        fs::create_dir(root.join("concepts")).unwrap();
        fs::write(root.join("concepts").join("甲.md"), concept).unwrap();
        fs::write(root.join("index.md"), previous).unwrap();

        let error = accept_personal_bundle(&root).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(fs::read(root.join("甲.md")).unwrap(), note.as_bytes());
        assert_eq!(
            fs::read(root.join("concepts").join("甲.md")).unwrap(),
            concept.as_bytes()
        );
        assert_eq!(fs::read_to_string(root.join("index.md")).unwrap(), previous);
        println!("inspection failure leaves the previous index.md");
    }

    #[test]
    fn accept_rejects_a_directory_that_is_not_the_personal_vault() {
        let scratch = Scratch::new();
        let error = accept_personal_bundle(scratch.path()).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("wiki/local"), "{error}");
        assert!(fs::read_dir(scratch.path()).unwrap().next().is_none());
    }

    fn set_mtime(path: &Path, when: SystemTime) {
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(when)
            .unwrap();
    }

    #[test]
    fn plan_lists_only_notes_that_need_a_concept() {
        let scratch = Scratch::new();
        let root = personal_vault(&scratch);
        let note = "见句子。\n";
        let empty = "\n";
        fs::write(root.join("甲.md"), note).unwrap();
        fs::write(root.join("空.md"), empty).unwrap();
        let deep = root.join("a").join("b").join("c").join("d").join("e");
        fs::create_dir_all(&deep).unwrap();
        fs::write(deep.join("深.md"), "太深的句子\n").unwrap();

        let pending = plan_personal_compile(&root).unwrap();
        assert_eq!(pending, vec!["甲.md".to_string()]);
        assert_eq!(fs::read(root.join("甲.md")).unwrap(), note.as_bytes());
        assert_eq!(fs::read(root.join("空.md")).unwrap(), empty.as_bytes());
        assert!(!root.join("concepts").join("甲.md").exists());
        let error = plan_personal_compile(scratch.path()).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn plan_lists_a_legacy_concept_without_a_generation() {
        let scratch = Scratch::new();
        let root = personal_vault(&scratch);
        let note = "见句子。\n";
        let newer_note = "新的句子。\n";
        let concept = "---\ntype: Document\ntitle: \"甲\"\n---\n\n见句子。\n";
        let newer_concept = "---\ntype: Document\ntitle: \"新\"\n---\n\n旧概念。\n";
        fs::write(root.join("甲.md"), note).unwrap();
        fs::write(root.join("新.md"), newer_note).unwrap();
        fs::create_dir_all(root.join("concepts")).unwrap();
        fs::write(root.join("concepts").join("甲.md"), concept).unwrap();
        fs::write(root.join("concepts").join("新.md"), newer_concept).unwrap();
        let older = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        let middle = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_050);
        let newer = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_100);
        set_mtime(&root.join("甲.md"), older);
        set_mtime(&root.join("concepts").join("甲.md"), middle);
        set_mtime(&root.join("新.md"), newer);
        set_mtime(&root.join("concepts").join("新.md"), older);

        assert_eq!(
            plan_personal_compile(&root).unwrap(),
            vec!["新.md".to_string(), "甲.md".to_string()]
        );
        assert_eq!(fs::read(root.join("甲.md")).unwrap(), note.as_bytes());
        assert_eq!(fs::read(root.join("新.md")).unwrap(), newer_note.as_bytes());
        assert_eq!(
            fs::read(root.join("concepts").join("甲.md")).unwrap(),
            concept.as_bytes()
        );
        assert_eq!(
            fs::read(root.join("concepts").join("新.md")).unwrap(),
            newer_concept.as_bytes()
        );
    }

    #[test]
    fn release_keeps_a_note_written_after_the_pin() {
        let scratch = Scratch::new();
        let root = personal_vault(&scratch);
        let note = "见句子。\n";
        let concept = "---\ntype: Document\ntitle: \"乙\"\n---\n\n乙的概念。\n";
        fs::write(root.join("甲.md"), note).unwrap();
        fs::create_dir_all(root.join("concepts")).unwrap();
        fs::write(root.join("concepts").join("乙.md"), concept).unwrap();
        let earlier = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        let later = earlier + std::time::Duration::from_secs(30);
        set_mtime(&root.join("甲.md"), earlier);
        set_mtime(&root.join("concepts").join("乙.md"), earlier);
        pin_personal_compile(&root, &["甲.md".to_string()]).unwrap();
        let written = "用户刚写的句子。\n";
        fs::write(root.join("甲.md"), written).unwrap();
        set_mtime(&root.join("甲.md"), later);
        fs::write(
            root.join("concepts").join("乙.md"),
            "---\ntype: Document\n---\n\n被改的概念。\n",
        )
        .unwrap();
        set_mtime(&root.join("concepts").join("乙.md"), later);

        release_personal_compile(&root).unwrap();
        assert_eq!(fs::read(root.join("甲.md")).unwrap(), written.as_bytes());
        assert_eq!(
            fs::read(root.join("concepts").join("乙.md")).unwrap(),
            concept.as_bytes()
        );
    }

    #[test]
    fn release_restores_notes_and_concepts_outside_the_plan() {
        let scratch = Scratch::new();
        let root = personal_vault(&scratch);
        let note = "见句子。\n";
        let other = "乙的句子。\n";
        let concept = "---\ntype: Document\ntitle: \"甲\"\n---\n\n旧概念。\n";
        let other_concept = "---\ntype: Document\ntitle: \"乙\"\n---\n\n乙的概念。\n";
        fs::write(root.join("甲.md"), note).unwrap();
        fs::write(root.join("乙.md"), other).unwrap();
        fs::create_dir_all(root.join("concepts")).unwrap();
        fs::write(root.join("concepts").join("甲.md"), concept).unwrap();
        fs::write(root.join("concepts").join("乙.md"), other_concept).unwrap();
        let same = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        set_mtime(&root.join("甲.md"), same);
        set_mtime(&root.join("乙.md"), same);
        pin_personal_compile(&root, &["甲.md".to_string()]).unwrap();
        fs::write(root.join("甲.md"), "被改的笔记。\n").unwrap();
        set_mtime(&root.join("甲.md"), same);
        fs::write(root.join("乙.md"), "被改的乙。\n").unwrap();
        set_mtime(&root.join("乙.md"), same);
        fs::write(
            root.join("concepts").join("甲.md"),
            "---\ntype: Document\n---\n\n新概念。\n",
        )
        .unwrap();
        fs::write(
            root.join("concepts").join("乙.md"),
            "---\ntype: Document\n---\n\n被改的概念。\n",
        )
        .unwrap();

        release_personal_compile(&root).unwrap();
        assert_eq!(fs::read(root.join("甲.md")).unwrap(), note.as_bytes());
        assert_eq!(fs::read(root.join("乙.md")).unwrap(), other.as_bytes());
        assert_eq!(
            fs::read(root.join("concepts").join("乙.md")).unwrap(),
            other_concept.as_bytes()
        );
        assert_eq!(
            fs::read(root.join("concepts").join("甲.md")).unwrap(),
            "---\ntype: Document\n---\n\n新概念。\n".as_bytes()
        );
        release_personal_compile(&root).unwrap();
        let error = pin_personal_compile(scratch.path(), &[]).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn published_generation_ignores_a_body_line() {
        let front =
            "---\ntype: Document\nsource_generation: \"abc\"\n---\n\nsource_generation: \"body\"\n";
        assert_eq!(published_generation(front), Some("abc"));
        assert_eq!(
            published_generation("见句子。\nsource_generation: \"abc\"\n"),
            None
        );
    }

    #[test]
    fn remember_stamps_generation_without_changing_prose() {
        let scratch = Scratch::new();
        let root = personal_vault(&scratch);
        let note = "见句子。\n";
        let concept = "---\ntype: Document\ntitle: \"甲\"\n---\n\n模型写的概念。\n";
        fs::write(root.join("甲.md"), note).unwrap();
        fs::create_dir_all(root.join("concepts")).unwrap();
        fs::write(root.join("concepts").join("甲.md"), concept).unwrap();
        let older = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        let newer = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_100);
        set_mtime(&root.join("concepts").join("甲.md"), older);
        set_mtime(&root.join("甲.md"), newer);

        assert_eq!(
            plan_personal_compile(&root).unwrap(),
            vec!["甲.md".to_string()]
        );
        assert_eq!(
            fs::read(root.join("concepts").join("甲.md")).unwrap(),
            concept.as_bytes()
        );
        remember_personal_generations(&root, &["甲.md".to_string()]).unwrap();
        assert_eq!(fs::read(root.join("甲.md")).unwrap(), note.as_bytes());
        let stamped = fs::read_to_string(root.join("concepts").join("甲.md")).unwrap();
        assert_eq!(
            body_after_frontmatter(&stamped),
            body_after_frontmatter(concept)
        );
        let generation = published_generation(&stamped).unwrap().to_string();
        assert!(plan_personal_compile(&root).unwrap().is_empty());
        let stale = stamped.replace(&generation, "stale");
        fs::write(root.join("concepts").join("甲.md"), &stale).unwrap();
        assert_eq!(
            plan_personal_compile(&root).unwrap(),
            vec!["甲.md".to_string()]
        );
        assert_eq!(
            body_after_frontmatter(
                &fs::read_to_string(root.join("concepts").join("甲.md")).unwrap()
            ),
            body_after_frontmatter(&stale)
        );
        let error = remember_personal_generations(scratch.path(), &[]).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    }
}
