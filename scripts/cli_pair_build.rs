// Both binaries stamp the same source bytes, including uncommitted Rust changes.
use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, Stdio};

pub fn stamp(root: &Path) -> io::Result<()> {
    let listed = Command::new("git")
        .current_dir(root)
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            "apps/aster",
            "crates/code",
            "crates/terminal",
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            "scripts/cli_pair_build.rs",
        ])
        .output()?;
    if !listed.status.success() {
        return Err(io::Error::other("cannot enumerate CLI pair sources"));
    }
    let mut paths = listed
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8(path.to_vec()).map_err(io::Error::other))
        .collect::<io::Result<Vec<_>>>()?;
    paths.sort();
    paths.dedup();
    let mut hash = Command::new("git")
        .current_dir(root)
        .args(["hash-object", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let mut input = hash
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("missing hash input"))?;
    for path in paths {
        let file = root.join(&path);
        let Ok(metadata) = file.symlink_metadata() else {
            continue;
        };
        if !metadata.is_file() && !metadata.file_type().is_symlink() {
            continue;
        }
        println!("cargo:rerun-if-changed={}", file.display());
        let bytes = if metadata.file_type().is_symlink() {
            std::fs::read_link(&file)?
                .to_string_lossy()
                .as_bytes()
                .to_vec()
        } else {
            std::fs::read(&file)?
        };
        input.write_all(path.as_bytes())?;
        input.write_all(&[0])?;
        input.write_all(&(bytes.len() as u64).to_le_bytes())?;
        input.write_all(&bytes)?;
    }
    drop(input);
    let output = hash.wait_with_output()?;
    if !output.status.success() {
        return Err(io::Error::other("cannot hash CLI pair sources"));
    }
    let digest = String::from_utf8(output.stdout).map_err(io::Error::other)?;
    let target = std::env::var("TARGET").map_err(io::Error::other)?;
    println!(
        "cargo:rustc-env=ASTER_PAIR_BUILD_ID=aster-cli-pair-v1:{}:{target}",
        digest.trim()
    );
    Ok(())
}
