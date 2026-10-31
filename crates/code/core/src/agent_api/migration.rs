use super::*;
use crate::error::CodeError;
use crate::store::{
    snapshot_content_digest, LegacyStudioEntry, LegacyStudioMigrationCompletion,
    LegacyStudioMigrationReceipt, LegacyStudioMigrationRequest, SessionStore,
    TranscriptEntryPayload, TranscriptLegacyHistoryDisplay, LEGACY_STUDIO_MIGRATION_SCHEMA_V1,
};
use aster_effect::{Fact, LogStore, NewFact};
use std::collections::HashSet;

pub(crate) const LEGACY_STUDIO_ENTRY_FACT_KIND: &str = "legacy.studio.entry";
pub(crate) const LEGACY_STUDIO_COMPLETION_FACT_KIND: &str = "legacy.studio.migration_complete";
const MAX_ENTRIES: usize = 4_096;
const MAX_TEXT_BYTES: usize = 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 8 * 1024 * 1024;
const MAX_ATTACHMENTS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LegacyStudioEntryFact {
    pub source_session_id: String,
    pub target_session_id: String,
    pub source_digest: String,
    pub entry: LegacyStudioEntry,
}

fn migration_error(error: impl std::fmt::Display) -> CodeError {
    CodeError::Session(format!("legacy Studio migration: {error}"))
}

fn display(entry: &LegacyStudioEntry) -> TranscriptLegacyHistoryDisplay {
    TranscriptLegacyHistoryDisplay {
        source_entry_id: entry.id.clone(),
        role: entry.role,
        text: entry.text.clone(),
        attachments: entry.attachments.clone(),
    }
}

/// Compute the expected digest for the exact closed Studio input set. The
/// workspace is canonicalized in the same way as session construction.
pub fn legacy_studio_source_digest(
    source_session_id: &str,
    expected_workspace: &Path,
    entries: &[LegacyStudioEntry],
) -> Result<String> {
    let workspace = safe_canonicalize(expected_workspace);
    crate::content_digest::digest_json(
        "aster.code.legacy-studio-history-source.v1",
        &(source_session_id, workspace, entries),
    )
    .map_err(migration_error)
}

impl LegacyStudioMigrationRequest {
    /// Compute `source_digest` for a request being assembled by an offline host.
    pub fn compute_source_digest(
        source_session_id: &str,
        expected_workspace: &Path,
        entries: &[LegacyStudioEntry],
    ) -> Result<String> {
        legacy_studio_source_digest(source_session_id, expected_workspace, entries)
    }
}

fn target_digest(entries: &[LegacyStudioEntry]) -> Result<String> {
    let displays = entries.iter().map(display).collect::<Vec<_>>();
    crate::content_digest::digest_json("aster.code.legacy-studio-history-target.v1", &displays)
        .map_err(migration_error)
}

fn bounded_add(total: &mut usize, amount: usize) -> Result<()> {
    *total = total
        .checked_add(amount)
        .ok_or_else(|| migration_error("input byte count overflow"))?;
    if *total > MAX_TOTAL_BYTES {
        return Err(migration_error(format!(
            "input exceeds {MAX_TOTAL_BYTES} bytes"
        )));
    }
    Ok(())
}

fn validate_opaque_id(value: &str, field: &str) -> Result<()> {
    if value.is_empty() || value.len() > 256 || value.trim() != value {
        return Err(migration_error(format!(
            "{field} must be a bounded opaque id"
        )));
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '=' | '+'))
    {
        return Err(migration_error(format!(
            "{field} contains non-opaque or path characters"
        )));
    }
    Ok(())
}

