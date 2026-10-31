#![cfg(test)]

use super::*;

#[test]
fn cross_workspace_search_requires_an_explicit_boolean_host_capability() {
    let parse = |value| {
        crate::host_context::studio_host_from_meta(Some(&serde_json::Map::from_iter([(
            "asterHost".into(),
            value,
        )])))
    };
    assert!(!parse(json!({})).unwrap().workspace_wiki_search);
    assert!(
        parse(json!({"workspaceWikiSearch":true}))
            .unwrap()
            .workspace_wiki_search
    );
    assert!(parse(json!({"workspaceWikiSearch":"all"})).is_err());
}

pub(super) fn fixture() -> (tempfile::TempDir, std::path::PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let source = b"pub fn workspace_answer() -> u32 { 42 }\n";
    std::fs::write(root.path().join("source.rs"), source).unwrap();
    let input = aster_okf::WorkspaceKnowledgeInput::new(
        root.path(),
        "workspace-tool-scope",
        &format!("sha256:{}", "a".repeat(64)),
        vec![aster_okf::WorkspaceKnowledgeSource::text("source.rs", source).unwrap()],
    )
    .unwrap();
    let job = aster_okf::begin_workspace_knowledge_maintenance(&input)
        .unwrap()
        .unwrap();
    let stage = std::path::Path::new(&job.staging);
    std::fs::write(stage.join("concepts/overview.md"), "---\ntype: Synthesis\ntitle: Workspace overview\n---\n\nworkspaceanswer42marker is the documented result.\n").unwrap();
    std::fs::write(stage.join(".wiki-changes.json"), serde_json::to_vec(&json!({
        "sources":job.sources,"summary":"Workspace overview","categories":{},
        "pages":[{"path":"concepts/overview.md","title":"Workspace overview","kind":"synthesis","change":"create","reason":"From admitted evidence","requires_review":false,"sources":job.sources}]
    })).unwrap()).unwrap();
    aster_okf::finish_workspace_knowledge_maintenance(&input, &job.id).unwrap();
    (root, input.output_root().to_path_buf())
}

async fn indexed(root: &std::path::Path) -> Arc<WikiIndex> {
    let index = Arc::new(WikiIndex::open(root).unwrap());
    let admitted = index.reconcile_now(None).await.unwrap();
    assert!(index.wait_published(admitted.source_revision).await);
    index
}

#[tokio::test]
async fn workspace_search_wiki_uses_only_the_current_owned_publication() {
    let (root, wiki) = fixture();
    std::fs::create_dir_all(root.path().join("wiki/local")).unwrap();
    std::fs::write(
        root.path().join("wiki/local/private.md"),
        "workspaceanswer42marker PERSONAL_SENTINEL",
    )
    .unwrap();
    let index = indexed(&wiki).await;
    let tool = SearchWikiTool::new(Some(Arc::clone(&index)), Arc::default());
    let ctx = ToolContext::new(root.path().to_path_buf());
    let result = tool
        .execute(&json!({"query":"workspaceanswer42marker"}), &ctx)
        .await
        .unwrap();
    assert!(result.success, "{result:?}");
    assert!(result.content.contains("Workspace overview"));
    assert!(result.content.contains("concepts/overview.md"));
    assert!(!result.content.contains("PERSONAL_SENTINEL"));
    let wire: Value = serde_json::from_str(&result.content).unwrap();
    assert_eq!(wire["hits"].as_array().unwrap().len(), 1);
    assert_eq!(wire["hits"][0]["path"], "concepts/overview.md");
    assert!(root.path().join(".aster-agent-retrievals").is_file());
    assert!(!wiki.join(".aster-agent-retrievals").exists());
    let shared = search_published(&wiki, "workspaceanswer42marker");
    assert_eq!(shared.phase, WikiSearchPhase::Published);
    assert_eq!(
        shared.source_revision,
        index
            .search("workspaceanswer42marker")
            .unwrap()
            .source_revision
    );
    assert!(shared
        .hits
        .iter()
        .any(|hit| hit.path == "concepts/overview.md"));
    let foreign = tempfile::tempdir().unwrap();
    assert!(
        !tool
            .execute(
                &json!({"query":"workspaceanswer42marker"}),
                &ToolContext::new(foreign.path().into())
            )
            .await
            .unwrap()
            .success
    );
}

#[tokio::test]
async fn cold_workspace_wiki_search_reuses_original_index_and_refuses_unowned_or_personal_roots() {
    let (root, wiki) = fixture();
    let body = search_admitted_body(&wiki, "workspaceanswer42marker").await;
    assert_eq!(body["phase"], "published");
    assert_eq!(body["hits"].as_array().unwrap().len(), 1);
    let first = shared_index(&wiki).unwrap();
    let second = shared_index(&wiki).unwrap();
    assert!(Arc::ptr_eq(&first, &second));
    let personal = root.path().join("wiki/local");
    std::fs::create_dir_all(&personal).unwrap();
    std::fs::write(personal.join("note.md"), "PRIVATE_SENTINEL").unwrap();
    assert_eq!(
        search_admitted_body(&personal, "PRIVATE_SENTINEL").await["phase"],
        "not-ready"
    );
    assert!(registered(&personal).is_none());
}

