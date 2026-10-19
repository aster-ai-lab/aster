#![cfg(test)]

use super::super::reference_tasks_live_tests::{REVIEW_DOCUMENT_READS, publish_reference_review};
use super::tests::{harness, message, until_done};
use super::*;
use sha2::{Digest, Sha256};
use std::io::Write;

fn word(path: &Path, text: &str) {
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    zip.start_file(
        "word/document.xml",
        zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated),
    )
    .unwrap();
    zip.write_all(
        format!(
            "<w:document><w:body><w:p><w:r><w:t>{text}</w:t></w:r></w:p></w:body></w:document>"
        )
        .as_bytes(),
    )
    .unwrap();
    zip.finish().unwrap();
}

#[tokio::test]
async fn too_many_source_choices_are_rejected_before_creating_a_turn() {
    let h = harness("review-source-limit", "default").expect("node fixture");
    publish_reference_review(&h.dir);
    let paths = (0..9)
        .map(|index| format!("/tmp/{index}.docx"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut body = message(&format!("选择后校核\n{paths}"));
    body["turnContext"] = json!({"agentId":"legal-expert"});
    assert!(h.turns.open(&h.session_id, &body).is_err());
    assert!(h.store.messages(&h.session_id).is_empty());
    assert!(!h.dir.join("turn-trace.json").exists());
}

#[test]
fn an_oversized_decompressed_document_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("oversized.docx");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    zip.start_file(
        "word/document.xml",
        zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated),
    )
    .unwrap();
    let block = vec![b' '; 1024 * 1024];
    for _ in 0..33 {
        zip.write_all(&block).unwrap();
    }
    zip.finish().unwrap();
    assert!(std::fs::metadata(&path).unwrap().len() < 32 * 1024 * 1024);
    assert!(
        super::super::review_prompt::read_review_document(path.to_str().unwrap(), true, "review")
            .is_none()
    );
}

#[test]
fn a_reading_keeps_the_exact_byte_revision_when_the_path_is_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.docx");
    word(&path, "付款以经完成，条款A。");
    let before = std::fs::read(&path).unwrap();
    let revision = format!("{:x}", Sha256::digest(&before));
    let reading =
        super::super::review_prompt::read_review_document(path.to_str().unwrap(), true, "review")
            .unwrap();
    assert_eq!(
        reading.document_revision.as_deref(),
        Some(revision.as_str())
    );
    let original_time = std::fs::metadata(&path).unwrap().modified().unwrap();
    word(&path, "付款以经完成，条款B。");
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(original_time))
        .unwrap();
    let after = std::fs::read(&path).unwrap();
    assert_eq!(before.len(), after.len());
    assert_eq!(
        std::fs::metadata(&path).unwrap().modified().unwrap(),
        original_time
    );
    let next =
        super::super::review_prompt::read_review_document(path.to_str().unwrap(), true, "review")
            .unwrap();
    assert_ne!(before, after);
    assert_eq!(
        next.document_revision,
        Some(format!("{:x}", Sha256::digest(&after)))
    );
    assert_ne!(reading.document_revision, next.document_revision);
    assert!(reading.segments[0].contains("条款A"));
    assert!(next.segments[0].contains("条款B"));
}

