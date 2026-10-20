#![cfg(test)]

use super::*;

#[test]
fn default_aster_profile_never_opens_the_legacy_work_directory() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let old = root.join("aster");
    fs::create_dir_all(&old).unwrap();
    let credentials = b"synthetic-old-work-credential-do-not-import";
    fs::write(old.join("member-session.json"), credentials).unwrap();
    fs::write(old.join("preferences.acl"), b"legacy-work-preferences").unwrap();
    let profile = profile_dir_under(root, "aster", None);
    assert_eq!(profile, root.join("studio/aster"));
    assert!(!profile.join("member-session.json").exists());
    fs::create_dir_all(&profile).unwrap();
    fs::write(profile.join("providers.json"), b"synthetic-new-studio").unwrap();
    assert_eq!(
        fs::read(old.join("member-session.json")).unwrap(),
        credentials
    );
    assert_eq!(
        fs::read(old.join("preferences.acl")).unwrap(),
        b"legacy-work-preferences"
    );
}

#[test]
fn default_aster_instance_ignores_old_work_flat_profiles() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir_all(temp.path().join("aster-side1")).unwrap();
    assert_eq!(
        profile_dir_under(temp.path(), "aster-side1", Some("side1")),
        temp.path().join("studio/aster/instances/side1")
    );
    assert_eq!(
        bundle_identifier("aster-side1", Some("side1"), true),
        "dev.a3s.studio.aster.side1"
    );
}

#[test]
fn default_aster_preserves_the_published_toyou_bundle_id() {
    assert_eq!(bundle_identifier("toyou", None, true), "dev.a3s.studio");
    assert_eq!(bundle_identifier("toyou", None, false), "dev.a3s.studio");
    assert_eq!(
        bundle_identifier("aster", None, true),
        "dev.a3s.studio.aster"
    );
}

#[test]
fn default_aster_identity_is_closed_and_the_compiled_product_is_authoritative() {
    for (slug, product, bundle_id) in [
        ("aster", "AsterDream", "dev.a3s.studio.aster"),
        ("toyou", "ToWork", "dev.a3s.studio"),
    ] {
        let identity = HostIdentity::parse(slug).unwrap();
        assert_eq!(identity.as_str(), slug);
        assert_eq!(identity.product(), product);
        assert_eq!(identity.bundle_id(), bundle_id);
    }
    assert_eq!(
        selected_development_slug(None).unwrap(),
        HostIdentity::compiled().as_str()
    );
    for slug in ["a3s", "kense", "towork-cua"] {
        assert!(selected_development_slug(Some(slug)).is_err());
        assert!(HostIdentity::parse(slug).is_err());
    }
    for slug in ["", " ", "Aster", "../toyou", "aster/child"] {
        assert!(selected_development_slug(Some(slug)).is_err());
        assert!(HostIdentity::parse(slug).is_err());
    }
}

#[test]
fn default_aster_platform_roots_skip_all_synthetic_legacy_work_state() {
    use super::profile::{ProfileEnv, ProfileOs, resolve_profile_location};
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    let legacy = home.join("Library/Application Support/dev.a3s.studio");
    fs::create_dir_all(legacy.join("aster")).unwrap();
    fs::create_dir_all(legacy.join("toyou")).unwrap();
    fs::write(legacy.join("computer-id"), "synthetic-old-machine").unwrap();
    let legacy_member = r#"{"token":"a3s_ms_synthetic-old-token","member_id":"11111111-1111-4111-8111-111111111111"}"#;
    fs::write(legacy.join("aster/member-session.json"), legacy_member).unwrap();
    fs::write(legacy.join("aster/preferences.acl"), "synthetic-old-prefs").unwrap();
    let appdata = temp.path().join("appdata");
    let xdg = temp.path().join("xdg");
    for (os, root) in [
        (ProfileOs::Macos, legacy.clone()),
        (ProfileOs::Linux, xdg.join("dev.a3s.studio")),
        (ProfileOs::Windows, appdata.join("dev.a3s.studio")),
    ] {
        let env = ProfileEnv {
            home: Some(home.clone()),
            user_profile: Some(home.clone()),
            appdata: Some(appdata.clone()),
            xdg_data_home: Some(xdg.clone()),
            os,
        };
        let primary = resolve_profile_location("aster", None, &env).unwrap();
        assert_eq!(primary.directory, root.join("studio/aster"));
        assert_eq!(primary.machine_root, primary.directory);
        let member = crate::member_session::MemberSessionStore::open_profile(primary, "aster");
        assert!(!member.has_session());
        assert_ne!(member.computer_id(), "synthetic-old-machine");
        let side = resolve_profile_location("aster-side1", Some("side1"), &env).unwrap();
        assert_eq!(side.directory, root.join("studio/aster/instances/side1"));
        assert_eq!(side.machine_root, root.join("studio/aster"));
        assert_eq!(
            resolve_profile_location("toyou", None, &env)
                .unwrap()
                .directory,
            legacy.join("toyou")
        );
    }
    assert_eq!(
        fs::read_to_string(legacy.join("aster/member-session.json")).unwrap(),
        legacy_member
    );
    assert_eq!(
        fs::read_to_string(legacy.join("aster/preferences.acl")).unwrap(),
        "synthetic-old-prefs"
    );
    assert_eq!(
        fs::read_to_string(legacy.join("computer-id")).unwrap(),
        "synthetic-old-machine"
    );
}

