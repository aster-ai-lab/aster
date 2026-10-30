#![cfg(test)]

use super::*;

fn file(bytes: &str) -> ToolFileIdentityV1 {
    ToolFileIdentityV1 {
        kind: ToolFileKind::File,
        content_digest: Some(crate::content_digest::digest_bytes(
            "fixture",
            bytes.as_bytes(),
        )),
        mode: Some(0o644),
    }
}

fn missing() -> ToolFileIdentityV1 {
    ToolFileIdentityV1 {
        kind: ToolFileKind::Missing,
        content_digest: None,
        mode: None,
    }
}

fn change(path: &str, before: ToolFileIdentityV1, after: ToolFileIdentityV1) -> ToolPathChangeV1 {
    ToolPathChangeV1 {
        path: path.into(),
        before: Some(before),
        after,
    }
}

#[test]
fn net_zero_existing_and_created_paths_do_not_earn_progress() {
    for original in [file("original"), missing()] {
        let mut progress = MutationProgress::default();
        assert!(!progress
            .observe(Some(vec![
                change("source", original.clone(), file("temporary")),
                change("source", file("temporary"), original),
            ]))
            .unwrap());
    }
}

#[test]
fn previously_observed_terminal_states_do_not_refresh_mutation_progress() {
    let mut progress = MutationProgress::default();
    for (from, to, expected) in [
        ("a", "b", true),
        ("b", "c", true),
        ("c", "b", false),
        ("b", "a", false),
        ("a", "d", true),
    ] {
        assert_eq!(
            progress
                .observe(Some(vec![change("source", file(from), file(to))]))
                .unwrap(),
            expected
        );
    }
}

#[test]
fn first_observation_of_another_path_does_not_rename_previous_states() {
    let mut progress = MutationProgress::default();
    for (path, from, to, expected) in [
        ("one", "a", "b", true),
        ("two", "x", "y", true),
        ("two", "y", "x", false),
        ("one", "b", "a", false),
    ] {
        assert_eq!(
            progress
                .observe(Some(vec![change(path, file(from), file(to))]))
                .unwrap(),
            expected
        );
    }
}

#[test]
fn overlapping_identical_observations_are_coherent_but_a_gap_cannot_prove_progress() {
    let mut progress = MutationProgress::default();
    assert!(progress
        .observe(Some(vec![
            change("source", file("a"), file("b")),
            change("source", file("a"), file("b"))
        ]))
        .unwrap());
    assert!(!progress
        .observe(Some(vec![
            change("source", file("b"), file("c")),
            change("source", file("unknown"), file("d"))
        ]))
        .unwrap());
    assert!(progress
        .observe(Some(vec![change("source", file("d"), file("e"))]))
        .unwrap());
}

#[test]
fn unavailable_evidence_does_not_reset_visited_states() {
    let mut progress = MutationProgress::default();
    assert!(progress
        .observe(Some(vec![change("source", file("a"), file("b"))]))
        .unwrap());
    assert!(!progress.observe(None).unwrap());
    assert!(!progress
        .observe(Some(vec![change("source", file("b"), file("a"))]))
        .unwrap());
    assert!(!progress
        .observe(Some(vec![change("source", file("a"), file("b"))]))
        .unwrap());
}

#[test]
fn original_continuity_recovers_without_forgetting_already_seen_states() {
    let mut progress = MutationProgress::default();
    assert!(progress
        .observe(Some(vec![change("source", file("a"), file("b"))]))
        .unwrap());
    assert!(progress.last_observation_is_coherent());
    assert!(!progress
        .observe(Some(vec![change("source", file("external"), file("a"))]))
        .unwrap());
    assert!(!progress.last_observation_is_coherent());
    assert!(!progress
        .observe(Some(vec![change("source", file("a"), file("b"))]))
        .unwrap());
    assert!(progress.last_observation_is_coherent());
}

#[test]
fn missing_observation_cannot_supply_a_continuity_proof() {
    let mut progress = MutationProgress::default();
    assert!(progress
        .observe(Some(vec![change("source", file("a"), file("b"))]))
        .unwrap());
    assert!(!progress.observe(None).unwrap());
    assert!(!progress.last_observation_is_coherent());
    assert!(!progress
        .observe(Some(vec![change("source", file("b"), file("a"))]))
        .unwrap());
    assert!(progress.last_observation_is_coherent());
}

#[test]
fn unknown_initial_identity_cannot_supply_progress_but_later_known_changes_can() {
    let mut progress = MutationProgress::default();
    assert!(!progress
        .observe(Some(vec![ToolPathChangeV1 {
            path: "source".into(),
            before: None,
            after: file("a")
        }]))
        .unwrap());
    assert!(progress
        .observe(Some(vec![change("source", file("a"), file("b"))]))
        .unwrap());
    assert!(!progress
        .observe(Some(vec![change("source", file("b"), file("a"))]))
        .unwrap());
}

#[test]
fn known_modes_and_symlink_targets_are_states_but_new_metadata_alone_is_not() {
    let mut unknown_mode = file("a");
    unknown_mode.mode = None;
    assert!(!MutationProgress::default()
        .observe(Some(vec![change("source", unknown_mode, file("a"))]))
        .unwrap());
    let mut executable = file("a");
    executable.mode = Some(0o755);
    let mut progress = MutationProgress::default();
    assert!(progress
        .observe(Some(vec![change("source", file("a"), executable.clone())]))
        .unwrap());
    assert!(!progress
        .observe(Some(vec![change("source", executable, file("a"))]))
        .unwrap());
    let mut from = file("target-a");
    from.kind = ToolFileKind::Symlink;
    let mut to = file("target-b");
    to.kind = ToolFileKind::Symlink;
    assert!(MutationProgress::default()
        .observe(Some(vec![change("link", from, to)]))
        .unwrap());
}
