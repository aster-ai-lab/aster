use super::*;

#[test]
fn a_failed_reread_is_not_an_empty_mutation() {
    let before = vec!["?? guest.txt".to_string()];
    let observed = combine_delta(Some(&before), None, Some("abc"), None, None);
    assert!(observed.incomplete);
    assert!(observed.paths.is_empty());
}

#[test]
fn a_quoted_porcelain_path_is_a_mutation() {
    let after = vec!["?? \"my file.txt\"".to_string()];
    let parsed = diff_porcelain(&[], &after);
    assert!(!parsed.incomplete);
    assert_eq!(parsed.paths, vec!["my file.txt".to_string()]);
    let octal = vec!["?? \"\\350\\257\\264\\346\\230\\216.md\"".to_string()];
    let decoded = diff_porcelain(&[], &octal);
    assert!(!decoded.incomplete);
    assert_eq!(decoded.paths, vec!["说明.md".to_string()]);
}

#[test]
fn an_undecodable_quoted_path_is_not_an_empty_mutation() {
    let after = vec!["?? \"unterminated".to_string()];
    let parsed = diff_porcelain(&[], &after);
    assert!(parsed.incomplete);
    assert!(parsed.paths.is_empty());
}

#[test]
fn a_renamed_quoted_pair_records_both_paths() {
    let after = vec!["R  \"old name.txt\" -> \"new name.txt\"".to_string()];
    let parsed = diff_porcelain(&[], &after);
    assert!(!parsed.incomplete);
    assert_eq!(
        parsed.paths,
        vec!["new name.txt".to_string(), "old name.txt".to_string()]
    );
}

#[test]
fn hidden_index_tags_are_paths_and_ordinary_tags_are_not() {
    let output = b"H tracked.txt\0S skipped.txt\0h assumed.txt\0s both.txt\0";
    let (paths, incomplete) = parse_hidden_index_paths(output);
    assert!(!incomplete);
    assert_eq!(
        paths,
        vec![
            "skipped.txt".to_string(),
            "assumed.txt".to_string(),
            "both.txt".to_string()
        ]
    );
}

#[test]
fn nul_status_keeps_a_path_with_a_space() {
    let output = b"?? my file.txt\0?? \xe8\xaf\xb4\xe6\x98\x8e.md\0";
    let parsed_status = porcelain_records(output);
    assert!(!parsed_status.rejected);
    assert!(!parsed_status.budget);
    let parsed = diff_porcelain(&[], &parsed_status.records);
    assert!(parsed.paths.iter().any(|path| path == "my file.txt"));
    assert!(parsed.paths.iter().any(|path| path == "说明.md"));
}

#[test]
fn porcelain_keeps_the_suffix_past_thirty_two_paths() {
    let after: Vec<String> = (0..33).map(|index| format!("?? f{index:02}.txt")).collect();
    let paths = changed_paths(&[], &after);
    assert_eq!(paths.len(), 33);
    assert!(paths.iter().any(|path| path == "f32.txt"));
}

#[test]
fn a_clean_revision_path_is_not_kept_when_git_was_not_invoked() {
    let mut paths = vec!["guest.txt".to_string(), "local.txt".to_string()];
    drop_foreign_revision(
        &mut paths,
        &["guest.txt".to_string()],
        &["local.txt".to_string()],
    );
    assert_eq!(paths, vec!["local.txt".to_string()]);
}

#[test]
fn a_revision_path_left_dirty_stays_on_the_command() {
    let mut paths = vec!["guest.txt".to_string()];
    drop_foreign_revision(
        &mut paths,
        &["guest.txt".to_string()],
        &["guest.txt".to_string()],
    );
    assert_eq!(paths, vec!["guest.txt".to_string()]);
}

#[test]
fn a_missed_status_with_a_known_head_is_not_an_empty_tree() {
    let before = snapshot_from_parts(None, Some("abc".to_string()), Vec::new());
    assert!(before.observes_git());
    assert!(before.stamps_incomplete);
    assert!(before.stamp_read_failed);
}

