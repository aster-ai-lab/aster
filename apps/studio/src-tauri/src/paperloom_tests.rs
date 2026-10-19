#![cfg(test)]

use super::*;

const MEMBER: &str = "00000000-0000-4000-8000-000000000001";

#[test]
fn stale_or_duplicate_member_scopes_refuse_before_using_any_connection() {
    assert_eq!(
        member_query(MEMBER, &format!("studio_member={MEMBER}&q=paper+title")),
        Some("q=paper+title".to_owned())
    );
    assert!(member_query(MEMBER, "q=paper").is_none());
    assert!(member_query(MEMBER, "studio_member=00000000-0000-4000-8000-000000000002").is_none());
    assert!(
        member_query(
            MEMBER,
            &format!("studio_member={MEMBER}&studio_member={MEMBER}")
        )
        .is_none()
    );
}

#[test]
fn connection_refuses_a_remote_processor_and_stores_no_key() {
    let root = tempfile::tempdir().unwrap();
    let remote =
        json!({"endpoint":"http://127.0.0.1:49001","apiKey":"private-token","expectedRevision":""});
    assert_eq!(
        connection(root.path(), MEMBER, "PUT", remote.to_string().as_bytes()).0,
        400
    );
    assert!(!root.path().join("paperloom").exists());
    let ready = connection(root.path(), MEMBER, "GET", b"");
    assert_eq!(ready.0, 200);
    let ready: Value = serde_json::from_slice(&ready.3).unwrap();
    assert_eq!(ready["apiKeyConfigured"], true);
    assert_eq!(ready["endpoint"], "");
    assert!(!ready.to_string().contains("private-token"));
    let stale = json!({"endpoint":"","expectedRevision":"not-the-native-revision"});
    assert_eq!(
        connection(root.path(), MEMBER, "PUT", stale.to_string().as_bytes()).0,
        409
    );
}

#[test]
fn native_ocr_reads_the_text_layer_and_refuses_another_provider() {
    let root = tempfile::tempdir().unwrap();
    let bytes = include_bytes!("../../tests/fixtures/paperloom/two-pages.pdf");
    let upload = proxy(
        root.path(),
        MEMBER,
        "POST",
        &["uploads"],
        "name=native-proof.pdf",
        bytes,
    );
    assert_eq!(upload.0, 200, "{}", String::from_utf8_lossy(&upload.3));
    let upload: Value = serde_json::from_slice(&upload.3).unwrap();
    let id = upload["data"]["document_id"].as_str().unwrap();
    assert_eq!(id, hex_digest(bytes));
    assert_eq!(upload["data"]["page_count"], 2);
    let source = proxy(
        root.path(),
        MEMBER,
        "GET",
        &["documents", id, "source.pdf"],
        "",
        b"",
    );
    assert_eq!(source.2, "application/pdf");
    assert_eq!(source.3, bytes);
    let refused = proxy(
        root.path(),
        MEMBER,
        "POST",
        &["documents", id, "ocr"],
        "",
        json!({"workflow":"ocr","ocr":{"provider":"paddle"}})
            .to_string()
            .as_bytes(),
    );
    assert_eq!(refused.0, 400);
    let job = proxy(
        root.path(),
        MEMBER,
        "POST",
        &["documents", id, "ocr"],
        "",
        json!({"workflow":"ocr","ocr":{"provider":"a3s_gateway"}})
            .to_string()
            .as_bytes(),
    );
    assert_eq!(job.0, 200, "{}", String::from_utf8_lossy(&job.3));
    let job: Value = serde_json::from_slice(&job.3).unwrap();
    assert_eq!(job["data"]["status"], "succeeded");
    let job_id = job["data"]["job_id"].as_str().unwrap();
    let markdown = proxy(
        root.path(),
        MEMBER,
        "GET",
        &["jobs", job_id, "markdown"],
        "raw=true",
        b"",
    );
    assert_eq!(markdown.0, 200);
    let text = String::from_utf8(markdown.3).unwrap();
    assert!(text.contains("Page 1"), "{text}");
    assert!(text.contains("Page 2"), "{text}");
    let ask = proxy(
        root.path(),
        MEMBER,
        "POST",
        &["ai", "ask"],
        "",
        json!({"document_id":id,"question":"Page 1","assistant_mode":"operations","stream":true,"confirm_document_operation":true}).to_string().as_bytes(),
    );
    assert_eq!(ask.0, 200, "{}", String::from_utf8_lossy(&ask.3));
    let ask: Value = serde_json::from_slice(&ask.3).unwrap();
    assert!(ask["data"]["answer"].as_str().unwrap().contains("Page 1"));
    assert_eq!(ask["data"]["persisted"], false);
    assert!(ask["data"].get("confirm_document_operation").is_none());
    assert!(!public_route("POST", &["jobs", "old", "retry-stage"]));
}

