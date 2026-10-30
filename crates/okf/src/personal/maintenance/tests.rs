#![cfg(test)]

use super::*;
use serde_json::json;

#[path = "source_mutation_tests.rs"]
mod source_mutation;

struct Vault(PathBuf);

impl Vault {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "aster-wiki-test-{}-{}-{}",
            std::process::id(),
            clock_ms().unwrap(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let root = directory.join("wiki/local");
        fs::create_dir_all(&root).unwrap();
        Self(root.canonicalize().unwrap())
    }
    fn write(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn begin(&self) -> KnowledgeJob {
        begin_knowledge_maintenance(&self.0, &[], MaintenanceOperation::Ingest).unwrap()
    }
    fn propose(&self, job: &KnowledgeJob, change: &str) {
        let staging = Path::new(&job.staging);
        fs::create_dir_all(staging.join("concepts")).unwrap();
        fs::write(
            staging.join("concepts/shared.md"),
            "---\ntype: Concept\ntitle: Shared decision\n---\n\nEvidence from the meeting.\n",
        )
        .unwrap();
        fs::write(staging.join(MANIFEST), serde_json::to_vec(&json!({
            "sources":job.sources,"summary":"The meeting provides new evidence.","categories":{"concepts/shared.md":["Meetings"]},
            "pages":[{"path":"concepts/shared.md","title":"Shared decision","kind":"concept","change":change,"reason":"The meeting provides new evidence.","requires_review":false,"sources":job.sources}]
        })).unwrap()).unwrap();
    }
}

impl Drop for Vault {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(self.0.parent().unwrap().parent().unwrap());
    }
}

#[test]
fn staging_owns_the_output_directory_before_the_agent_runs() {
    let vault = Vault::new();
    vault.write("source.md", "# Source\nFrozen source bytes.\n");
    let job = vault.begin();
    let concepts = Path::new(&job.staging).join("concepts");
    assert!(concepts.is_dir());
    assert_eq!(fs::read_dir(&concepts).unwrap().count(), 0);
    fs::remove_dir(&concepts).unwrap();
    let resumed = vault.begin();
    assert_eq!(resumed.id, job.id);
    assert!(concepts.is_dir());
    assert_eq!(
        fs::read_to_string(vault.0.join("source.md")).unwrap(),
        "# Source\nFrozen source bytes.\n"
    );
    abandon_knowledge_maintenance(&vault.0, &job.id).unwrap();
}

#[test]
fn compile_admission_requires_the_current_unpublished_job() {
    let vault = Vault::new();
    vault.write("source.md", "Frozen evidence.");
    let job = vault.begin();
    assert_eq!(
        knowledge_compile_workspace(&vault.0, &job.id).unwrap(),
        PathBuf::from(&job.staging)
    );
    assert!(knowledge_compile_workspace(&vault.0, &"0".repeat(64)).is_err());
    vault.propose(&job, "create");
    finish_knowledge_maintenance(&vault.0, &job.id).unwrap();
    assert!(knowledge_compile_workspace(&vault.0, &job.id).is_err());
    assert_eq!(
        fs::read_to_string(vault.0.join("source.md")).unwrap(),
        "Frozen evidence."
    );
}

#[test]
fn concurrent_jobs_can_create_shared_staging_parents_without_admitting_links() {
    let vault = Vault::new();
    let parent = vault.0.join("shared");
    let barrier = std::sync::Barrier::new(8);
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                let mut errors = Vec::new();
                for index in 0..24 {
                    barrier.wait();
                    if let Err(error) =
                        create_owned_directory(&parent.join(index.to_string()).join("nested"))
                    {
                        errors.push(error);
                    }
                }
                assert!(errors.is_empty(), "{errors:?}");
            });
        }
    });
    #[cfg(unix)]
    {
        let linked = vault.0.join("linked");
        std::os::unix::fs::symlink(&parent, &linked).unwrap();
        assert!(create_owned_directory(&linked.join("new")).is_err());
        assert!(!parent.join("new").exists());
    }
}