#[tokio::test]
async fn a_status_budget_does_not_diff_a_prefix_or_fail_the_read() {
    let root = tempfile::tempdir().unwrap();
    git_repo(root.path());
    std::fs::write(root.path().join("kept.txt"), "x").unwrap();
    let mut snapshot = snapshot_from_parts(None, Some("abc".into()), Vec::new());
    snapshot.status_budget = true;
    snapshot.stamp_read_failed = false;
    let watch = Watch {
        snapshot,
        files: None,
    };
    let baseline = watch.receipt_baseline();
    assert_eq!(baseline["git"]["status_budget"], true);
    assert_eq!(baseline["git"]["status_complete"], false);
    assert_eq!(baseline["git"]["stamp_read_failed"], false);
    assert!(baseline["git"]["status_digest"].is_null());
    std::fs::write(root.path().join("new.txt"), "y").unwrap();
    let observed = watch.finish_observed_with_revision(root.path(), true).await;
    assert!(observed.budget_exceeded);
    assert!(!observed.incomplete);
    assert!(observed.paths.is_empty());
}

#[test]
fn a_missed_status_outside_git_is_not_a_git_observation() {
    let before = snapshot_from_parts(None, None, Vec::new());
    assert!(!before.observes_git());
    assert!(!before.stamps_incomplete);
    assert!(!before.stamp_read_failed);
}

#[tokio::test]
async fn an_unchanged_git_worktree_is_a_complete_empty_delta() {
    let root = tempfile::tempdir().unwrap();
    git_repo(root.path());
    std::fs::write(root.path().join("README.md"), "base\n").unwrap();
    git_at(root.path(), &["add", "README.md"]);
    git_at(root.path(), &["commit", "-m", "base"]);

    let watch = Watch::start(root.path()).await;
    assert!(watch.snapshot.observes_git());
    assert!(!watch.snapshot.stamps_incomplete);
    assert!(watch.files.is_none());
    let paths = watch.finish(root.path()).await;
    assert!(
        paths.is_empty(),
        "an unchanged worktree was reported as a mutation: {paths:?}"
    );
}

