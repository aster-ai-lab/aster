#![cfg(test)]

use super::*;

static STATE_TEST: Mutex<()> = Mutex::new(());

#[test]
fn installed_identity_is_cached_only_after_success_and_for_the_exact_executable() {
    let cache = OnceLock::new();
    let calls = std::cell::Cell::new(0);
    let first = Path::new("/fixture/ToWork.app/Contents/MacOS/Studio");
    let other = Path::new("/other/ToWork.app/Contents/MacOS/Studio");
    let validate = |_: &Path| {
        calls.set(calls.get() + 1);
        Ok(Some("toyou".to_owned()))
    };
    assert_eq!(
        cached_installed_tenant(&cache, first, validate)
            .unwrap()
            .as_deref(),
        Some("toyou")
    );
    assert_eq!(
        cached_installed_tenant(&cache, first, |_| panic!("must use validated identity"))
            .unwrap()
            .as_deref(),
        Some("toyou")
    );
    assert_eq!(calls.get(), 1);
    assert!(matches!(
        cached_installed_tenant(&cache, other, |_| Err(UpdateFail::ProductMismatch)),
        Err(UpdateFail::ProductMismatch)
    ));
}

#[test]
fn absent_or_failed_installed_identity_is_retried() {
    let cache = OnceLock::new();
    let exe = Path::new("/fixture/Studio");
    assert!(
        cached_installed_tenant(&cache, exe, |_| Ok(None))
            .unwrap()
            .is_none()
    );
    assert!(cache.get().is_none());
    assert!(cached_installed_tenant(&cache, exe, |_| Err(UpdateFail::IntegrityFailed)).is_err());
    assert!(cache.get().is_none());
    assert_eq!(
        cached_installed_tenant(&cache, exe, |_| Ok(Some("toyou".into())))
            .unwrap()
            .as_deref(),
        Some("toyou")
    );
}

fn ready() -> ReadyUpdate {
    ReadyUpdate {
        version: "2.1.4".into(),
        notes: String::new(),
        sha256: "ab".repeat(32),
        path: PathBuf::from("unused-update-fixture"),
        size: 4,
        published_at: String::new(),
    }
}

#[test]
fn installing_owns_busy_until_settled() {
    let _lock = STATE_TEST.lock().unwrap();
    store_ready(ready()).unwrap();
    take_ready().unwrap();
    let busy = matches!(*PHASE.lock().unwrap(), Phase::Busy);
    *PHASE.lock().unwrap() = Phase::Idle;
    drop(_lock);
    assert!(busy, "taking an installer must retain exclusive ownership");
}

#[test]
fn a_failed_check_preserves_the_download_for_retry() {
    let _lock = STATE_TEST.lock().unwrap();
    store_ready(ready()).unwrap();
    drop(BusyGuard::enter().unwrap());
    let retained = matches!(*PHASE.lock().unwrap(), Phase::Ready(_));
    *PHASE.lock().unwrap() = Phase::Idle;
    drop(_lock);
    assert!(
        retained,
        "transient check errors must not lose a ready download"
    );
}

#[test]
fn an_executable_without_companions_is_not_a_studio_delivery() {
    let temp = tempfile::tempdir().unwrap();
    let bin = temp.path().join("Contents/MacOS");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join("Studio"), b"host").unwrap();
    assert!(ensure_bundle_executable(temp.path()).is_err());
}

#[test]
fn bare_host_updates_are_refused() {
    let temp = tempfile::tempdir().unwrap();
    let host = temp.path().join("Studio");
    fs::write(&host, [0xcf, 0xfa, 0xed, 0xfe, 0, 0, 0, 0]).unwrap();
    assert!(matches!(classify(&host), Err(UpdateFail::InvalidPackage)));
}

#[test]
fn recoverable_install_failure_restores_ready_and_blocks_another_installer() {
    let _lock = STATE_TEST.lock().unwrap();
    store_ready(ready()).unwrap();
    let ready = take_ready().unwrap();
    let guard = BusyGuard {
        armed: true,
        previous: Some(ready),
    };
    assert!(matches!(take_ready(), Err(UpdateFail::InProgress)));
    assert!(matches!(BusyGuard::enter(), Err(UpdateFail::InProgress)));
    drop(guard);
    assert!(matches!(*PHASE.lock().unwrap(), Phase::Ready(_)));
    *PHASE.lock().unwrap() = Phase::Idle;
}

