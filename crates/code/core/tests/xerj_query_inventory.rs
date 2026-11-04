//! Structural coverage of the frozen reference; this is not query execution qualification.

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Component, Path};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Inventory {
    schema: String,
    reference: Reference,
    qualification: String,
    owner: String,
    owner_paths: Vec<String>,
    adapters: BTreeMap<String, Vec<String>>,
    variants: Vec<Variant>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reference {
    commit: String,
    path: String,
    sha256: String,
    enum_name: String,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Variant {
    name: String,
    fields: Vec<String>,
}

#[test]
fn fixed_query_ast_has_exact_ordered_owner_coverage() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let inventory: Inventory = serde_json::from_slice(&read_bounded(
        &root.join("specs/033-xerj-replacement-contracts/contracts/query-owners.json"),
        128 * 1024,
    ))
    .unwrap();
    assert_eq!(inventory.schema, "aster.xerj.query-owners.v1");
    assert_eq!(
        inventory.reference.commit,
        "682d7ce072c644239cfa13ca1a9ca9a0e45d794b"
    );
    assert_eq!(
        inventory.reference.path,
        "engine/crates/xerj-query/src/ast.rs"
    );
    assert_eq!(inventory.reference.enum_name, "QueryNode");
    assert_eq!(
        inventory.reference.sha256,
        "8426eabd8b4a28901baa0d9f44ee9751cfe4288cd17adbd64002a4febd123433"
    );
    assert_eq!(inventory.qualification, "NOT_RUN");
    assert_eq!(inventory.owner, "aster-vec");
    assert!(!inventory.owner_paths.is_empty());
    for path in inventory
        .owner_paths
        .iter()
        .chain(inventory.adapters.values().flatten())
    {
        assert!(Path::new(path)
            .components()
            .all(|part| matches!(part, Component::Normal(_))));
        assert!(root.join(path).is_file(), "missing candidate owner: {path}");
    }
    for name in inventory.adapters.keys() {
        assert!(inventory
            .variants
            .iter()
            .any(|variant| &variant.name == name));
    }
    let source = read_bounded(
        &root
            .join("docs/evidence/code/2026-11-01-xerj-replacement/references")
            .join(&inventory.reference.path),
        1024 * 1024,
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(&source)),
        inventory.reference.sha256
    );
    let actual = query_variants(std::str::from_utf8(&source).unwrap()).unwrap();
    assert_eq!(actual.len(), 44);
    assert_eq!(actual, inventory.variants);
}

#[test]
fn syntax_reader_refuses_missing_duplicate_unnamed_and_invalid_declarations() {
    for source in [
        "enum Other { MatchAll }",
        "enum QueryNode { A } enum QueryNode { B }",
        "enum QueryNode { Unsupported(String) }",
        "enum QueryNode {",
    ] {
        assert!(
            query_variants(source).is_err(),
            "admitted unsupported source: {source}"
        );
    }
    let variants =
        query_variants("enum QueryNode { MatchAll, Term { field: String, value: String } }")
            .unwrap();
    assert_eq!(
        variants,
        vec![
            Variant {
                name: "MatchAll".into(),
                fields: vec![]
            },
            Variant {
                name: "Term".into(),
                fields: vec!["field".into(), "value".into()]
            },
        ]
    );
}

fn read_bounded(path: &Path, maximum: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .unwrap()
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .unwrap();
    assert!(
        bytes.len() <= maximum,
        "reference exceeds its bound: {}",
        path.display()
    );
    bytes
}

fn query_variants(source: &str) -> syn::Result<Vec<Variant>> {
    let file = syn::parse_file(source)?;
    let mut declarations = file.items.iter().filter_map(|item| match item {
        syn::Item::Enum(item) if item.ident == "QueryNode" => Some(item),
        _ => None,
    });
    let declaration = declarations
        .next()
        .ok_or_else(|| syn::Error::new_spanned(&file, "missing QueryNode"))?;
    if declarations.next().is_some() {
        return Err(syn::Error::new_spanned(declaration, "duplicate QueryNode"));
    }
    declaration
        .variants
        .iter()
        .map(|variant| {
            let fields = match &variant.fields {
                syn::Fields::Unit => Vec::new(),
                syn::Fields::Named(fields) => fields
                    .named
                    .iter()
                    .map(|field| {
                        // A named field's identifier is guaranteed by syn's grammar.
                        field.ident.as_ref().unwrap().to_string()
                    })
                    .collect(),
                syn::Fields::Unnamed(fields) => {
                    return Err(syn::Error::new_spanned(
                        fields,
                        "unsupported unnamed fields",
                    ))
                }
            };
            Ok(Variant {
                name: variant.ident.to_string(),
                fields,
            })
        })
        .collect()
}
