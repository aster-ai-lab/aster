#![cfg(test)]

use super::*;

pub(super) fn repair(fixture: &Fixture) {
    std::fs::write(fixture.workspace.join("input.rs"), "pub fn quantity(text:&str)->Result<u32,&'static str>{text.trim().parse::<u32>().ok().filter(|n|(1..=1_000_000).contains(n)).ok_or(\"invalid quantity\")}\n").unwrap();
    std::fs::write(fixture.workspace.join("calc.rs"), "pub fn total(q:u32,p:u32,r:u32)->Result<u64,&'static str>{if !(1..=1_000_000).contains(&q)||!(1..=1_000_000).contains(&p)||r>10000{return Err(\"invalid amount\");}let n=u64::from(q)*u64::from(p);Ok(n+(n*u64::from(r)+5000)/10000)}\n").unwrap();
    std::fs::write(
        fixture.workspace.join("display.rs"),
        "pub fn money(c:u64)->String{format!(\"{}.{:02}\",c/100,c%100)}\n",
    )
    .unwrap();
    std::fs::write(
        fixture.workspace.join("CHANGELOG.md"),
        "Repair integer validation, rounding, and cent formatting.\n",
    )
    .unwrap();
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "real compiler VM oracle controls; requires explicit qualified ASTER_TEST_OCI_ASSETS"]
async fn guest_oracle_rejects_baseline_current_wrong_bytes_and_protected_changes() {
    let assets =
        PathBuf::from(std::env::var_os("ASTER_TEST_OCI_ASSETS").expect("explicit guest assets"));
    let root = tempfile::tempdir().unwrap();
    let fixture = Fixture::create(root.path()).unwrap();
    let baseline = fixture.check_in_guest(&assets).await.unwrap();
    assert!(baseline.compile_ok && !baseline.correct && !baseline.tests_ok);
    repair(&fixture);
    let repaired = fixture.check_in_guest(&assets).await.unwrap();
    assert!(repaired.correct, "{}", repaired.diagnostic);
    std::fs::write(fixture.workspace.join("display.rs"), DISPLAY).unwrap();
    assert!(!fixture.check_in_guest(&assets).await.unwrap().correct);
    repair(&fixture);
    std::fs::write(fixture.workspace.join("notes.txt"), "changed").unwrap();
    let changed = fixture.check_in_guest(&assets).await.unwrap();
    assert!(changed.tests_ok && !changed.protected_unchanged && !changed.correct);
    println!(
        "GUEST_ORACLE_CONTROLS {}",
        serde_json::json!({"baseline":baseline,"repaired":repaired,"changed":changed})
    );
}

#[tokio::test]
async fn independent_oracle_rejects_original_bugs_and_accepts_a_complete_repair() {
    let root = tempfile::tempdir().unwrap();
    let fixture = Fixture::create(root.path()).unwrap();
    let original = fixture.check().await.unwrap();
    assert!(original.compile_ok);
    assert!(!original.correct);
    repair(&fixture);
    assert!(fixture.check().await.unwrap().correct);
}

#[tokio::test]
async fn cached_success_and_modified_checks_cannot_hide_current_wrong_bytes() {
    let root = tempfile::tempdir().unwrap();
    let fixture = Fixture::create(root.path()).unwrap();
    repair(&fixture);
    assert!(fixture.check().await.unwrap().correct);
    std::fs::write(fixture.workspace.join("display.rs"), DISPLAY).unwrap();
    assert!(!fixture.check().await.unwrap().correct);
    repair(&fixture);
    std::fs::write(
        fixture.workspace.join("lib.rs"),
        LIB.replace("assert_eq!(display::money(109), \"1.09\");", ""),
    )
    .unwrap();
    let changed = fixture.check().await.unwrap();
    assert!(!changed.protected_unchanged);
    assert!(!changed.correct);
    std::fs::write(&fixture.checker, "#[test] fn forged() {}\n").unwrap();
    assert!(!fixture.check().await.unwrap().correct);
}

