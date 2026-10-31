use std::os::unix::fs::PermissionsExt;

use super::{load, store, RunnerState};

fn state() -> RunnerState {
    RunnerState {
        runner_id: "6f3c1d2a-0000-4000-8000-000000000001".into(),
        credential: "aster_rc_example".into(),
    }
}

#[test]
fn a_missing_state_file_is_none() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(load(&dir.path().join("state.acl")).expect("load").is_none());
}

#[test]
fn the_state_round_trips_owner_only() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("nested/state.acl");
    store(&path, &state()).expect("store");
    assert!(load(&path).expect("load") == Some(state()));
    let mode = std::fs::metadata(&path).expect("meta").permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
    let parent = std::fs::metadata(dir.path().join("nested"))
        .expect("meta")
        .permissions()
        .mode();
    assert_eq!(parent & 0o777, 0o700);

    let replaced = RunnerState {
        credential: "aster_rc_rotated".into(),
        ..state()
    };
    store(&path, &replaced).expect("replace");
    assert!(load(&path).expect("load") == Some(replaced));
    assert!(!dir.path().join("nested/state.acl.tmp").exists());
}

#[test]
fn a_state_file_others_can_read_refuses_to_start() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("state.acl");
    store(&path, &state()).expect("store");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("chmod");
    let error = load(&path).err().expect("refused");
    assert!(error.ends_with("must not be readable by group or others (chmod 600)"));
}

#[test]
fn a_malformed_state_file_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("state.acl");
    for text in [
        "",
        "runner_state {\n  runner_id = \"x\"\n}\n",
        "other {\n}\n",
    ] {
        std::fs::write(&path, text).expect("write");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).expect("chmod");
        assert_eq!(
            load(&path).err().as_deref(),
            Some("state file must hold one runner_state block"),
            "{text:?}"
        );
    }
}
