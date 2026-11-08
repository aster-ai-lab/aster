#![cfg(test)]

use std::fs;
use std::io::{Cursor, Write};

use crate::{
    payload_sha256, PackageFile, PackageSpec, Payload, Publication, ReconcileError, ReconcileStore,
    SurfaceKind, SurfaceSpec,
};

fn published(zip: bool) -> (tempfile::TempDir, ReconcileStore, Publication) {
    let root = tempfile::tempdir().unwrap();
    let store = ReconcileStore::open(root.path()).unwrap();
    let files = vec![
        PackageFile {
            path: "ui/index.html".into(),
            bytes: b"<html>owned</html>".to_vec(),
        },
        PackageFile {
            path: "ui/view.js".into(),
            bytes: b"document.body.textContent = 'verified'".to_vec(),
        },
    ];
    let payload = if zip {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for file in files {
            writer
                .start_file(file.path, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(&file.bytes).unwrap();
        }
        Payload::Zip(writer.finish().unwrap().into_inner())
    } else {
        Payload::Files(files)
    };
    let publication = store
        .apply_sync(&PackageSpec {
            package_id: "proof/view".into(),
            surfaces: vec![SurfaceSpec {
                kind: SurfaceKind::Ui,
                id: "view".into(),
                sha256: payload_sha256(&payload).unwrap(),
                entry: Some("ui/index.html".into()),
                payload,
            }],
        })
        .unwrap();
    (root, store, publication)
}

// Catch callers receiving filesystem bytes independently of the verified payload.
#[test]
fn files_and_zip_reads_return_owned_verified_bytes_and_refuse_later_tampering() {
    for zip in [false, true] {
        let (_root, _store, publication) = published(zip);
        let surface = &publication.surfaces[0];
        let body = surface
            .read_verified_file("ui/index.html")
            .unwrap()
            .unwrap();
        fs::write(surface.directory.join("ui/index.html"), b"replaced").unwrap();
        assert_eq!(body, b"<html>owned</html>");
        assert!(matches!(
            surface.read_verified_file("ui/index.html"),
            Err(ReconcileError::DigestMismatch)
        ));
    }
}

// A healthy requested file must not conceal damage elsewhere in its generation.
#[test]
fn verified_read_checks_sibling_bytes_and_injected_files() {
    let (_root, _store, publication) = published(false);
    let surface = &publication.surfaces[0];
    let script = surface.directory.join("ui/view.js");
    let original = fs::read(&script).unwrap();
    fs::write(&script, b"changed").unwrap();
    assert!(matches!(
        surface.read_verified_file("ui/index.html"),
        Err(ReconcileError::DigestMismatch)
    ));
    fs::write(script, original).unwrap();
    fs::write(surface.directory.join("unpublished"), b"injected").unwrap();
    assert!(matches!(
        surface.read_verified_file("ui/index.html"),
        Err(ReconcileError::DigestMismatch)
    ));
}

// Catch a whole-surface read that skips the digest a single-file read enforces.
#[test]
fn reading_every_file_verifies_the_surface_once_and_refuses_tampering() {
    for zip in [false, true] {
        let (_root, _store, publication) = published(zip);
        let surface = &publication.surfaces[0];
        let files = surface.read_verified_files().unwrap();
        let index = files
            .iter()
            .find(|file| file.path == "ui/index.html")
            .unwrap();
        assert_eq!(index.bytes, b"<html>owned</html>");
        fs::write(surface.directory.join("ui/index.html"), b"replaced").unwrap();
        assert!(matches!(
            surface.read_verified_files(),
            Err(ReconcileError::DigestMismatch)
        ));
    }
}

#[test]
fn absent_file_is_optional_only_in_a_valid_generation_and_paths_cannot_escape() {
    let (_root, _store, publication) = published(false);
    let surface = &publication.surfaces[0];
    assert_eq!(surface.read_verified_file("missing.acl").unwrap(), None);
    for path in [
        "",
        "/outside",
        "../outside",
        "ui/../index.html",
        "ui//index.html",
        "ui\\index.html",
    ] {
        assert!(matches!(
            surface.read_verified_file(path),
            Err(ReconcileError::InvalidPath(_))
        ));
    }
    fs::write(surface.directory.join("ui/view.js"), b"changed").unwrap();
    assert!(matches!(
        surface.read_verified_file("missing.acl"),
        Err(ReconcileError::DigestMismatch)
    ));
}

#[test]
fn retained_verified_reads_do_not_reopen_withdrawn_admission() {
    let (_root, store, publication) = published(false);
    let lease = store.acquire_sync(&publication.cursor()).unwrap();
    store.withdraw_sync("proof/view").unwrap();
    assert!(matches!(
        store.acquire_sync(&publication.cursor()),
        Err(ReconcileError::StalePublication)
    ));
    assert_eq!(
        lease.publication().surfaces[0]
            .read_verified_file("ui/index.html")
            .unwrap()
            .unwrap(),
        b"<html>owned</html>"
    );
    lease.release_sync().unwrap();
    assert!(!publication.surfaces[0].directory.exists());
    assert!(publication.surfaces[0]
        .read_verified_file("ui/index.html")
        .is_err());
}

#[test]
fn verified_read_refuses_a_forged_mcp_projection() {
    let root = tempfile::tempdir().unwrap();
    let store = ReconcileStore::open(root.path()).unwrap();
    let payload = Payload::Files(vec![PackageFile {
        path: "server.json".into(),
        bytes: br#"{"type":"stdio","command":"safe"}"#.to_vec(),
    }]);
    let mut publication = store
        .apply_sync(&PackageSpec {
            package_id: "proof/mcp".into(),
            surfaces: vec![SurfaceSpec {
                kind: SurfaceKind::Mcp,
                id: "server".into(),
                sha256: payload_sha256(&payload).unwrap(),
                entry: None,
                payload,
            }],
        })
        .unwrap();
    let surface = &mut publication.surfaces[0];
    assert_eq!(
        surface.read_verified_file("server.json").unwrap().unwrap(),
        br#"{"type":"stdio","command":"safe"}"#
    );
    surface.mcp = Some(serde_json::json!({"type":"stdio","command":"unadmitted"}));
    assert!(matches!(
        surface.read_verified_file("server.json"),
        Err(ReconcileError::McpInvalid)
    ));
}

#[cfg(unix)]
#[test]
fn verified_read_refuses_linked_siblings_even_for_a_regular_requested_file() {
    let (root, _store, publication) = published(false);
    let surface = &publication.surfaces[0];
    let outside = root.path().join("outside");
    fs::write(&outside, b"document.body.textContent = 'verified'").unwrap();
    let script = surface.directory.join("ui/view.js");
    fs::remove_file(&script).unwrap();
    std::os::unix::fs::symlink(outside, script).unwrap();
    assert!(matches!(
        surface.read_verified_file("ui/index.html"),
        Err(ReconcileError::LinkRefused { .. })
    ));
}
