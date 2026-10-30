use super::*;

fn source(path: &str) -> Source {
    Source {
        area: SourceArea::Personal,
        path: path.into(),
        name: path.rsplit('/').next().unwrap().into(),
        parent: path.rsplit_once('/').map(|p| p.0).unwrap_or("").into(),
        kind: FileKind::Document,
        size_bytes: 12,
        modified_ms: 100,
        revision: "r1".into(),
    }
}

fn chunk(body: &str, entity: &str, vector: Vec<f32>) -> Chunk {
    Chunk {
        heading: "Source heading".into(),
        body: body.into(),
        event: body.into(),
        embedding: vector.clone(),
        event_embedding: vector,
        entities: vec![Entity {
            name: entity.into(),
            embedding: vec![0.0, 1.0, 0.0],
        }],
    }
}

struct TestIndex {
    _directory: tempfile::TempDir,
    index: Index,
}
impl std::ops::Deref for TestIndex {
    type Target = Index;
    fn deref(&self) -> &Index {
        &self.index
    }
}
impl std::ops::DerefMut for TestIndex {
    fn deref_mut(&mut self) -> &mut Index {
        &mut self.index
    }
}
fn index() -> TestIndex {
    let directory = tempfile::tempdir().unwrap();
    let index = Index::open(&directory.path().join("sag"), "fixture-v1", 3).unwrap();
    TestIndex {
        _directory: directory,
        index,
    }
}

fn query() -> CatalogQuery {
    CatalogQuery {
        view: View::All,
        folder: String::new(),
        search: String::new(),
        kind: None,
        sort_by: Sort::Modified,
        sort_order: Order::Desc,
        page: 1,
        limit: 20,
    }
}

#[test]
fn catalog_uses_declared_collection_ownership_for_personal_and_workspace_views() {
    let mut db = index();
    let personal = source("materials/own.md");
    let mut artifact = source("sessions/result.md");
    artifact.area = SourceArea::Workspace;
    db.sync(&[personal, artifact]).unwrap();
    let mut q = query();
    let personal_page = db.catalog(&q).unwrap();
    assert_eq!(personal_page.total, 1);
    assert_eq!(personal_page.entries[0].source.path, "materials/own.md");
    q.view = View::Artifacts;
    let workspace_page = db.catalog(&q).unwrap();
    assert_eq!(workspace_page.total, 1);
    assert_eq!(workspace_page.entries[0].source.path, "sessions/result.md");
    db.favorite("sessions/result.md", true).unwrap();
    q.view = View::Favorites;
    assert_eq!(
        db.catalog(&q).unwrap().entries[0].source.area,
        SourceArea::Workspace
    );
    assert!(SourceArea::parse("inferred").is_err());
}

#[test]
fn precise_association_expansion_returns_related_original_evidence() {
    let mut db = index();
    db.sync(&[source("a.md"), source("b.md"), source("unrelated.md")])
        .unwrap();
    db.replace(
        "a.md",
        "r1",
        &"a".repeat(64),
        "model-v1",
        &[chunk(
            "Aurora was founded by Mira.",
            "Mira",
            vec![1.0, 0.0, 0.0],
        )],
    )
    .unwrap();
    db.replace(
        "b.md",
        "r1",
        &"b".repeat(64),
        "model-v1",
        &[chunk(
            "Mira was born in Harbor.",
            "Mira",
            vec![0.0, 0.0, 1.0],
        )],
    )
    .unwrap();
    db.replace(
        "unrelated.md",
        "r1",
        &"c".repeat(64),
        "model-v1",
        &[chunk(
            "Orion builds bicycles.",
            "Orion",
            vec![0.0, 0.0, 1.0],
        )],
    )
    .unwrap();
    let fast = db
        .search("Aurora", &[1.0, 0.0, 0.0], SearchMode::Fast, 10, &[])
        .unwrap();
    assert_eq!(fast.hits.len(), 1);
    let precise = db
        .search("Aurora", &[1.0, 0.0, 0.0], SearchMode::Precise, 10, &[])
        .unwrap();
    assert_eq!(precise.hits.len(), 2);
    let related = precise.hits.iter().find(|h| h.path == "b.md").unwrap();
    assert!(related.related);
    assert_eq!(related.body, "Mira was born in Harbor.");
    assert_eq!(related.generation, "b".repeat(64));
    assert_eq!(related.ordinal, 0);
    assert_eq!(related.entities, vec!["Mira"]);
    let scoped = db
        .search(
            "Aurora",
            &[1.0, 0.0, 0.0],
            SearchMode::Precise,
            10,
            &["a.md".into()],
        )
        .unwrap();
    assert_eq!(scoped.hits.len(), 1);
}

