#![cfg(test)]

use super::*;

#[test]
fn public_handoff_evidence_is_bounded_and_omits_private_model_fields() {
    let root = tempfile::tempdir().unwrap();
    let fact = |seq, kind: &str, payload| aster_effect::Fact {
        seq,
        kind: kind.into(),
        key: String::new(),
        cause: None,
        payload,
    };
    let private = "SYNTHETIC-PRIVATE-CONTINUATION";
    let facts = [
        fact(1, "model.turn", serde_json::json!({"reasoning":private})),
        fact(
            2,
            "compaction.done",
            serde_json::json!({"summary":"Exact public state Ω", "kept":["tool\npublic observation"],"reasoning":private}),
        ),
        fact(
            3,
            "compaction.done",
            serde_json::json!({"summary":"z".repeat(3*1024*1024),"kept":[]}),
        ),
        fact(
            4,
            "compaction.done",
            serde_json::json!({"summary":"Later public state", "kept":[]}),
        ),
    ];
    write_public_compaction_evidence(root.path(), &facts).unwrap();
    let bytes = std::fs::read(root.path().join("public-compaction-handoffs.json")).unwrap();
    assert!(bytes.len() <= 2 * 1024 * 1024);
    assert!(!String::from_utf8_lossy(&bytes).contains(private));
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["omitted_entries"], 1);
    assert_eq!(value["entries"].as_array().unwrap().len(), 2);
    assert_eq!(value["entries"][0]["seq"], 2);
    assert_eq!(value["entries"][0]["summary"], "Exact public state Ω");
    assert_eq!(value["entries"][0]["kept"][0], "tool\npublic observation");
    assert_eq!(value["entries"][1]["seq"], 4);
}

#[test]
fn bounded_artifacts_refuse_directories_oversize_and_traversal() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("valid"), b"current bytes").unwrap();
    assert_eq!(read(root.path(), "valid").unwrap(), b"current bytes");
    std::fs::create_dir(root.path().join("directory")).unwrap();
    std::fs::write(root.path().join("oversize"), vec![0; LIMIT + 1]).unwrap();
    for name in [
        "directory",
        "oversize",
        "../outside",
        "/absolute",
        "nested/file",
        "missing",
    ] {
        assert!(read(root.path(), name).is_err(), "{name}");
    }
}

#[cfg(unix)]
#[test]
fn fixture_links_and_nonregular_files_are_refused_before_reading() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("original"), b"protected").unwrap();
    std::os::unix::fs::symlink("original", root.path().join("symbolic")).unwrap();
    std::fs::hard_link(root.path().join("original"), root.path().join("hard")).unwrap();
    for name in ["symbolic", "hard", "original"] {
        assert!(read(root.path(), name).is_err(), "{name}");
    }
    let _socket = std::os::unix::net::UnixListener::bind(root.path().join("socket")).unwrap();
    assert!(read(root.path(), "socket").is_err());
}
