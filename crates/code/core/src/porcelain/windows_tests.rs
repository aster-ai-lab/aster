#![cfg(test)]

use super::*;

#[tokio::test]
async fn native_sandbox_read_has_no_completion_debt() {
    use crate::tools::Tool;

    let base = std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("Temp");
    let root = tempfile::Builder::new()
        .prefix("aster-observe-")
        .tempdir_in(base)
        .unwrap();
    let workspace = root.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    std::fs::write(workspace.join("proof.txt"), b"READ-ONLY-PROOF").unwrap();
    let sandbox = crate::sandbox::native::NativeBashSandbox::new(&workspace).unwrap();
    let context = crate::tools::ToolContext::new(workspace.clone())
        .with_sandbox(std::sync::Arc::new(sandbox));
    let before = observe_identity_windows(&workspace, "proof.txt", &mut 1024, false).unwrap();
    let watch = Watch::start(&workspace).await;
    let result = crate::tools::builtin::bash::BashTool
        .execute(&serde_json::json!({"command":"cat proof.txt"}), &context)
        .await
        .unwrap();
    assert!(result.success, "{}", result.content);
    assert!(result.content.contains("READ-ONLY-PROOF"));
    let after = observe_identity_windows(&workspace, "proof.txt", &mut 1024, false).unwrap();
    assert_eq!(before.content_digest, after.content_digest);
    assert_eq!(before.mode, after.mode);
    assert_eq!(before.node, after.node);
    assert_eq!(before.modified, after.modified);
    let effects = watch.finish_effects_with_revision(&workspace, false).await;
    assert_eq!(
        effects.completeness,
        a3s_effect::ToolObservationCompleteness::Complete
    );
    assert!(effects
        .changes
        .iter()
        .all(|change| change.before.as_ref() == Some(&change.after)));
    let mut ledger = crate::harness_loop::MutationLedger::default();
    ledger.observe_background_receipt("native-read-fixture", &effects);
    assert!(
        ledger.is_empty(),
        "identical file states must not create verification debt"
    );
}

#[test]
fn actual_powershell_read_keeps_the_observation_stable() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    std::fs::write(workspace.join("proof.txt"), b"READ-ONLY-PROOF").unwrap();
    let script = root.path().join("read.ps1");
    std::fs::write(
        &script,
        "Get-Content -LiteralPath (Join-Path $PSScriptRoot 'workspace/proof.txt')\n",
    )
    .unwrap();
    let before = observe_identity_windows(&workspace, "proof.txt", &mut 1024, false).unwrap();
    let index = index_files(&workspace);
    let shell = std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap())
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let output = std::process::Command::new(shell)
        .args(["-NoProfile", "-NonInteractive", "-File"])
        .arg(script)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "READ-ONLY-PROOF"
    );
    let after = observe_identity_windows(&workspace, "proof.txt", &mut 1024, false).unwrap();
    assert_eq!(before, after);
    let delta = file_delta(&workspace, &index);
    assert!(!delta.incomplete);
    assert!(
        delta.paths.is_empty(),
        "read-only command observed as a mutation: {delta:?}"
    );
}

#[test]
fn actual_windows_files_have_bounded_byte_identities() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("file.txt"), b"no-newline").unwrap();
    let stamp = observe_identity_windows(root.path(), "file.txt", &mut 10, false).unwrap();
    assert_eq!(
        stamp.content_digest,
        Some(format!("{:x}", Sha256::digest(b"no-newline")))
    );
    assert_eq!(stamp.len, 10);
    assert!(observe_identity_windows(root.path(), "file.txt", &mut 9, false).is_none());
    let metadata = observe_identity_windows(root.path(), "file.txt", &mut 0, true).unwrap();
    assert!(metadata.content_digest.is_none());
    assert!(
        !observe_identity_windows(root.path(), "missing/child", &mut 0, false)
            .unwrap()
            .present
    );
}

#[test]
fn actual_windows_walk_detects_changes_without_git() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("before.txt"), b"before").unwrap();
    let before = index_files(root.path());
    assert!(!before.incomplete);
    std::fs::write(root.path().join("created.txt"), b"created").unwrap();
    let delta = file_delta(root.path(), &before);
    assert!(!delta.incomplete);
    assert!(delta.paths.contains(&"created.txt".to_string()));
    let effects = observe_path_effects(root.path(), delta);
    assert_eq!(
        effects.completeness,
        a3s_effect::ToolObservationCompleteness::Complete
    );
    assert!(effects
        .changes
        .iter()
        .any(|c| c.path == "created.txt" && c.after.content_digest.is_some()));
}

#[test]
fn windows_observer_refuses_escapes_and_streams() {
    let root = tempfile::tempdir().unwrap();
    assert!(
        observe_identity_windows(&root.path().join("missing-root"), "child", &mut 1024, false)
            .is_none()
    );
    for path in ["../outside", "C:\\Windows\\win.ini", "file.txt:stream", ""] {
        assert!(
            observe_identity_windows(root.path(), path, &mut 1024, false).is_none(),
            "{path}"
        );
    }
    std::fs::write(root.path().join("file"), b"file").unwrap();
    assert!(observe_identity_windows(root.path(), "file/child", &mut 1024, false).is_none());
}

#[test]
fn windows_observer_refuses_reparse_point_ancestors() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret.txt"), b"outside").unwrap();
    // A junction requires no symlink privilege and exercises a directory reparse point.
    let status = std::process::Command::new("cmd.exe")
        .args(["/C", "mklink", "/J"])
        .arg(root.path().join("escape"))
        .arg(outside.path())
        .output()
        .unwrap();
    assert!(status.status.success());
    assert!(observe_identity_windows(root.path(), "escape/secret.txt", &mut 1024, false).is_none());
    assert!(
        observe_identity_windows(&root.path().join("escape"), "secret.txt", &mut 1024, false)
            .is_none()
    );
    std::fs::remove_dir(root.path().join("escape")).unwrap();
}
