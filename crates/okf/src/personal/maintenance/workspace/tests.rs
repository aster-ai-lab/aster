use super::*;

#[test]
fn text_snapshot_citations_use_original_lines_and_preserve_verbatim_bytes() {
    let workspace = Workspace::new();
    let original = "# 原文\r\n\r\n- 窗口：04:10–04:35。\r\n最后一行没有换行";
    let source = WorkspaceKnowledgeSource::text("手册.md", original.as_bytes()).unwrap();
    let input = WorkspaceKnowledgeInput::new(
        &workspace.0,
        "line-citations",
        &format!("sha256:{}", content_generation(original.as_bytes())),
        vec![source],
    )
    .unwrap();
    let job = begin_workspace_knowledge_maintenance(&input)
        .unwrap()
        .unwrap();
    let packet = fs::read_to_string(Path::new(&job.staging).join(&job.sources[0])).unwrap();
    let body = split_note(&packet).body;
    assert!(body.contains("L3: - 窗口：04:10–04:35。\r\n"));
    let restored = body
        .strip_prefix('\n')
        .unwrap()
        .strip_suffix('\n')
        .unwrap()
        .split_inclusive('\n')
        .enumerate()
        .map(|(index, line)| {
            let (coordinate, text) = line.split_once(": ").unwrap();
            assert_eq!(coordinate, format!("L{}", index + 1));
            text
        })
        .collect::<String>();
    assert_eq!(restored.as_bytes(), original.as_bytes());
    assert!(packet.contains(&format!(
        "source_generation: \"sha256:{}\"",
        content_generation(original.as_bytes())
    )));
    cancel_workspace_knowledge_maintenance(&input, &job.id).unwrap();
}

#[test]
fn original_line_labels_recompile_an_existing_publication_without_source_edits() {
    let workspace = Workspace::new();
    let bytes = fs::read(workspace.0.join("src/lib.rs")).unwrap();
    let legacy = WorkspaceKnowledgeInput::new(
        &workspace.0,
        "coordinate-upgrade",
        &format!("sha256:{}", content_generation(&bytes)),
        vec![WorkspaceKnowledgeSource::new(
            "src/lib.rs",
            &bytes,
            "source_start_line: 1\nsource_end_line: 1\n",
            std::str::from_utf8(&bytes).unwrap(),
        )
        .unwrap()],
    )
    .unwrap();
    let job = begin_workspace_knowledge_maintenance(&legacy)
        .unwrap()
        .unwrap();
    workspace.propose(&job, "An existing explanation.", false, false);
    finish_workspace_knowledge_maintenance(&legacy, &job.id).unwrap();
    let current = workspace.input("coordinate-upgrade");
    let next = begin_workspace_knowledge_maintenance(&current)
        .unwrap()
        .unwrap();
    let packet = fs::read_to_string(Path::new(&next.staging).join(&next.sources[0])).unwrap();
    assert!(packet.contains("L1: pub fn answer() -> u8 { 42 }\n"));
    assert_eq!(fs::read(workspace.0.join("src/lib.rs")).unwrap(), bytes);
    assert!(
        fs::read_to_string(current.output_root().join("concepts/overview.md"))
            .unwrap()
            .contains("An existing explanation.")
    );
    assert!(workspace_knowledge_page(&current, "concepts/overview.md").is_err());
    cancel_workspace_knowledge_maintenance(&current, &next.id).unwrap();
}

#[test]
fn workspace_document_reuses_only_original_verified_frozen_projection() {
    let workspace = Workspace::new();
    let bytes = b"admitted image bytes";
    let policy = crate::ExtractPolicy {
        recognize: None,
        max_text_bytes: MAX_TEXT_BYTES,
        max_package_bytes: MAX_PACKAGE_BYTES,
        max_output: MAX_TEXT_BYTES,
        deadline: std::time::Instant::now() + std::time::Duration::from_secs(5),
    };
    let crate::WorkspaceDocumentRead::Parsed(projection) = crate::project_workspace_document(
        bytes,
        "processor:1",
        crate::Extracted::Parsed {
            blocks: vec![crate::ExtractedBlock {
                kind: crate::BlockKind::Paragraph,
                level: 0,
                text: "Frozen recognized fact".into(),
            }],
            truncated: false,
        },
        &policy,
    ) else {
        panic!("projection")
    };
    let input = WorkspaceKnowledgeInput::new(
        &workspace.0,
        "binding",
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        vec![WorkspaceKnowledgeSource::document("scan.png", bytes, &projection).unwrap()],
    )
    .unwrap();
    assert!(retained_workspace_document_projection(
        &workspace.0,
        "binding",
        "scan.png",
        bytes,
        "processor:1"
    )
    .unwrap()
    .is_none());
    assert!(!input.output_root().exists());
    let job = begin_workspace_knowledge_maintenance(&input)
        .unwrap()
        .unwrap();
    let retained = retained_workspace_document_projection(
        &workspace.0,
        "binding",
        "scan.png",
        bytes,
        "processor:1",
    )
    .unwrap()
    .unwrap();
    assert_eq!(retained, projection);
    assert!(retained_workspace_document_projection(
        &workspace.0,
        "binding",
        "scan.png",
        b"changed",
        "processor:1"
    )
    .unwrap()
    .is_none());
    assert!(retained_workspace_document_projection(
        &workspace.0,
        "binding",
        "scan.png",
        bytes,
        "processor:2"
    )
    .unwrap()
    .is_none());
    cancel_workspace_knowledge_maintenance(&input, &job.id).unwrap();
    assert_eq!(
        retained_workspace_document_projection(
            &workspace.0,
            "binding",
            "scan.png",
            bytes,
            "processor:1"
        )
        .unwrap(),
        Some(projection)
    );
    fs::write(
        input
            .output_root()
            .join(".wiki-history")
            .join(&job.id)
            .join("sources")
            .join(&job.sources[0]),
        "tampered",
    )
    .unwrap();
    assert!(retained_workspace_document_projection(
        &workspace.0,
        "binding",
        "scan.png",
        bytes,
        "processor:1"
    )
    .is_err());
}