#[test]
#[cfg(unix)]
fn resuming_a_job_refuses_a_linked_output_directory() {
    let vault = Vault::new();
    vault.write("source.md", "Frozen source bytes.");
    let job = vault.begin();
    let concepts = Path::new(&job.staging).join("concepts");
    let outside = vault.0.parent().unwrap().join("outside");
    fs::create_dir(&outside).unwrap();
    fs::remove_dir(&concepts).unwrap();
    std::os::unix::fs::symlink(&outside, &concepts).unwrap();
    let error =
        begin_knowledge_maintenance(&vault.0, &[], MaintenanceOperation::Ingest).unwrap_err();
    assert!(error.to_string().contains("knowledge.symlink_refused"));
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
    assert_eq!(
        fs::read_to_string(vault.0.join("source.md")).unwrap(),
        "Frozen source bytes."
    );
    abandon_knowledge_maintenance(&vault.0, &job.id).unwrap();
}

#[test]
fn new_knowledge_is_incremental_cited_and_never_rewrites_sources() {
    let vault = Vault::new();
    let original = "# Meeting\nAster launch date was confirmed.\n";
    vault.write("meeting.md", original);
    let job = vault.begin();
    assert!(Path::new(&job.staging).ends_with("workspace/wiki/local"));
    vault.propose(&job, "create");
    let view = finish_knowledge_maintenance(&vault.0, &job.id).unwrap();
    assert!(view.active.is_none());
    assert_eq!(view.sources[0].status, SourceStatus::Current);
    assert_eq!(view.pages, 1);
    assert!(!Path::new(&job.staging).exists());
    assert_eq!(view.activity[0].published, 1);
    assert_eq!(
        fs::read_to_string(vault.0.join("meeting.md")).unwrap(),
        original
    );
    assert!(fs::read_to_string(vault.0.join("concepts/shared.md"))
        .unwrap()
        .contains("../meeting.md"));
    assert!(fs::read_to_string(vault.0.join("index.md"))
        .unwrap()
        .contains("Shared decision"));
    assert!(begin_knowledge_maintenance(&vault.0, &[], MaintenanceOperation::Ingest).is_err());
    vault.write("meeting.md", "# Meeting\nThe date changed.\n");
    assert_eq!(
        knowledge_maintenance_plan(&vault.0).unwrap().sources[0].status,
        SourceStatus::Pending
    );
}

#[test]
fn an_existing_page_survives_until_the_reviewed_revision_is_accepted() {
    let vault = Vault::new();
    vault.write("meeting.md", "New evidence.");
    let previous = "---\ntype: Concept\ntitle: Shared decision\n---\n\nOriginal conclusion.\n";
    vault.write("concepts/shared.md", previous);
    let job = vault.begin();
    vault.propose(&job, "update");
    let view = finish_knowledge_maintenance(&vault.0, &job.id).unwrap();
    assert_eq!(
        fs::read_to_string(vault.0.join("concepts/shared.md")).unwrap(),
        previous
    );
    let recovered = knowledge_maintenance_plan(&vault.0)
        .unwrap()
        .active
        .unwrap();
    assert!(recovered.ready);
    let detail = knowledge_change(&vault.0, &job.id, "concepts/shared.md").unwrap();
    assert_eq!(detail.before, previous);
    assert!(detail.after.contains("Evidence from the meeting"));
    assert_eq!(detail.evidence["meeting.md"], "New evidence.");
    assert!(
        review_knowledge_change(&vault.0, &job.id, &detail.change.path, "stale-review", true)
            .is_err()
    );
    assert!(view.active.is_some());
    let result = review_knowledge_change(
        &vault.0,
        &job.id,
        &detail.change.path,
        &detail.change.generation,
        true,
    )
    .unwrap();
    assert!(result.active.is_none());
    assert!(fs::read_to_string(vault.0.join("concepts/shared.md"))
        .unwrap()
        .contains("Evidence from the meeting"));
    assert_eq!(
        knowledge_change(&vault.0, &job.id, "concepts/shared.md")
            .unwrap()
            .before,
        previous
    );
}