#[tokio::test]
async fn a_status_list_past_the_stamp_cap_is_a_complete_empty_delta() {
    let root = tempfile::tempdir().unwrap();
    git_repo(root.path());
    for index in 0..=FILE_WALK_CAP {
        std::fs::write(root.path().join(format!("f{index:04}.txt")), "x").unwrap();
    }

    let watch = Watch::start(root.path()).await;
    assert!(
        watch
            .snapshot
            .porcelain
            .as_ref()
            .is_some_and(|lines| lines.len() == FILE_WALK_CAP + 1),
        "status past the stamp cap was dropped: {:?}",
        watch.snapshot.porcelain.as_ref().map(Vec::len)
    );
    assert!(watch.snapshot.stamps_incomplete);
    assert!(!watch.snapshot.stamp_read_failed);
    let baseline = watch.receipt_baseline();
    assert!(serde_json::to_vec(&baseline).unwrap().len() < 4 * 1024);
    assert_eq!(baseline["git"]["status_count"], FILE_WALK_CAP + 1);
    assert_eq!(baseline["git"]["status_complete"], true);
    assert_eq!(baseline["git"]["stamp_read_failed"], false);
    assert_eq!(baseline["git"]["stamps_incomplete"], true);
    assert!(baseline["git"].get("porcelain").is_none());
    assert!(baseline["git"]["status_digest"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    let unchanged = watch
        .finish_observed_with_revision(root.path(), false)
        .await;
    assert!(!unchanged.incomplete, "{unchanged:?}");
    assert!(!unchanged.budget_exceeded, "{unchanged:?}");
    assert!(
        unchanged.paths.is_empty(),
        "an unchanged overflow was a mutation: {:?}",
        unchanged.paths
    );

    let watch = Watch::start(root.path()).await;
    std::fs::write(root.path().join("owned.txt"), "owned\n").unwrap();
    let owned = watch
        .finish_observed_with_revision(root.path(), false)
        .await;
    assert!(!owned.incomplete, "{owned:?}");
    assert!(!owned.budget_exceeded, "{owned:?}");
    assert_eq!(owned.paths, vec!["owned.txt".to_string()]);
}

#[test]
fn a_run_level_budget_closes_the_gate_and_a_command_path_list_does_not() {
    use crate::harness_loop::{decide_completion, CompletionGate, MutationLedger};
    const GATE: &str = "completion gate: workspace observation is incomplete, so this digest is not the effect. A final answer, a waiver, or a verification of a partial list does not close it.";
    let status_budget = fold_run_observation(
        PathDelta {
            paths: vec!["prefix.txt".into()],
            incomplete: false,
            budget_exceeded: false,
            status_budget: false,
        },
        false,
        true,
        Some(PathDelta {
            paths: vec!["from-walk.txt".into()],
            incomplete: false,
            budget_exceeded: false,
            status_budget: false,
        }),
    );
    assert!(status_budget.budget_exceeded);
    assert!(!status_budget.incomplete);
    assert!(status_budget.paths.is_empty());
    let mut ledger = MutationLedger::default();
    ledger.set_observation_incomplete(status_budget.incomplete || status_budget.budget_exceeded);
    match decide_completion(&ledger, &[], &[], false) {
        CompletionGate::Incomplete { message } => assert_eq!(message, GATE),
        other => panic!("status budget left the gate open: {other:?}"),
    }

    let fresh = fold_run_observation(
        PathDelta {
            paths: vec!["prefix.txt".into()],
            incomplete: false,
            budget_exceeded: true,
            status_budget: true,
        },
        false,
        false,
        Some(PathDelta {
            paths: vec!["from-walk.txt".into()],
            incomplete: false,
            budget_exceeded: false,
            status_budget: false,
        }),
    );
    assert!(fresh.budget_exceeded);
    assert!(fresh.status_budget);
    assert!(!fresh.incomplete);
    assert!(fresh.paths.is_empty());
    let mut ledger = MutationLedger::default();
    ledger.set_observation_incomplete(fresh.incomplete || fresh.budget_exceeded);
    match decide_completion(&ledger, &[], &[], false) {
        CompletionGate::Incomplete { message } => assert_eq!(message, GATE),
        other => panic!("a fresh status cap left the gate open: {other:?}"),
    }

    let names_only = fold_run_observation(
        PathDelta {
            paths: vec!["kept.txt".into(), "also.txt".into()],
            incomplete: false,
            budget_exceeded: true,
            status_budget: false,
        },
        false,
        false,
        None,
    );
    assert!(!names_only.budget_exceeded);
    assert!(!names_only.incomplete);
    assert_eq!(
        names_only.paths,
        vec!["kept.txt".to_string(), "also.txt".to_string()]
    );
    let mut ledger = MutationLedger::default();
    ledger.set_observation_incomplete(names_only.incomplete || names_only.budget_exceeded);
    assert!(
        !ledger.observation_incomplete(),
        "a command path list closed the run observation"
    );

    let missed = fold_run_observation(
        PathDelta {
            paths: vec!["prefix.txt".into()],
            incomplete: false,
            budget_exceeded: true,
            status_budget: false,
        },
        true,
        true,
        None,
    );
    assert!(missed.incomplete);
    assert!(!missed.budget_exceeded);

    let walk_budget = fold_run_observation(
        PathDelta::default(),
        false,
        false,
        Some(PathDelta {
            paths: vec!["prefix.txt".into()],
            incomplete: false,
            budget_exceeded: true,
            status_budget: false,
        }),
    );
    assert!(walk_budget.budget_exceeded);
    assert!(walk_budget.paths.is_empty());
    assert!(!walk_budget.incomplete);

    let walk_failed = fold_run_observation(
        PathDelta::default(),
        false,
        false,
        Some(PathDelta {
            paths: vec!["prefix.txt".into()],
            incomplete: true,
            budget_exceeded: true,
            status_budget: false,
        }),
    );
    assert!(walk_failed.incomplete);
    assert!(!walk_failed.budget_exceeded);
    assert!(walk_failed.paths.is_empty());
}

#[tokio::test]
async fn a_foreign_commit_is_not_a_command_mutation_unless_this_command_invoked_git() {
    let root = tempfile::tempdir().unwrap();
    git_repo(root.path());
    std::fs::write(root.path().join("README.md"), "base\n").unwrap();
    git_at(root.path(), &["add", "README.md"]);
    git_at(root.path(), &["commit", "-m", "base"]);

    let watch = Watch::start(root.path()).await;
    std::fs::write(root.path().join("guest.txt"), "foreign\n").unwrap();
    git_at(root.path(), &["add", "guest.txt"]);
    git_at(root.path(), &["commit", "-m", "foreign"]);
    let foreign = watch.finish_with_revision(root.path(), false).await;
    assert!(
        !foreign.iter().any(|path| path == "guest.txt"),
        "parallel commit entered the command delta: {foreign:?}"
    );

    let watch = Watch::start(root.path()).await;
    std::fs::write(root.path().join("owned.txt"), "owned\n").unwrap();
    git_at(root.path(), &["add", "owned.txt"]);
    git_at(root.path(), &["commit", "-m", "owned"]);
    let owned = watch.finish_with_revision(root.path(), true).await;
    assert!(
        owned.iter().any(|path| path == "owned.txt"),
        "this command's commit was dropped: {owned:?}"
    );

    let watch = Watch::start(root.path()).await;
    std::fs::write(root.path().join("local.txt"), "local\n").unwrap();
    let local = watch.finish_with_revision(root.path(), false).await;
    assert!(
        local.iter().any(|path| path == "local.txt"),
        "an uncommitted write was dropped: {local:?}"
    );
}

#[tokio::test]
async fn a_status_record_cap_is_a_budget_and_drops_the_prefix() {
    let root = tempfile::tempdir().unwrap();
    git_repo(root.path());
    for index in 0..=STATUS_RECORD_CAP {
        std::fs::write(root.path().join(format!("f{index:05}")), b"x").unwrap();
    }
    let watch = Watch::start(root.path()).await;
    assert!(watch.snapshot.status_budget);
    assert!(!watch.snapshot.stamp_read_failed);
    assert!(watch.snapshot.porcelain.is_none());
    let baseline = watch.receipt_baseline();
    assert_eq!(baseline["git"]["status_complete"], false);
    assert!(baseline["git"]["status_digest"].is_null());
    std::fs::write(root.path().join("after.txt"), b"y").unwrap();
    let observed = watch
        .finish_observed_with_revision(root.path(), false)
        .await;
    assert!(observed.budget_exceeded);
    assert!(!observed.incomplete);
    assert!(observed.paths.is_empty());
}

#[tokio::test]
async fn a_status_byte_cap_is_a_budget_and_a_hidden_index_byte_cap_keeps_the_status() {
    let root = tempfile::tempdir().unwrap();
    git_repo(root.path());
    let leaf = "n".repeat(250);
    // `ls-files -v -z` is the shorter of the two capped outputs (`H <name>\0`).
    let count = (GIT_OUTPUT_BYTE_CAP as usize) / (2 + 5 + leaf.len() + 1) + 8;
    for index in 0..count {
        std::fs::write(root.path().join(format!("{index:05}{leaf}")), b"x").unwrap();
    }
    let watch = Watch::start(root.path()).await;
    assert!(watch.snapshot.status_budget);
    assert!(!watch.snapshot.stamp_read_failed);
    assert!(watch.snapshot.porcelain.is_none());
    let baseline = watch.receipt_baseline();
    assert_eq!(baseline["git"]["status_complete"], false);
    assert!(baseline["git"]["status_digest"].is_null());
    std::fs::write(root.path().join("new.txt"), b"y").unwrap();
    let observed = watch
        .finish_observed_with_revision(root.path(), false)
        .await;
    assert!(observed.budget_exceeded);
    assert!(!observed.incomplete);
    assert!(observed.paths.is_empty());

    git_at(root.path(), &["add", "-A"]);
    git_at(root.path(), &["commit", "-m", "tracked"]);
    let watch = Watch::start(root.path()).await;
    assert!(!watch.snapshot.status_budget);
    assert!(!watch.snapshot.stamp_read_failed);
    assert!(watch.snapshot.porcelain.is_some());
    assert!(watch.snapshot.stamps_incomplete);
    std::fs::write(root.path().join("added-after.txt"), b"z").unwrap();
    let observed = watch
        .finish_observed_with_revision(root.path(), false)
        .await;
    assert!(!observed.incomplete, "{observed:?}");
    assert!(!observed.budget_exceeded, "{observed:?}");
    assert!(
        observed.paths.iter().any(|path| path == "added-after.txt"),
        "{:?}",
        observed.paths
    );
}

fn git_repo(root: &Path) {
    git_at(root, &["init"]);
    git_at(root, &["config", "user.email", "test@example.com"]);
    git_at(root, &["config", "user.name", "test"]);
    git_at(root, &["config", "commit.gpgsign", "false"]);
}

fn git_at(root: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap_or_else(|error| panic!("git {args:?} failed to spawn: {error}"));
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