#[test]
fn default_aster_toyou_keeps_flat_and_nested_instance_compatibility() {
    use super::profile::{ProfileEnv, ProfileOs, resolve_profile_location};
    let temp = tempfile::tempdir().unwrap();
    let root = temp
        .path()
        .join("Library/Application Support/dev.a3s.studio");
    let env = ProfileEnv {
        home: Some(temp.path().into()),
        user_profile: None,
        appdata: None,
        xdg_data_home: None,
        os: ProfileOs::Macos,
    };
    let nested = root.join("toyou/instances/side1");
    fs::create_dir_all(&nested).unwrap();
    assert_eq!(
        resolve_profile_location("toyou-side1", Some("side1"), &env)
            .unwrap()
            .directory,
        nested
    );
    assert_eq!(
        bundle_identifier("toyou-side1", Some("side1"), false),
        "dev.a3s.studio.toyou.side1"
    );
    let flat = root.join("toyou-side1");
    fs::create_dir_all(&flat).unwrap();
    fs::write(flat.join("member-session.json"), "synthetic-toyou-side").unwrap();
    assert_eq!(
        resolve_profile_location("toyou-side1", Some("side1"), &env)
            .unwrap()
            .directory,
        flat
    );
    assert_eq!(
        bundle_identifier("toyou-side1", Some("side1"), true),
        "dev.a3s.studio.toyou-side1"
    );
    assert_eq!(
        fs::read_to_string(flat.join("member-session.json")).unwrap(),
        "synthetic-toyou-side"
    );
}

#[test]
fn default_aster_runtime_and_browser_scopes_cannot_overlap() {
    let root = std::path::Path::new("/synthetic-temp");
    assert_eq!(
        runtime_path_under(root, "aster", None).unwrap(),
        root.join("a3s-studio/aster/runtime.json")
    );
    assert_eq!(
        runtime_path_under(root, "toyou", Some("side1")).unwrap(),
        root.join("a3s-studio/toyou/instances/side1/runtime.json")
    );
    assert_ne!(
        runtime_path_under(root, "toyou", None).unwrap(),
        runtime_path_under(root, "aster", None).unwrap()
    );
    assert!(runtime_path_under(root, "../aster", None).is_err());
    assert!(runtime_path_under(root, "aster", Some("../side1")).is_err());
    use super::webview::isolated_store_id;
    assert!(isolated_store_id("toyou", None, [0; 16]).is_none());
    let aster = isolated_store_id("aster", None, [0; 16]).unwrap();
    assert_ne!(
        aster,
        isolated_store_id("aster-side1", Some("side1"), [0; 16]).unwrap()
    );
    assert_ne!(
        aster,
        isolated_store_id("toyou-side1", Some("side1"), [0; 16]).unwrap()
    );
    assert_ne!(aster, isolated_store_id("aster", None, [1; 16]).unwrap());
}

#[test]
fn default_aster_installed_identity_cannot_be_changed_by_launch_environment() {
    for compiled in [HostIdentity::Aster, HostIdentity::ToYou] {
        let slug = compiled.as_str();
        let other = if compiled == HostIdentity::Aster {
            "toyou"
        } else {
            "aster"
        };
        assert_eq!(
            selected_slug_for_identity(None, Some(slug), compiled).unwrap(),
            slug
        );
        assert_eq!(
            selected_slug_for_identity(Some(slug), Some(slug), compiled).unwrap(),
            slug
        );
        assert!(selected_slug_for_identity(Some(other), Some(slug), compiled).is_err());
        assert!(selected_slug_for_identity(None, Some(other), compiled).is_err());
        assert!(selected_slug_for_identity(Some(other), None, compiled).is_err());
        assert!(selected_slug_for_identity(Some(""), Some(slug), compiled).is_err());
        assert_eq!(
            selected_slug_for_identity(None, None, compiled).unwrap(),
            slug
        );
        assert!(selected_slug_for_identity(Some(""), None, compiled).is_err());
    }
    assert_eq!(
        load_slug("aster").unwrap().product,
        HostIdentity::Aster.product()
    );
    assert_eq!(
        load_slug("toyou").unwrap().product,
        HostIdentity::ToYou.product()
    );
}

#[test]
fn default_aster_platform_fallbacks_and_long_instance_ids_stay_isolated() {
    use super::profile::{ProfileEnv, ProfileOs, resolve_profile_location};
    let temp = tempfile::tempdir().unwrap();
    let mut env = ProfileEnv {
        home: Some(temp.path().into()),
        user_profile: None,
        appdata: None,
        xdg_data_home: None,
        os: ProfileOs::Linux,
    };
    assert_eq!(
        resolve_profile_location("aster", None, &env)
            .unwrap()
            .directory,
        temp.path().join(".local/share/dev.a3s.studio/studio/aster")
    );
    env.os = ProfileOs::Windows;
    env.home = None;
    assert!(resolve_profile_location("aster", None, &env).is_none());
    env.user_profile = Some(temp.path().into());
    assert_eq!(
        resolve_profile_location("aster", None, &env)
            .unwrap()
            .directory,
        temp.path()
            .join("AppData/Roaming/dev.a3s.studio/studio/aster")
    );
    assert!(resolve_profile_location("../aster", None, &env).is_none());
    assert!(resolve_profile_location("aster-side", Some("../side"), &env).is_none());
    let brand = "a".repeat(31);
    let instance = "b".repeat(32);
    let storage = storage_slug_for(&brand, Some(&instance)).unwrap();
    assert_eq!(storage.len(), 64);
    assert!(
        resolve_profile_location(&storage, Some(&instance), &env)
            .unwrap()
            .directory
            .ends_with(format!("{brand}/instances/{instance}"))
    );
}
