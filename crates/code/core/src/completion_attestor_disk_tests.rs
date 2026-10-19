#![cfg(test)]
#![cfg(unix)]

use super::*;
use std::io::Write;

fn record(path: &str, content: &str) -> MutatedPathRecord {
    MutatedPathRecord {
        path: path.into(),
        content_digest: sha256::digest(content),
    }
}

#[test]
fn every_mutation_must_have_matching_content_evidence() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("one"), "one").unwrap();
    std::fs::write(root.path().join("two"), "two").unwrap();
    let attestor = DiskContentAttestor::new(root.path());
    let paths = [record("one", "one"), record("two", "two")];
    assert!(attestor.attest("effect", &paths).is_some());
    let missing = MutatedPathRecord {
        path: "two".into(),
        content_digest: String::new(),
    };
    assert!(
        attestor
            .attest("effect", &[paths[0].clone(), missing])
            .is_none()
    );
    std::fs::write(root.path().join("two"), "tampered after verification").unwrap();
    assert!(attestor.attest("effect", &paths).is_none());
    assert!(attestor.attest("", &paths).is_none());
}

#[test]
fn path_boundaries_reject_absolute_parent_symlink_and_special_files() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret"), "secret").unwrap();
    std::fs::write(root.path().join("safe..name"), "safe").unwrap();
    symlink(outside.path(), root.path().join("escape")).unwrap();
    symlink(root.path().join("safe..name"), root.path().join("link")).unwrap();
    let attestor = DiskContentAttestor::new(root.path());
    for path in [
        "../secret",
        "/etc/hosts",
        "escape/secret",
        "link",
        ".",
        "missing",
        "",
    ] {
        assert!(
            attestor
                .attest("effect", &[record(path, "secret")])
                .is_none(),
            "accepted {path}"
        );
    }
    assert!(
        attestor
            .attest("effect", &[record("./safe..name", "safe")])
            .is_some()
    );
}

#[test]
fn large_file_is_hashed_with_the_shared_streaming_observer() {
    use sha2::{Digest, Sha256};
    let root = tempfile::tempdir().unwrap();
    let mut file = std::fs::File::create(root.path().join("large")).unwrap();
    let buffer = [b'x'; 64 * 1024];
    let mut hash = Sha256::new();
    for _ in 0..1024 {
        file.write_all(&buffer).unwrap();
        hash.update(buffer);
    }
    drop(file);
    let paths = [MutatedPathRecord {
        path: "large".into(),
        content_digest: format!("{:x}", hash.finalize()),
    }];
    assert!(
        DiskContentAttestor::new(root.path())
            .attest("effect", &paths)
            .is_some()
    );
}
