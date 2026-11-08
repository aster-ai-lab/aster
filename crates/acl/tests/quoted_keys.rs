use std::collections::BTreeMap;

use aster_acl::{generate, parse, parse_with_limits, ParseLimits, Value};

#[test]
fn quoted_header_keys_and_attributes_are_preserved() {
    let source = r#"provider { "api_key" = "synthetic-key" headers = { "X-Key" = "synthetic-header", "X-Second": "second" } }"#;
    let document = parse(source).unwrap();
    let provider = &document.blocks[0];
    assert_eq!(
        provider.attributes["api_key"],
        Value::String("synthetic-key".into())
    );
    let Value::Object(headers) = &provider.attributes["headers"] else {
        panic!("headers object")
    };
    assert_eq!(
        headers.iter().cloned().collect::<BTreeMap<_, _>>(),
        BTreeMap::from([
            ("X-Key".into(), Value::String("synthetic-header".into())),
            ("X-Second".into(), Value::String("second".into())),
        ])
    );
    assert_eq!(
        generate(&parse(&generate(&document)).unwrap()),
        generate(&document)
    );
}

#[test]
fn object_key_round_trip_uses_the_lexer_instead_of_a_header_name_list() {
    let keys = [
        "X-Key",
        "200",
        "true",
        "null",
        "",
        "with space",
        "a:b",
        "quote\"",
        "line\nkey",
        "中文",
    ];
    let expected: BTreeMap<_, _> = keys
        .iter()
        .enumerate()
        .map(|(index, key)| (key.to_string(), Value::String(format!("value-{index}"))))
        .collect();
    let mut document = parse("config { values = {} }").unwrap();
    document.blocks[0].attributes.insert(
        "values".into(),
        Value::Object(expected.clone().into_iter().collect()),
    );
    let rendered = generate(&document);
    let reparsed = parse(&rendered).unwrap();
    let Value::Object(values) = &reparsed.blocks[0].attributes["values"] else {
        panic!("values object")
    };
    assert_eq!(values.iter().cloned().collect::<BTreeMap<_, _>>(), expected);
    assert_eq!(generate(&reparsed), rendered);
}

#[test]
fn quoted_keys_keep_syntax_errors_and_collection_limits() {
    for source in [r#"config { "key" }"#, r#"config { "key" "value" }"#] {
        assert!(
            parse(source).is_err(),
            "malformed quoted assignment was ignored"
        );
    }
    let limits = ParseLimits {
        max_collection_items: 2,
        ..ParseLimits::default()
    };
    let error = parse_with_limits(
        r#"config { values = { "one" = 1, "two" = 2, "three" = 3 } }"#,
        limits,
    )
    .unwrap_err();
    assert!(error.code.is_limit());
}