struct Workspace(PathBuf);

impl Workspace {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "okf-workspace-{}-{}-{}",
            std::process::id(),
            clock_ms().unwrap(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/lib.rs"), "pub fn answer() -> u8 { 42 }\n").unwrap();
        Self(root)
    }

    fn input(&self, revision: &str) -> WorkspaceKnowledgeInput {
        let bytes = fs::read(self.0.join("src/lib.rs")).unwrap();
        WorkspaceKnowledgeInput::new(
            &self.0,
            revision,
            &format!("sha256:{}", content_generation(&bytes)),
            vec![WorkspaceKnowledgeSource::text("src/lib.rs", &bytes).unwrap()],
        )
        .unwrap()
    }

    fn propose(&self, job: &KnowledgeJob, text: &str, update: bool, review: bool) {
        let root = Path::new(&job.staging);
        fs::write(
            root.join("concepts/overview.md"),
            format!("---\ntype: Synthesis\ntitle: Workspace overview\n---\n{text}\n"),
        )
        .unwrap();
        fs::write(root.join(MANIFEST), serde_json::to_vec(&serde_json::json!({
            "sources": job.sources, "summary": "Explained the supplied source", "categories": {},
            "pages": [{"path":"concepts/overview.md","title":"Workspace overview","kind":"synthesis","change":if update {"update"} else {"create"},"reason":"Grounded in the supplied code","requires_review":review,"sources":job.sources}]
        })).unwrap()).unwrap();
    }