#[tokio::test(flavor = "multi_thread")]
async fn selecting_b_reads_only_b_and_binds_its_findings_even_with_duplicate_file_names() {
    let h = harness("review-source-b", "default").expect("node fixture");
    publish_reference_review(&h.dir);
    let a = h.dir.join("A").join("合同.docx");
    let b = h.dir.join("B").join("合同.docx");
    std::fs::create_dir(a.parent().unwrap()).unwrap();
    std::fs::create_dir(b.parent().unwrap()).unwrap();
    word(&a, "未选择的机密原文A禁止发送给模型。");
    word(&b, "需要校核的原文B以经完成。");
    let mut body = message(&format!(
        "请选择文档后校核。\n{}\n{}",
        a.display(),
        b.display()
    ));
    body["turnContext"] = json!({"agentId":"legal-expert"});
    let mut events = h.hub.subscribe();
    h.turns.open(&h.session_id, &body).unwrap();
    let mut asked = false;
    let seen = until_done(&mut events, |event| {
        let chunk = &event["payload"]["chunk"];
        if chunk["type"] != "data-form" || chunk["data"]["state"] != "form-requested" {
            return;
        }
        let reads = REVIEW_DOCUMENT_READS.lock().unwrap();
        assert!(!reads.contains(&a) && !reads.contains(&b));
        drop(reads);
        let trace: Value =
            serde_json::from_slice(&std::fs::read(h.dir.join("turn-trace.json")).unwrap()).unwrap();
        assert_eq!(trace["prompts"], json!([]));
        let spec: Question = serde_json::from_str(chunk["data"]["spec"].as_str().unwrap()).unwrap();
        assert!(!spec.allow_free_text);
        assert_eq!(
            spec.choices
                .iter()
                .map(|c| c.label.as_str())
                .collect::<Vec<_>>(),
            [a.to_str().unwrap(), b.to_str().unwrap()]
        );
        let binding: FormBinding =
            serde_json::from_value(chunk["data"]["binding"].clone()).unwrap();
        assert!(!h.turns.submit_form(
            binding.clone(),
            serde_json::from_value(json!({"choice":"question-9"})).unwrap()
        ));
        assert!(!h.turns.submit_form(
            binding.clone(),
            serde_json::from_value(json!({"answer":b.to_str().unwrap()})).unwrap()
        ));
        assert!(h.turns.submit_form(
            binding,
            serde_json::from_value(json!({"choice":"question-1"})).unwrap()
        ));
        asked = true;
    })
    .await;
    assert!(asked);
    assert_eq!(seen.last().unwrap()["payload"]["status"], "success");
    let reads = REVIEW_DOCUMENT_READS.lock().unwrap();
    assert!(!reads.contains(&a));
    assert_eq!(reads.iter().filter(|p| **p == b).count(), 1);
    drop(reads);
    let memory = h.turns.load_review_memory(&h.session_id).unwrap();
    assert_eq!(memory.document, b.to_str().unwrap());
    let revision = format!("{:x}", Sha256::digest(std::fs::read(&b).unwrap()));
    assert_eq!(memory.document_revision.as_deref(), Some(revision.as_str()));
    h.turns.lock_review().clear();
    assert_eq!(
        h.turns
            .load_review_memory(&h.session_id)
            .unwrap()
            .document_revision
            .as_deref(),
        Some(revision.as_str())
    );
    assert!(memory.findings.contains("需要校核的原文B"));
    let trace = std::fs::read_to_string(h.dir.join("turn-trace.json")).unwrap();
    assert!(!trace.contains("未选择的机密原文A"));
    let worker: Value =
        serde_json::from_slice(&std::fs::read(h.dir.join("session-new.json")).unwrap()).unwrap();
    assert_eq!(worker["_meta"]["a3sHost"]["sourceReview"], true);
    let rows = h.store.messages(&h.session_id);
    assert!(
        rows.iter()
            .flat_map(|m| m.parts())
            .any(|p| p["type"] == "data-review-source" && p["data"]["path"] == b.to_str().unwrap())
    );
    h.turns.close_test_runtimes().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn cancelled_selection_and_unreadable_selected_file_never_fall_back_to_a() {
    for cancel in [false, true] {
        let h = harness(
            if cancel {
                "review-source-cancel"
            } else {
                "review-source-missing"
            },
            "default",
        )
        .expect("node fixture");
        publish_reference_review(&h.dir);
        let a = h.dir.join("A.docx");
        let b = h.dir.join("missing-B.docx");
        word(&a, "A 有正文但未获选择。");
        let mut body = message(&format!("选择后校核\n{}\n{}", a.display(), b.display()));
        body["turnContext"] = json!({"agentId":"legal-expert"});
        let mut events = h.hub.subscribe();
        h.turns.open(&h.session_id, &body).unwrap();
        let mut binding = None;
        let seen = until_done(&mut events, |event| {
            let chunk = &event["payload"]["chunk"];
            if chunk["type"] != "data-form" || chunk["data"]["state"] != "form-requested" {
                return;
            }
            let owner: FormBinding =
                serde_json::from_value(chunk["data"]["binding"].clone()).unwrap();
            if cancel {
                h.turns.abort(&h.session_id);
            }
            assert_eq!(
                h.turns.submit_form(
                    owner.clone(),
                    serde_json::from_value(json!({"choice":"question-1"})).unwrap()
                ),
                !cancel
            );
            binding = Some(owner);
        })
        .await;
        assert!(binding.is_some());
        let reads = REVIEW_DOCUMENT_READS.lock().unwrap();
        assert!(!reads.contains(&a));
        assert_eq!(reads.contains(&b), !cancel);
        drop(reads);
        let trace: Value =
            serde_json::from_slice(&std::fs::read(h.dir.join("turn-trace.json")).unwrap()).unwrap();
        assert_eq!(trace["prompts"], json!([]));
        assert!(!h.turns.submit_form(
            binding.unwrap(),
            serde_json::from_value(json!({"choice":"question-0"})).unwrap()
        ));
        assert_eq!(
            seen.last().unwrap()["channel"],
            if cancel {
                "ai.stream.done"
            } else {
                "ai.stream.error"
            }
        );
        assert_eq!(
            h.store.messages(&h.session_id).last().unwrap().status,
            if cancel { "paused" } else { "error" }
        );
        h.turns.close_test_runtimes().await;
    }
}
