use aster_use_core::{
    inspect_okf_bundle, okf_document_references, OkfBundleFile, OkfBundleLimits,
    OkfDocumentReference, OkfFormatVersion,
};

#[test]
fn references_include_explicit_sources_resources_markdown_and_images_but_exclude_code() {
    let text = br#"---
type: Concept
resource: workspace:original.md
sources:
  - resource: ../declared.md
  - resource: private source descriptor
executor:
  resource: ../run.md
attester:
  resource: https://example.test/attest
---
[source](../notes/source.md#passage-1)
![image](../images/a.png)
`[code](../fake.md)`
```
[code](../also-fake.md)
```
"#;
    let refs =
        okf_document_references("concepts/page.md", text, OkfBundleLimits::default()).unwrap();
    assert_eq!(
        refs,
        vec![
            OkfDocumentReference {
                target: "../notes/source.md#passage-1".into(),
                resolved_path: Some("notes/source.md".into())
            },
            OkfDocumentReference {
                target: "../images/a.png".into(),
                resolved_path: Some("images/a.png".into())
            },
            OkfDocumentReference {
                target: "workspace:original.md".into(),
                resolved_path: None
            },
            OkfDocumentReference {
                target: "../run.md".into(),
                resolved_path: Some("run.md".into())
            },
            OkfDocumentReference {
                target: "https://example.test/attest".into(),
                resolved_path: None
            },
            OkfDocumentReference {
                target: "../declared.md".into(),
                resolved_path: Some("declared.md".into())
            },
            OkfDocumentReference {
                target: "private source descriptor".into(),
                resolved_path: None
            },
        ]
    );
}

#[test]
fn encoded_unicode_plus_percent_and_reference_style_links_reuse_bundle_resolution() {
    let text = "---\ntype: Concept\n---\n[来源][source]\n\n[source]: ../%E4%B8%AD%E6%96%87%F0%9F%A6%BE.md#段落\n\n[plus](../a+b.md) [percent](../100%2525.md)";
    let refs = okf_document_references(
        "concepts/page.md",
        text.as_bytes(),
        OkfBundleLimits::default(),
    )
    .unwrap();
    assert_eq!(
        refs.iter()
            .map(|r| r.resolved_path.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("中文🦾.md"), Some("a+b.md"), Some("100%25.md")]
    );
    let inspection = inspect_okf_bundle(
        OkfFormatVersion::V0_2,
        OkfBundleLimits::default(),
        [OkfBundleFile::new("concepts/page.md", text)],
    )
    .unwrap();
    let mut actual = inspection
        .diagnostics
        .into_iter()
        .map(|d| d.target)
        .collect::<Vec<_>>();
    actual.sort();
    let mut expected = refs
        .into_iter()
        .filter_map(|r| r.resolved_path)
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(actual, expected);
}

#[test]
fn malformed_unsafe_and_oversized_inputs_refuse() {
    let limits = OkfBundleLimits::default();
    for path in ["", "../page.md", "/page.md", "a/./page.md", "a\\page.md"] {
        assert!(
            okf_document_references(path, b"body", limits.clone()).is_err(),
            "{path}"
        );
    }
    for text in [
        b"\xff".as_slice(),
        b"---\ntype: [\n---\nbody",
        b"[outside](../../escape.md)",
        b"[unsafe](file:/etc/passwd)",
    ] {
        assert!(okf_document_references("concepts/page.md", text, limits.clone()).is_err());
    }
    let mut bounded = limits.clone();
    bounded.max_document_bytes = 8;
    assert!(okf_document_references("concepts/page.md", b"123456789", bounded).is_err());
    let mut bounded = limits;
    bounded.max_links_per_document = 1;
    assert!(
        okf_document_references("concepts/page.md", b"[a](../a.md) [b](../b.md)", bounded).is_err()
    );
}
