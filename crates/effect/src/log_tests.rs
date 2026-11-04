#![cfg(test)]

use super::*;

fn user_fact(key: &str, text: &str) -> NewFact {
    NewFact {
        kind: "user.message".into(),
        key: key.into(),
        payload: serde_json::json!({ "text": text }),
    }
}

fn log_with(dir: &tempfile::TempDir, count: u64) -> FileLog {
    let log = FileLog::open(dir.path()).unwrap();
    for seq in 1..=count {
        log.append(
            "thread-1",
            &[user_fact(&format!("m-{seq}"), &format!("note {seq}"))],
            None,
        )
        .unwrap();
    }
    log
}

#[test]
fn truncate_after_keeps_the_prefix_and_reports_the_removed_count() {
    let dir = tempfile::tempdir().unwrap();
    let log = log_with(&dir, 5);
    let removed = log.truncate_after("thread-1", 3).unwrap();
    assert_eq!(removed, 2);
    let facts = log.read("thread-1").unwrap();
    assert_eq!(facts.len(), 3);
    assert_eq!(facts.last().unwrap().seq, 3);
    assert_eq!(facts.last().unwrap().key, "m-3");
    // The rewrite must survive a reopen and stay owner-only.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let file = dir.path().join("thread-1.jsonl");
        assert_eq!(
            std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600,
            "the rewritten fact log must stay owner-only"
        );
    }
    let reopened = FileLog::open(dir.path()).unwrap();
    assert_eq!(reopened.read("thread-1").unwrap().len(), 3);
    // The next append continues the retained timeline: seq 4 is free again.
    reopened
        .append("thread-1", &[user_fact("m-after-fork", "next")], None)
        .unwrap();
    let facts = reopened.read("thread-1").unwrap();
    assert_eq!(facts.last().unwrap().seq, 4);
    assert_eq!(facts.last().unwrap().key, "m-after-fork");
    // The scratch file never survives a successful rename.
    let scratch = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().contains("tmp-"))
        .count();
    assert_eq!(
        scratch, 0,
        "a successful rewrite must leave no scratch file"
    );
}

#[test]
fn truncate_after_beyond_the_end_removes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let log = log_with(&dir, 3);
    let file = dir.path().join("thread-1.jsonl");
    let before = std::fs::read(&file).unwrap();
    assert_eq!(log.truncate_after("thread-1", 99).unwrap(), 0);
    assert_eq!(
        std::fs::read(&file).unwrap(),
        before,
        "a no-op cut must not rewrite the log"
    );
    assert_eq!(log.read("thread-1").unwrap().len(), 3);
    // A thread with no log file at all is a no-op, not an error.
    assert_eq!(log.truncate_after("absent-thread", 4).unwrap(), 0);
}

#[test]
fn truncate_after_at_the_same_cut_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let log = log_with(&dir, 5);
    assert_eq!(log.truncate_after("thread-1", 3).unwrap(), 2);
    let file = dir.path().join("thread-1.jsonl");
    let settled = std::fs::read(&file).unwrap();
    assert_eq!(log.truncate_after("thread-1", 3).unwrap(), 0);
    assert_eq!(std::fs::read(&file).unwrap(), settled);
    assert_eq!(log.read("thread-1").unwrap().len(), 3);
}

#[test]
fn truncate_after_rejects_an_invalid_thread_id() {
    let dir = tempfile::tempdir().unwrap();
    let log = log_with(&dir, 2);
    assert!(matches!(
        log.truncate_after("bad/thread", 1),
        Err(ActorError::BadThreadId { .. })
    ));
    assert_eq!(log.read("thread-1").unwrap().len(), 2);
}

#[test]
fn truncate_after_on_a_corrupt_line_leaves_the_log_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let log = log_with(&dir, 2);
    drop(log);
    let file = dir.path().join("thread-1.jsonl");
    let mut handle = std::fs::OpenOptions::new()
        .append(true)
        .open(&file)
        .unwrap();
    writeln!(handle, "{{").unwrap();
    drop(handle);
    let before = std::fs::read(&file).unwrap();
    let log = FileLog::open(dir.path()).unwrap();
    assert!(matches!(
        log.truncate_after("thread-1", 1),
        Err(ActorError::Schema(_))
    ));
    assert_eq!(std::fs::read(&file).unwrap(), before);
}

fn typed_fact(kind: &str, key: &str) -> NewFact {
    NewFact {
        kind: kind.into(),
        key: key.into(),
        payload: serde_json::json!({ "text": key }),
    }
}