#[test]
fn route_authority_fails_closed() {
    for (method, route) in [
        ("GET", "internal/worker"),
        ("POST", "ai/operations/op/commit"),
        ("POST", "integrations/obsidian/export"),
        ("GET", "documents/../source.pdf"),
        ("GET", "documents/%2e%2e/source.pdf"),
        ("PUT", "ai/runtime-config"),
        ("GET", "collections"),
        ("POST", "collections/old/documents"),
        ("DELETE", "collections/old/documents/source"),
        ("PATCH", "favorites/old"),
        ("POST", "documents"),
        ("DELETE", "documents/source"),
    ] {
        assert!(
            !public_route(method, &route.split('/').collect::<Vec<_>>()),
            "{route}"
        );
    }
    assert!(public_route("GET", &["documents", "sha256", "source.pdf"]));
    assert!(public_route("POST", &["jobs", "job-id", "cancel"]));
}

fn data(response: &Response) -> Value {
    assert_eq!(response.0, 200, "{}", String::from_utf8_lossy(&response.3));
    let value: Value = serde_json::from_slice(&response.3).unwrap();
    value["data"].clone()
}

fn query(pairs: &[(&str, &str)]) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in pairs {
        serializer.append_pair(key, value);
    }
    serializer.finish()
}

fn stored_pages(root: &std::path::Path, id: &str) -> Vec<String> {
    let path = root
        .join("paperloom-native")
        .join(MEMBER)
        .join("docs")
        .join(format!("{id}.json"));
    let document: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    document["pages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|page| page.as_str().unwrap().to_owned())
        .collect()
}

fn search_hits(root: &std::path::Path, id: &str, term: &str) -> Vec<Value> {
    let found = data(&proxy(
        root,
        MEMBER,
        "GET",
        &["search"],
        &query(&[("q", term), ("document_id", id)]),
        b"",
    ));
    found["hits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|hit| {
            json!({
                "page_idx": hit["page_idx"],
                "source_snippet": hit["source_snippet"]
            })
        })
        .collect()
}

fn ask_view(root: &std::path::Path, id: &str, question: &str) -> Value {
    let answer = data(&proxy(
        root,
        MEMBER,
        "POST",
        &["ai", "ask"],
        "",
        json!({
            "document_id": id,
            "question": question,
            "assistant_mode": "reading",
            "stream": true,
            "confirm_document_operation": true
        })
        .to_string()
        .as_bytes(),
    ));
    assert!(answer.get("confirm_document_operation").is_none());
    json!({
        "answer": answer["answer"],
        "persisted": answer["persisted"],
        "job_id": answer["citations"].as_array().unwrap().first().map(|item| item["job_id"].clone()),
        "citations": answer["citations"].as_array().unwrap().iter().map(|item| json!({
            "ref": item["ref"],
            "page_idx": item["page_idx"],
            "block_id": item["block_id"],
            "snippet": item["snippet"]
        })).collect::<Vec<_>>()
    })
}

