use std::time::Duration;

use super::super::{WikiIndex, WikiSearchPhase, WIKI_PASSAGE_CHARS};

const JIA: &str = "jia.md";
const YI: &str = "yi.md";
const BING: &str = "bing.md";
const TAIL: &str = "TAIL-MARKER-9f3a-not-in-passage";
const SECRET: &str = "SECRET-WIKI-9f3a";
const NEW_TEXT: &str = "未发布丙句9f3a只在新甲页";
const BURST_OLD: &str = "突发版本甲独有9f3a";
const BURST_NEW: &str = "突发版本乙独有9f3a更长";

fn page(title: &str, description: &str, body: &str) -> String {
    format!("---\ntitle: \"{title}\"\ndescription: \"{description}\"\n---\n{body}\n")
}

#[tokio::test]
async fn workspace_wiki_search_excludes_history_and_returns_one_passage_per_page() {
    let root = tempfile::tempdir().unwrap();
    let source = b"Source evidence.\n";
    std::fs::write(root.path().join("source.md"), source).unwrap();
    let input = a3s_okf::WorkspaceKnowledgeInput::new(
        root.path(),
        "dedup-scope",
        &format!("sha256:{}", "a".repeat(64)),
        vec![a3s_okf::WorkspaceKnowledgeSource::text("source.md", source).unwrap()],
    )
    .unwrap();
    a3s_okf::begin_workspace_knowledge_maintenance(&input).unwrap();
    let wiki = input.output_root();
    std::fs::create_dir_all(wiki.join("concepts")).unwrap();
    let body = (0..800)
        .map(|index| format!("dedupworkspacearticle marker at line {index}.\n"))
        .collect::<String>();
    std::fs::write(
        wiki.join("concepts/current.md"),
        page("Current", "Current encyclopedia", &body),
    )
    .unwrap();
    for path in [
        ".wiki-history/old/sources/source.md",
        ".wiki-staging/old/candidate.md",
        "_sources/source.md",
        "index.md",
        "log.md",
    ] {
        let path = wiki.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            path,
            page(
                "Duplicate",
                "History must stay out",
                "dedupworkspacearticle HISTORY_SENTINEL",
            ),
        )
        .unwrap();
    }
    let index = WikiIndex::open(wiki).unwrap();
    let admitted = index.reconcile_now(None).await.unwrap();
    assert_eq!(admitted.read_paths, ["concepts/current.md"]);
    assert!(index.wait_published(admitted.source_revision).await);
    let result = index.search("dedupworkspacearticle").unwrap();
    assert_eq!(result.phase, WikiSearchPhase::Published);
    assert_eq!(result.hits.len(), 1);
    assert_eq!(result.hits[0].path, "concepts/current.md");
    assert!(!result.hits[0].passage.contains("HISTORY_SENTINEL"));
    assert!(index
        .content_reads()
        .iter()
        .all(|path| path == "concepts/current.md"));
}

#[tokio::test]
async fn workspace_wiki_search_deduplicates_identical_pages_and_reconciles_removal() {
    let root = tempfile::tempdir().unwrap();
    let text = page(
        "Same page",
        "Published copy",
        "dedupcopytoken Shared facts.",
    );
    for path in ["first.md", "second.md"] {
        std::fs::write(root.path().join(path), &text).unwrap();
    }
    let index = WikiIndex::open(root.path()).unwrap();
    let admitted = index.reconcile_now(None).await.unwrap();
    assert!(index.wait_published(admitted.source_revision).await);
    let hits = index.search("dedupcopytoken").unwrap().hits;
    assert_eq!(hits.len(), 1);
    assert!(hits[0].passage.contains("Shared facts"));
    std::fs::remove_file(root.path().join(&hits[0].path)).unwrap();
    let next = index.reconcile_now(None).await.unwrap();
    assert!(index.wait_published(next.source_revision).await);
    let remaining = index.search("dedupcopytoken").unwrap().hits;
    assert_eq!(remaining.len(), 1);
    assert_ne!(hits[0].path, remaining[0].path);
}