#[test]
fn message_compaction_preserves_evidence_and_reopens_sequence_gaps() {
    let dir = tempfile::tempdir().unwrap();
    let log = FileLog::open(dir.path()).unwrap();
    for (kind, key) in [
        ("session.source", "source"),
        ("user.message", "old-user"),
        ("model.turn", "old-model"),
        ("tool.result", "old-tool"),
        ("verification.report", "proof"),
        ("compaction.done", "compaction"),
        ("goal.definition", "goal"),
        ("user.message", "after-compaction"),
    ] {
        log.append("thread-1", &[typed_fact(kind, key)], None)
            .unwrap();
    }
    let opened = log.read_opened("thread-1", false).unwrap().unwrap();
    let retained = compacted_prefix(&opened.facts, &[]).unwrap();
    log.rewrite_compacted("thread-1", &opened, &retained)
        .unwrap();
    drop(opened);

    let facts = log.read("thread-1").unwrap();
    let keys = facts
        .iter()
        .map(|fact| fact.key.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        keys,
        [
            "source",
            "old-user",
            "old-model",
            "old-tool",
            "proof",
            "compaction",
            "goal",
            "after-compaction",
        ]
    );
    for key in ["old-user", "old-model", "old-tool"] {
        let fact = facts.iter().find(|fact| fact.key == key).unwrap();
        assert_eq!(fact.kind, "compaction.pruned");
        assert_eq!(fact.payload, serde_json::json!({}));
    }
    assert_eq!(
        facts.iter().map(|fact| fact.seq).collect::<Vec<_>>(),
        [1, 2, 3, 4, 5, 6, 7, 8]
    );
    let reopened = FileLog::open(dir.path()).unwrap();
    reopened
        .append("thread-1", &[user_fact("next", "next")], None)
        .unwrap();
    assert_eq!(reopened.read("thread-1").unwrap().last().unwrap().seq, 9);
}

#[test]
fn byte_limit_compacts_only_the_already_summarized_message_prefix() {
    let dir = tempfile::tempdir().unwrap();
    let log = FileLog::open(dir.path()).unwrap();
    log.append("thread-1", &[typed_fact("session.source", "source")], None)
        .unwrap();
    let blob = "x".repeat(1024 * 1024);
    for index in 0..14 {
        log.append(
            "thread-1",
            &[user_fact(&format!("old-{index}"), &blob)],
            None,
        )
        .unwrap();
    }
    for (kind, key) in [
        ("verification.report", "proof"),
        ("compaction.done", "compaction"),
        ("goal.definition", "goal"),
    ] {
        log.append("thread-1", &[typed_fact(kind, key)], None)
            .unwrap();
    }
    for index in 0..4 {
        log.append(
            "thread-1",
            &[user_fact(&format!("after-{index}"), &blob)],
            None,
        )
        .unwrap();
    }

    let facts = log.read("thread-1").unwrap();
    let keys = facts
        .iter()
        .map(|fact| fact.key.as_str())
        .collect::<Vec<_>>();
    for required in ["source", "proof", "compaction", "goal", "after-3"] {
        assert!(keys.contains(&required), "missing {required}: {keys:?}");
    }
    assert_eq!(
        facts.iter().find(|fact| fact.key == "old-0").unwrap().kind,
        "compaction.pruned"
    );
    assert_eq!(facts.last().unwrap().seq, 22);
    assert!(
        std::fs::metadata(dir.path().join("thread-1.jsonl"))
            .unwrap()
            .len()
            <= MAX_LOG_BYTES
    );
}

fn blob_fact(kind: &str, key: &str, bytes: usize) -> NewFact {
    NewFact {
        kind: kind.into(),
        key: key.into(),
        payload: serde_json::json!({ "text": "x".repeat(bytes) }),
    }
}

#[test]
fn byte_limit_prunes_prepared_tool_calls_and_superseded_summaries() {
    let dir = tempfile::tempdir().unwrap();
    let log = FileLog::open(dir.path()).unwrap();
    for round in 0..6 {
        for (kind, key) in [
            ("tool.prepared", format!("prepared-{round}")),
            ("compaction.done", format!("compaction-{round}")),
        ] {
            log.append("thread-1", &[blob_fact(kind, &key, 1024 * 1024)], None)
                .unwrap();
        }
    }
    for index in 0..12 {
        log.append(
            "thread-1",
            &[blob_fact(
                "tool.prepared",
                &format!("late-{index}"),
                1024 * 1024,
            )],
            None,
        )
        .unwrap();
    }
    let facts = log.read("thread-1").unwrap();
    let kind_of = |key: &str| {
        facts
            .iter()
            .find(|fact| fact.key == key)
            .unwrap()
            .kind
            .clone()
    };
    assert_eq!(kind_of("compaction-0"), "compaction.pruned");
    assert_eq!(kind_of("prepared-0"), "compaction.pruned");
    assert_eq!(kind_of("compaction-5"), "compaction.done");
    assert_eq!(kind_of("late-11"), "tool.prepared");
    assert_eq!(
        facts.iter().map(|fact| fact.seq).collect::<Vec<_>>(),
        (1..=facts.len() as u64).collect::<Vec<_>>()
    );
}

