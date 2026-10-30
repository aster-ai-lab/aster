#![cfg(test)]

use super::*;

#[test]
fn each_current_revision_requires_its_own_correct_artifact() {
    let root = tempfile::tempdir().unwrap();
    let fixture = NativeFixture::create(root.path()).unwrap();
    let golden = [
        (547, 0, 41, 588, "USD"),
        (922, 0, 69, 991, "USD"),
        (952, 0, 0, 952, "USD"),
        (952, 48, 68, 972, "USD"),
        (952, 48, 68, 972, "EUR"),
        (952, 48, 68, 972, "EUR"),
    ];
    for revision in 1..=6 {
        let expected = fixture.prepare(revision).unwrap();
        let (subtotal, discount, tax, total, currency) = golden[(revision - 1) as usize];
        assert_eq!(
            (
                expected["subtotal_cents"].as_u64().unwrap(),
                expected["discount_cents"].as_u64().unwrap(),
                expected["tax_cents"].as_u64().unwrap(),
                expected["total_cents"].as_u64().unwrap(),
                expected["currency"].as_str().unwrap()
            ),
            (subtotal, discount, tax, total, currency)
        );
        assert!(!fixture.check(&expected));
        std::fs::write(fixture.workspace.join("report.json"), expected.to_string()).unwrap();
        assert!(fixture.check(&expected));
        let mut wrong = expected.clone();
        wrong["total_cents"] = 0.into();
        std::fs::write(fixture.workspace.join("report.json"), wrong.to_string()).unwrap();
        assert!(!fixture.check(&expected));
    }
}

#[test]
fn missing_oversized_external_and_tampered_artifacts_are_refused() {
    let root = tempfile::tempdir().unwrap();
    let fixture = NativeFixture::create(root.path()).unwrap();
    let expected = fixture.prepare(1).unwrap();
    assert!(!fixture.check(&expected));
    std::fs::write(
        fixture.workspace.join("report.json"),
        vec![b' '; 128 * 1024 + 1],
    )
    .unwrap();
    assert!(fixture.read("report.json").is_err());
    assert!(fixture.read("../outside").is_err());
    #[cfg(unix)]
    {
        let external = root.path().join("outside");
        std::fs::write(&external, "private-fixture-canary").unwrap();
        std::os::unix::fs::symlink(&external, fixture.workspace.join("escape")).unwrap();
        assert!(fixture.read("escape").is_err());
    }
    std::fs::write(fixture.workspace.join("report.json"), expected.to_string()).unwrap();
    std::fs::write(fixture.workspace.join("notes.txt"), "changed").unwrap();
    assert!(!fixture.check(&expected));
}
