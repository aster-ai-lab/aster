#![cfg(test)]

use super::*;

struct Home(PathBuf);
impl Drop for Home {
    fn drop(&mut self) {
        let _ = release_catalog_review(&self.0.join("agents"));
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn review_spec_bytes(html: &[u8]) -> PackageSpec {
    let skill = vec![PackageFile {
        path: "SKILL.md".into(),
        bytes: b"# Review".to_vec(),
    }];
    let ui = vec![PackageFile {
        path: "ui/picker/index.html".into(),
        bytes: html.to_vec(),
    }];
    PackageSpec {
        package_id: PACKAGE_ID.into(),
        surfaces: vec![
            SurfaceSpec {
                kind: SurfaceKind::Skill,
                id: SKILL_ID.into(),
                sha256: files_sha256(&skill).unwrap(),
                entry: None,
                payload: Payload::Files(skill),
            },
            SurfaceSpec {
                kind: SurfaceKind::Ui,
                id: PICKER_ID.into(),
                sha256: files_sha256(&ui).unwrap(),
                entry: Some("ui/picker/index.html".into()),
                payload: Payload::Files(ui),
            },
        ],
    }
}

fn installed() -> (Home, ReconcileStore, PathBuf) {
    let root = std::env::temp_dir().join(format!("studio-review-leases-{}", uuid::Uuid::now_v7()));
    let agents = root.join("agents");
    let skill = root.join("unpacked/skills/review");
    fs::create_dir_all(&agents).unwrap();
    fs::create_dir_all(&skill).unwrap();
    fs::write(skill.join("SKILL.md"), b"# Review").unwrap();
    let store = ReconcileStore::open(root.join("package-generations")).unwrap();
    (Home(root), store, skill)
}

#[test]
fn cached_review_assets_retain_their_generation_through_replacement_and_retire_on_handoff() {
    let _gate = lock_review_tests();
    let (home, store, skill) = installed();
    let agents = home.0.join("agents");
    let one = store
        .apply_sync(&review_spec_bytes(b"<p>old picker</p>"))
        .unwrap();
    let old_entry = one
        .surfaces
        .iter()
        .find(|surface| surface.kind == SurfaceKind::Ui)
        .unwrap()
        .entry
        .clone()
        .unwrap();
    adopt_catalog_review(&agents, &skill).unwrap();
    let published_skill = one
        .surfaces
        .iter()
        .find(|surface| surface.kind == SurfaceKind::Skill)
        .unwrap();
    assert_eq!(
        fs::canonicalize(agents.join("review")).unwrap(),
        fs::canonicalize(&published_skill.directory).unwrap(),
        "directory discovery must use the exact projected generation"
    );
    assert_eq!(picker_http_path("").3, b"<p>old picker</p>");
    let two = store
        .apply_sync(&review_spec_bytes(b"<p>new picker</p>"))
        .unwrap();
    assert_eq!(
        store
            .collect_retired_sync(PACKAGE_ID)
            .unwrap()
            .retained_generations,
        vec![one.generation]
    );
    assert_eq!(cached_picker_http().3, b"<p>old picker</p>");
    assert!(old_entry.is_file());
    adopt_catalog_review(&agents, &skill).unwrap();
    assert_eq!(picker_http_path("").3, b"<p>new picker</p>");
    assert!(!old_entry.exists());
    let new_entry = two
        .surfaces
        .iter()
        .find(|surface| surface.kind == SurfaceKind::Ui)
        .unwrap()
        .entry
        .clone()
        .unwrap();
    release_catalog_review(&agents).unwrap();
    assert_eq!(cached_picker_http().0, 404);
    assert!(store.current_sync(PACKAGE_ID).unwrap().is_none());
    assert!(!new_entry.exists());
}

#[test]
fn corrupted_published_assets_cannot_enter_the_review_cache() {
    let _gate = lock_review_tests();
    let (home, store, skill) = installed();
    let agents = home.0.join("agents");
    let current = store
        .apply_sync(&review_spec_bytes(b"<p>verified</p>"))
        .unwrap();
    let entry = current
        .surfaces
        .iter()
        .find(|surface| surface.kind == SurfaceKind::Ui)
        .unwrap()
        .entry
        .as_ref()
        .unwrap();
    fs::write(entry, b"<p>changed</p>").unwrap();
    let error = adopt_catalog_review(&agents, &skill).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(!agents.join("review").exists());
}