#[test]
fn model_declared_conflicts_wait_for_review_even_for_new_pages() {
    let vault = Vault::new();
    vault.write("meeting.md", "Conflicting dates.");
    let job = vault.begin();
    vault.propose(&job, "conflict");
    let view = finish_knowledge_maintenance(&vault.0, &job.id).unwrap();
    assert!(!vault.0.join("concepts/shared.md").exists());
    let change = &view.active.unwrap().changes[0];
    let result =
        review_knowledge_change(&vault.0, &job.id, &change.path, &change.generation, false)
            .unwrap();
    assert_eq!(result.sources[0].status, SourceStatus::Attention);
    assert!(!vault.0.join("concepts/shared.md").exists());
    let detail = knowledge_change(&vault.0, &job.id, &change.path).unwrap();
    assert_eq!(detail.before, "");
    assert!(detail.after.contains("Evidence from the meeting"));
}

#[test]
fn changed_sources_or_pages_refuse_publication_without_overwriting_them() {
    for changed in ["meeting.md", "concepts/shared.md"] {
        let vault = Vault::new();
        vault.write("meeting.md", "Original input.");
        vault.write(
            "concepts/shared.md",
            "---\ntype: Concept\ntitle: Shared decision\n---\nOld knowledge.\n",
        );
        let job = vault.begin();
        vault.propose(&job, "update");
        let view = finish_knowledge_maintenance(&vault.0, &job.id).unwrap();
        let change = &view.active.unwrap().changes[0];
        let edited = "Someone saved a newer version.\n";
        vault.write(changed, edited);
        let error =
            review_knowledge_change(&vault.0, &job.id, &change.path, &change.generation, true)
                .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(vault.0.join(changed)).unwrap(), edited);
    }
}

#[test]
fn source_mutation_and_unlisted_writes_are_rejected_before_any_page_is_published() {
    for modified in ["meeting.md", "concepts/undeclared.md"] {
        let vault = Vault::new();
        vault.write("meeting.md", "Original input.");
        let job = vault.begin();
        vault.propose(&job, "create");
        fs::write(
            Path::new(&job.staging).join(modified),
            "An unapproved write.",
        )
        .unwrap();
        assert!(finish_knowledge_maintenance(&vault.0, &job.id).is_err());
        assert!(!vault.0.join("concepts/shared.md").exists());
        assert_eq!(
            fs::read_to_string(vault.0.join("meeting.md")).unwrap(),
            "Original input."
        );
    }
}

