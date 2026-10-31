use anyhow::{Context, bail};
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn check_protoc_good(protoc: &Path) -> anyhow::Result<()> {
    let output = Command::new(protoc)
        .arg("--version")
        .output()
        .context("Failed to execute protoc")?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "protoc --version failed, likely dotslash is missing; \
             try `cargo install dotslash`; stdout: {stdout:?}, stderr: {stderr:?}"
        );
    }
    Ok(())
}

fn is_github_actions() -> bool {
    env::var_os("GITHUB_ACTIONS").is_some()
}

/// Find `protoc` command.
///
/// Search order:
/// 1. `$PROTOC` environment variable (set by Bazel `build_script_env` or user override)
/// 2. `bin/protoc` walking up parent directories (dotslash wrapper for local dev)
/// 3. `protoc` on `$PATH` (system install or other tooling)
///
/// When `bin/protoc` exists but fails to execute (e.g. the dotslash wrapper running
/// in Bazel remote execution where `dotslash` is not installed), the error is not fatal —
/// we fall through to the PATH-based lookup instead.
///
/// Returns `Ok(None)` if not found and not in a strict environment (GitHub Actions).
pub fn find_protoc() -> anyhow::Result<Option<PathBuf>> {
    println!("cargo:rerun-if-env-changed=PROTOC");
    println!("cargo:rerun-if-env-changed=PATH");
    // 1. Check the PROTOC env var first. This is the standard override used by prost-build
    //    and is set by Bazel cargo_build_script build_script_env to point at a hermetic
    //    protoc binary instead of the dotslash wrapper.
    if let Ok(protoc_env) = env::var("PROTOC") {
        let protoc = PathBuf::from(&protoc_env);
        if protoc.try_exists()? {
            check_protoc_good(&protoc)?;
            return Ok(Some(protoc));
        }
    }

    // 2. Walk up directories looking for bin/protoc (dotslash wrapper).
    let cwd = env::current_dir()?;
    let mut dir = cwd.clone();
    let mut dir_rel = PathBuf::new();
    loop {
        // Return relative path to make build more deterministic.
        let protoc = dir_rel.join("bin/protoc");
        if protoc.try_exists()? {
            match check_protoc_good(&protoc) {
                Ok(()) => return Ok(Some(protoc)),
                Err(e) => {
                    // bin/protoc exists but can't execute — likely the dotslash wrapper
                    // in an environment without dotslash (e.g. Bazel remote execution).
                    // Fall through to PATH-based lookup below.
                    eprintln!(
                        "bin/protoc found at `{}` but failed to execute: {e:#}; \
                         trying protoc from PATH as fallback",
                        protoc.display()
                    );
                    break;
                }
            }
        }
        if !dir.pop() {
            break;
        }
        dir_rel.push("..");
    }

    // 3. Try protoc from PATH (system install or other tooling).
    if let Some(paths) = env::var_os("PATH") {
        for directory in env::split_paths(&paths) {
            let protoc =
                std::path::absolute(directory.join(format!("protoc{}", env::consts::EXE_SUFFIX)))?;
            if check_protoc_good(&protoc).is_ok() {
                return Ok(Some(protoc));
            }
        }
    }

    // 4. Not found anywhere.
    if is_github_actions() {
        return Err(anyhow::anyhow!(
            "`protoc` not found (checked $PROTOC env, bin/protoc, and PATH)"
        ));
    }
    eprintln!("`protoc` not found; likely it is missing in docker image");
    Ok(None)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn path_lookup_returns_the_existing_compiler_file() {
        const PROBE: &str = "ASTER_PROTOC_PATH_PROBE";
        if env::var_os(PROBE).is_some() {
            let compiler = find_protoc().unwrap().expect("fixture compiler");
            assert!(
                compiler.is_absolute(),
                "a PATH command is not a Cargo file dependency: {compiler:?}"
            );
            assert!(
                compiler.is_file(),
                "watched compiler must exist: {compiler:?}"
            );
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let empty = root.path().join("empty");
        let valid = root.path().join("installed compiler");
        std::fs::create_dir(&empty).unwrap();
        std::fs::create_dir(&valid).unwrap();
        let compiler = valid.join("protoc");
        std::fs::write(&compiler, "#!/bin/sh\nprintf 'fixture protoc\\n'\n").unwrap();
        std::fs::set_permissions(&compiler, std::fs::Permissions::from_mode(0o755)).unwrap();
        let output = Command::new(env::current_exe().unwrap())
            .args([
                "find_protoc::tests::path_lookup_returns_the_existing_compiler_file",
                "--exact",
                "--nocapture",
            ])
            .current_dir(root.path())
            .env_remove("PROTOC")
            .env(
                "PATH",
                env::join_paths([PathBuf::new(), empty, valid]).unwrap(),
            )
            .env(PROBE, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
    }
}