#[tokio::test]
async fn independent_oracle_rejects_a_parser_that_returns_the_wrong_quantity() {
    let root = tempfile::tempdir().unwrap();
    let fixture = Fixture::create(root.path()).unwrap();
    repair(&fixture);
    std::fs::write(fixture.workspace.join("input.rs"), "pub fn quantity(text:&str)->Result<u32,&'static str>{text.trim().parse::<u32>().ok().filter(|n|(1..=1_000_000).contains(n)).map(|_|1).ok_or(\"invalid quantity\")}\n").unwrap();
    assert!(
        !fixture.check().await.unwrap().correct,
        "valid inputs must return their actual parsed values"
    );
}

#[test]
fn test_output_requires_the_original_cases_and_allows_extra_tests() {
    let output =
        "test original ... ok\ntest added ... ok\ntest result: ok. 2 passed; 0 failed; 0 ignored";
    assert!(tests_passed(output, &["original"]));
    assert!(!tests_passed(output, &["missing"]));
    assert!(!tests_passed(
        "test result: ok. 2 passed; 0 failed; 0 ignored",
        &["original"]
    ));
    assert!(!tests_passed(
        &output.replace("0 failed", "1 failed"),
        &["original"]
    ));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "regrade retained delivery bytes with current hidden oracle; no model request or original-result overwrite"]
async fn retained_rust_delivery_passes_the_strengthened_guest_oracle() {
    let retained = PathBuf::from(
        std::env::var_os("A3S_LH_RECHECK_DIR").expect("explicit retained Rust case directory"),
    );
    let assets = PathBuf::from(
        std::env::var_os("ASTER_TEST_OCI_ASSETS").expect("explicit qualified guest assets"),
    );
    assert!(retained.is_absolute() && assets.is_absolute());
    let result: serde_json::Value =
        serde_json::from_slice(&super::super::artifact::read(&retained, "result.json").unwrap())
            .unwrap();
    assert_eq!(
        result["metadata"]["record"]["variant"],
        "rust-multifile-host-contract-real-diagnostic"
    );
    let root = tempfile::tempdir().unwrap();
    let fixture = Fixture::create(root.path()).unwrap();
    for name in ARTIFACTS {
        let bytes = super::super::artifact::read(&retained, name).unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            result["oracle"]["Ok"]["current_files"][name]
                .as_str()
                .unwrap()
        );
        std::fs::write(fixture.workspace.join(name), bytes).unwrap();
    }
    let oracle = fixture.check_in_guest(&assets).await.unwrap();
    let report = serde_json::json!({"scope":"same retained delivery bytes, stronger independent functional checker only; original whole-run status retained","original_pass":result["pass"],"current_oracle":oracle,"paid_model_calls":0});
    let output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(retained.join("strengthened-oracle-recheck.json"))
        .unwrap();
    serde_json::to_writer_pretty(output, &report).unwrap();
    assert!(oracle.correct, "{}", oracle.diagnostic);
    println!("RETAINED_RUST_ORACLE {}", report);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "real VM test-system regression; requires explicit qualified assets"]
async fn guest_oracle_accepts_additional_legitimate_module_tests() {
    let root = tempfile::tempdir().unwrap();
    let fixture = Fixture::create(root.path()).unwrap();
    repair(&fixture);
    let path = fixture.workspace.join("calc.rs");
    let source = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path,format!("{source}\n#[test] fn additional_boundary_case() {{ assert_eq!(total(1,1,0),Ok(1)); }}\n")).unwrap();
    let assets =
        PathBuf::from(std::env::var_os("ASTER_TEST_OCI_ASSETS").expect("explicit guest assets"));
    let oracle = fixture.check_in_guest(&assets).await.unwrap();
    assert!(
        oracle.correct,
        "legitimate extra tests must not be rejected: {}",
        oracle.diagnostic
    );
}