#[test]
fn incremental_replacement_removes_old_entities_and_deleted_evidence() {
    let mut db = index();
    let mut a = source("a.md");
    db.sync(&[a.clone(), source("b.md")]).unwrap();
    db.replace(
        "a.md",
        "r1",
        &"a".repeat(64),
        "model",
        &[chunk(
            "Aurora belongs to Mira.",
            "Mira",
            vec![1.0, 0.0, 0.0],
        )],
    )
    .unwrap();
    db.replace(
        "b.md",
        "r1",
        &"b".repeat(64),
        "model",
        &[chunk("Mira lives in Harbor.", "Mira", vec![0.0, 0.0, 1.0])],
    )
    .unwrap();
    a.revision = "r2".into();
    db.sync(&[a.clone(), source("b.md")]).unwrap();
    assert!(db
        .search("Aurora", &[1.0, 0.0, 0.0], SearchMode::Fast, 5, &[])
        .unwrap()
        .hits
        .is_empty());
    assert!(matches!(
        db.replace(
            "a.md",
            "r1",
            &"c".repeat(64),
            "model",
            &[chunk(
                "Aurora belongs to Mira.",
                "Mira",
                vec![1.0, 0.0, 0.0]
            )]
        ),
        Err(Error::Conflict)
    ));
    db.replace(
        "a.md",
        "r2",
        &"d".repeat(64),
        "model",
        &[chunk("Aurora belongs to Nia.", "Nia", vec![1.0, 0.0, 0.0])],
    )
    .unwrap();
    let results = db
        .search("Aurora", &[1.0, 0.0, 0.0], SearchMode::Precise, 5, &[])
        .unwrap();
    assert_eq!(results.hits.len(), 1);
    db.sync(&[source("b.md")]).unwrap();
    assert_eq!(db.stats().unwrap().chunks, 1);
    assert!(db
        .search("Aurora", &[1.0, 0.0, 0.0], SearchMode::Fast, 5, &[])
        .unwrap()
        .hits
        .is_empty());
}

#[test]
fn model_entities_are_exact_source_spans_and_validation_is_atomic() {
    let mut db = index();
    db.sync(&[source("a.md")]).unwrap();
    let hallucination = chunk("An original sentence.", "InventedName", vec![1.0, 0.0, 0.0]);
    assert!(matches!(
        db.replace("a.md", "r1", &"a".repeat(64), "model", &[hallucination]),
        Err(Error::Invalid(_))
    ));
    assert_eq!(db.stats().unwrap().chunks, 0);
    let bad = chunk("Mira", "Mira", vec![f32::NAN, 0.0, 0.0]);
    assert!(matches!(
        db.replace("a.md", "r1", &"a".repeat(64), "model", &[bad]),
        Err(Error::Invalid(_))
    ));
    assert_eq!(db.stats().unwrap().entities, 0);
    assert!(SearchMode::parse("graph").is_err());
}