fn validate_request(request: &LegacyStudioMigrationRequest) -> Result<()> {
    validate_opaque_id(&request.source_session_id, "source_session_id")?;
    validate_opaque_id(&request.target_session_id, "target_session_id")?;
    validate_opaque_id(&request.backup_reference, "backup_reference")?;
    if request.source_session_id == request.target_session_id {
        return Err(migration_error("source and target sessions must differ"));
    }
    if request.entries.is_empty() {
        return Err(migration_error("at least one legacy entry is required"));
    }
    if request.entries.len() > MAX_ENTRIES {
        return Err(migration_error(format!(
            "entry count exceeds {MAX_ENTRIES}"
        )));
    }
    let mut ids = HashSet::with_capacity(request.entries.len());
    let mut total = 0usize;
    for entry in &request.entries {
        validate_opaque_id(&entry.id, "legacy entry id")?;
        if !ids.insert(&entry.id) {
            return Err(migration_error(format!(
                "duplicate legacy entry id {:?}",
                entry.id
            )));
        }
        if entry.text.trim().is_empty() || entry.text.len() > MAX_TEXT_BYTES {
            return Err(migration_error(format!(
                "legacy entry {:?} text is empty or exceeds {MAX_TEXT_BYTES} bytes",
                entry.id
            )));
        }
        if entry.attachments.len() > MAX_ATTACHMENTS {
            return Err(migration_error(format!(
                "legacy entry {:?} has too many attachments",
                entry.id
            )));
        }
        bounded_add(&mut total, entry.id.len())?;
        bounded_add(&mut total, entry.text.len())?;
        for attachment in &entry.attachments {
            validate_opaque_id(&attachment.name, "attachment name")?;
            if attachment.media_type.is_empty()
                || attachment.media_type.len() > 256
                || attachment.media_type.trim() != attachment.media_type
            {
                return Err(migration_error("invalid attachment media type"));
            }
            bounded_add(&mut total, attachment.name.len())?;
            bounded_add(&mut total, attachment.media_type.len())?;
        }
    }
    Ok(())
}

fn receipt_for(request: &LegacyStudioMigrationRequest) -> Result<LegacyStudioMigrationReceipt> {
    Ok(LegacyStudioMigrationReceipt {
        schema: LEGACY_STUDIO_MIGRATION_SCHEMA_V1.to_string(),
        source_session_id: request.source_session_id.clone(),
        target_session_id: request.target_session_id.clone(),
        imported_count: request.entries.len(),
        unmapped_count: 0,
        conflicting_count: 0,
        non_executable_count: request.entries.len(),
        source_digest: request.source_digest.clone(),
        target_digest: target_digest(&request.entries)?,
        completion: LegacyStudioMigrationCompletion::Completed,
        backup_reference: request.backup_reference.clone(),
    })
}

fn decode_entry_fact(fact: &Fact) -> Result<LegacyStudioEntryFact> {
    serde_json::from_value(fact.payload.clone()).map_err(|error| {
        migration_error(format!(
            "corrupt legacy entry fact {} at seq {}: {error}",
            fact.key, fact.seq
        ))
    })
}

