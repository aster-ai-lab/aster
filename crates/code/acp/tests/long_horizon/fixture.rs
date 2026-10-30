#![cfg(test)]

use a3s_code_core::tools::{execute_owned_process, OwnedProcessTermination};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

const LIB: &str = r#"pub mod input;
pub mod calc;
pub mod display;
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn quantity_rejects_zero() { assert_eq!(input::quantity("0"), Err("invalid quantity")); }
    #[test] fn rounds_half_up() { assert_eq!(calc::total(1, 5, 1000), Ok(6)); }
    #[test] fn formats_cents() { assert_eq!(display::money(109), "1.09"); }
}
"#;
const INPUT: &str = "pub fn quantity(text: &str) -> Result<u32, &'static str> { text.trim().parse::<u32>().map_err(|_| \"invalid quantity\") }\n";
const CALC: &str = "pub fn total(quantity: u32, unit_cents: u32, tax_bps: u32) -> Result<u64, &'static str> { let net = u64::from(quantity) * u64::from(unit_cents); Ok(net + net * u64::from(tax_bps) / 10000) }\n";
const DISPLAY: &str =
    "pub fn money(cents: u64) -> String { format!(\"{}.{:01}\", cents / 100, cents % 100) }\n";
pub const LOCAL_CHECK: &str =
    "rustc --edition=2021 --test lib.rs -o checks && ./checks && test -s CHANGELOG.md";
const REQUIREMENTS: &str = r#"Repair the three public Rust modules without changing their public signatures.
input::quantity trims whitespace, accepts decimal integers from 1 to 1_000_000 inclusive, and returns Err("invalid quantity") for everything else.
calc::total accepts quantity and unit_cents in 1..=1_000_000 and tax_bps in 0..=10_000. Invalid arguments return Err("invalid amount"). Valid totals use integer cents and round tax to the nearest cent, with half cents rounded up. Do not use floating point.
display::money formats every u64 cent amount as whole units, a dot, and exactly two fractional digits.
Add CHANGELOG.md with a concise explanation. Preserve lib.rs, README.md, and notes.txt byte for byte. Do not remove or weaken tests.
"#;

pub const ARTIFACTS: [&str; 7] = [
    "lib.rs",
    "input.rs",
    "calc.rs",
    "display.rs",
    "README.md",
    "notes.txt",
    "CHANGELOG.md",
];

pub struct Fixture {
    pub workspace: PathBuf,
    state: PathBuf,
    checker: PathBuf,
    protected: BTreeMap<String, String>,
    checker_hash: String,
}

#[derive(Serialize)]
pub struct Oracle {
    pub correct: bool,
    pub compile_ok: bool,
    pub tests_ok: bool,
    pub protected_unchanged: bool,
    pub checker_unchanged: bool,
    pub changelog_present: bool,
    pub current_files: BTreeMap<String, String>,
    pub diagnostic: String,
}

fn checker_source(library: &Path) -> String {
    format!(
        r##"#[path = {:?}] mod fixture;
#[test] fn independent_contract() {{
    for (value, expected) in [("1",1),("1000000",1_000_000),(" 17 ",17),("007",7),("\t42\n",42)] {{ assert_eq!(fixture::input::quantity(value), Ok(expected)); }}
    for value in ["0", "1000001", "-1", "", "4294967296", "one"] {{ assert_eq!(fixture::input::quantity(value), Err("invalid quantity")); }}
    for q in [1, 2, 99, 1_000_000] {{ for p in [1, 5, 101, 1_000_000] {{ for rate in [0, 1, 500, 1000, 9999, 10000] {{
        let net = u64::from(q) * u64::from(p);
        assert_eq!(fixture::calc::total(q,p,rate), Ok(net + (net * u64::from(rate) + 5000) / 10000));
    }} }} }}
    for (q,p,r) in [(0,1,0),(1,0,0),(1_000_001,1,0),(1,1_000_001,0),(1,1,10_001),(u32::MAX,u32::MAX,u32::MAX)] {{ assert_eq!(fixture::calc::total(q,p,r), Err("invalid amount")); }}
    for cents in [0,1,9,10,99,100,109,1001,u64::MAX] {{ assert_eq!(fixture::display::money(cents), format!("{{}}.{{:02}}", cents/100,cents%100)); }}
}}
"##,
        library
    )
}