#[tokio::test]
async fn workspace_wiki_index_waits_for_the_original_publication_lock_and_refuses_changed_pages() {
    let root = tempfile::tempdir().unwrap();
    let source = b"pub fn example() {}\n";
    std::fs::write(root.path().join("source.rs"), source).unwrap();
    let input = a3s_okf::WorkspaceKnowledgeInput::new(
        root.path(),
        "index-publication",
        &format!("sha256:{}", "a".repeat(64)),
        vec![a3s_okf::WorkspaceKnowledgeSource::text("source.rs", source).unwrap()],
    )
    .unwrap();
    let job = a3s_okf::begin_workspace_knowledge_maintenance(&input)
        .unwrap()
        .unwrap();
    let wiki = input.output_root();
    std::fs::create_dir_all(wiki.join("concepts")).unwrap();
    let file = wiki.join("concepts/a.md");
    std::fs::write(
        &file,
        page(
            "Publication",
            "Published version",
            "uniquepublicationoldtoken",
        ),
    )
    .unwrap();
    let index = WikiIndex::open(wiki).unwrap();
    let admitted = index.reconcile_now(None).await.unwrap();
    assert!(index.wait_published(admitted.source_revision).await);
    assert!(!index
        .search("uniquepublicationoldtoken")
        .unwrap()
        .hits
        .is_empty());
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(wiki.join(".wiki-maintenance.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    assert_eq!(
        index.search("uniquepublicationoldtoken").unwrap().phase,
        WikiSearchPhase::NotReady
    );
    assert!(index.reconcile_now(None).await.is_err());
    drop(lock);
    std::fs::write(
        &file,
        page("Publication", "New version", "uniquepublicationnewtoken"),
    )
    .unwrap();
    assert!(index
        .search("uniquepublicationoldtoken")
        .unwrap()
        .hits
        .is_empty());
    let next = index
        .reconcile_now(Some(vec!["concepts/a.md".into()]))
        .await
        .unwrap();
    assert!(index.wait_published(next.source_revision).await);
    assert!(!index
        .search("uniquepublicationnewtoken")
        .unwrap()
        .hits
        .is_empty());
    let relocated = root.path().join("relocated-wiki");
    std::fs::rename(wiki, &relocated).unwrap();
    assert_eq!(
        index.search("uniquepublicationnewtoken").unwrap().phase,
        WikiSearchPhase::NotReady
    );
    std::fs::rename(&relocated, wiki).unwrap();
    drop(index);
    a3s_okf::cancel_workspace_knowledge_job(wiki, input.binding_revision(), &job.id).unwrap();
}

async fn wait_for(index: &WikiIndex, query: &str, needle: &str) -> bool {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
    while tokio::time::Instant::now() < deadline {
        let found = index.search(query).expect("poll");
        if found.phase == WikiSearchPhase::Published
            && found.lexical_engine == "a3s_vec_fts_v1"
            && found
                .hits
                .iter()
                .any(|hit| hit.passage.contains(needle) || hit.name == needle)
        {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    false
}

fn jia_body() -> String {
    format!(
        "北城豆沙月饼烘烤只写在甲页。北城豆沙月饼烘烤只写在甲页。北城豆沙月饼烘烤只写在甲页。{}{TAIL}",
        "甲".repeat(1_700)
    )
}

#[tokio::test]
async fn wiki_index_serves_published_a3s_vec_snapshots() {
    let root = tempfile::tempdir().expect("wiki root");
    let wiki = root.path();
    std::fs::create_dir_all(wiki.join(".ssh")).unwrap();
    std::fs::write(
        wiki.join(".ssh").join("page.md"),
        page("密钥页", "不可索引", SECRET),
    )
    .unwrap();
    std::fs::write(wiki.join("id_rsa.md"), page("私钥", "不可索引", SECRET)).unwrap();
    std::fs::write(wiki.join("notes.txt"), SECRET).unwrap();
    std::fs::write(
        wiki.join(JIA),
        page("产品手册甲", "甲页短说明", &jia_body()),
    )
    .unwrap();
    std::fs::write(
        wiki.join(YI),
        page(
            "产品手册乙",
            "乙页短说明",
            "南城桂花豆沙烘烤只写在乙页。乙页独有句子。",
        ),
    )
    .unwrap();
    std::fs::write(
        wiki.join(BING),
        page("产品手册丙", "丙页短说明", "西城核桃酥只写在丙页。"),
    )
    .unwrap();

    let index = WikiIndex::open(wiki).expect("wiki index");
    index.hold_publication(true);
    let admitted = index.reconcile_now(None).await.expect("admit");
    assert!(admitted.read_paths.iter().all(|path| path.ends_with(".md")));
    assert!(!admitted
        .indexed_paths
        .iter()
        .any(|path| path.contains(".ssh") || path.contains("id_rsa")));
    assert!(admitted.indexed_paths.contains(&JIA.to_owned()));
    assert!(admitted.indexed_paths.contains(&YI.to_owned()));
    assert!(admitted.indexed_paths.contains(&BING.to_owned()));
    assert!(index
        .content_reads()
        .iter()
        .all(|path| !path.contains(".ssh") && !path.contains("id_rsa")));
    let reads = index.content_reads();
    let pending = index.search("北城豆沙").expect("search while unpublished");
    assert_eq!(pending.phase, WikiSearchPhase::NotReady);
    assert!(pending.hits.is_empty());
    assert_eq!(index.content_reads(), reads);
    assert!(!pending.lexical_engine.is_empty());
    for hit in &pending.hits {
        assert!(!hit.passage.contains("北城豆沙"));
        assert!(!hit.passage.contains(SECRET));
        assert!(!hit.passage.contains(TAIL));
    }

    index.hold_publication(false);
    assert!(
        index.wait_published(admitted.source_revision).await,
        "first generation did not publish"
    );
    let found = index.search("北城豆沙").expect("published search");
    assert_eq!(found.phase, WikiSearchPhase::Published);
    assert_eq!(found.lexical_engine, "a3s_vec_fts_v1");
    assert!(!found.hits.is_empty());
    assert!(found
        .hits
        .iter()
        .all(|hit| hit.source_revision == found.source_revision));
    assert_eq!(found.hits[0].name, "产品手册甲");
    assert!(found.hits[0].passage.contains("北城豆沙"));
    assert!(found.hits[0].passage.chars().count() <= WIKI_PASSAGE_CHARS);
    assert!(!found.hits[0].passage.contains(TAIL));
    assert!(!found.hits.iter().any(|hit| hit.passage.contains(SECRET)));
    if found.hits.len() > 1 {
        assert!(found.hits[0].score >= found.hits[1].score);
        assert_eq!(found.hits[0].name, "产品手册甲");
    }
    let missed = index.search("火星殖民地供水").expect("miss");
    assert_eq!(missed.phase, WikiSearchPhase::Published);
    assert!(missed.hits.is_empty());
    assert_eq!(missed.lexical_engine, "a3s_vec_fts_v1");

    index.hold_publication(true);
    std::fs::write(wiki.join(JIA), page("产品手册甲", "甲页短说明", NEW_TEXT)).unwrap();
    let edited = index
        .reconcile_now(Some(vec![JIA.to_owned()]))
        .await
        .expect("edit one page");
    assert_eq!(edited.read_paths, vec![JIA.to_owned()]);
    let reads = index.content_reads();
    let during = index.search(NEW_TEXT).expect("search during build");
    assert_eq!(index.content_reads(), reads);
    assert!(during
        .hits
        .iter()
        .all(|hit| !hit.passage.contains(NEW_TEXT)));
    assert_ne!(during.phase, WikiSearchPhase::NotReady);
    let still = index.search("北城豆沙").expect("previous generation");
    assert!(still
        .hits
        .iter()
        .any(|hit| hit.passage.contains("北城豆沙")));
    assert!(still.hits.iter().all(|hit| !hit.passage.contains(NEW_TEXT)));
    assert!(still
        .hits
        .iter()
        .all(|hit| hit.source_revision == still.source_revision));

    index.hold_publication(false);
    assert!(index.wait_published(edited.source_revision).await);
    let updated = index.search(NEW_TEXT).expect("new generation");
    assert_eq!(updated.lexical_engine, "a3s_vec_fts_v1");
    assert_eq!(updated.hits[0].name, "产品手册甲");
    assert!(updated.hits[0].passage.contains(NEW_TEXT));
    assert!(updated
        .hits
        .iter()
        .all(|hit| !hit.passage.contains("北城豆沙")));
    let updated_page = updated
        .hits
        .iter()
        .find(|hit| hit.name == "产品手册甲")
        .expect("updated page");
    assert_eq!(updated_page.source_revision, updated.source_revision);
    assert!(updated
        .hits
        .iter()
        .filter(|hit| hit.name != "产品手册甲")
        .all(|hit| hit.source_revision < updated.source_revision));

    std::fs::write(
        wiki.join(YI),
        page("产品手册乙", "乙页短说明", "乙页改成了桂花新句。"),
    )
    .unwrap();
    let one = index
        .reconcile_now(Some(vec![YI.to_owned()]))
        .await
        .expect("one page");
    assert_eq!(one.read_paths, vec![YI.to_owned()]);
    assert!(!one
        .read_paths
        .iter()
        .any(|path| path == JIA || path == BING));
    assert!(index.wait_published(one.source_revision).await);

    index.hold_publication(true);
    let builds = index.builds();
    std::fs::write(wiki.join(YI), page("产品手册乙", "乙页短说明", BURST_OLD)).unwrap();
    index
        .reconcile_now(Some(vec![YI.to_owned()]))
        .await
        .expect("burst old");
    std::fs::write(wiki.join(YI), page("产品手册乙", "乙页短说明", BURST_NEW)).unwrap();
    let burst = index
        .reconcile_now(Some(vec![YI.to_owned()]))
        .await
        .expect("burst new");
    index.hold_publication(false);
    assert!(index.wait_published(burst.source_revision).await);
    assert_eq!(
        index.builds(),
        builds + 1,
        "a burst published one generation"
    );
    let latest = index.search(BURST_NEW).expect("latest burst");
    assert!(latest
        .hits
        .iter()
        .any(|hit| hit.passage.contains(BURST_NEW)));
    assert!(latest
        .hits
        .iter()
        .all(|hit| !hit.passage.contains(BURST_OLD)));

    std::fs::remove_file(wiki.join(BING)).unwrap();
    let removed = index.reconcile_now(None).await.expect("delete");
    assert!(removed.removed_paths.iter().any(|path| path == BING));
    assert!(removed.read_paths.is_empty());
    assert!(index.wait_published(removed.source_revision).await);
    let gone = index.search("西城核桃酥").expect("deleted page");
    assert!(gone.hits.is_empty());

    std::fs::remove_file(wiki.join(JIA)).unwrap();
    std::fs::remove_file(wiki.join(YI)).unwrap();
    let empty = index.reconcile_now(None).await.expect("empty");
    assert!(index.wait_published(empty.source_revision).await);
    let none = index.search("北城豆沙").expect("empty index");
    assert_eq!(none.phase, WikiSearchPhase::Published);
    assert!(none.hits.is_empty());
    assert_eq!(none.lexical_engine, "a3s_vec_fts_v1");
    let _ = Duration::from_millis(1);
}

#[tokio::test]
async fn wiki_index_rebuilds_when_a_page_is_saved() {
    let root = tempfile::tempdir().expect("wiki root");
    let wiki = root.path();
    std::fs::write(
        wiki.join(JIA),
        page("产品手册甲", "甲页短说明", "北城豆沙月饼烘烤只写在甲页。"),
    )
    .unwrap();
    let index = WikiIndex::start(wiki).expect("auto wiki index");
    let published = wait_for(&index, "北城豆沙", "产品手册甲").await;
    assert!(published, "the index did not build itself");

    std::fs::write(
        wiki.join(YI),
        page("产品手册乙", "乙页短说明", "南城新增句子只写在乙页。"),
    )
    .unwrap();
    let saw_new = wait_for(&index, "南城新增句子", "南城新增句子").await;
    assert!(saw_new, "a saved page did not schedule its own build");

    std::fs::write(
        wiki.join(YI),
        page("产品手册乙", "更新", "乙页换成全新内容。"),
    )
    .unwrap();
    assert!(wait_for(&index, "全新内容", "乙页换成全新内容").await);
    let revision = index.search("全新内容").unwrap().source_revision;
    std::fs::remove_file(wiki.join(YI)).unwrap();
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let result = index.search("全新内容").unwrap();
            if result.phase == WikiSearchPhase::Published && result.source_revision > revision {
                assert!(result.hits.is_empty());
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("a removed page must disappear without manual reconciliation");

    let (closed, observed) = std::sync::mpsc::channel();
    let owner = std::thread::spawn(move || {
        drop(index);
        closed.send(()).unwrap();
    });
    observed
        .recv_timeout(Duration::from_secs(3))
        .expect("closing the index must not wait on platform watcher registration");
    owner.join().unwrap();
}

#[tokio::test]
async fn wiki_index_finds_a_nested_studio_note_without_waiting_on_the_caller() {
    let root = tempfile::tempdir().expect("wiki root");
    let wiki = root.path().join("wiki");
    let local = wiki.join("local");
    std::fs::create_dir_all(&local).unwrap();
    std::fs::write(
        local.join("食谱.md"),
        "# 家常食谱\n\n红烧排骨只写在个人笔记9f3a。\n",
    )
    .unwrap();
    let index = WikiIndex::start(&wiki).expect("auto wiki index");
    let pending = index
        .search("红烧排骨")
        .expect("search returns before the build");
    if pending.phase == WikiSearchPhase::NotReady {
        assert!(pending.hits.is_empty());
    }
    assert!(
        wait_for(&index, "红烧排骨", "红烧排骨只写在个人笔记9f3a").await,
        "a nested personal note was not published"
    );
    let found = index.search("红烧排骨").expect("published");
    assert_eq!(found.hits[0].path, "local/食谱.md");
    assert_eq!(found.lexical_engine, "a3s_vec_fts_v1");
}

#[tokio::test]
async fn wiki_index_hits_the_compiled_sentence_and_misses_the_dropped_tail() {
    let root = tempfile::tempdir().expect("wiki root");
    let wiki = root.path();
    // a3s-okf's compile_concept output for 33 passages "句子 N 在这一段": it
    // keeps 32 and marks the tail absent. a3s-okf tests that truncation itself.
    let compiled = include_str!("wiki_index_okf_page.md");
    assert!(compiled.contains("句子 0 在这一段"));
    assert!(!compiled.contains("句子 32 在这一段"));
    std::fs::write(wiki.join("规范.md"), compiled).unwrap();
    std::fs::write(wiki.join("index.md"), "---\nokf_version: \"0.2\"\n---\n").unwrap();
    let index = WikiIndex::open(wiki).expect("wiki index");
    index.hold_publication(true);
    let admitted = index.reconcile_now(None).await.expect("admit");
    let pending = index.search("句子 0 在这一段").expect("unpublished");
    assert_eq!(pending.phase, WikiSearchPhase::NotReady);
    assert!(pending
        .hits
        .iter()
        .all(|hit| !hit.passage.contains("句子 0 在这一段")));
    index.hold_publication(false);
    assert!(index.wait_published(admitted.source_revision).await);
    let found = index.search("句子 0 在这一段").expect("published");
    assert_eq!(found.phase, WikiSearchPhase::Published);
    assert!(found
        .hits
        .iter()
        .any(|hit| hit.passage.contains("句子 0 在这一段")));
    assert!(found
        .hits
        .iter()
        .all(|hit| !hit.passage.contains("句子 32 在这一段")));
    let missed = index.search("句子 32 在这一段").expect("tail");
    assert_eq!(missed.phase, WikiSearchPhase::Published);
    assert!(missed
        .hits
        .iter()
        .all(|hit| !hit.passage.contains("句子 32 在这一段")));
}