fn verify_readback(
    workspace: &Path,
    facts: &[Fact],
    receipt: &LegacyStudioMigrationReceipt,
) -> Result<()> {
    if receipt.schema != LEGACY_STUDIO_MIGRATION_SCHEMA_V1
        || receipt.completion != LegacyStudioMigrationCompletion::Completed
        || receipt.imported_count == 0
        || receipt.unmapped_count != 0
        || receipt.conflicting_count != 0
        || receipt.non_executable_count != receipt.imported_count
    {
        return Err(migration_error(
            "receipt is not a completed closed v1 import",
        ));
    }
    validate_opaque_id(&receipt.source_session_id, "source_session_id")?;
    validate_opaque_id(&receipt.target_session_id, "target_session_id")?;
    validate_opaque_id(&receipt.backup_reference, "backup_reference")?;
    if receipt.source_session_id == receipt.target_session_id {
        return Err(migration_error("receipt source and target sessions match"));
    }
    let marker_indexes = facts
        .iter()
        .enumerate()
        .filter(|(_, fact)| fact.kind == LEGACY_STUDIO_COMPLETION_FACT_KIND)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if marker_indexes != [receipt.imported_count] {
        return Err(migration_error(
            "completion marker is missing, duplicated, or out of order",
        ));
    }
    let marker_index = marker_indexes[0];
    if facts
        .iter()
        .skip(marker_index + 1)
        .any(|fact| fact.kind == LEGACY_STUDIO_ENTRY_FACT_KIND)
    {
        return Err(migration_error(
            "legacy entries follow the completion marker",
        ));
    }
    let marker = &facts[marker_index];
    let stored_receipt: LegacyStudioMigrationReceipt =
        serde_json::from_value(marker.payload.clone())
            .map_err(|error| migration_error(format!("corrupt completion receipt: {error}")))?;
    if &stored_receipt != receipt {
        return Err(migration_error("completion receipt differs from read-back"));
    }

    let mut entries = Vec::with_capacity(receipt.imported_count);
    for fact in facts.iter().take(receipt.imported_count) {
        if fact.kind != LEGACY_STUDIO_ENTRY_FACT_KIND {
            return Err(migration_error(
                "imported prefix contains a non-legacy execution fact",
            ));
        }
        let stored = decode_entry_fact(fact)?;
        if stored.source_session_id != receipt.source_session_id
            || stored.target_session_id != receipt.target_session_id
            || stored.source_digest != receipt.source_digest
        {
            return Err(migration_error(
                "legacy entry source/target binding differs",
            ));
        }
        entries.push(stored.entry);
    }
    let actual_source =
        legacy_studio_source_digest(&receipt.source_session_id, workspace, &entries)?;
    if actual_source != receipt.source_digest {
        return Err(migration_error("source digest differs after read-back"));
    }
    if target_digest(&entries)? != receipt.target_digest {
        return Err(migration_error("target digest differs after read-back"));
    }

    let projected = super::transcript::project_entries(&facts[..=marker_index])?;
    if projected.len() != entries.len() {
        return Err(migration_error("read-back entry count differs"));
    }
    let mut entry_ids = HashSet::with_capacity(projected.len());
    let mut entry_digests = HashSet::with_capacity(projected.len());
    for (actual, expected) in projected.iter().zip(&entries) {
        let TranscriptEntryPayload::LegacyHistory {
            display: Some(actual_display),
            ..
        } = &actual.payload
        else {
            return Err(migration_error(
                "read-back entry is not displayable LegacyHistory",
            ));
        };
        if actual_display != &display(expected)
            || !entry_ids.insert(&actual.entry_id)
            || !entry_digests.insert(&actual.digest)
        {
            return Err(migration_error(
                "read-back entry identity, order, or digest differs",
            ));
        }
    }
    Ok(())
}

fn target_already_exists(workspace: &Path, target_session_id: &str) -> Result<bool> {
    if crate::session_folder::find_session_dir(workspace, target_session_id)
        .map_err(migration_error)?
        .is_some()
        || crate::session_folder::legacy_effect_log_exists(workspace, target_session_id)
    {
        return Ok(true);
    }
    let directory = crate::session_folder::effect_log_dir(workspace, target_session_id);
    if !directory.try_exists().map_err(migration_error)? {
        return Ok(false);
    }
    let log = aster_effect::FileLog::open(directory).map_err(migration_error)?;
    Ok(!log
        .read(&crate::fact_control::thread_for_session(target_session_id))
        .map_err(migration_error)?
        .is_empty())
}