#[test]
fn byte_limit_uses_a_summary_arriving_with_the_append() {
    let dir = tempfile::tempdir().unwrap();
    let log = FileLog::open(dir.path()).unwrap();
    for index in 0..15 {
        log.append(
            "thread-1",
            &[blob_fact(
                "tool.result",
                &format!("result-{index}"),
                1024 * 1024,
            )],
            None,
        )
        .unwrap();
    }
    // The summary arrives with an append that needs room; pruning uses the
    // incoming compaction.done as the anchor over the already-written prefix.
    log.append(
        "thread-1",
        &[
            typed_fact("compaction.done", "compaction"),
            blob_fact("user.message", "after", 2 * 1024 * 1024),
        ],
        None,
    )
    .unwrap();
    let facts = log.read("thread-1").unwrap();
    assert_eq!(facts.last().unwrap().kind, "user.message");
    assert!(facts.iter().any(|fact| fact.kind == "compaction.done"));
    assert_eq!(facts[0].kind, "compaction.pruned");
}

#[test]
fn byte_limit_without_a_summary_fails_with_a_diagnostic_and_keeps_the_log() {
    let dir = tempfile::tempdir().unwrap();
    let log = FileLog::open(dir.path()).unwrap();
    for index in 0..15 {
        log.append(
            "thread-1",
            &[blob_fact(
                "tool.result",
                &format!("result-{index}"),
                1024 * 1024,
            )],
            None,
        )
        .unwrap();
    }
    let before = std::fs::read(dir.path().join("thread-1.jsonl")).unwrap();
    let error = log
        .append(
            "thread-1",
            &[blob_fact("tool.result", "overflow", 2 * 1024 * 1024)],
            None,
        )
        .unwrap_err();
    let ActorError::Schema(message) = error else {
        panic!("expected a schema error");
    };
    assert!(
        message.starts_with("fact log byte limit exceeded: "),
        "{message}"
    );
    assert!(message.contains("largest kind: tool.result"), "{message}");
    assert_eq!(
        std::fs::read(dir.path().join("thread-1.jsonl")).unwrap(),
        before
    );
}

#[test]
fn byte_limit_keeps_the_log_when_pruning_cannot_make_room() {
    let dir = tempfile::tempdir().unwrap();
    let log = FileLog::open(dir.path()).unwrap();
    log.append(
        "thread-1",
        &[typed_fact("compaction.done", "compaction")],
        None,
    )
    .unwrap();
    for index in 0..15 {
        log.append(
            "thread-1",
            &[blob_fact(
                "goal.definition",
                &format!("goal-{index}"),
                1024 * 1024,
            )],
            None,
        )
        .unwrap();
    }
    log.append(
        "thread-1",
        &[blob_fact("goal.definition", "goal-last", 900 * 1024)],
        None,
    )
    .unwrap();
    let before = std::fs::read(dir.path().join("thread-1.jsonl")).unwrap();
    assert!(log
        .append(
            "thread-1",
            &[blob_fact("user.message", "next", 200 * 1024)],
            None
        )
        .is_err());
    assert_eq!(
        std::fs::read(dir.path().join("thread-1.jsonl")).unwrap(),
        before
    );
}

#[test]
fn rewind_cannot_cut_into_compacted_history() {
    let dir = tempfile::tempdir().unwrap();
    let log = FileLog::open(dir.path()).unwrap();
    for (kind, key) in [
        ("user.message", "old-user"),
        ("compaction.done", "compaction"),
        ("user.message", "after"),
    ] {
        log.append("thread-1", &[typed_fact(kind, key)], None)
            .unwrap();
    }
    let opened = log.read_opened("thread-1", false).unwrap().unwrap();
    let retained = compacted_prefix(&opened.facts, &[]).unwrap();
    log.rewrite_compacted("thread-1", &opened, &retained)
        .unwrap();
    drop(opened);
    assert!(log.truncate_after("thread-1", 1).is_err());
    assert_eq!(log.truncate_after("thread-1", 2).unwrap(), 1);
}
