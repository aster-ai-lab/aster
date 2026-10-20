#![cfg(all(test, unix))]

use super::*;
use std::fs::{File, FileTimes, OpenOptions};
use std::io::{Seek, SeekFrom, Write};

fn large_fixture(root: &Path) -> std::path::PathBuf {
    assert!(std::process::Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(root)
        .status()
        .unwrap()
        .success());
    let path = root.join("large.bin");
    File::create(&path)
        .unwrap()
        .set_len(OBSERVATION_BYTE_CAP + 1)
        .unwrap();
    path
}

#[tokio::test]
async fn a_large_dirty_file_edit_has_a_receipt_even_with_restored_mtime() {
    use a3s_effect::ToolObservationCompleteness;
    for restore_mtime in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let file = large_fixture(root.path());
        let modified = std::fs::metadata(&file).unwrap().modified().unwrap();
        let watch = Watch::start(root.path()).await;
        assert!(!watch.snapshot.stamps_incomplete);
        assert_eq!(watch.receipt_baseline()["git"]["unhashed_stamp_count"], 1);
        assert!(watch.snapshot.stamps[0].content_digest.is_none());
        let mut writable = OpenOptions::new().write(true).open(&file).unwrap();
        writable.seek(SeekFrom::Start(4096)).unwrap();
        writable.write_all(b"real mutation").unwrap();
        writable.sync_all().unwrap();
        if restore_mtime {
            writable
                .set_times(FileTimes::new().set_modified(modified))
                .unwrap();
        }
        let receipt = watch.finish_effects_with_revision(root.path(), true).await;
        assert_eq!(receipt.completeness, ToolObservationCompleteness::Complete);
        assert_eq!(receipt.changes.len(), 1);
        assert_eq!(receipt.changes[0].path, "large.bin");
        assert!(
            receipt.changes[0].before.is_none(),
            "an unhashed preimage is not byte evidence"
        );
        assert!(receipt.changes[0]
            .after
            .content_digest
            .as_ref()
            .unwrap()
            .starts_with("sha256:"));
    }
}

#[tokio::test]
async fn an_unchanged_large_file_does_not_poison_the_next_observation() {
    let root = tempfile::tempdir().unwrap();
    large_fixture(root.path());
    let watch = Watch::start(root.path()).await;
    let delta = watch
        .finish_observed_with_revision(root.path(), false)
        .await;
    assert!(delta.paths.is_empty());
    assert!(!delta.incomplete && !delta.budget_exceeded);
    let mut budget = 0;
    assert!(observe_file_identity(root.path(), "large.bin", &mut budget).is_none());
    assert_eq!(budget, 0, "strict receipt reads must keep their byte bound");
}
