//! DOC-DIET1: every top-level `manual/` document is listed exactly once in
//! `manual/README.md`, under one of its three kinds, and every index link
//! resolves. Evaluation reports live in `docs/evidence/evaluations/`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const KINDS: [&str; 3] = ["Guide", "Reference", "Release"];

fn manual_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../manual")
}

/// `(kind, link target)` for each `- [title](target)` line of the index.
fn index_entries(index: &str) -> Result<Vec<(String, String)>, String> {
    let mut kind: Option<String> = None;
    let mut headings = Vec::new();
    let mut entries = Vec::new();
    for line in index.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            headings.push(heading.to_string());
            kind = Some(heading.to_string());
            continue;
        }
        let Some(item) = line.strip_prefix("- [") else {
            continue;
        };
        let target = item
            .split_once("](")
            .and_then(|(_, rest)| rest.strip_suffix(')'))
            .ok_or_else(|| format!("malformed index line: {line}"))?;
        let kind = kind
            .clone()
            .ok_or_else(|| format!("entry before any kind heading: {line}"))?;
        entries.push((kind, target.to_string()));
    }
    if headings != KINDS {
        return Err(format!("index kinds are {headings:?}, expected {KINDS:?}"));
    }
    Ok(entries)
}

#[test]
fn every_manual_document_is_indexed_once_under_one_kind() {
    let dir = manual_dir();
    let index = std::fs::read_to_string(dir.join("README.md")).expect("manual/README.md");
    let entries = index_entries(&index).unwrap_or_else(|error| panic!("{error}"));
    let mut problems = Vec::new();
    let mut listed: BTreeMap<&str, usize> = BTreeMap::new();
    for (_, target) in &entries {
        if !dir.join(target).is_file() {
            problems.push(format!("index links a missing file: {target}"));
        }
        *listed.entry(target.as_str()).or_default() += 1;
    }
    let mut documents: Vec<String> = std::fs::read_dir(&dir)
        .expect("manual dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".md") && name != "README.md")
        .collect();
    documents.sort();
    for name in &documents {
        match listed.get(name.as_str()) {
            None => problems.push(format!("{name} is not in manual/README.md")),
            Some(1) => {}
            Some(count) => problems.push(format!("{name} is listed {count} times")),
        }
    }
    assert!(
        problems.is_empty(),
        "manual index ({} documents):\n{}",
        documents.len(),
        problems.join("\n")
    );
}

#[test]
fn current_manuals_only_document_the_local_rust_sdk() {
    let dir = manual_dir();
    let retired = [
        "@a3s-lab/code",
        "from a3s_code import",
        "github.com/A3S-Lab/Code/sdk/go",
        "sdk/node/",
        "sdk/python/",
        "sdk/go/",
        "sdk_api_alignment_check.mjs",
    ];
    let mut problems = Vec::new();
    let mut directories = vec![dir.clone()];
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(directory).expect("manual directory") {
            let entry = entry.expect("manual entry");
            let path = entry.path();
            if entry.file_type().expect("manual file type").is_dir() {
                directories.push(path);
                continue;
            }
            if path.extension().and_then(|value| value.to_str()) != Some("md") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("manual text");
            for instruction in retired {
                if text.contains(instruction) {
                    problems.push(format!("{}: {instruction}", path.display()));
                }
            }
        }
    }
    assert!(!dir.join("NPM_TRUSTED_PUBLISHING.md").exists());
    for retired in [
        "sdk/node",
        "sdk/python",
        "sdk/go",
        "bindings/node",
        "bindings/python",
    ] {
        assert!(
            !dir.join("..").join(retired).exists(),
            "retired SDK: {retired}"
        );
    }
    assert!(
        problems.is_empty(),
        "retired SDK instructions:\n{}",
        problems.join("\n")
    );
}

#[test]
fn the_index_rejects_an_unknown_kind_and_an_unlisted_entry_shape() {
    assert!(index_entries("## Guide\n## Reference\n## Notes\n").is_err());
    assert!(index_entries("- [a](a.md)\n## Guide\n## Reference\n## Release\n").is_err());
    assert!(index_entries("## Guide\n- [a](a.md\n## Reference\n## Release\n").is_err());
    let entries = index_entries("## Guide\n- [a](a.md)\n## Reference\n## Release\n- [b](b.md)\n")
        .expect("valid index");
    assert_eq!(
        entries,
        [
            ("Guide".to_string(), "a.md".to_string()),
            ("Release".to_string(), "b.md".to_string())
        ]
    );
}