#[test]
fn unknown_manifest_values_and_unsafe_paths_fail_closed() {
    for invalid in [
        "../../outside.md",
        "concepts/../outside.md",
        "concepts/.hidden/page.md",
        "concepts/sub/index.md",
        "concepts/a/b/c/d/page.md",
    ] {
        let vault = Vault::new();
        vault.write("meeting.md", "Input.");
        let job = vault.begin();
        vault.propose(&job, "create");
        let manifest = Path::new(&job.staging).join(MANIFEST);
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
        value["pages"][0]["path"] = json!(invalid);
        fs::write(&manifest, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(finish_knowledge_maintenance(&vault.0, &job.id).is_err());
        assert!(!vault.0.join("concepts/shared.md").exists());
    }
    assert!(PageKind::parse("guessed-entity").is_err());
    assert_eq!(PageKind::parse("entity").unwrap().as_str(), "entity");
    assert!(MaintenanceOperation::parse("rewrite-everything").is_err());
}

#[test]
fn a_changed_review_candidate_cannot_be_accepted() {
    let vault = Vault::new();
    vault.write("meeting.md", "Conflicting dates.");
    let job = vault.begin();
    vault.propose(&job, "conflict");
    let view = finish_knowledge_maintenance(&vault.0, &job.id).unwrap();
    let change = &view.active.unwrap().changes[0];
    vault.write(
        &format!(".wiki-history/{}/after/{}", job.id, change.path),
        "Changed after review.",
    );
    assert!(
        review_knowledge_change(&vault.0, &job.id, &change.path, &change.generation, true).is_err()
    );
    assert!(!vault.0.join(&change.path).exists());
}

#[test]
fn cancellation_is_durable_and_releases_sources_without_publishing_late_work() {
    let vault = Vault::new();
    vault.write("meeting.md", "Unchanged source.");
    let job = vault.begin();
    assert!(with_knowledge_source_mutation(&vault.0, |_| Ok(())).is_err());
    assert!(cancel_knowledge_maintenance(&vault.0, "wrong-job").is_err());
    let cancelled = cancel_knowledge_maintenance(&vault.0, &job.id).unwrap();
    assert!(cancelled.active.is_none());
    assert_eq!(cancelled.activity[0].status, ActivityStatus::Cancelled);
    assert_eq!(cancelled.sources[0].status, SourceStatus::Pending);
    assert_eq!(
        knowledge_maintenance_plan(&vault.0).unwrap().activity[0].status,
        ActivityStatus::Cancelled
    );
    assert_eq!(
        knowledge_maintenance_job(&vault.0, &job.id)
            .unwrap()
            .error
            .as_deref(),
        Some("knowledge.agent_cancelled")
    );
    assert!(!Path::new(&job.staging).exists());
    assert!(finish_knowledge_maintenance(&vault.0, &job.id).is_err());
    with_knowledge_source_mutation(&vault.0, |_| Ok(())).unwrap();
    assert_eq!(
        fs::read_to_string(vault.0.join("meeting.md")).unwrap(),
        "Unchanged source."
    );
    let resumed = vault.begin();
    assert_ne!(resumed.id, job.id);
    assert_eq!(resumed.sources, job.sources);
    cancel_knowledge_maintenance(&vault.0, &resumed.id).unwrap();
}

#[test]
fn cancelling_an_audit_preserves_current_sources_and_cannot_discard_reviewable_changes() {
    let vault = Vault::new();
    vault.write("meeting.md", "Input.");
    let initial = vault.begin();
    vault.propose(&initial, "create");
    finish_knowledge_maintenance(&vault.0, &initial.id).unwrap();
    let published = fs::read(vault.0.join("concepts/shared.md")).unwrap();
    let audit = begin_knowledge_maintenance(&vault.0, &[], MaintenanceOperation::Audit).unwrap();
    let cancelled = cancel_knowledge_maintenance(&vault.0, &audit.id).unwrap();
    assert_eq!(cancelled.sources[0].status, SourceStatus::Current);
    assert_eq!(cancelled.pages, 1);
    assert_eq!(
        fs::read(vault.0.join("concepts/shared.md")).unwrap(),
        published
    );
    let update = begin_knowledge_maintenance(
        &vault.0,
        &["meeting.md".into()],
        MaintenanceOperation::Ingest,
    )
    .unwrap();
    vault.propose(&update, "update");
    finish_knowledge_maintenance(&vault.0, &update.id).unwrap();
    assert!(cancel_knowledge_maintenance(&vault.0, &update.id).is_err());
    assert!(
        knowledge_maintenance_plan(&vault.0)
            .unwrap()
            .active
            .unwrap()
            .ready
    );
    abandon_knowledge_maintenance(&vault.0, &update.id).unwrap();
}

#[test]
fn abandoned_review_retains_the_proposal_for_history_readback() {
    let vault = Vault::new();
    vault.write("meeting.md", "Input.");
    vault.write(
        "concepts/shared.md",
        "---\ntype: Concept\ntitle: Shared decision\n---\n\nCurrent version.\n",
    );
    let job = vault.begin();
    vault.propose(&job, "update");
    let ready = finish_knowledge_maintenance(&vault.0, &job.id).unwrap();
    assert!(ready.active.as_ref().unwrap().ready);
    let before = knowledge_change(&vault.0, &job.id, "concepts/shared.md").unwrap();
    abandon_knowledge_maintenance(&vault.0, &job.id).unwrap();
    let archived = knowledge_change(&vault.0, &job.id, "concepts/shared.md").unwrap();
    assert_eq!(archived.after, before.after);
    assert!(fs::read_to_string(vault.0.join("concepts/shared.md"))
        .unwrap()
        .contains("Current version."));
}

#[test]
fn failed_work_is_durable_and_explicit_retry_does_not_destroy_sources() {
    let vault = Vault::new();
    vault.write("meeting.md", "Input.");
    let job = vault.begin();
    assert!(finish_knowledge_maintenance(&vault.0, &job.id).is_err());
    assert_eq!(vault.begin().id, job.id);
    assert_eq!(
        knowledge_maintenance_plan(&vault.0)
            .unwrap()
            .active
            .unwrap()
            .id,
        job.id
    );
    assert!(Path::new(&job.staging).exists());
    let view = abandon_knowledge_maintenance(&vault.0, &job.id).unwrap();
    assert!(!Path::new(&job.staging).exists());
    assert!(view.active.is_none());
    assert_eq!(view.sources[0].status, SourceStatus::Attention);
    assert_eq!(view.activity[0].status, ActivityStatus::Discarded);
    let retry = begin_knowledge_maintenance(
        &vault.0,
        &["meeting.md".into()],
        MaintenanceOperation::Ingest,
    )
    .unwrap();
    assert_ne!(retry.id, job.id);
    assert_eq!(
        fs::read_to_string(vault.0.join("meeting.md")).unwrap(),
        "Input."
    );
}

#[test]
fn consequential_new_claims_require_confirmation() {
    let vault = Vault::new();
    vault.write("meeting.md", "A consequential decision.");
    let job = vault.begin();
    vault.propose(&job, "create");
    let manifest = Path::new(&job.staging).join(MANIFEST);
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
    value["pages"][0]["requires_review"] = json!(true);
    fs::write(&manifest, serde_json::to_vec(&value).unwrap()).unwrap();
    let view = finish_knowledge_maintenance(&vault.0, &job.id).unwrap();
    assert!(view.active.unwrap().changes[0].requires_review);
    assert!(!vault.0.join("concepts/shared.md").exists());
}

#[test]
fn healthy_audits_record_findings_without_inventing_a_change() {
    let vault = Vault::new();
    vault.write("meeting.md", "Evidence.");
    let ingest = vault.begin();
    vault.propose(&ingest, "create");
    finish_knowledge_maintenance(&vault.0, &ingest.id).unwrap();
    let audit = begin_knowledge_maintenance(&vault.0, &[], MaintenanceOperation::Audit).unwrap();
    fs::write(Path::new(&audit.staging).join(MANIFEST), serde_json::to_vec(&json!({
        "sources": audit.sources, "summary": "No contradictory evidence or unsupported claims were found.", "pages": []
    })).unwrap()).unwrap();
    let view = finish_knowledge_maintenance(&vault.0, &audit.id).unwrap();
    assert!(view.active.is_none());
    assert_eq!(view.activity[0].published, 0);
    assert!(view.activity[0].summary.contains("No contradictory"));
    assert_eq!(view.sources[0].status, SourceStatus::Current);
    assert_eq!(view.pages, 1);
}

#[test]
fn an_audit_cannot_mark_unprocessed_sources_current_without_evidence() {
    let vault = Vault::new();
    vault.write("meeting.md", "New evidence.");
    let audit = begin_knowledge_maintenance(&vault.0, &[], MaintenanceOperation::Audit).unwrap();
    fs::write(
        Path::new(&audit.staging).join(MANIFEST),
        serde_json::to_vec(&json!({
            "sources": audit.sources, "summary": "Nothing to do.", "pages": []
        }))
        .unwrap(),
    )
    .unwrap();
    assert!(finish_knowledge_maintenance(&vault.0, &audit.id).is_err());
    assert!(knowledge_maintenance_plan(&vault.0)
        .unwrap()
        .active
        .unwrap()
        .error
        .is_some());
}

#[cfg(unix)]
#[test]
fn source_symlinks_and_unbounded_files_are_refused() {
    let vault = Vault::new();
    let external = vault.0.parent().unwrap().join("external.md");
    fs::write(&external, "Private outside source.").unwrap();
    std::os::unix::fs::symlink(&external, vault.0.join("link.md")).unwrap();
    assert!(knowledge_maintenance_plan(&vault.0).is_err());
    fs::remove_file(vault.0.join("link.md")).unwrap();
    fs::write(vault.0.join("large.md"), vec![b'x'; MAX_TEXT_BYTES + 1]).unwrap();
    assert!(knowledge_maintenance_plan(&vault.0).is_err());
}

#[test]
fn archived_evidence_survives_later_source_edits() {
    let vault = Vault::new();
    vault.write("meeting.md", "Original cited evidence.");
    let job = vault.begin();
    vault.propose(&job, "create");
    finish_knowledge_maintenance(&vault.0, &job.id).unwrap();
    vault.write("meeting.md", "Later evidence.");
    let detail = knowledge_change(&vault.0, &job.id, "concepts/shared.md").unwrap();
    assert_eq!(detail.evidence["meeting.md"], "Original cited evidence.");
    vault.write(
        &format!(".wiki-history/{}/sources/meeting.md", job.id),
        "Tampered history.",
    );
    assert!(knowledge_change(&vault.0, &job.id, "concepts/shared.md").is_err());
}

#[test]
fn source_citations_encode_literal_url_characters() {
    let body = cited_body(
        b"---\ntype: Concept\ntitle: Evidence\n---\nEvidence.\n",
        &["notes/a%20b?# (final).md".into()],
        "concepts/evidence.md",
    )
    .unwrap();
    assert!(body.contains("../notes/a%2520b%3F%23%20%28final%29.md"));
}

#[test]
fn persisted_provenance_paths_are_revalidated_before_reading_evidence() {
    let vault = Vault::new();
    vault.write("meeting.md", "Evidence.");
    let job = vault.begin();
    vault.propose(&job, "conflict");
    finish_knowledge_maintenance(&vault.0, &job.id).unwrap();
    let state = vault.0.join(STATE);
    let mut ledger: serde_json::Value = serde_json::from_slice(&fs::read(&state).unwrap()).unwrap();
    ledger["active"]["source_generations"]["../../outside.md"] = json!("forged");
    ledger["active"]["changes"][0]["sources"] = json!(["../../outside.md"]);
    fs::write(state, serde_json::to_vec(&ledger).unwrap()).unwrap();
    assert_eq!(
        knowledge_change(&vault.0, &job.id, "concepts/shared.md")
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidData
    );
}

#[cfg(unix)]
#[test]
fn literal_backslashes_in_filenames_cannot_become_parent_traversals() {
    let vault = Vault::new();
    vault.write(r"x\..\..\escaped.md", "Untrusted filename.");
    assert!(knowledge_maintenance_plan(&vault.0).is_err());
    assert!(begin_knowledge_maintenance(&vault.0, &[], MaintenanceOperation::Ingest).is_err());
}

#[test]
fn classified_pages_publish_with_valid_index_and_depth_correct_source_citations() {
    let vault = Vault::new();
    vault.write("meeting.md", "Original evidence remains unchanged.");
    let job = vault.begin();
    vault.propose(&job, "create");
    let path = "concepts/产品 规划/极光/决策.md";
    let staging = Path::new(&job.staging);
    let target = staging.join(path);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::rename(staging.join("concepts/shared.md"), &target).unwrap();
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(staging.join(MANIFEST)).unwrap()).unwrap();
    manifest["pages"][0]["path"] = json!(path);
    manifest["categories"] = json!({path:["产品 规划", "极光"]});
    fs::write(
        staging.join(MANIFEST),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let result = finish_knowledge_maintenance(&vault.0, &job.id).unwrap();
    assert!(result.active.is_none());
    assert_eq!(result.pages, 1);
    let published = fs::read_to_string(vault.0.join(path)).unwrap();
    assert!(published.contains("[meeting.md](../../../meeting.md)"));
    let index = fs::read_to_string(vault.0.join("index.md")).unwrap();
    assert!(index.contains("/concepts/产品%20规划/极光/决策.md"));
    assert_eq!(
        fs::read_to_string(vault.0.join("meeting.md")).unwrap(),
        "Original evidence remains unchanged."
    );
    assert!(knowledge_page_path(path));
    for path in [
        "concepts/../a.md",
        "concepts/a//b.md",
        "concepts/a/.private/b.md",
        "concepts/a/b/c/d/e.md",
        "concepts/a/log.md",
    ] {
        assert!(!knowledge_page_path(path), "{path}");
    }
}

#[test]
fn legacy_page_classification_keeps_paths_bytes_and_links_and_is_durable() {
    let vault = Vault::new();
    let old = "---\ntype: Concept\ntitle: 发布计划\n---\n\nSee [index](./indexing.md).\n";
    vault.write("concepts/plan.md", old);
    vault.write(
        "concepts/indexing.md",
        "---\ntype: Concept\ntitle: 离线索引\n---\n\nExisting evidence.\n",
    );
    let job = begin_knowledge_maintenance(&vault.0, &[], MaintenanceOperation::Audit).unwrap();
    assert!(job.sources.is_empty());
    assert!(job.classification_required);
    let categories =
        json!({"concepts/plan.md":["产品","极光"],"concepts/indexing.md":["技术","检索"]});
    let manifest = json!({"sources":[],"summary":"Classified existing pages by their subject matter without changing their content.","pages":[],"categories":categories});
    fs::write(
        Path::new(&job.staging).join(MANIFEST),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let result = finish_knowledge_maintenance(&vault.0, &job.id).unwrap();
    assert!(result.active.is_none());
    assert_eq!(result.categories["concepts/plan.md"], vec!["产品", "极光"]);
    assert_eq!(
        result.categories["concepts/indexing.md"],
        vec!["技术", "检索"]
    );
    assert_eq!(
        fs::read_to_string(vault.0.join("concepts/plan.md")).unwrap(),
        old
    );
    assert!(!vault.0.join("concepts/产品/极光/plan.md").exists());
    assert_eq!(
        knowledge_maintenance_plan(&vault.0).unwrap().categories,
        result.categories
    );
    assert_eq!(
        knowledge_maintenance_job(&vault.0, &job.id)
            .unwrap()
            .categories,
        result.categories
    );
}

#[test]
fn classification_refuses_missing_pages_unsafe_topics_and_changed_legacy_content() {
    for (path, topics, changed) in [
        ("concepts/missing.md", vec!["Product"], false),
        ("concepts/old.md", vec!["../outside"], false),
        ("concepts/old.md", vec![".private"], false),
        ("concepts/old.md", vec!["Product"], true),
    ] {
        let vault = Vault::new();
        vault.write(
            "concepts/old.md",
            "---\ntype: Concept\ntitle: Old\n---\n\nOriginal.\n",
        );
        let job = begin_knowledge_maintenance(&vault.0, &[], MaintenanceOperation::Audit).unwrap();
        if changed {
            vault.write("concepts/old.md", "External edit must survive.");
        }
        let manifest = json!({"sources":[],"summary":"Classify existing content.","pages":[],"categories":{path:topics}});
        fs::write(
            Path::new(&job.staging).join(MANIFEST),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        assert!(finish_knowledge_maintenance(&vault.0, &job.id).is_err());
        assert!(knowledge_maintenance_plan(&vault.0)
            .unwrap()
            .categories
            .is_empty());
        if changed {
            assert_eq!(
                fs::read_to_string(vault.0.join("concepts/old.md")).unwrap(),
                "External edit must survive."
            );
        }
    }
}
