#![cfg(test)]

use super::*;

#[test]
fn linux_bundle_reads_its_explicit_tenant_without_checkout_or_process_environment() {
    let temp = tempfile::tempdir().unwrap();
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    let resources = temp.path().join("Resources");
    fs::create_dir_all(&resources).unwrap();
    let marker = resources.join("studio-tenant");
    fs::write(&marker, "towork-cua\n").unwrap();
    assert_eq!(
        packaged_tenant_from(&bin.join("Studio"))
            .unwrap()
            .as_deref(),
        Some("towork-cua")
    );
    for invalid in ["../toyou\n", "towork-cua\nextra", "", " ToYou "] {
        fs::write(&marker, invalid).unwrap();
        assert!(packaged_tenant_from(&bin.join("Studio")).is_err());
    }
}
