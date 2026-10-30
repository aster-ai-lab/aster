#![cfg(test)]

use super::*;

fn write_member(root: &Path, name: &str, body: &[u8]) -> Member {
    let path = root.join(name);
    std::fs::write(&path, body).unwrap();
    Member {
        path,
        sha256: format!("{:x}", Sha256::digest(body)),
        bytes: body.len() as u64,
    }
}

fn fixture() -> (tempfile::TempDir, Candidate, PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let candidate = Candidate {
        schema: "a3s.code.lh.candidate.v1".into(),
        build_id: "declared-pair".into(),
        aster: write_member(root.path(), "declared-aster", b"declared terminal bytes"),
        acp: write_member(root.path(), "declared-acp", b"declared ACP bytes"),
    };
    let path = root.path().join("candidate.json");
    std::fs::write(&path, serde_json::to_vec(&candidate).unwrap()).unwrap();
    (root, candidate, path)
}

#[test]
fn registered_pair_copies_only_the_declared_bytes_despite_other_candidates() {
    let (root, candidate, path) = fixture();
    write_member(
        root.path(),
        "a3s-code-acp",
        b"other build with the same source id",
    );
    let loaded = Candidate::load(&path, "declared-pair").unwrap();
    let output = root.path().join("frozen");
    std::fs::create_dir(&output).unwrap();
    loaded.copy_into(&output).unwrap();
    assert_eq!(
        std::fs::read(output.join("a3s-code-acp")).unwrap(),
        std::fs::read(candidate.acp.path).unwrap()
    );
    assert_eq!(
        std::fs::read(output.join("aster")).unwrap(),
        std::fs::read(candidate.aster.path).unwrap()
    );
}

#[test]
fn same_build_id_does_not_admit_changed_binary_bytes_or_a_partial_pair() {
    let (_root, candidate, path) = fixture();
    std::fs::write(&candidate.acp.path, b"different ACP bytes").unwrap();
    assert!(Candidate::load(&path, "declared-pair").is_err());
    std::fs::remove_file(&candidate.aster.path).unwrap();
    assert!(Candidate::load(&path, "declared-pair").is_err());
}

#[test]
fn a_member_changed_after_selection_cannot_be_frozen_as_approved() {
    let (root, candidate, path) = fixture();
    let loaded = Candidate::load(&path, "declared-pair").unwrap();
    let old = std::fs::read(&candidate.acp.path).unwrap();
    std::fs::write(&candidate.acp.path, vec![b'x'; old.len()]).unwrap();
    let output = root.path().join("changed-freeze");
    std::fs::create_dir(&output).unwrap();
    assert_eq!(
        loaded.copy_into(&output).unwrap_err(),
        "candidate_member_changed"
    );
}

#[test]
fn unknown_manifest_fields_stale_ids_and_oversized_records_fail_closed() {
    let (root, candidate, path) = fixture();
    assert_eq!(
        Candidate::load(&path, "other-pair").unwrap_err(),
        "candidate_build_identity"
    );
    let mut value = serde_json::to_value(candidate).unwrap();
    value["fallback"] = true.into();
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(
        Candidate::load(&path, "declared-pair").unwrap_err(),
        "candidate_manifest_schema"
    );
    std::fs::write(&path, vec![b' '; super::super::artifact::LIMIT + 1]).unwrap();
    assert_eq!(
        Candidate::load(&path, "declared-pair").unwrap_err(),
        "candidate_manifest_read"
    );
    assert_eq!(
        Candidate::load(Path::new("candidate.json"), "declared-pair").unwrap_err(),
        "candidate_manifest_path"
    );
    assert!(root.path().exists());
}

#[cfg(unix)]
#[test]
fn linked_or_nonregular_members_are_refused_without_reading_them() {
    let (root, mut candidate, _) = fixture();
    let link = root.path().join("alias");
    std::os::unix::fs::symlink(&candidate.acp.path, &link).unwrap();
    candidate.acp.path = link;
    assert!(candidate.acp.verify().is_err());
    candidate.acp.path = root.path().to_owned();
    assert!(candidate.acp.verify().is_err());
}
