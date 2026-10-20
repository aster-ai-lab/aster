#![cfg(test)]

use super::*;

#[test]
fn only_the_explicit_debug_relaunch_can_use_checkout_resources() {
    assert!(is_development_relaunch(true, Some("1")));
    for flag in [None, Some(""), Some("true"), Some(" 1 ")] {
        assert!(!is_development_relaunch(true, flag));
    }
    assert!(!is_development_relaunch(false, Some("1")));
}

fn fixture(root: &Path) -> Value {
    let mut files = serde_json::Map::new();
    for name in REQUIRED.iter().copied().chain([
        "Contents/Resources/tenant/toyou/INFO.md",
        "Contents/Resources/tenant/toyou/logo-light.png",
        "Contents/Resources/tenant/toyou/mark-light.png",
        "Contents/Resources/tenant/toyou/icon.png",
    ]) {
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let bytes = if name.ends_with("/INFO.md") {
            b"---\nname:\n  en: Synthetic Tenant\nproduct: ToWork\nlogos:\n  light_surface: logo-light.png\n---\n".as_slice()
        } else {
            name.as_bytes()
        };
        fs::write(path, bytes).unwrap();
        files.insert(
            name.into(),
            json!({"size": bytes.len(), "sha256": hex_encode(Sha256::digest(bytes))}),
        );
    }
    fs::create_dir_all(root.join("Contents/MacOS")).unwrap();
    fs::write(root.join(HOST), b"host sealed by final codesign").unwrap();
    json!({"schema":1,"product":"a3s-studio","version":"2.1.4","tenant":"toyou","files":files})
}

fn write_marker(root: &Path, marker: &Value) {
    fs::write(root.join(MARKER), serde_json::to_vec(marker).unwrap()).unwrap();
}

#[test]
fn missing_native_brand_images_refuse_even_when_the_remaining_files_are_sealed() {
    for image in ["logo-light.png", "mark-light.png", "icon.png"] {
        let temp = tempfile::tempdir().unwrap();
        let mut marker = fixture(temp.path());
        let relative = format!("Contents/Resources/tenant/toyou/{image}");
        marker["files"].as_object_mut().unwrap().remove(&relative);
        let _ = fs::remove_file(temp.path().join(relative));
        write_marker(temp.path(), &marker);
        assert!(validate(temp.path(), "2.1.4", "toyou").is_err(), "{image}");
    }
}

#[test]
fn complete_generation_rejects_changed_or_missing_companions() {
    let temp = tempfile::tempdir().unwrap();
    let marker = fixture(temp.path());
    write_marker(temp.path(), &marker);
    validate(temp.path(), "2.1.4", "toyou").unwrap();
    let office = temp.path().join("Contents/Resources/agent/a3s-office");
    fs::write(&office, b"old office").unwrap();
    assert!(matches!(
        validate(temp.path(), "2.1.4", "toyou"),
        Err(UpdateFail::IntegrityFailed)
    ));
    fs::remove_file(office).unwrap();
    assert!(matches!(
        validate(temp.path(), "2.1.4", "toyou"),
        Err(UpdateFail::InvalidPackage)
    ));
}

#[test]
fn product_tenant_and_release_version_are_exact_even_with_a_shared_bundle_id() {
    let temp = tempfile::tempdir().unwrap();
    let mut marker = fixture(temp.path());
    write_marker(temp.path(), &marker);
    assert!(matches!(
        validate(temp.path(), "2.1.3", "toyou"),
        Err(UpdateFail::ProductMismatch)
    ));
    assert!(matches!(
        validate(temp.path(), "2.1.4", "towork-cua"),
        Err(UpdateFail::ProductMismatch)
    ));
    marker["product"] = json!("aster-dream");
    write_marker(temp.path(), &marker);
    assert!(matches!(
        validate(temp.path(), "2.1.4", "toyou"),
        Err(UpdateFail::ProductMismatch)
    ));
}

#[test]
fn missing_declaration_and_resource_escape_refuse_activation() {
    let temp = tempfile::tempdir().unwrap();
    let mut marker = fixture(temp.path());
    marker["files"].as_object_mut().unwrap().remove(REQUIRED[1]);
    write_marker(temp.path(), &marker);
    assert!(validate(temp.path(), "2.1.4", "toyou").is_err());
    let mut marker = fixture(temp.path());
    marker["files"]["Contents/../../outside"] = json!({"size":1,"sha256":"ab".repeat(32)});
    write_marker(temp.path(), &marker);
    assert!(validate(temp.path(), "2.1.4", "toyou").is_err());
    #[cfg(unix)]
    {
        let marker = fixture(temp.path());
        write_marker(temp.path(), &marker);
        let path = temp.path().join(REQUIRED[1]);
        fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(temp.path().join(HOST), &path).unwrap();
        assert!(validate(temp.path(), "2.1.4", "toyou").is_err());
    }
}
