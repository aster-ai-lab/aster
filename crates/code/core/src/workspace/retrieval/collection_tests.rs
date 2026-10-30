#![cfg(test)]

use super::*;
use crate::workspace::{
    ChunkCatalogLimits, ChunkingConfig, FixedWindowChunkingOptions, LocalWorkspaceFile,
    LocalWorkspaceFileStatus, WorkspaceCatalogDocument, WorkspaceChunkCatalog,
    WorkspaceChunkingStrategy, WorkspaceLexicalEngine,
};
use std::sync::Arc;

fn document(path: &str, text: &str) -> WorkspaceCatalogDocument {
    WorkspaceCatalogDocument {
        manifest: LocalWorkspaceFile {
            path: path.into(),
            size: text.len() as u64,
            modified_ms: None,
            language: Some("markdown".into()),
            status: LocalWorkspaceFileStatus::Unknown,
            binary: false,
            generated: false,
        },
        text: text.into(),
    }
}

fn catalog() -> Arc<WorkspaceChunkCatalog> {
    WorkspaceChunkCatalog::new_with_engine(
        ChunkingConfig::default(),
        ChunkCatalogLimits::default(),
        WorkspaceLexicalEngine::Portable,
    )
    .unwrap()
}

fn query(documents: Vec<WorkspaceCatalogDocument>) -> WorkspaceCollectionQuery {
    WorkspaceCollectionQuery::new(
        catalog().replace_documents(7, documents).unwrap(),
        "tenant/vault/notes".into(),
    )
    .unwrap()
}

#[test]
fn names_query_reaches_every_title_and_path_without_body_only_matches() {
    let mut documents = (0..255)
        .map(|i| {
            document(
                &format!("资料/{i:03}.md"),
                &format!("---\ntitle: 名称目标{i}\n---\n正文"),
            )
        })
        .collect::<Vec<_>>();
    documents.push(document("正文.md", "名称目标只在正文"));
    let owner = query(documents);
    let mut request = WorkspaceCollectionRequest {
        search: "名称目标".into(),
        limit: 100,
        ..Default::default()
    };
    let mut paths = BTreeSet::new();
    loop {
        let page = owner
            .query_names_with_budget(&request, 128 * 1024, &[])
            .unwrap();
        assert_eq!(page.matched_files, 255);
        assert!(page
            .hits
            .iter()
            .all(|hit| hit.matched_field == WorkspaceCollectionMatch::Title));
        for hit in page.hits {
            assert!(paths.insert(hit.path));
        }
        request.cursor = page.next_cursor;
        if request.cursor.is_none() {
            break;
        }
    }
    assert!(paths.contains("资料/250.md"));
    assert_eq!(paths.len(), 255);
    let path = owner
        .query_names_with_budget(
            &WorkspaceCollectionRequest {
                search: "资料/250".into(),
                ..Default::default()
            },
            128 * 1024,
            &[],
        )
        .unwrap();
    assert_eq!(path.hits[0].matched_field, WorkspaceCollectionMatch::Path);
    assert_eq!(
        owner
            .query(&WorkspaceCollectionRequest {
                search: "名称目标".into(),
                ..Default::default()
            })
            .unwrap()
            .matched_files,
        256
    );
}

#[test]
fn names_query_orders_admitted_recents_and_binds_cursors_to_mode_and_order() {
    let owner = query(vec![
        document("a.md", "目标"),
        document("b.md", "目标"),
        document("c.md", "目标"),
    ]);
    let recent = vec!["c.md".into(), "deleted.md".into(), "b.md".into()];
    let request = WorkspaceCollectionRequest {
        limit: 1,
        ..Default::default()
    };
    let first = owner
        .query_names_with_budget(&request, 128 * 1024, &recent)
        .unwrap();
    assert_eq!(first.hits[0].path, "c.md");
    let next = WorkspaceCollectionRequest {
        cursor: first.next_cursor,
        ..request
    };
    assert_eq!(
        owner
            .query_names_with_budget(&next, 128 * 1024, &recent)
            .unwrap()
            .hits[0]
            .path,
        "b.md"
    );
    assert!(matches!(
        owner.query(&next),
        Err(WorkspaceCollectionError::InvalidCursor)
    ));
    assert!(matches!(
        owner.query_names_with_budget(&next, 128 * 1024, &[]),
        Err(WorkspaceCollectionError::InvalidCursor)
    ));
    for bad in [
        vec!["../outside.md".into()],
        vec!["a.md".into(), "a.md".into()],
        vec!["x.md".into(); 21],
    ] {
        assert!(owner
            .query_names_with_budget(&WorkspaceCollectionRequest::default(), 128 * 1024, &bad)
            .is_err());
    }
}

