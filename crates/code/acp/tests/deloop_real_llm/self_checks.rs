use super::*;

#[test]
fn feed_oracle_rejects_missing_wrong_and_protected_changes() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path();
    std::fs::write(path.join("README.md"), CONTRACT).unwrap();
    std::fs::write(path.join("notes.txt"), NOTES).unwrap();
    assert!(!correct(path));
    std::fs::write(path.join("report.json"), expected_report().to_string()).unwrap();
    assert!(correct(path));
    let mut wrong = expected_report();
    wrong["total_cents"] = json!(0);
    std::fs::write(path.join("report.json"), wrong.to_string()).unwrap();
    assert!(!correct(path));
    std::fs::write(path.join("report.json"), expected_report().to_string()).unwrap();
    std::fs::write(path.join("notes.txt"), "modified").unwrap();
    assert!(!correct(path));
    assert_eq!(feed(1).lines().count(), 49);
    assert_ne!(feed(1), feed(2));
}

#[test]
fn feed_publication_ignores_failed_other_and_duplicate_events() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("观察 workspace");
    let mut pending = BTreeSet::new();
    let mut completed = BTreeSet::new();
    let event = |id: &str, path: &str, status: &str| json!({"update":{"toolCallId":id,"kind":"read","rawInput":{"file_path":path},"status":status}});
    observe_feed_reads(
        &[
            event("other", "report.json", "completed"),
            event("failed", "feed.jsonl", "failed"),
            event("one", "feed.jsonl", "in_progress"),
        ],
        &workspace,
        &mut pending,
        &mut completed,
    );
    assert!(completed.is_empty());
    let terminal = json!({"update":{"toolCallId":"one","status":"completed"}});
    observe_feed_reads(
        &[terminal.clone(), terminal],
        &workspace,
        &mut pending,
        &mut completed,
    );
    assert_eq!(completed.len(), 1);
    observe_feed_reads(
        &[
            json!({"update":{"toolCallId":"batch","kind":"read","rawInput":{"files":[{"path":"README.md"},{"path":"feed.jsonl"}]},"status":"completed"}}),
        ],
        &workspace,
        &mut pending,
        &mut completed,
    );
    assert_eq!(completed.len(), 2);
    assert!(path_matches(
        workspace.join("feed.jsonl").to_str().unwrap(),
        &workspace,
        "feed.jsonl"
    ));
}