    fn propose_pair(&self, job: &KnowledgeJob, version: &str, update: bool, review: bool) {
        let mut pages = Vec::new();
        for name in ["a", "b"] {
            let path = format!("concepts/{name}.md");
            fs::write(
                Path::new(&job.staging).join(&path),
                format!("---\ntype: Synthesis\ntitle: Page {name}\n---\n{version} {name}\n"),
            )
            .unwrap();
            pages.push(serde_json::json!({"path":path,"title":format!("Page {name}"),"kind":"synthesis","change":if update {"update"} else {"create"},"reason":"Based on the source","requires_review":review && name == "b","sources":job.sources}));
        }
        fs::write(Path::new(&job.staging).join(MANIFEST), serde_json::to_vec(&serde_json::json!({"sources":job.sources,"summary":"A coherent pair of pages","pages":pages})).unwrap()).unwrap();
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        if let Ok(scopes) = fs::read_dir(self.0.join("wiki/workspace")) {
            for scope in scopes.flatten() {
                if let Ok(path) = job_directory(&scope.path(), &"a".repeat(64)) {
                    if let Some(parent) = path.parent() {
                        let _ = fs::remove_dir_all(parent);
                    }
                }
            }
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn workspace_wiki_reuses_only_current_equivalent_evidence_without_merging_bindings() {
    let fixture = Workspace::new();
    let previous = fixture.input("previous-authorized-binding");
    let job = begin_workspace_knowledge_maintenance(&previous)
        .unwrap()
        .unwrap();
    fixture.propose_pair(&job, "Reusable explanation", false, false);
    finish_workspace_knowledge_maintenance(&previous, &job.id).unwrap();
    let original = fs::read(previous.output_root().join("concepts/a.md")).unwrap();
    let input = fixture.input("another-authorized-binding");
    assert!(reuse_workspace_knowledge_maintenance(&input, &previous).unwrap());
    assert_ne!(input.output_root(), previous.output_root());
    let view = workspace_knowledge_maintenance_plan(&input).unwrap();
    assert_eq!(view.pages, 2);
    assert!(view.active.is_none());
    assert_eq!(view.activity.len(), 1);
    assert!(workspace_knowledge_page(&input, "concepts/a.md")
        .unwrap()
        .body
        .contains("Reusable explanation"));
    assert!(begin_workspace_knowledge_maintenance(&input)
        .unwrap()
        .is_none());
    assert!(!reuse_workspace_knowledge_maintenance(&input, &previous).unwrap());
    retire_workspace_knowledge(input.output_root(), input.binding_revision()).unwrap();
    assert_eq!(
        fs::read(previous.output_root().join("concepts/a.md")).unwrap(),
        original
    );

    let other = Workspace::new();
    let foreign = other.input("foreign-root");
    assert!(!reuse_workspace_knowledge_maintenance(&foreign, &previous).unwrap());
    assert!(!foreign.output_root().exists());
    fs::write(
        fixture.0.join("src/lib.rs"),
        "pub fn answer() -> u8 { 43 }\n",
    )
    .unwrap();
    let changed = fixture.input("changed-evidence");
    assert!(!reuse_workspace_knowledge_maintenance(&changed, &previous).unwrap());
    assert!(!changed.output_root().exists());
    let current_previous = fixture.input("previous-authorized-binding");
    assert!(!reuse_workspace_knowledge_maintenance(&changed, &current_previous).unwrap());
    assert!(!changed.output_root().exists());
}

#[test]
fn workspace_wiki_reuse_refuses_modified_pages_and_preserves_existing_candidates() {
    let fixture = Workspace::new();
    let previous = fixture.input("published-binding");
    let job = begin_workspace_knowledge_maintenance(&previous)
        .unwrap()
        .unwrap();
    fixture.propose_pair(&job, "Verified explanation", false, false);
    finish_workspace_knowledge_maintenance(&previous, &job.id).unwrap();
    let input = fixture.input("busy-binding");
    let active = begin_workspace_knowledge_maintenance(&input)
        .unwrap()
        .unwrap();
    assert!(!reuse_workspace_knowledge_maintenance(&input, &previous).unwrap());
    assert_eq!(
        workspace_knowledge_maintenance_plan(&input)
            .unwrap()
            .active
            .unwrap()
            .id,
        active.id
    );
    cancel_workspace_knowledge_maintenance(&input, &active.id).unwrap();
    let fresh = fixture.input("fresh-binding");
    fs::write(
        previous.output_root().join("concepts/a.md"),
        "---\ntype: Synthesis\ntitle: Page a\n---\nAltered explanation\n",
    )
    .unwrap();
    assert!(!reuse_workspace_knowledge_maintenance(&fresh, &previous).unwrap());
    assert!(!fresh.output_root().exists());
}

#[test]
fn workspace_wiki_catalog_summary_uses_existing_owner_without_scanning_or_creating() {
    let fixture = Workspace::new();
    let input = fixture.input("catalog-summary");
    let summary =
        || workspace_knowledge_summary(input.output_root(), input.binding_revision()).unwrap();
    assert_eq!(summary().status, WorkspaceKnowledgeStatus::NotGenerated);
    assert!(!input.output_root().exists());
    let job = begin_workspace_knowledge_maintenance(&input)
        .unwrap()
        .unwrap();
    assert_eq!(summary().status, WorkspaceKnowledgeStatus::Generating);
    fail_workspace_knowledge_job(input.output_root(), input.binding_revision(), &job.id).unwrap();
    assert_eq!(summary().status, WorkspaceKnowledgeStatus::Failed);
    fixture.propose(&job, "The source returns 42.", false, true);
    let ready = finish_workspace_knowledge_maintenance(&input, &job.id).unwrap();
    assert_eq!(summary().status, WorkspaceKnowledgeStatus::Review);
    let change = &ready.active.unwrap().changes[0];
    review_workspace_knowledge_change(&input, &job.id, &change.path, &change.generation, true)
        .unwrap();
    let state = fs::read(input.output_root().join(STATE)).unwrap();
    fs::remove_file(fixture.0.join("src/lib.rs")).unwrap();
    let published = summary();
    assert_eq!(published.status, WorkspaceKnowledgeStatus::Published);
    assert!(published.updated_ms.is_some());
    assert_eq!(published.pages, 1);
    assert_eq!(published.sources, 1);
    assert_eq!(fs::read(input.output_root().join(STATE)).unwrap(), state);
    // Published means a saved generation exists, never that live sources are current.
    assert!(workspace_knowledge_summary(input.output_root(), "another-scope").is_err());
    with_workspace(input.output_root(), input.binding_revision(), |_, _| {
        assert_eq!(
            workspace_knowledge_summary(input.output_root(), input.binding_revision())
                .unwrap_err()
                .kind(),
            io::ErrorKind::WouldBlock
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn workspace_wiki_failed_candidate_returns_to_review_only_after_owner_validation() {
    let f = Workspace::new();
    let input = f.input("failed-candidate");
    let job = begin_workspace_knowledge_maintenance(&input)
        .unwrap()
        .unwrap();
    fail_workspace_knowledge_job(input.output_root(), input.binding_revision(), &job.id).unwrap();
    fs::write(Path::new(&job.staging).join(MANIFEST), b"{}").unwrap();
    assert!(finish_workspace_knowledge_maintenance(&input, &job.id).is_err());
    assert!(workspace_knowledge_maintenance_plan(&input)
        .unwrap()
        .active
        .unwrap()
        .error
        .is_some());
    f.propose_pair(&job, "Recovered candidate", false, true);
    let plan = finish_workspace_knowledge_maintenance(&input, &job.id).unwrap();
    let recovered = plan.active.unwrap();
    assert!(recovered.ready && recovered.error.is_none());
    assert!(recovered
        .changes
        .iter()
        .any(|change| change.decision == ChangeDecision::Pending));
    assert!(!input.output_root().join("concepts/a.md").exists());
    assert!(workspace_knowledge_maintenance_plan(&input)
        .unwrap()
        .active
        .unwrap()
        .error
        .is_none());
    assert_eq!(
        fs::read_to_string(f.0.join("src/lib.rs")).unwrap(),
        "pub fn answer() -> u8 { 42 }\n"
    );
}

#[test]
fn workspace_wiki_generation_waits_for_all_review_decisions_before_replacing_pages() {
    let f = Workspace::new();
    let old = f.input("batch-scope");
    let first = begin_workspace_knowledge_maintenance(&old)
        .unwrap()
        .unwrap();
    f.propose_pair(&first, "Old generation", false, false);
    finish_workspace_knowledge_maintenance(&old, &first.id).unwrap();
    let before = snapshot(old.output_root()).unwrap();
    fs::write(f.0.join("src/lib.rs"), "pub fn answer() -> u8 { 43 }\n").unwrap();
    let input = f.input("batch-scope");
    let job = begin_workspace_knowledge_maintenance(&input)
        .unwrap()
        .unwrap();
    f.propose_pair(&job, "New generation", true, true);
    let plan = finish_workspace_knowledge_maintenance(&input, &job.id).unwrap();
    let pending = plan.active.unwrap();
    assert_eq!(pending.changes[0].decision, ChangeDecision::Approved);
    assert_eq!(pending.changes[1].decision, ChangeDecision::Pending);
    assert_eq!(snapshot(input.output_root()).unwrap(), before);
    let review = &pending.changes[1];
    let plan =
        review_workspace_knowledge_change(&input, &job.id, &review.path, &review.generation, true)
            .unwrap();
    assert!(plan.active.is_none());
    for name in ["a", "b"] {
        assert!(
            fs::read_to_string(input.output_root().join(format!("concepts/{name}.md")))
                .unwrap()
                .contains("New generation")
        );
    }
    let old_page = workspace_knowledge_change(&input, &first.id, "concepts/a.md").unwrap();
    assert!(old_page.after.contains("Old generation"));
}

fn pending_pair(
    f: &Workspace,
) -> (
    WorkspaceKnowledgeInput,
    KnowledgeJob,
    BTreeMap<String, Vec<u8>>,
) {
    let old = f.input("batch-recovery");
    let first = begin_workspace_knowledge_maintenance(&old)
        .unwrap()
        .unwrap();
    f.propose_pair(&first, "Old generation", false, false);
    finish_workspace_knowledge_maintenance(&old, &first.id).unwrap();
    let before = snapshot(old.output_root()).unwrap();
    fs::write(f.0.join("src/lib.rs"), "pub fn answer() -> u8 { 43 }\n").unwrap();
    let input = f.input("batch-recovery");
    let job = begin_workspace_knowledge_maintenance(&input)
        .unwrap()
        .unwrap();
    f.propose_pair(&job, "New generation", true, true);
    finish_workspace_knowledge_maintenance(&input, &job.id).unwrap();
    (input, job, before)
}

#[test]
fn workspace_wiki_generation_write_failure_restores_every_page_and_refuses_partial_reads() {
    let f = Workspace::new();
    let (input, _, before) = pending_pair(&f);
    let error = with_workspace(
        input.output_root(),
        input.binding_revision(),
        |root, ledger| {
            for change in &mut ledger.active.as_mut().unwrap().changes {
                change.decision = ChangeDecision::Approved;
            }
            publication::publish_with(root, ledger, &input, |_| {
                assert_eq!(
                    workspace_knowledge_read(&root.join("concepts/a.md"))
                        .unwrap_err()
                        .kind(),
                    io::ErrorKind::WouldBlock
                );
                assert!(fs::read_to_string(root.join("concepts/a.md"))
                    .unwrap()
                    .contains("New generation"));
                Err(io::Error::other("injected write failure"))
            })
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("injected write failure"));
    assert_eq!(snapshot(input.output_root()).unwrap(), before);
    assert_eq!(
        fs::read_to_string(f.0.join("src/lib.rs")).unwrap(),
        "pub fn answer() -> u8 { 43 }\n"
    );
}

#[test]
fn workspace_wiki_generation_read_recovers_an_interrupted_original_publication() {
    let f = Workspace::new();
    let (input, _, before) = pending_pair(&f);
    let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_workspace(
            input.output_root(),
            input.binding_revision(),
            |root, ledger| {
                for change in &mut ledger.active.as_mut().unwrap().changes {
                    change.decision = ChangeDecision::Approved;
                }
                publication::publish_with(root, ledger, &input, |_| {
                    panic!("publication process interrupted")
                })
            },
        )
        .unwrap();
    }));
    assert!(crashed.is_err());
    assert_ne!(snapshot(input.output_root()).unwrap(), before);
    assert_eq!(
        try_workspace_knowledge_read(input.output_root())
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    assert_ne!(snapshot(input.output_root()).unwrap(), before);
    let read = workspace_knowledge_read(&input.output_root().join("concepts/a.md")).unwrap();
    assert!(read.is_some());
    assert_eq!(snapshot(input.output_root()).unwrap(), before);
    drop(read);
    let plan = workspace_knowledge_maintenance_plan(&input).unwrap();
    assert_eq!(
        plan.active.unwrap().error.as_deref(),
        Some("knowledge.publication_interrupted")
    );
}

#[test]
fn workspace_wiki_generation_recovery_never_overwrites_a_later_external_edit() {
    let f = Workspace::new();
    let (input, _, _) = pending_pair(&f);
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_workspace(
            input.output_root(),
            input.binding_revision(),
            |root, ledger| {
                for change in &mut ledger.active.as_mut().unwrap().changes {
                    change.decision = ChangeDecision::Approved;
                }
                publication::publish_with(root, ledger, &input, |_| {
                    panic!("publication process interrupted")
                })
            },
        )
        .unwrap();
    }));
    let path = input.output_root().join("concepts/a.md");
    fs::write(&path, "User edit after interruption.").unwrap();
    let guard = workspace_knowledge_read(input.output_root()).unwrap();
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "User edit after interruption."
    );
    assert!(
        fs::read_to_string(input.output_root().join("concepts/b.md"))
            .unwrap()
            .contains("Old generation")
    );
    drop(guard);
    let plan = workspace_knowledge_maintenance_plan(&input).unwrap();
    assert_eq!(
        plan.active.unwrap().error.as_deref(),
        Some("knowledge.publication_conflict")
    );
}

#[test]
fn workspace_wiki_citations_open_frozen_evidence_and_report_changed_or_deleted_sources() {
    let workspace = Workspace::new();
    let input = workspace.input("scope-a");
    let job = begin_workspace_knowledge_maintenance(&input)
        .unwrap()
        .unwrap();
    let reference = format!("../{}#L1", job.sources[0]);
    let foreign = format!(
        "../.wiki-history/{}/sources/{}",
        "a".repeat(64),
        job.sources[0]
    );
    workspace.propose(
        &job,
        &format!("The function returns [42]({reference}). [Foreign snapshot]({foreign})"),
        false,
        false,
    );
    finish_workspace_knowledge_maintenance(&input, &job.id).unwrap();
    let page = "concepts/overview.md";
    let citation = workspace_knowledge_citation(&input, page, &reference).unwrap();
    assert!(workspace_knowledge_citation(&input, page, &foreign).is_err());
    assert_eq!(citation.source_path, "src/lib.rs");
    assert!(citation.current);
    assert_eq!(
        citation.source_generation,
        format!(
            "sha256:{}",
            content_generation(&fs::read(workspace.0.join("src/lib.rs")).unwrap())
        )
    );
    let frozen = fs::read(&citation.snapshot_path).unwrap();
    assert!(String::from_utf8_lossy(&frozen).contains("pub fn answer() -> u8 { 42 }"));
    let footer = format!("../.wiki-history/{}/sources/{}", job.id, job.sources[0]);
    assert_eq!(
        workspace_knowledge_citation(&input, page, &footer)
            .unwrap()
            .snapshot_path,
        citation.snapshot_path
    );
    fs::write(
        workspace.0.join("src/lib.rs"),
        "pub fn answer() -> u8 { 43 }\n",
    )
    .unwrap();
    assert!(
        !workspace_knowledge_citation(&workspace.input("scope-a"), page, &reference)
            .unwrap()
            .current
    );
    fs::remove_file(workspace.0.join("src/lib.rs")).unwrap();
    let deleted = WorkspaceKnowledgeInput::new(
        &workspace.0,
        "scope-a",
        &format!("sha256:{}", "a".repeat(64)),
        vec![],
    )
    .unwrap();
    assert!(
        !workspace_knowledge_citation(&deleted, page, &reference)
            .unwrap()
            .current
    );
    assert_eq!(fs::read(&citation.snapshot_path).unwrap(), frozen);
    for invalid in [
        "../_sources/missing.md",
        "../../outside.md",
        "https://example.com",
    ] {
        assert!(workspace_knowledge_citation(&input, page, invalid).is_err());
    }
    let other = WorkspaceKnowledgeInput::new(
        &workspace.0,
        "scope-b",
        &format!("sha256:{}", "a".repeat(64)),
        vec![],
    )
    .unwrap();
    assert!(workspace_knowledge_citation(&other, page, &reference).is_err());
}

#[test]
fn workspace_wiki_citations_refuse_modified_pages_or_evidence() {
    let workspace = Workspace::new();
    let input = workspace.input("scope-a");
    let job = begin_workspace_knowledge_maintenance(&input)
        .unwrap()
        .unwrap();
    let reference = format!("../{}", job.sources[0]);
    workspace.propose(&job, &format!("[Source]({reference})"), false, false);
    finish_workspace_knowledge_maintenance(&input, &job.id).unwrap();
    let page = "concepts/overview.md";
    let citation = workspace_knowledge_citation(&input, page, &reference).unwrap();
    let original_page = fs::read(input.output_root().join(page)).unwrap();
    fs::write(
        input.output_root().join(page),
        [&original_page[..], b"\nChanged"].concat(),
    )
    .unwrap();
    assert!(workspace_knowledge_citation(&input, page, &reference)
        .unwrap_err()
        .to_string()
        .contains("published_page_changed"));
    fs::write(input.output_root().join(page), original_page).unwrap();
    fs::write(citation.snapshot_path, "forged evidence").unwrap();
    assert!(workspace_knowledge_citation(&input, page, &reference)
        .unwrap_err()
        .to_string()
        .contains("history_changed"));
}

#[test]
fn workspace_wiki_answer_pages_require_published_bytes_and_current_sources() {
    let workspace = Workspace::new();
    let input = workspace.input("scope-a");
    let job = begin_workspace_knowledge_maintenance(&input)
        .unwrap()
        .unwrap();
    let reference = format!("../{}", job.sources[0]);
    workspace.propose(
        &job,
        &format!("Answer returns [42]({reference})."),
        false,
        false,
    );
    finish_workspace_knowledge_maintenance(&input, &job.id).unwrap();
    let path = "concepts/overview.md";
    let page = workspace_knowledge_page(&input, path).unwrap();
    assert!(page.body.contains("[42]"));
    assert!(!page.sources.is_empty());
    assert!(page
        .sources
        .iter()
        .all(|source| source.current && source.page_generation == page.generation));
    fs::write(
        input.output_root().join(path),
        format!("{}\nEdited", page.body),
    )
    .unwrap();
    assert!(workspace_knowledge_page(&input, path).is_err());
    fs::write(input.output_root().join(path), &page.body).unwrap();
    fs::write(
        workspace.0.join("src/lib.rs"),
        "pub fn answer() -> u8 { 43 }\n",
    )
    .unwrap();
    assert!(workspace_knowledge_page(&workspace.input("scope-a"), path).is_err());
    assert!(workspace_knowledge_page(&input, "../outside.md").is_err());
}

#[cfg(unix)]
#[test]
fn workspace_wiki_citations_refuse_symlinked_history() {
    let workspace = Workspace::new();
    let input = workspace.input("scope-a");
    let job = begin_workspace_knowledge_maintenance(&input)
        .unwrap()
        .unwrap();
    let reference = format!("../{}", job.sources[0]);
    workspace.propose(&job, &format!("[Source]({reference})"), false, false);
    finish_workspace_knowledge_maintenance(&input, &job.id).unwrap();
    let citation =
        workspace_knowledge_citation(&input, "concepts/overview.md", &reference).unwrap();
    let outside = workspace.0.join("copied-evidence.md");
    fs::copy(&citation.snapshot_path, &outside).unwrap();
    fs::remove_file(&citation.snapshot_path).unwrap();
    std::os::unix::fs::symlink(&outside, &citation.snapshot_path).unwrap();
    assert!(workspace_knowledge_citation(&input, "concepts/overview.md", &reference).is_err());
}

#[test]
fn workspace_wiki_plan_never_prepares_a_vault_or_starts_a_job() {
    let workspace = Workspace::new();
    let input = workspace.input("tenant-a:permission-a");
    let plan = workspace_knowledge_maintenance_plan(&input).unwrap();
    assert_eq!(plan.sources.len(), 1);
    assert_eq!(plan.sources[0].title, "src/lib.rs");
    assert_eq!(plan.sources[0].status, SourceStatus::Pending);
    assert!(plan.active.is_none());
    assert!(!workspace.0.join("wiki").exists());
}

#[test]
fn workspace_wiki_uses_the_original_job_and_keeps_live_sources_editable() {
    let workspace = Workspace::new();
    let input = workspace.input("scope-a");
    let job = begin_workspace_knowledge_maintenance(&input)
        .unwrap()
        .unwrap();
    let staging =
        workspace_knowledge_compile_workspace(input.output_root(), &job.id, "scope-a").unwrap();
    assert_eq!(staging, Path::new(&job.staging));
    assert!(staging.join(&job.sources[0]).is_file());
    assert!(!input.output_root().join(&job.sources[0]).exists());
    assert!(knowledge_maintenance_plan(input.output_root()).is_err());
    assert!(knowledge_compile_workspace(input.output_root(), &job.id).is_err());
    assert!(begin_knowledge_maintenance(&workspace.0, &[], MaintenanceOperation::Ingest).is_err());
    workspace.propose(&job, "The entry point returns 42.", false, false);
    fs::write(
        workspace.0.join("src/lib.rs"),
        "pub fn answer() -> u8 { 43 }\n",
    )
    .unwrap();
    let fresh = workspace.input("scope-a");
    let error = finish_workspace_knowledge_maintenance(&fresh, &job.id).unwrap_err();
    assert!(error.to_string().contains("workspace_source_changed"));
    assert!(!input.output_root().join("index.md").exists());
    cancel_workspace_knowledge_maintenance(&fresh, &job.id).unwrap();
    assert!(
        workspace_knowledge_compile_workspace(input.output_root(), &job.id, "scope-a").is_err()
    );
}

#[test]
fn workspace_wiki_publishes_frozen_evidence_and_unchanged_inputs_start_no_new_job() {
    let workspace = Workspace::new();
    let input = workspace.input("scope-a");
    let before = fs::read(workspace.0.join("src/lib.rs")).unwrap();
    let job = begin_workspace_knowledge_maintenance(&input)
        .unwrap()
        .unwrap();
    workspace.propose(&job, "The supplied entry point returns 42.", false, false);
    let plan = finish_workspace_knowledge_maintenance(&input, &job.id).unwrap();
    assert!(plan.active.is_none());
    assert_eq!(plan.pages, 1);
    assert_eq!(plan.sources[0].status, SourceStatus::Current);
    let page = fs::read_to_string(input.output_root().join("concepts/overview.md")).unwrap();
    assert!(page.contains("The supplied entry point returns 42."));
    assert!(page.contains(&format!(
        "../.wiki-history/{}/sources/{}",
        job.id, job.sources[0]
    )));
    let evidence = fs::read_to_string(
        input
            .output_root()
            .join(".wiki-history")
            .join(&job.id)
            .join("sources")
            .join(&job.sources[0]),
    )
    .unwrap();
    assert!(evidence.contains("resource: \"workspace:src/lib.rs\""));
    assert!(evidence.contains("source_start_line: 1\nsource_end_line: 1"));
    assert!(evidence.contains("pub fn answer() -> u8 { 42 }"));
    assert_eq!(fs::read(workspace.0.join("src/lib.rs")).unwrap(), before);
    assert!(begin_workspace_knowledge_maintenance(&input)
        .unwrap()
        .is_none());
    assert!(!Path::new(&job.staging).exists());
}

#[test]
fn workspace_wiki_machine_updates_reuse_publication_without_personal_review_rules() {
    let workspace = Workspace::new();
    let first = workspace.input("scope-a");
    let job = begin_workspace_knowledge_maintenance(&first)
        .unwrap()
        .unwrap();
    workspace.propose(&job, "Version one, from the original source.", false, false);
    finish_workspace_knowledge_maintenance(&first, &job.id).unwrap();
    fs::write(
        workspace.0.join("src/lib.rs"),
        "pub fn answer() -> u8 { 43 }\n",
    )
    .unwrap();
    let second = workspace.input("scope-a");
    let next = begin_workspace_knowledge_maintenance(&second)
        .unwrap()
        .unwrap();
    assert_ne!(job.id, next.id);
    workspace.propose(&next, "Version two, from the changed source.", true, false);
    let plan = finish_workspace_knowledge_maintenance(&second, &next.id).unwrap();
    assert!(plan.active.is_none());
    assert_eq!(plan.pages, 1);
    assert!(
        fs::read_to_string(second.output_root().join("concepts/overview.md"))
            .unwrap()
            .contains("Version two")
    );
    let old = workspace_knowledge_change(&second, &job.id, "concepts/overview.md").unwrap();
    assert!(old.after.contains("Version one"));
    assert!(old
        .evidence
        .values()
        .any(|body| body.contains("returns") || body.contains("{ 42 }")));
}

#[test]
fn workspace_wiki_rejects_scope_reuse_and_model_changes_to_source_snapshots() {
    let workspace = Workspace::new();
    let input = workspace.input("scope-a");
    let job = begin_workspace_knowledge_maintenance(&input)
        .unwrap()
        .unwrap();
    let other = workspace.input("scope-b");
    assert_ne!(input.output_root(), other.output_root());
    assert!(workspace_knowledge_maintenance_plan(&other)
        .unwrap()
        .active
        .is_none());
    let other_job = begin_workspace_knowledge_maintenance(&other)
        .unwrap()
        .unwrap();
    assert!(finish_workspace_knowledge_maintenance(&other, &job.id).is_err());
    cancel_workspace_knowledge_maintenance(&other, &other_job.id).unwrap();
    assert!(
        workspace_knowledge_compile_workspace(input.output_root(), &job.id, "scope-b").is_err()
    );
    workspace.propose(&job, "A candidate must not rewrite evidence.", false, false);
    fs::write(
        Path::new(&job.staging).join(&job.sources[0]),
        "Invented evidence",
    )
    .unwrap();
    assert!(finish_workspace_knowledge_maintenance(&input, &job.id)
        .unwrap_err()
        .to_string()
        .contains("source_modified"));
    assert!(!input.output_root().join("concepts/overview.md").exists());
    assert!(fs::read_to_string(workspace.0.join("src/lib.rs"))
        .unwrap()
        .contains("{ 42 }"));
}

#[test]
fn workspace_wiki_uncertain_updates_keep_the_original_review_and_freshness_checks() {
    let workspace = Workspace::new();
    let input = workspace.input("scope-a");
    let job = begin_workspace_knowledge_maintenance(&input)
        .unwrap()
        .unwrap();
    workspace.propose(&job, "An uncertain claim awaiting review.", false, true);
    let plan = finish_workspace_knowledge_maintenance(&input, &job.id).unwrap();
    let change = &plan.active.as_ref().unwrap().changes[0];
    assert!(change.requires_review);
    assert!(!input.output_root().join("concepts/overview.md").exists());
    let plan =
        review_workspace_knowledge_change(&input, &job.id, &change.path, &change.generation, true)
            .unwrap();
    assert!(plan.active.is_none());
    assert!(input.output_root().join("concepts/overview.md").exists());
}

#[test]
fn workspace_wiki_refuses_bad_inputs_and_never_adopts_an_unowned_output() {
    let workspace = Workspace::new();
    assert!(WorkspaceKnowledgeSource::text("../outside.rs", b"source").is_err());
    assert!(
        WorkspaceKnowledgeSource::text("src/large.rs", &vec![b'a'; MAX_TEXT_BYTES + 1]).is_err()
    );
    assert!(WorkspaceKnowledgeInput::new(&workspace.0, "scope", "unknown", vec![]).is_err());
    let digest = format!("sha256:{}", "0".repeat(64));
    assert!(WorkspaceKnowledgeInput::new(
        &workspace.0,
        "scope",
        &digest,
        vec![
            WorkspaceKnowledgeSource::text("src/a.rs", b"a").unwrap(),
            WorkspaceKnowledgeSource::text("src/a.rs", b"a").unwrap()
        ]
    )
    .is_err());
    let input = workspace.input("scope");
    fs::create_dir_all(input.output_root()).unwrap();
    fs::write(
        input.output_root().join("manual.md"),
        "User-authored content",
    )
    .unwrap();
    assert!(begin_workspace_knowledge_maintenance(&input).is_err());
    assert_eq!(
        fs::read_to_string(input.output_root().join("manual.md")).unwrap(),
        "User-authored content"
    );
    assert!(!input.output_root().join(STATE).exists());
}

#[test]
fn workspace_wiki_review_requires_the_complete_current_packet_even_for_unchanged_cited_sources() {
    let workspace = Workspace::new();
    let input = workspace.input("scope-a");
    let job = begin_workspace_knowledge_maintenance(&input)
        .unwrap()
        .unwrap();
    workspace.propose(&job, "An uncertain claim.", false, true);
    let plan = finish_workspace_knowledge_maintenance(&input, &job.id).unwrap();
    let change = &plan.active.unwrap().changes[0];
    fs::write(workspace.0.join("new.md"), "New evidence.").unwrap();
    let fresh = WorkspaceKnowledgeInput::new(
        &workspace.0,
        "scope-a",
        &format!("sha256:{}", "d".repeat(64)),
        vec![
            WorkspaceKnowledgeSource::text(
                "src/lib.rs",
                &fs::read(workspace.0.join("src/lib.rs")).unwrap(),
            )
            .unwrap(),
            WorkspaceKnowledgeSource::text("new.md", b"New evidence.").unwrap(),
        ],
    )
    .unwrap();
    assert!(review_workspace_knowledge_change(
        &fresh,
        &job.id,
        &change.path,
        &change.generation,
        true
    )
    .unwrap_err()
    .to_string()
    .contains("workspace_source_changed"));
    assert!(!input.output_root().join(&change.path).exists());
    abandon_workspace_knowledge_job(input.output_root(), input.binding_revision(), &job.id)
        .unwrap();
}

#[test]
fn workspace_wiki_bindings_cannot_inherit_each_others_sources_or_published_context() {
    let workspace = Workspace::new();
    let first = workspace.input("scope-a");
    let job = begin_workspace_knowledge_maintenance(&first)
        .unwrap()
        .unwrap();
    workspace.propose(&job, "Context from scope A only.", false, false);
    finish_workspace_knowledge_maintenance(&first, &job.id).unwrap();
    let other = WorkspaceKnowledgeInput::new(
        &workspace.0,
        "scope-b",
        &format!("sha256:{}", "b".repeat(64)),
        vec![WorkspaceKnowledgeSource::text("README.md", b"Scope B evidence only.").unwrap()],
    )
    .unwrap();
    let plan = workspace_knowledge_maintenance_plan(&other).unwrap();
    assert_eq!(plan.pages, 0);
    assert_eq!(plan.sources.len(), 1);
    assert_eq!(plan.sources[0].title, "README.md");
    let next = begin_workspace_knowledge_maintenance(&other)
        .unwrap()
        .unwrap();
    let stage = Path::new(&next.staging);
    assert!(!stage.join("concepts/overview.md").exists());
    assert!(!next
        .sources
        .iter()
        .any(|source| job.sources.contains(source)));
    assert!(
        fs::read_to_string(first.output_root().join("concepts/overview.md"))
            .unwrap()
            .contains("scope A")
    );
    cancel_workspace_knowledge_maintenance(&other, &next.id).unwrap();
}

#[test]
fn workspace_wiki_retirement_revokes_running_and_review_jobs_without_reading_live_sources() {
    let workspace = Workspace::new();
    let input = workspace.input("scope-a");
    let job = begin_workspace_knowledge_maintenance(&input)
        .unwrap()
        .unwrap();
    fs::remove_file(workspace.0.join("src/lib.rs")).unwrap();
    retire_workspace_knowledge(input.output_root(), input.binding_revision()).unwrap();
    retire_workspace_knowledge(input.output_root(), input.binding_revision()).unwrap();
    assert!(workspace_knowledge_compile_workspace(
        input.output_root(),
        &job.id,
        input.binding_revision()
    )
    .is_err());
    assert!(!Path::new(&job.staging).exists());
    let plan = workspace_knowledge_maintenance_plan(&input).unwrap();
    assert!(plan.active.is_none());
    assert_eq!(plan.activity[0].status, ActivityStatus::Discarded);
    assert!(plan
        .sources
        .iter()
        .all(|source| source.status == SourceStatus::Pending));
    fs::write(workspace.0.join("src/lib.rs"), "pub fn next() {}\n").unwrap();
    let next = workspace.input("scope-a");
    let job = begin_workspace_knowledge_maintenance(&next)
        .unwrap()
        .unwrap();
    workspace.propose(&job, "Uncertain context.", false, true);
    assert!(
        finish_workspace_knowledge_maintenance(&next, &job.id)
            .unwrap()
            .active
            .unwrap()
            .ready
    );
    retire_workspace_knowledge(next.output_root(), next.binding_revision()).unwrap();
    assert!(workspace_knowledge_maintenance_plan(&next)
        .unwrap()
        .active
        .is_none());
    assert!(!next.output_root().join("concepts/overview.md").exists());
}