#[test]
fn collection_search_finds_note_250_and_pages_every_match_once() {
    let query = query(
        (0..255)
            .map(|index| {
                document(
                    &format!("资料/{index:03}.md"),
                    if index == 250 {
                        "正文：知识检索🚀唯一片段，全部笔记。"
                    } else {
                        "全部笔记。"
                    },
                )
            })
            .collect(),
    );
    let unique = query
        .query(&WorkspaceCollectionRequest {
            search: "识检索🚀唯一".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(unique.matched_files, 1);
    assert_eq!(unique.hits[0].path, "资料/250.md");
    assert_eq!(unique.hits[0].matched_field, WorkspaceCollectionMatch::Body);
    let range = unique.hits[0].source_range.as_ref().unwrap();
    assert_eq!(
        &"正文：知识检索🚀唯一片段，全部笔记。"[range.start..range.end],
        "识检索🚀唯一"
    );

    let mut request = WorkspaceCollectionRequest {
        search: "全部笔记".into(),
        limit: 37,
        sort_by: WorkspaceCollectionSort::Path,
        ..Default::default()
    };
    let mut paths = Vec::new();
    loop {
        let page = query.query(&request).unwrap();
        assert_eq!(page.matched_files, 255);
        assert_eq!(page.indexed_files, 255);
        assert_eq!(page.eligible_files, 255);
        assert_eq!(page.failed_files, 0);
        assert_eq!(page.source_revision, 7);
        assert_eq!(page.metadata_skipped_files, 0);
        assert!(page.hits.len() <= request.limit);
        assert!(serde_json::to_vec(&page).unwrap().len() <= MAX_PAGE_BYTES);
        paths.extend(page.hits.iter().map(|hit| hit.path.clone()));
        let repeated = query.query(&request).unwrap();
        assert_eq!(repeated.next_cursor, page.next_cursor);
        request.cursor = page.next_cursor;
        if request.cursor.is_none() {
            break;
        }
    }
    assert_eq!(paths.len(), 255);
    assert_eq!(paths.iter().collect::<BTreeSet<_>>().len(), 255);
    assert!(paths.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn collection_reassembles_overlaps_and_matches_across_chunk_boundaries() {
    let catalog = WorkspaceChunkCatalog::new_with_strategy_and_engine(
        WorkspaceChunkingStrategy::FixedWindow(FixedWindowChunkingOptions::new(8, 3).unwrap()),
        ChunkingConfig::default(),
        ChunkCatalogLimits::default(),
        WorkspaceLexicalEngine::Portable,
    )
    .unwrap();
    let text = "abc知识检索🚀跨越分块边界def";
    let snapshot = catalog
        .replace_documents(1, vec![document("a.md", text), document("empty.md", "")])
        .unwrap();
    assert!(snapshot.chunk_count() > 2);
    assert_eq!(
        snapshot
            .source_text(&WorkspacePath::from_normalized("a.md"))
            .unwrap()
            .as_deref(),
        Some(text)
    );
    assert_eq!(
        snapshot
            .source_text(&WorkspacePath::from_normalized("empty.md"))
            .unwrap(),
        Some(String::new())
    );
    assert_eq!(
        snapshot
            .source_text(&WorkspacePath::from_normalized("missing.md"))
            .unwrap(),
        None
    );
    let query = WorkspaceCollectionQuery::new(snapshot, "vault".into()).unwrap();
    let page = query
        .query(&WorkspaceCollectionRequest {
            search: "识检索🚀跨越分块".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(page.matched_files, 1);
    let range = page.hits[0].source_range.as_ref().unwrap();
    assert_eq!(&text[range.start..range.end], "识检索🚀跨越分块");
}

#[test]
fn collection_unicode_case_mapping_returns_original_source_bytes() {
    let text = "前文🙂İSTANBUL ΣΟΣ 后文";
    let query = query(vec![document("note.md", text)]);
    for search in ["i", "i\u{307}stan", "σος"] {
        let page = query
            .query(&WorkspaceCollectionRequest {
                search: search.into(),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(page.matched_files, 1, "{search}");
        let hit = &page.hits[0];
        let range = hit.source_range.as_ref().unwrap();
        assert!(text.is_char_boundary(range.start) && text.is_char_boundary(range.end));
        assert_eq!(
            &hit.context[hit.context_range.start..hit.context_range.end],
            &text[range.start..range.end]
        );
    }
}

#[test]
fn collection_filters_stored_values_and_keeps_missing_dates_last() {
    let mut a = document(
        "产品/a.md",
        "---\ntitle: 产品设计\ntags: [明确标签, 第二标签]\n---\n正文 #正文标签",
    );
    a.manifest.modified_ms = Some(2);
    let mut b = document("产品/b.md", "---\ntags: 明确标签\n---\n正文");
    b.manifest.modified_ms = Some(9);
    let query = query(vec![
        a,
        b,
        document("产品/c.md", "正文 #明确标签"),
        document("产品旧/a.md", "#明确标签"),
    ]);
    let mut request = WorkspaceCollectionRequest {
        folder: Some("产品".into()),
        tag: Some("明确标签".into()),
        sort_by: WorkspaceCollectionSort::Modified,
        ..Default::default()
    };
    let page = query.query(&request).unwrap();
    assert_eq!(
        page.hits
            .iter()
            .map(|hit| hit.path.as_str())
            .collect::<Vec<_>>(),
        ["产品/a.md", "产品/b.md", "产品/c.md"]
    );
    assert_eq!(page.hits[0].title, "产品设计");
    assert!(page.tags.contains(&"正文标签".into()));
    request.sort_order = WorkspaceCollectionOrder::Desc;
    assert_eq!(
        query
            .query(&request)
            .unwrap()
            .hits
            .iter()
            .map(|hit| hit.path.as_str())
            .collect::<Vec<_>>(),
        ["产品/b.md", "产品/a.md", "产品/c.md"]
    );
    request.tag = Some("明确".into());
    assert_eq!(query.query(&request).unwrap().matched_files, 0);
}

#[test]
fn collection_reports_unsupported_metadata_instead_of_false_absence() {
    let query = query(vec![
        document("good.md", "---\ntitle: 合法\n---\n检索目标"),
        document("bad-title.md", "---\ntitle: [不是标题]\n---\n检索目标"),
        document("bad-tags.md", "---\ntags: [标签, 23]\n---\n检索目标"),
        document("bad-yaml.md", "---\ntitle: [\n---\n检索目标"),
    ]);
    let page = query
        .query(&WorkspaceCollectionRequest {
            search: "检索目标".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(page.indexed_files, 4);
    assert_eq!(page.matched_files, 1);
    assert_eq!(page.metadata_skipped_files, 3);
    assert_eq!(page.metadata_skipped_paths.len(), 3);
}

#[test]
fn collection_retains_catalog_admission_gaps_and_failure_counts() {
    let catalog = catalog();
    let admitted = catalog
        .replace_documents(1, vec![document("ready.md", "目标")])
        .unwrap();
    let partial = catalog
        .publish_reconciliation(admitted.revision(), 2, admitted.state.files.clone(), 4, 2)
        .unwrap();
    let query = WorkspaceCollectionQuery::new(partial, "vault".into()).unwrap();
    let page = query
        .query(&WorkspaceCollectionRequest {
            search: "目标".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(page.indexed_files, 1);
    assert_eq!(page.eligible_files, 4);
    assert_eq!(page.failed_files, 2);
    assert_eq!(page.matched_files, 1);
    assert_eq!(page.metadata_skipped_files, 0);
}

#[test]
fn collection_cursors_refuse_foreign_queries_owners_and_snapshots() {
    let catalog = catalog();
    let snapshot = catalog
        .replace_documents(
            1,
            vec![
                document("a.md", "共同"),
                document("b.md", "共同"),
                document("c.md", "共同"),
            ],
        )
        .unwrap();
    let owner = WorkspaceCollectionQuery::new(snapshot.clone(), "owner-a".into()).unwrap();
    let mut request = WorkspaceCollectionRequest {
        search: "共同".into(),
        limit: 1,
        ..Default::default()
    };
    request.cursor = owner.query(&request).unwrap().next_cursor;
    assert_eq!(owner.query(&request).unwrap().hits[0].path, "b.md");
    for mut foreign in [
        WorkspaceCollectionRequest {
            search: "不同".into(),
            ..request.clone()
        },
        WorkspaceCollectionRequest {
            limit: 2,
            ..request.clone()
        },
        WorkspaceCollectionRequest {
            folder: Some("folder".into()),
            ..request.clone()
        },
        WorkspaceCollectionRequest {
            tag: Some("标签".into()),
            ..request.clone()
        },
        WorkspaceCollectionRequest {
            sort_by: WorkspaceCollectionSort::Path,
            ..request.clone()
        },
        WorkspaceCollectionRequest {
            sort_order: WorkspaceCollectionOrder::Desc,
            ..request.clone()
        },
    ] {
        assert!(matches!(
            owner.query(&foreign),
            Err(WorkspaceCollectionError::InvalidCursor)
        ));
        foreign.cursor = Some("not-a-cursor".into());
        assert!(matches!(
            owner.query(&foreign),
            Err(WorkspaceCollectionError::InvalidCursor)
        ));
    }
    let foreign = WorkspaceCollectionQuery::new(snapshot, "owner-b".into()).unwrap();
    assert!(matches!(
        foreign.query(&request),
        Err(WorkspaceCollectionError::InvalidCursor)
    ));
    let updated = catalog
        .replace_documents(2, vec![document("new.md", "共同")])
        .unwrap();
    let new_query = WorkspaceCollectionQuery::new(updated, "owner-a".into()).unwrap();
    assert!(matches!(
        new_query.query(&request),
        Err(WorkspaceCollectionError::InvalidCursor)
    ));
    assert_eq!(owner.query(&request).unwrap().hits[0].path, "b.md");
}

#[test]
fn collection_evicts_old_cursors_at_the_declared_bound() {
    let query = query(vec![document("a.md", ""), document("b.md", "")]);
    let mut first = WorkspaceCollectionRequest {
        search: " ".into(),
        limit: 1,
        ..Default::default()
    };
    first.cursor = query.query(&first).unwrap().next_cursor;
    for index in 2..=MAX_CURSORS + 1 {
        query
            .query(&WorkspaceCollectionRequest {
                search: " ".repeat(index),
                limit: 1,
                ..Default::default()
            })
            .unwrap();
    }
    assert!(matches!(
        query.query(&first),
        Err(WorkspaceCollectionError::InvalidCursor)
    ));
    assert_eq!(query.cursors.lock().unwrap().values.len(), MAX_CURSORS);
}

#[test]
fn collection_page_budget_includes_escaped_paths_facets_and_cursor() {
    let long_path = "\t".repeat(3900);
    let mut documents = Vec::new();
    for index in 0..100 {
        documents.push(document(
            &format!("{index:03}-{long_path}.md"),
            "---\ntitle: [拒绝]\n---",
        ));
        let tags = (0..8)
            .map(|tag| format!("\"{index}-{tag}-{}\"", "文".repeat(120)))
            .collect::<Vec<_>>()
            .join(",");
        documents.push(document(
            &format!("ok-{index:03}-{long_path}.md"),
            &format!(
                "---\ntitle: {}\ntags: [{tags}]\n---\n正文",
                "题".repeat(500)
            ),
        ));
    }
    let query = query(documents);
    let mut request = WorkspaceCollectionRequest {
        limit: 100,
        ..Default::default()
    };
    let mut found = BTreeSet::new();
    loop {
        let page = query.query(&request).unwrap();
        assert!(serde_json::to_vec(&page).unwrap().len() <= MAX_PAGE_BYTES);
        assert_eq!(page.metadata_skipped_files, 100);
        assert!(page.metadata_skipped_paths.len() < 20);
        assert!(page.tags_capped);
        assert!(!page.hits.is_empty());
        for hit in &page.hits {
            assert!(found.insert(hit.path.clone()));
        }
        request.cursor = page.next_cursor;
        if request.cursor.is_none() {
            break;
        }
    }
    assert_eq!(found.len(), 100);
}

#[test]
fn collection_refuses_unknown_sets_and_invalid_requests() {
    assert!(WorkspaceCollectionSort::parse("score")
        .unwrap_err()
        .to_string()
        .contains("WorkspaceCollectionSort"));
    assert!(WorkspaceCollectionOrder::parse("ascending").is_err());
    assert!(WorkspaceCollectionMatch::parse("all").is_err());
    assert_eq!(
        WorkspaceCollectionSort::parse("relevance")
            .unwrap()
            .as_str(),
        "relevance"
    );
    let query = query(vec![document("a.md", "")]);
    for request in [
        WorkspaceCollectionRequest {
            search: "中".repeat(513),
            ..Default::default()
        },
        WorkspaceCollectionRequest {
            search: "\0".into(),
            ..Default::default()
        },
        WorkspaceCollectionRequest {
            limit: 0,
            ..Default::default()
        },
        WorkspaceCollectionRequest {
            limit: 101,
            ..Default::default()
        },
        WorkspaceCollectionRequest {
            tag: Some(String::new()),
            ..Default::default()
        },
    ] {
        assert!(matches!(
            query.query(&request),
            Err(WorkspaceCollectionError::InvalidRequest(_))
        ));
    }
    for folder in [
        "",
        "/absolute",
        "../outside",
        "a/../b",
        "a//b",
        "a/./b",
        "a\\b",
        "a\0b",
    ] {
        assert!(matches!(
            query.query(&WorkspaceCollectionRequest {
                folder: Some(folder.into()),
                ..Default::default()
            }),
            Err(WorkspaceCollectionError::InvalidRequest(_))
        ));
    }
}

#[test]
fn collection_batch_refusal_retains_the_previous_published_generation() {
    let catalog = catalog();
    let before = catalog
        .replace_documents(9, vec![document("keep.md", "原始字节")])
        .unwrap();
    for documents in [
        vec![document("duplicate.md", ""), document("duplicate.md", "")],
        vec![document("../outside.md", "")],
        vec![document("a/./b.md", "")],
        vec![document("a//b.md", "")],
        vec![document("a\\b.md", "")],
        vec![document("nul\0.md", "")],
    ] {
        assert!(catalog.replace_documents(10, documents).is_err());
        assert_eq!(catalog.snapshot().unwrap().revision(), before.revision());
    }
    let mut wrong_size = document("wrong.md", "bytes");
    wrong_size.manifest.size = 0;
    assert!(catalog.replace_documents(10, vec![wrong_size]).is_err());
    assert!(matches!(
        catalog.replace_documents(8, vec![]),
        Err(WorkspaceIndexError::StaleRevision { .. })
    ));
    assert_eq!(catalog.snapshot().unwrap().paths(), ["keep.md"]);
    let after = catalog.replace_documents(10, vec![]).unwrap();
    assert_eq!(after.file_count(), 0);
    assert_eq!(
        before
            .source_text(&WorkspacePath::from_normalized("keep.md"))
            .unwrap()
            .as_deref(),
        Some("原始字节")
    );
}

#[test]
fn collection_batch_checks_source_budget_before_building_chunks() {
    let catalog = WorkspaceChunkCatalog::new_with_engine(
        ChunkingConfig {
            max_chunks_per_file: 1,
            max_bytes: 4,
            ..Default::default()
        },
        ChunkCatalogLimits {
            max_text_bytes: 8,
            ..Default::default()
        },
        WorkspaceLexicalEngine::Portable,
    )
    .unwrap();
    let error = catalog
        .replace_documents(1, vec![document("large.md", "123456789")])
        .unwrap_err();
    assert!(matches!(
        error,
        WorkspaceIndexError::BudgetExceeded {
            resource: "text byte",
            ..
        }
    ));
    assert_eq!(catalog.snapshot().unwrap().revision(), 0);
}

#[test]
fn collection_owner_can_reserve_envelope_bytes_without_losing_continuation() {
    let query = query(
        (0..100)
            .map(|index| {
                document(
                    &format!("{index:03}-{}.md", "长路径".repeat(100)),
                    &format!("---\ntitle: {}\n---\n正文", "题".repeat(500)),
                )
            })
            .collect(),
    );
    let mut request = WorkspaceCollectionRequest {
        limit: 100,
        ..Default::default()
    };
    let mut found = BTreeSet::new();
    loop {
        let page = query.query_with_budget(&request, 32 * 1024).unwrap();
        assert!(serde_json::to_vec(&page).unwrap().len() <= 32 * 1024);
        for hit in &page.hits {
            assert!(found.insert(hit.path.clone()));
        }
        request.cursor = page.next_cursor;
        if request.cursor.is_none() {
            break;
        }
    }
    assert_eq!(found.len(), 100);
    assert!(query.query_with_budget(&request, 1024).is_err());
    assert!(query
        .query_with_budget(&request, MAX_PAGE_BYTES + 1)
        .is_err());
}

#[test]
fn collection_cancellation_does_not_publish_a_partial_generation() {
    use std::cell::Cell;
    let catalog = catalog();
    let before = catalog
        .replace_documents(1, vec![document("keep.md", "原始")])
        .unwrap();
    let checks = Cell::new(0);
    let cancelled = || {
        let next = checks.get() + 1;
        checks.set(next);
        next >= 2
    };
    assert!(matches!(
        catalog.replace_documents_cancellable(
            2,
            vec![document("a.md", "一"), document("b.md", "二")],
            cancelled
        ),
        Err(WorkspaceIndexError::Cancelled)
    ));
    let after = catalog.snapshot().unwrap();
    assert_eq!(after.revision(), before.revision());
    assert_eq!(after.paths(), ["keep.md"]);
}

#[cfg(feature = "a3s-vec-fts")]
#[test]
fn collection_results_do_not_depend_on_lexical_partition_backend() {
    let mut previous = None;
    for engine in [
        WorkspaceLexicalEngine::A3sVec,
        WorkspaceLexicalEngine::Portable,
    ] {
        let catalog = WorkspaceChunkCatalog::new_with_engine(
            ChunkingConfig::default(),
            ChunkCatalogLimits::default(),
            engine,
        )
        .unwrap();
        let snapshot = catalog
            .replace_documents(
                1,
                vec![
                    document("title.md", "---\ntitle: 知识检索🚀\n---\n正文"),
                    document("知识检索🚀.md", "正文"),
                    document("body.md", "前文知识检索🚀后文"),
                ],
            )
            .unwrap();
        let query = WorkspaceCollectionQuery::new(snapshot, "same-owner".into()).unwrap();
        let page = query
            .query(&WorkspaceCollectionRequest {
                search: "知识检索🚀".into(),
                ..Default::default()
            })
            .unwrap();
        let value = serde_json::to_value(page).unwrap();
        if let Some(previous) = previous {
            assert_eq!(value, previous);
        }
        previous = Some(value);
    }
}

#[test]
fn collection_metadata_admission_can_be_cancelled_without_a_query_handle() {
    let catalog = catalog();
    let snapshot = catalog
        .replace_documents(1, vec![document("a.md", "一"), document("b.md", "二")])
        .unwrap();
    let checks = std::cell::Cell::new(0);
    let result = WorkspaceCollectionQuery::new_cancellable(snapshot, "scope".into(), || {
        let next = checks.get() + 1;
        checks.set(next);
        next >= 2
    });
    assert!(matches!(
        result,
        Err(WorkspaceCollectionError::Index(
            WorkspaceIndexError::Cancelled
        ))
    ));
}

#[test]
fn collection_folder_filter_does_not_hide_catalog_metadata_failures() {
    let query = query(vec![
        document("selected/good.md", "正文"),
        document("other/bad.md", "---\ntitle: [unsupported]\n---\n正文"),
    ]);
    let page = query
        .query(&WorkspaceCollectionRequest {
            folder: Some("selected".into()),
            search: "正文".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(page.matched_files, 1);
    assert_eq!(page.indexed_files, 2);
    assert_eq!(page.metadata_skipped_files, 1);
    assert_eq!(page.metadata_skipped_paths, ["other/bad.md"]);
}