pub fn digest(path: &Path) -> std::io::Result<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut bytes = [0; 64 * 1024];
    loop {
        let count = file.read(&mut bytes)?;
        if count == 0 {
            break;
        }
        digest.update(&bytes[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

pub fn tests_passed(output: &str, names: &[&str]) -> bool {
    output
        .lines()
        .any(|line| line.starts_with("test result: ok.") && line.contains("; 0 failed;"))
        && names.iter().all(|name| {
            output
                .lines()
                .any(|line| line == format!("test {name} ... ok"))
        })
}

impl Fixture {
    pub fn create(root: &Path) -> std::io::Result<Self> {
        let workspace = root.join("workspace");
        let state = root.join("oracle");
        std::fs::create_dir_all(&workspace)?;
        std::fs::create_dir_all(&state)?;
        for (name, text) in [
            ("lib.rs", LIB),
            ("input.rs", INPUT),
            ("calc.rs", CALC),
            ("display.rs", DISPLAY),
            ("README.md", REQUIREMENTS),
            (
                "notes.txt",
                "Unrelated user work: keep exactly this byte sequence.\n",
            ),
        ] {
            std::fs::write(workspace.join(name), text)?;
        }
        std::fs::write(
            workspace.join("README.md"),
            format!("{REQUIREMENTS}Local check: {LOCAL_CHECK}\n"),
        )?;
        let protected = ["lib.rs", "README.md", "notes.txt"]
            .into_iter()
            .map(|name| Ok((name.to_owned(), digest(&workspace.join(name))?)))
            .collect::<std::io::Result<_>>()?;
        let checker = state.join("acceptance.rs");
        let source = checker_source(&workspace.join("lib.rs"));
        std::fs::write(&checker, source)?;
        let checker_hash = digest(&checker)?;
        Ok(Self {
            workspace,
            state,
            checker,
            protected,
            checker_hash,
        })
    }

    pub fn request(&self) -> &'static str {
        "Read README.md and complete its requested repair. Run the local check before and after your changes, and report the final result. You may repair several modules in one edit or separately. Do not change the protected files or weaken checks."
    }

    /// Only trusted, static C0 fixture sources may execute on the test host.
    pub async fn check(&self) -> Result<Oracle, &'static str> {
        let binary = self.state.join("acceptance");
        let mut command = tokio::process::Command::new("rustc");
        command
            .args(["--edition=2021", "--test"])
            .arg(&self.checker)
            .arg("-o")
            .arg(&binary);
        let compile = execute_owned_process(
            &mut command,
            &[],
            Duration::from_secs(30),
            &CancellationToken::new(),
        )
        .await
        .map_err(|_| "oracle_compile_io")?;
        let compile_ok = compile.termination == OwnedProcessTermination::Completed
            && compile.status.is_some_and(|status| status.success());
        let mut diagnostic = compile.stderr;
        let tests_ok = if compile_ok {
            let output = execute_owned_process(
                &mut tokio::process::Command::new(&binary),
                &[],
                Duration::from_secs(10),
                &CancellationToken::new(),
            )
            .await
            .map_err(|_| "oracle_run_io")?;
            diagnostic.push_str(&output.stdout);
            diagnostic.push_str(&output.stderr);
            output.termination == OwnedProcessTermination::Completed
                && output.status.is_some_and(|status| status.success())
        } else {
            false
        };
        let current_files = [
            "lib.rs",
            "input.rs",
            "calc.rs",
            "display.rs",
            "README.md",
            "notes.txt",
            "CHANGELOG.md",
        ]
        .into_iter()
        .filter_map(|name| {
            digest(&self.workspace.join(name))
                .ok()
                .map(|hash| (name.to_owned(), hash))
        })
        .collect();
        let protected_unchanged = self
            .protected
            .iter()
            .all(|(name, hash)| digest(&self.workspace.join(name)).ok().as_ref() == Some(hash));
        let checker_unchanged = digest(&self.checker).ok().as_ref() == Some(&self.checker_hash);
        let changelog_present = std::fs::read_to_string(self.workspace.join("CHANGELOG.md"))
            .is_ok_and(|text| !text.trim().is_empty());
        Ok(Oracle {
            correct: compile_ok
                && tests_ok
                && protected_unchanged
                && checker_unchanged
                && changelog_present,
            compile_ok,
            tests_ok,
            protected_unchanged,
            checker_unchanged,
            changelog_present,
            current_files,
            diagnostic,
        })
    }

    /// Called only after the ACP owner is reaped, or before it is started.
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    pub async fn check_in_guest(&self, assets: &Path) -> Result<Oracle, String> {
        use a3s_code_core::sandbox::oci::{OciBashSandbox, OciSandboxAssets};
        use a3s_code_core::sandbox::{BashSandbox, SandboxCommandRequest};
        let snapshot = tempfile::tempdir_in(&self.state).map_err(|_| "oracle_snapshot")?;
        let mut current_files = BTreeMap::new();
        for name in ARTIFACTS {
            let bytes = match super::artifact::read(&self.workspace, name) {
                Ok(bytes) => bytes,
                Err(error)
                    if name == "CHANGELOG.md" && error.kind() == std::io::ErrorKind::NotFound =>
                {
                    continue
                }
                Err(_) => return Err(format!("oracle_artifact_refused:{name}")),
            };
            std::fs::write(snapshot.path().join(name), &bytes)
                .map_err(|_| "oracle_snapshot_write")?;
            current_files.insert(name.to_owned(), format!("{:x}", Sha256::digest(&bytes)));
        }
        let hidden = snapshot.path().join("acceptance.rs");
        std::fs::write(&hidden, checker_source(Path::new("/workspace/lib.rs")))
            .map_err(|_| "oracle_checker_write")?;
        let hidden_hash = digest(&hidden).map_err(|_| "oracle_checker_identity")?;
        let sandbox = OciBashSandbox::from_artifacts(
            snapshot.path(),
            OciSandboxAssets::from_directory(assets).map_err(|_| "oracle_assets")?,
        )
        .map_err(|_| "oracle_sandbox")?;
        let output = sandbox.exec(SandboxCommandRequest {
            command: "rustc --edition=2021 --test acceptance.rs -o /tmp/acceptance && printf 'A3S_ORACLE_COMPILE_OK\\n' && /tmp/acceptance --test-threads=1".into(),
            guest_workspace: "/workspace".into(), timeout_ms: 30_000, output_observer: None, env: None,
        }).await;
        sandbox.shutdown().await;
        let output = output.map_err(|_| "oracle_guest_execution")?;
        let unchanged = current_files.iter().all(|(name, hash)| {
            super::artifact::read(snapshot.path(), name)
                .ok()
                .map(|bytes| format!("{:x}", Sha256::digest(bytes)))
                .as_ref()
                == Some(hash)
                && super::artifact::read(&self.workspace, name)
                    .ok()
                    .map(|bytes| format!("{:x}", Sha256::digest(bytes)))
                    .as_ref()
                    == Some(hash)
        });
        let checker_unchanged = unchanged
            && super::artifact::read(snapshot.path(), "acceptance.rs")
                .ok()
                .map(|bytes| format!("{:x}", Sha256::digest(bytes)))
                .as_ref()
                == Some(&hidden_hash)
            && digest(&self.checker).ok().as_ref() == Some(&self.checker_hash);
        let protected_unchanged = self
            .protected
            .iter()
            .all(|(name, hash)| current_files.get(name) == Some(hash));
        let changelog_present = super::artifact::read(&self.workspace, "CHANGELOG.md")
            .is_ok_and(|bytes| !String::from_utf8_lossy(&bytes).trim().is_empty());
        let compile_ok = !output.timed_out
            && output
                .stdout
                .lines()
                .any(|line| line == "A3S_ORACLE_COMPILE_OK");
        let tests_ok = compile_ok
            && output.exit_code == 0
            && tests_passed(
                &output.stdout,
                &[
                    "independent_contract",
                    "fixture::tests::formats_cents",
                    "fixture::tests::quantity_rejects_zero",
                    "fixture::tests::rounds_half_up",
                ],
            );
        Ok(Oracle {
            correct: tests_ok && protected_unchanged && checker_unchanged && changelog_present,
            compile_ok,
            tests_ok,
            protected_unchanged,
            checker_unchanged,
            changelog_present,
            current_files,
            diagnostic: format!("{}{}", output.stdout, output.stderr),
        })
    }
}

#[path = "fixture_tests.rs"]
mod tests;