fn plain_pdf(count: usize, text: Option<&str>) -> Vec<u8> {
    use lopdf::content::{Content, Operation};
    use lopdf::{Document, Object, Stream, dictionary};

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let (content_id, resources_id) = match text {
        Some(text) => {
            let font_id = doc.add_object(dictionary! {
                "Type" => "Font",
                "Subtype" => "Type1",
                "BaseFont" => "Courier",
            });
            let resources_id = doc.add_object(dictionary! {
                "Font" => dictionary! { "F1" => font_id },
            });
            let content = Content {
                operations: vec![
                    Operation::new("BT", vec![]),
                    Operation::new("Tf", vec!["F1".into(), 12.into()]),
                    Operation::new("Td", vec![72.into(), 720.into()]),
                    Operation::new("Tj", vec![Object::string_literal(text)]),
                    Operation::new("ET", vec![]),
                ],
            };
            (
                doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap())),
                Some(resources_id),
            )
        }
        None => (doc.add_object(Stream::new(dictionary! {}, vec![])), None),
    };
    let mut kids = Vec::with_capacity(count);
    for _ in 0..count {
        kids.push(
            doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "Contents" => content_id,
                "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
            })
            .into(),
        );
    }
    let mut pages = dictionary! {
        "Type" => "Pages",
        "Kids" => kids,
        "Count" => count as i64,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
    };
    if let Some(resources_id) = resources_id {
        pages.set("Resources", resources_id);
    }
    doc.objects.insert(pages_id, Object::Dictionary(pages));
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    bytes
}

fn encrypted_pdf() -> Vec<u8> {
    use lopdf::{Document, EncryptionState, EncryptionVersion, Object, Permissions};
    use std::io::Write;

    let bytes = plain_pdf(1, Some("sealed"));
    let mut doc = Document::load_mem(&bytes).unwrap();
    doc.trailer.set(
        "ID",
        vec![
            Object::string_literal("complex-paper-file-id"),
            Object::string_literal("complex-paper-file-id"),
        ],
    );
    let permissions = Permissions::PRINTABLE
        | Permissions::COPYABLE
        | Permissions::COPYABLE_FOR_ACCESSIBILITY
        | Permissions::PRINTABLE_IN_HIGH_QUALITY;
    let state = EncryptionState::try_from(EncryptionVersion::V1 {
        document: &doc,
        owner_password: "owner-secret",
        user_password: "user-secret",
        permissions,
    })
    .unwrap();
    doc.encrypt(&state).unwrap();
    let mut sealed = Vec::new();
    doc.save_to(&mut sealed).unwrap();
    let _ = Write::flush(&mut sealed);
    sealed
}

