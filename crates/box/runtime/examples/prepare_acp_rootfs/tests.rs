#![cfg(test)]

use super::*;

#[test]
fn qualification_refuses_mutable_tags_and_invalid_digests() {
    assert!(pinned_image(IMAGE).is_ok());
    assert!(pinned_image("docker.io/library/python:3.13-bookworm").is_err());
    assert!(pinned_image("docker.io/library/python@sha256:abc").is_err());
    assert!(pinned_image("docker.io/library/python@sha512:abc").is_err());
}

#[test]
fn output_must_be_explicit_and_cannot_replace_an_existing_directory() {
    assert!(new_output(Path::new("relative-rootfs")).is_err());
    let owned = tempfile::tempdir().unwrap();
    let sentinel = owned.path().join("sentinel");
    std::fs::write(&sentinel, "preserve").unwrap();
    assert!(new_output(owned.path()).is_err());
    assert_eq!(std::fs::read_to_string(sentinel).unwrap(), "preserve");
    let next = owned.path().join("new-rootfs");
    new_output(&next).unwrap();
    assert!(next.is_dir());
}