#[test]
fn isolated_bundle_upgrade_and_rollback_keep_the_whole_generation() {
    let temp = tempfile::tempdir().unwrap();
    let old = temp.path().join("ToWork.app");
    let new = temp.path().join("stage/ToWork.app");
    for (root, content) in [(&old, b"old"), (&new, b"new")] {
        for file in [
            "Contents/MacOS/Studio",
            "Contents/Resources/agent/a3s-code-acp",
            "Contents/Resources/agent/a3s-office",
        ] {
            let path = root.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
    }
    let backup = replace_bundle(&new, &old).unwrap();
    assert_eq!(
        fs::read(old.join("Contents/Resources/agent/a3s-code-acp")).unwrap(),
        b"new"
    );
    assert_eq!(
        fs::read(backup.join("Contents/Resources/agent/a3s-office")).unwrap(),
        b"old"
    );
    restore_bundle(&old, &backup).unwrap();
    assert_eq!(fs::read(old.join("Contents/MacOS/Studio")).unwrap(), b"old");
    assert_eq!(
        fs::read(old.join("Contents/Resources/agent/a3s-code-acp")).unwrap(),
        b"old"
    );
}

#[test]
fn a_previous_backup_is_preserved_without_touching_either_generation() {
    let temp = tempfile::tempdir().unwrap();
    let current = temp.path().join("ToWork.app");
    let backup = temp.path().join("ToWork.app.previous");
    let new = temp.path().join("new.app");
    for (path, content) in [
        (&current, b"old".as_slice()),
        (&backup, b"saved".as_slice()),
        (&new, b"new".as_slice()),
    ] {
        fs::create_dir_all(path).unwrap();
        fs::write(path.join("generation"), content).unwrap();
    }
    assert!(matches!(
        replace_bundle(&new, &current),
        Err(UpdateFail::InstallFailed)
    ));
    assert_eq!(fs::read(current.join("generation")).unwrap(), b"old");
    assert_eq!(fs::read(backup.join("generation")).unwrap(), b"saved");
    assert_eq!(fs::read(new.join("generation")).unwrap(), b"new");
}

#[cfg(unix)]
#[test]
fn relaunch_open_failure_restores_the_old_bundle_and_retains_the_failed_generation() {
    let temp = tempfile::tempdir().unwrap();
    let current = temp.path().join("To Work.app");
    let backup = temp.path().join("To Work.app.previous");
    fs::create_dir_all(&current).unwrap();
    fs::create_dir_all(&backup).unwrap();
    fs::write(current.join("generation"), "new").unwrap();
    fs::write(backup.join("generation"), "old").unwrap();
    let opener = temp.path().join("open-fixture");
    fs::write(&opener, "#!/bin/sh\nexit 1\n").unwrap();
    set_executable(&opener).unwrap();
    let mut exited_parent = Command::new("/bin/sh")
        .args(["-c", "exit 0"])
        .spawn()
        .unwrap();
    let pid = exited_parent.id();
    exited_parent.wait().unwrap();
    let result = relaunch_command(&current, Some(&backup), pid, &opener)
        .status()
        .unwrap();
    assert!(!result.success());
    assert_eq!(fs::read(current.join("generation")).unwrap(), b"old");
    assert!(!backup.exists());
    assert!(
        fs::read_dir(temp.path())
            .unwrap()
            .filter_map(Result::ok)
            .any(|entry| entry
                .file_name()
                .to_string_lossy()
                .starts_with("To Work.app.failed-"))
    );
}

#[cfg(unix)]
#[test]
fn successful_relaunch_removes_only_the_supplied_backup() {
    let temp = tempfile::tempdir().unwrap();
    let current = temp.path().join("ToWork.app");
    let backup = temp.path().join("ToWork.app.previous");
    let unrelated = temp.path().join("other.app.previous");
    for path in [&current, &backup, &unrelated] {
        fs::create_dir_all(path).unwrap();
    }
    let opener = temp.path().join("open-fixture");
    fs::write(&opener, "#!/bin/sh\nexit 0\n").unwrap();
    set_executable(&opener).unwrap();
    let mut exited_parent = Command::new("/bin/sh")
        .args(["-c", "exit 0"])
        .spawn()
        .unwrap();
    let pid = exited_parent.id();
    exited_parent.wait().unwrap();
    assert!(
        relaunch_command(&current, Some(&backup), pid, &opener)
            .status()
            .unwrap()
            .success()
    );
    assert!(current.is_dir());
    assert!(!backup.exists());
    assert!(unrelated.is_dir());
}
#[test]
fn update_errors_round_trip_and_refuse_unknown_codes() {
    for fail in [
        UpdateFail::InProgress,
        UpdateFail::NotPublished,
        UpdateFail::UpToDate,
        UpdateFail::Unavailable,
        UpdateFail::Failed,
        UpdateFail::InvalidPackage,
        UpdateFail::ProductMismatch,
        UpdateFail::IntegrityFailed,
        UpdateFail::InstallFailed,
        UpdateFail::RestartFailed,
    ] {
        assert_eq!(UpdateFail::parse(fail.as_str()), Ok(fail));
    }
    for code in [
        "",
        "UPDATE_FAILED",
        " update_failed",
        "update_failed ",
        "unknown",
    ] {
        assert!(UpdateFail::parse(code).unwrap_err().contains("UpdateFail"));
    }
}