#[test]
fn chinese_lexical_search_and_literal_fts_operators_are_safe() {
    let mut db = index();
    db.sync(&[source("中文.md")]).unwrap();
    db.replace(
        "中文.md",
        "r1",
        &"a".repeat(64),
        "model",
        &[chunk(
            "星海项目使用本地资料库。",
            "星海项目",
            vec![0.0, 0.0, 1.0],
        )],
    )
    .unwrap();
    assert_eq!(
        db.search("本地资料", &[1.0, 0.0, 0.0], SearchMode::Fast, 5, &[])
            .unwrap()
            .hits
            .len(),
        1
    );
    db.search(
        "\" OR * NOT column:",
        &[1.0, 0.0, 0.0],
        SearchMode::Precise,
        5,
        &[],
    )
    .unwrap();
}

#[test]
fn catalog_filters_then_pages_and_recovers_after_removal() {
    let mut db = index();
    let mut files: Vec<_> = (0..45)
        .map(|n| source(&format!("folder/report-{n:02}.md")))
        .collect();
    db.sync(&files).unwrap();
    db.visit("folder/report-00.md", 1000).unwrap();
    db.favorite("folder/report-01.md", true).unwrap();
    let mut q = query();
    q.page = 3;
    let page = db.catalog(&q).unwrap();
    assert_eq!((page.total, page.page, page.entries.len()), (45, 3, 5));
    q.search = "report-0".into();
    let page = db.catalog(&q).unwrap();
    assert_eq!((page.total, page.page, page.entries.len()), (10, 1, 10));
    q = query();
    q.view = View::Recent;
    assert_eq!(
        db.catalog(&q).unwrap().entries[0].source.path,
        "folder/report-00.md"
    );
    q.view = View::Favorites;
    assert_eq!(
        db.catalog(&q).unwrap().entries[0].source.path,
        "folder/report-01.md"
    );
    files.truncate(20);
    db.sync(&files).unwrap();
    q = query();
    q.page = 3;
    assert_eq!(db.catalog(&q).unwrap().page, 1);
    q.limit = 0;
    assert!(db.catalog(&q).is_err());
}

#[test]
fn index_reopens_and_refuses_embedding_identity_drift() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("sag");
    {
        let mut db = Index::open(&path, "fixture", 3).unwrap();
        db.sync(&[source("a.md")]).unwrap();
        db.replace(
            "a.md",
            "r1",
            &"a".repeat(64),
            "model",
            &[chunk("Mira wrote Aurora.", "Mira", vec![1.0, 0.0, 0.0])],
        )
        .unwrap();
    }
    let db = Index::open(&path, "fixture", 3).unwrap();
    assert!(db.current("a.md", "r1", "model").unwrap());
    assert_eq!(
        db.search("Aurora", &[1.0, 0.0, 0.0], SearchMode::Precise, 5, &[])
            .unwrap()
            .hits
            .len(),
        1
    );
    drop(db);
    assert!(matches!(
        Index::open(&path, "different", 3),
        Err(Error::Identity)
    ));
}

#[test]
fn bounded_retrieval_reports_a_measured_thousand_chunk_run() {
    let mut db = index();
    let sources: Vec<_> = (0..1000)
        .map(|n| source(&format!("source-{n}.md")))
        .collect();
    db.sync(&sources).unwrap();
    for (n, source) in sources.iter().enumerate() {
        let name = format!("Entity{n}");
        db.replace(
            &source.path,
            "r1",
            &format!("{n:064x}"),
            "model",
            &[chunk(
                &format!("{name} has its own original evidence."),
                &name,
                vec![1.0, 0.0, 0.0],
            )],
        )
        .unwrap();
    }
    let result = db
        .search("Entity500", &[1.0, 0.0, 0.0], SearchMode::Precise, 10, &[])
        .unwrap();
    assert_eq!(result.hits.len(), 10);
    assert!(result.hits.iter().any(|hit| hit.path == "source-500.md"));
    assert!(result.candidates <= 176);
    eprintln!(
        "SAG 1000 chunks, 1000 entities: {} ms; {} bounded candidates",
        result.elapsed_ms, result.candidates
    );
}