async fn perform_import(
    session: &AgentSession,
    store: &Arc<dyn SessionStore>,
    request: &LegacyStudioMigrationRequest,
) -> Result<LegacyStudioMigrationReceipt> {
    let receipt = receipt_for(request)?;
    let target = &request.target_session_id;
    let lease = session.run_admission.try_acquire(target)?;
    let log = lease.nonexecuting_fact_log(&session.run_admission, target)?;
    let writer = log.try_writer_fence().map_err(migration_error)?;
    let thread = crate::fact_control::thread_for_session(target);
    if !log.read(&thread).map_err(migration_error)?.is_empty() {
        return Err(migration_error("target already has facts"));
    }
    if store.exists(target).await.map_err(migration_error)? {
        return Err(migration_error("target already has a snapshot"));
    }

    let mut facts = request
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let stored = LegacyStudioEntryFact {
                source_session_id: request.source_session_id.clone(),
                target_session_id: target.clone(),
                source_digest: request.source_digest.clone(),
                entry: entry.clone(),
            };
            Ok(NewFact {
                kind: LEGACY_STUDIO_ENTRY_FACT_KIND.to_string(),
                key: format!("legacy.studio.entry:{index}:{}", entry.id),
                payload: serde_json::to_value(stored).map_err(migration_error)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    facts.push(NewFact {
        kind: LEGACY_STUDIO_COMPLETION_FACT_KIND.to_string(),
        key: format!("legacy.studio.completion:{}", request.source_digest),
        payload: serde_json::to_value(&receipt).map_err(migration_error)?,
    });
    let appended = log
        .append(&thread, &facts, Some("legacy-studio-single-writer"))
        .map_err(migration_error)?;
    let readback = log.read(&thread).map_err(migration_error)?;
    if readback != appended {
        return Err(migration_error("durable fact read-back differs"));
    }
    verify_readback(session.workspace(), &readback, &receipt)?;
    drop(writer);

    let persistence = session_persistence::SessionPersistenceContext::from_session(session);
    let snapshot = persistence.capture_snapshot().await?;
    if !snapshot.session.messages.is_empty()
        || !snapshot.run_records.is_empty()
        || !snapshot.verification_reports.is_empty()
        || !snapshot.subagent_tasks.is_empty()
        || !snapshot.session.tasks.is_empty()
        || snapshot.session.total_usage.total_tokens != 0
    {
        return Err(migration_error(
            "import fabricated executable session state",
        ));
    }
    store
        .save_snapshot(&snapshot)
        .await
        .map_err(migration_error)?;
    let persisted = store
        .load_snapshot(target)
        .await
        .map_err(migration_error)?
        .ok_or_else(|| migration_error("saved target snapshot is missing"))?;
    if snapshot_content_digest(&persisted).map_err(migration_error)?
        != snapshot_content_digest(&snapshot).map_err(migration_error)?
    {
        return Err(migration_error("saved target snapshot digest differs"));
    }
    let verified = session
        .legacy_studio_migration_receipt()?
        .ok_or_else(|| migration_error("completed receipt is missing after save"))?;
    if verified != receipt {
        return Err(migration_error("receipt differs after snapshot save"));
    }
    Ok(receipt)
}

impl Agent {
    /// Import ordinary offline Studio text history into one new Core session.
    /// The returned receipt is the only activation signal; no legacy fallback is
    /// consulted or created. Rich content parts are outside this closed v1 API.
    pub async fn import_legacy_studio_history_async(
        &self,
        workspace: impl Into<String>,
        request: LegacyStudioMigrationRequest,
        mut options: SessionOptions,
    ) -> Result<(AgentSession, LegacyStudioMigrationReceipt)> {
        validate_request(&request)?;
        let workspace = workspace.into();
        let canonical = safe_canonicalize(Path::new(&workspace));
        if canonical != safe_canonicalize(&request.expected_workspace) {
            return Err(migration_error("workspace mismatch"));
        }
        let actual_digest = legacy_studio_source_digest(
            &request.source_session_id,
            &request.expected_workspace,
            &request.entries,
        )?;
        if actual_digest != request.source_digest {
            return Err(migration_error("source digest conflict"));
        }
        if options
            .session_id
            .as_ref()
            .is_some_and(|id| id != &request.target_session_id)
        {
            return Err(migration_error("target session id does not match options"));
        }
        if options.work_policy.is_some() || options.durable_source.is_some() {
            return Err(migration_error(
                "migration refuses standing execution or policy evidence",
            ));
        }
        options.session_id = Some(request.target_session_id.clone());
        if self
            .list_sessions()
            .await
            .contains(&request.target_session_id)
        {
            return Err(migration_error("target is active"));
        }
        if target_already_exists(&canonical, &request.target_session_id)? {
            return Err(migration_error("target already exists"));
        }
        let store = session_config::resolve_session_store(&self.code_config, &options)
            .await?
            .ok_or_else(|| migration_error("an aggregate session store is required"))?;
        if !store.capabilities().atomic_session_snapshots {
            return Err(migration_error(
                "session store lacks atomic snapshot publication",
            ));
        }
        if store
            .exists(&request.target_session_id)
            .await
            .map_err(migration_error)?
        {
            return Err(migration_error("target already has a snapshot"));
        }
        options = options.with_session_store(Arc::clone(&store));
        let session = self.session_async(workspace, Some(options)).await?;
        let session_dir = crate::session_folder::find_session_dir(
            session.workspace(),
            &request.target_session_id,
        )
        .map_err(migration_error)?;
        let flat_log = (session_dir.is_none()).then(|| {
            crate::session_folder::effect_log_dir(session.workspace(), &request.target_session_id)
                .join(format!(
                    "{}.jsonl",
                    crate::fact_control::thread_for_session(&request.target_session_id)
                ))
        });
        let result = perform_import(&session, &store, &request).await;
        match result {
            Ok(receipt) => Ok((session, receipt)),
            Err(error) => {
                session.close().await;
                drop(session);
                let mut rollback_errors = Vec::new();
                if let Err(rollback) = store.delete(&request.target_session_id).await {
                    rollback_errors.push(format!("snapshot rollback: {rollback}"));
                }
                let rollback = match (&session_dir, &flat_log) {
                    (Some(directory), _) => std::fs::remove_dir_all(directory),
                    (None, Some(file)) => std::fs::remove_file(file),
                    (None, None) => Ok(()),
                };
                if let Err(rollback) = rollback {
                    if rollback.kind() != std::io::ErrorKind::NotFound {
                        rollback_errors.push(format!("fact rollback: {rollback}"));
                    }
                }
                if rollback_errors.is_empty() {
                    Err(error)
                } else {
                    Err(migration_error(format!(
                        "{error}; {}",
                        rollback_errors.join("; ")
                    )))
                }
            }
        }
    }
}

impl AgentSession {
    /// Revalidate the persisted completed migration receipt. A legacy entry
    /// without its marker is an incomplete import and fails closed.
    pub fn legacy_studio_migration_receipt(&self) -> Result<Option<LegacyStudioMigrationReceipt>> {
        let facts = super::transcript::read_facts(self)?;
        let mut markers = facts
            .iter()
            .filter(|fact| fact.kind == LEGACY_STUDIO_COMPLETION_FACT_KIND);
        let Some(marker) = markers.next() else {
            if facts
                .iter()
                .any(|fact| fact.kind.starts_with("legacy.studio."))
            {
                return Err(migration_error(
                    "incomplete import has no completed receipt",
                ));
            }
            return Ok(None);
        };
        if markers.next().is_some() {
            return Err(migration_error("multiple migration receipts"));
        }
        let receipt: LegacyStudioMigrationReceipt = serde_json::from_value(marker.payload.clone())
            .map_err(|error| migration_error(format!("corrupt completion receipt: {error}")))?;
        if receipt.target_session_id != self.session_id {
            return Err(migration_error("receipt/target session mismatch"));
        }
        verify_readback(&self.workspace, &facts, &receipt)?;
        Ok(Some(receipt))
    }
}

#[cfg(test)]
mod tests;