#[test]
fn workspace_search_host_reply_retains_attribution_and_refuses_unpublished_or_foreign_results() {
    let hit = json!({"taskId":"task-b","taskName":"Task B","workspaceId":"workspace-b","workspaceName":"Project B","sourceRoot":"/work/b","outputRoot":"/work/b/wiki/workspace/epoch","path":"concepts/overview.md","name":"Overview","passage":"Evidence"});
    let valid = json!({"sessionId":"session-a","scope":"all","query":"evidence","phase":"published","searched":2,"pending":0,"complete":true,"nextCursor":null,"hits":[hit]});
    validate_host_reply(&valid, "session-a", Scope::All, "evidence").unwrap();
    for invalid in [
        json!({"sessionId":"session-b"}),
        json!({"scope":"current"}),
        json!({"phase":"not-ready"}),
        json!({"hits":[{"path":"../private.md"}]}),
    ] {
        let mut value = valid.clone();
        for (key, v) in invalid.as_object().unwrap() {
            value[key] = v.clone();
        }
        assert!(validate_host_reply(&value, "session-a", Scope::All, "evidence").is_err());
    }
}

#[test]
fn library_search_evidence_requires_its_own_identity_without_fabricating_a_task() {
    let hit = json!({"ownerKind":"library","ownerId":"library-a","libraryId":"library-a","sourceRoot":"/work/materials","outputRoot":"/work/materials/wiki/workspace/epoch","path":"concepts/overview.md","name":"Overview","passage":"Source evidence"});
    let valid = json!({"sessionId":"session-a","scope":"all","query":"evidence","phase":"published","searched":1,"pending":0,"complete":true,"nextCursor":null,"hits":[hit]});
    validate_host_reply(&valid, "session-a", Scope::All, "evidence").unwrap();
    assert!(valid["hits"][0]["taskId"].is_null());
    for patch in [
        json!({"libraryId":""}),
        json!({"ownerId":"library-b"}),
        json!({"ownerId":"invalid\u{0000}id","libraryId":"invalid\u{0000}id"}),
        json!({"ownerKind":"guessed"}),
    ] {
        let mut invalid = valid.clone();
        for (key, value) in patch.as_object().unwrap() {
            invalid["hits"][0][key] = value.clone();
        }
        assert!(validate_host_reply(&invalid, "session-a", Scope::All, "evidence").is_err());
    }
}

#[tokio::test]
async fn workspace_search_wiki_refuses_scope_overrides_and_respects_live_denies() {
    let (root, wiki) = fixture();
    let denies = Arc::new(Mutex::new(Vec::new()));
    let tool = SearchWikiTool::new(Some(indexed(&wiki).await), Arc::clone(&denies));
    let ctx = ToolContext::new(root.path().into());
    // Legacy models still emit scope=local; admit it as current instead of failing closed.
    let local = tool
        .execute(&json!({"query":"workspaceanswer42marker","scope":"local"}), &ctx)
        .await
        .unwrap();
    assert!(local.success, "{local:?}");
    for args in [
        json!({"query":"answer","scope":"team"}),
        json!({"query":"answer","root":"/tmp"}),
        json!({"query":1}),
        json!({"query":""}),
    ] {
        assert!(!tool.execute(&args, &ctx).await.unwrap().success, "{args}");
    }
    denies.lock().unwrap().push("search_wiki".into());
    let result = tool
        .execute(&json!({"query":"workspaceanswer42marker"}), &ctx)
        .await
        .unwrap();
    assert!(!result.success);
    assert_eq!(result.content, "denied");
}

#[tokio::test]
async fn workspace_search_wiki_refuses_personal_roots_and_waits_for_publication_without_blocking() {
    let (root, wiki) = fixture();
    let index = indexed(&wiki).await;
    let tool = SearchWikiTool::new(Some(index), Arc::default());
    let ctx = ToolContext::new(root.path().into());
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(wiki.join(".wiki-maintenance.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        tool.execute(&json!({"query":"workspaceanswer42marker"}), &ctx),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result.content, "not-ready");
    drop(lock);
    let personal = root.path().join("wiki/local");
    std::fs::create_dir_all(&personal).unwrap();
    std::fs::write(
        personal.join("note.md"),
        "---\nparser_generation: builtin:1\n---\nPERSONAL_SENTINEL",
    )
    .unwrap();
    let tool = SearchWikiTool::new(Some(indexed(&personal).await), Arc::default());
    assert!(
        !tool
            .execute(&json!({"query":"PERSONAL_SENTINEL"}), &ctx)
            .await
            .unwrap()
            .success
    );
    assert_eq!(
        search_published(&personal, "PERSONAL_SENTINEL").phase,
        WikiSearchPhase::NotReady
    );
}
