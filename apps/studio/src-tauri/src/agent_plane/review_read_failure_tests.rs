#![cfg(test)]

use super::super::reference_tasks_live_tests::{REVIEW_DOCUMENT_READS, publish_reference_review};
use super::tests::{harness, message, until_done};
use super::*;
use std::io::Write;

#[tokio::test(flavor = "multi_thread")]
async fn one_explicit_unreadable_word_fails_without_a_model_or_clean_result() {
    for case in [
        "missing",
        "empty",
        "not-zip",
        "missing-document",
        "blank-document",
        "too-large",
    ] {
        let h = harness(&format!("review-read-{case}"), "default").expect("node fixture");
        publish_reference_review(&h.dir);
        let path = h.dir.join("source.docx");
        match case {
            "missing" => {}
            "empty" => std::fs::write(&path, []).unwrap(),
            "not-zip" => std::fs::write(&path, b"not a Word archive").unwrap(),
            "too-large" => std::fs::File::create(&path)
                .unwrap()
                .set_len(32 * 1024 * 1024 + 1)
                .unwrap(),
            _ => {
                let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
                let entry = if case == "missing-document" {
                    "word/other.xml"
                } else {
                    "word/document.xml"
                };
                zip.start_file(entry, zip::write::SimpleFileOptions::default())
                    .unwrap();
                zip.write_all(b"<w:document><w:body><w:p/></w:body></w:document>")
                    .unwrap();
                zip.finish().unwrap();
            }
        }
        h.turns
            .store_review_memory(&h.session_id, "/tmp/old.docx", "old finding");
        let mut body = message(&format!(
            "/review Check only the selected file.\n{}",
            path.display()
        ));
        body["turnContext"] = json!({"agentId":"legal-expert"});
        let mut events = h.hub.subscribe();
        let ack = h.turns.open(&h.session_id, &body).unwrap();
        assert_eq!(ack["mode"], "started");
        let seen = until_done(&mut events, |_| {}).await;
        let last = seen.last().unwrap();
        assert_eq!(last["channel"], "ai.stream.error", "{case}");
        assert_eq!(
            last["payload"]["error"]["message"],
            "The selected Word document could not be read."
        );
        assert!(
            !h.dir.join("turn-trace.json").exists(),
            "a process must not start: {case}"
        );
        let reads = REVIEW_DOCUMENT_READS.lock().unwrap();
        assert_eq!(
            reads.iter().filter(|p| **p == path).count(),
            1,
            "no second read: {case}"
        );
        drop(reads);
        let rows = h.store.messages(&h.session_id);
        let assistant = rows.last().unwrap();
        assert_eq!(assistant.status, "error");
        assert!(assistant.parts().iter().any(|p| p["type"] == "data-error"));
        assert!(
            !assistant
                .parts()
                .iter()
                .any(|p| p["type"] == "data-review-source")
        );
        assert!(
            !assistant.searchable_text.contains("a3s-review-finding")
                && assistant.searchable_text != "[]"
        );
        assert_eq!(h.turns.attach(&h.session_id)["status"], "not-found");
        let memory = h.turns.load_review_memory(&h.session_id).unwrap();
        assert_eq!(
            (memory.document.as_str(), memory.findings.as_str()),
            ("", "")
        );
        assert!(memory.document_revision.is_none());
    }
}

#[tokio::test]
async fn an_explicit_word_source_cannot_bypass_the_native_host_through_another_executor() {
    let h = harness("review-read-executor", "default").expect("node fixture");
    publish_reference_review(&h.dir);
    let mut body = message("/review Check this file.\n/tmp/source.docx");
    body["turnContext"] = json!({"agentId":"legal-expert","agentRuntime":{}});
    let error = h.turns.open(&h.session_id, &body).unwrap_err();
    assert!(error.to_string().contains("native review host"));
    assert!(h.store.messages(&h.session_id).is_empty());
    assert!(!h.dir.join("turn-trace.json").exists());
}