#[test]
fn complex_paper_handler_reads_every_text_page_and_repeats_markdown() {
    let root = tempfile::tempdir().unwrap();
    let bytes = include_bytes!("../../tests/fixtures/paperloom/complex-paper.pdf");
    let uploaded = data(&proxy(
        root.path(),
        MEMBER,
        "POST",
        &["uploads"],
        &query(&[("name", "复杂论文.pdf")]),
        bytes,
    ));
    let id = uploaded["document_id"].as_str().unwrap().to_owned();
    assert_eq!(id, hex_digest(bytes));
    assert_eq!(uploaded["page_count"], 8);
    let pages = stored_pages(root.path(), &id);
    assert_eq!(pages.len(), 8);
    assert!(pages[0].contains("复杂论文标题甲"), "{}", pages[0]);
    assert!(!pages[0].contains("页眉丙"), "{}", pages[0]);
    assert!(pages[1].contains("摘要句乙"), "{}", pages[1]);
    assert!(pages[1].contains("AlphaToken"), "{}", pages[1]);
    assert!(pages[1].contains("重复词辛"), "{}", pages[1]);
    assert!(pages[1].contains("页眉丙"), "{}", pages[1]);
    assert!(pages[2].contains("方法句癸"), "{}", pages[2]);
    assert!(
        pages[3].contains("左栏子句") && pages[3].contains("右栏子句"),
        "{}",
        pages[3]
    );
    assert!(
        pages[4].contains("表1列名") && pages[4].contains("表1甲 12"),
        "{}",
        pages[4]
    );
    assert!(pages[5].contains("图1说明戊"), "{}", pages[5]);
    assert!(pages[6].trim().is_empty(), "{:?}", pages[6]);
    assert!(
        pages[7].contains("参考文献己") && pages[7].contains("重复词辛"),
        "{}",
        pages[7]
    );
    assert!(pages[7].contains("页眉丙"), "{}", pages[7]);
    for page in [2, 3, 4, 5] {
        assert!(pages[page].contains("页眉丙"), "page {}", page + 1);
    }

    let first = data(&proxy(
        root.path(),
        MEMBER,
        "POST",
        &["documents", &id, "ocr"],
        "",
        json!({"workflow":"ocr","ocr":{"provider":"a3s_gateway"}})
            .to_string()
            .as_bytes(),
    ));
    assert_eq!(first["status"], "succeeded");
    let first_id = first["job_id"].as_str().unwrap().to_owned();
    let job = data(&proxy(
        root.path(),
        MEMBER,
        "GET",
        &["jobs", &first_id],
        "",
        b"",
    ));
    assert_eq!(job["markdown_ready"], true);
    assert_eq!(job["output_pdf_ready"], false);
    assert_eq!(job["bundle_ready"], false);
    assert_eq!(job["stages"]["ocr"]["state"], "completed");
    assert_eq!(job["stages"]["ocr"]["progress"]["current"], 8);
    assert_eq!(job["stages"]["ocr"]["progress"]["total"], 8);
    assert_eq!(job["stages"]["translation"]["state"], "skipped");
    assert_eq!(job["stages"]["render"]["state"], "skipped");
    assert_eq!(job["request_payload"]["source"]["upload_id"], id);
    let markdown = proxy(
        root.path(),
        MEMBER,
        "GET",
        &["jobs", &first_id, "markdown"],
        "raw=true",
        b"",
    );
    assert_eq!(markdown.0, 200);
    let markdown = String::from_utf8(markdown.3).unwrap();
    let expected = pages
        .iter()
        .map(|page| page.trim())
        .filter(|page| !page.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    assert_eq!(markdown, expected);
    assert!(markdown.contains("复杂论文标题甲"));
    assert!(markdown.contains("参考文献己"));
    assert_eq!(
        pages.iter().filter(|page| !page.trim().is_empty()).count(),
        7
    );

    let manifest = data(&proxy(
        root.path(),
        MEMBER,
        "GET",
        &["jobs", &first_id, "artifacts-manifest"],
        "",
        b"",
    ));
    let items = manifest["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["artifact_key"], "source_pdf");
    assert_eq!(items[0]["file_name"], "复杂论文.pdf");
    assert_eq!(items[1]["artifact_key"], "markdown_raw");
    assert_eq!(items[1]["file_name"], "paper.md");
    let source = proxy(
        root.path(),
        MEMBER,
        "GET",
        &["jobs", &first_id, "artifacts", "source_pdf"],
        "",
        b"",
    );
    assert_eq!(source.3, bytes);
    assert_eq!(
        items[0]["checksum"],
        format!("sha256:{}", hex_digest(&source.3))
    );
    let markdown_file = proxy(
        root.path(),
        MEMBER,
        "GET",
        &["jobs", &first_id, "artifacts", "markdown_raw"],
        "",
        b"",
    );
    assert_eq!(markdown_file.3, markdown.as_bytes());
    assert_eq!(
        items[1]["checksum"],
        format!("sha256:{}", hex_digest(&markdown_file.3))
    );

    let figure = search_hits(root.path(), &id, "图1说明戊");
    assert_eq!(figure.len(), 1);
    assert_eq!(figure[0]["page_idx"], 5);
    let repeated = search_hits(root.path(), &id, "重复词辛");
    assert_eq!(
        repeated
            .iter()
            .map(|hit| hit["page_idx"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        vec![1, 7]
    );
    assert!(search_hits(root.path(), &id, "alphatoken").is_empty());
    let latin = search_hits(root.path(), &id, "AlphaToken");
    assert_eq!(latin.len(), 1);
    assert_eq!(latin[0]["page_idx"], 1);
    let header = search_hits(root.path(), &id, "页眉丙");
    let header_pages = header
        .iter()
        .map(|hit| hit["page_idx"].as_u64().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(header_pages, vec![1, 2, 3, 4, 5, 7]);

    let second = data(&proxy(
        root.path(),
        MEMBER,
        "POST",
        &["documents", &id, "ocr"],
        "",
        json!({"workflow":"ocr","ocr":{"provider":"a3s_gateway"}})
            .to_string()
            .as_bytes(),
    ));
    let second_id = second["job_id"].as_str().unwrap().to_owned();
    assert_ne!(second_id, first_id);
    assert_eq!(second["status"], "succeeded");
    let second_markdown = proxy(
        root.path(),
        MEMBER,
        "GET",
        &["jobs", &second_id, "markdown"],
        "raw=true",
        b"",
    );
    assert_eq!(second_markdown.3, markdown.as_bytes());
    let document = data(&proxy(
        root.path(),
        MEMBER,
        "GET",
        &["documents", &id],
        "",
        b"",
    ));
    assert_eq!(document["active_job_id"], second_id);
    assert!(document.get("pages").is_none());

    let figure_answer = ask_view(root.path(), &id, "图1说明戊");
    assert_eq!(figure_answer["persisted"], false);
    assert_eq!(figure_answer["job_id"], second_id);
    assert_eq!(figure_answer["citations"].as_array().unwrap().len(), 1);
    assert_eq!(figure_answer["citations"][0]["page_idx"], 5);
    assert_eq!(figure_answer["answer"], pages[5]);
    let header_answer = ask_view(root.path(), &id, "页眉丙");
    assert_eq!(header_answer["persisted"], false);
    assert_eq!(
        header_answer["citations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["page_idx"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        header_pages
    );
    let absent = ask_view(root.path(), &id, "这句话不在论文里");
    assert_eq!(absent["answer"], "正文中没有这句话。");
    assert_eq!(absent["persisted"], false);
    assert!(absent["citations"].as_array().unwrap().is_empty());
    assert!(absent["job_id"].is_null());

    let mut figure_answer = figure_answer;
    let mut header_answer = header_answer;
    let mut absent = absent;
    figure_answer.as_object_mut().unwrap().remove("job_id");
    header_answer.as_object_mut().unwrap().remove("job_id");
    absent.as_object_mut().unwrap().remove("job_id");
    let capture = json!({
        "document_id": id,
        "page_count": 8,
        "pages": pages,
        "markdown": markdown,
        "artifacts": [
            {"artifact_key": "source_pdf", "file_name": "复杂论文.pdf", "checksum": items[0]["checksum"]},
            {"artifact_key": "markdown_raw", "file_name": "paper.md", "checksum": items[1]["checksum"]}
        ],
        "searches": {
            "图1说明戊": figure,
            "重复词辛": repeated,
            "alphatoken": [],
            "AlphaToken": latin,
            "页眉丙": header
        },
        "asks": {
            "图1说明戊": figure_answer,
            "页眉丙": header_answer,
            "这句话不在论文里": absent
        }
    });
    let actual = serde_json::to_string_pretty(&capture).unwrap() + "\n";
    if std::env::var_os("PAPERLOOM_WRITE_CAPTURE").is_some() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/paperloom/complex-paper.capture.json");
        std::fs::write(path, &actual).unwrap();
        return;
    }
    assert_eq!(
        actual,
        include_str!("../../tests/fixtures/paperloom/complex-paper.capture.json")
    );
}

#[test]
fn complex_paper_handler_refuses_closed_inputs() {
    let encrypted = encrypted_pdf();
    let root = tempfile::tempdir().unwrap();
    assert_eq!(
        proxy(
            root.path(),
            MEMBER,
            "POST",
            &["uploads"],
            "name=sealed.pdf",
            &encrypted
        )
        .0,
        400
    );
    assert_eq!(
        data(&proxy(root.path(), MEMBER, "GET", &["documents"], "", b""))["total"],
        0
    );

    let mut oversized = vec![0_u8; UPLOAD_LIMIT + 1];
    oversized[..5].copy_from_slice(b"%PDF-");
    let root = tempfile::tempdir().unwrap();
    assert_eq!(
        proxy(
            root.path(),
            MEMBER,
            "POST",
            &["uploads"],
            "name=oversized.pdf",
            &oversized
        )
        .0,
        400
    );

    let many = plain_pdf(2001, None);
    let root = tempfile::tempdir().unwrap();
    assert_eq!(
        proxy(
            root.path(),
            MEMBER,
            "POST",
            &["uploads"],
            "name=too-many.pdf",
            &many
        )
        .0,
        400
    );

    let named = plain_pdf(1, Some("RepeatToken"));
    let root = tempfile::tempdir().unwrap();
    assert_eq!(
        proxy(
            root.path(),
            MEMBER,
            "POST",
            &["uploads"],
            "name=notes.txt",
            &named
        )
        .0,
        400
    );

    let root = tempfile::tempdir().unwrap();
    let uploaded = data(&proxy(
        root.path(),
        MEMBER,
        "POST",
        &["uploads"],
        "name=one.pdf",
        &named,
    ));
    let id = uploaded["document_id"].as_str().unwrap();
    assert_eq!(
        proxy(
            root.path(),
            MEMBER,
            "POST",
            &["documents", id, "ocr"],
            "",
            json!({"workflow":"ocr","ocr":{"provider":"paddle"}})
                .to_string()
                .as_bytes(),
        )
        .0,
        400
    );
    assert!(
        data(&proxy(
            root.path(),
            MEMBER,
            "GET",
            &["documents", id, "jobs"],
            "",
            b""
        ))["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        proxy(
            root.path(),
            MEMBER,
            "POST",
            &["documents", id, "translate"],
            "",
            b"{}",
        )
        .0,
        400
    );
    assert_eq!(
        proxy(root.path(), MEMBER, "POST", &["jobs"], "", b"{}").0,
        400
    );

    let root = tempfile::tempdir().unwrap();
    assert_eq!(
        connection(
            root.path(),
            MEMBER,
            "PUT",
            br#"{"endpoint":"http://127.0.0.1:9","expectedRevision":""}"#,
        )
        .0,
        400
    );
    assert_eq!(
        connection(
            root.path(),
            MEMBER,
            "PUT",
            br#"{"endpoint":"","apiKey":"secret","expectedRevision":""}"#,
        )
        .0,
        400
    );
    assert!(!root.path().join("paperloom").exists());

    let repeated = plain_pdf(51, Some("RepeatToken"));
    let root = tempfile::tempdir().unwrap();
    let uploaded = data(&proxy(
        root.path(),
        MEMBER,
        "POST",
        &["uploads"],
        "name=repeat.pdf",
        &repeated,
    ));
    assert_eq!(uploaded["page_count"], 51);
    let id = uploaded["document_id"].as_str().unwrap().to_owned();
    let hits = search_hits(root.path(), &id, "RepeatToken");
    assert_eq!(hits.len(), 50);
    assert_eq!(hits[0]["page_idx"], 0);
    assert_eq!(hits[49]["page_idx"], 49);
    assert!(
        hits.iter()
            .all(|hit| hit["page_idx"].as_u64().unwrap() < 50)
    );
}
