use super::*;

#[test]
fn frozen_mise_executions_choose_the_same_versions() {
    #[derive(Deserialize)]
    struct Reference {
        versions: Vec<Row>,
        cases: Vec<Case>,
    }
    #[derive(Deserialize)]
    struct Row {
        version: String,
        prerelease: bool,
    }
    #[derive(Deserialize)]
    struct Case {
        request: String,
        exit: i32,
        stdout: String,
    }
    let reference: Reference = serde_json::from_str(include_str!(
        "../../../../../specs/031-cli-mise-replacement/contracts/mise-version-reference.json"
    ))
    .unwrap();
    assert_eq!(reference.cases.len(), 10);
    let listing = VersionListing::new(
        reference
            .versions
            .into_iter()
            .map(|row| (row.version, row.prerelease))
            .collect(),
        BTreeMap::new(),
    )
    .unwrap();
    for case in reference.cases {
        let result = listing.resolve(&VersionRequest::parse(&case.request).unwrap());
        if case.exit == 0 {
            assert_eq!(result.unwrap(), case.stdout.trim(), "{}", case.request);
        } else {
            assert!(
                matches!(result, Err(ToolAcquireError::VersionNotFound(_))),
                "{}",
                case.request
            );
        }
    }
}

fn listing() -> VersionListing {
    VersionListing::new(
        [
            "0.8.0",
            "0.8.9",
            "0.81.0",
            "1.2",
            "1.2.3",
            "1.2.9",
            "1.20.0",
            "2.78.9",
            "2.79.0",
            "3.2.1",
            "4.0.0-rc.1",
        ]
        .into_iter()
        .map(|version| (version.to_string(), semver_prerelease(version)))
        .collect(),
        BTreeMap::from([
            ("lts".into(), "prefix:1.2".into()),
            ("stable".into(), "lts".into()),
        ]),
    )
    .unwrap()
}

#[test]
fn exact_prefix_alias_subtraction_and_prerelease_selection() {
    let listing = listing();
    for (request, expected) in [
        ("latest", "3.2.1"),
        ("1.2", "1.2"),
        ("prefix:1.2", "1.2.9"),
        ("0.8", "0.8.9"),
        ("prefix:0.8", "0.8.9"),
        ("stable", "1.2.9"),
        ("sub-1:latest", "2.79.0"),
        ("sub-0.0.1:2.79.0", "2.78.9"),
        ("4.0.0-rc.1", "4.0.0-rc.1"),
    ] {
        assert_eq!(
            listing
                .resolve(&VersionRequest::parse(request).unwrap())
                .unwrap(),
            expected,
            "{request}"
        );
    }
    assert!(matches!(
        listing.resolve(&VersionRequest::parse("prefix:4").unwrap()),
        Err(ToolAcquireError::VersionNotFound(_))
    ));
}

#[test]
fn supported_request_syntax_is_explicit_and_canonical() {
    for (input, canonical) in [
        ("ref-main", "ref:main"),
        ("branch-feature/foo", "branch:feature/foo"),
        ("tag-v1.0.0", "tag:v1.0.0"),
        ("rev-abc123", "rev:abc123"),
        ("prefix:1.2", "prefix:1.2"),
        ("sub-0.1:latest", "sub-0.1:latest"),
        ("path:../工具 with spaces", "path:../工具 with spaces"),
        (
            "path:../工具 with trailing spaces  ",
            "path:../工具 with trailing spaces  ",
        ),
        ("system", "system"),
    ] {
        assert_eq!(VersionRequest::parse(input).unwrap().canonical(), canonical);
    }
    assert!(matches!(
        VersionRequest::parse("path:../tools").unwrap().kind(),
        VersionRequestKind::Path(_)
    ));
    assert!(matches!(
        VersionRequest::parse("system").unwrap().kind(),
        VersionRequestKind::System
    ));
    for request in [
        "",
        "prefix:",
        "sub-invalid:latest",
        "unknown:1",
        "../outside",
        "1\0.2",
        "1;touch marker",
        "branch:feature/%2e%2E/outside",
    ] {
        assert!(VersionRequest::parse(request).is_err(), "{request:?}");
    }
}

#[test]
fn alias_cycles_and_excessive_depth_fail_without_recursing_forever() {
    let listing = VersionListing::new(
        vec![("1.0.0".into(), false)],
        BTreeMap::from([
            ("first".into(), "second".into()),
            ("second".into(), "sub-1:first".into()),
        ]),
    )
    .unwrap();
    assert!(matches!(
        listing.resolve(&VersionRequest::parse("first").unwrap()),
        Err(ToolAcquireError::VersionAliasCycle { .. })
    ));
    assert!(matches!(
        VersionRequest::parse(&format!("{}latest", "sub-1:".repeat(34))),
        Err(ToolAcquireError::VersionResolutionLimit)
    ));
}

#[test]
fn subtraction_borrows_without_wrapping_and_rejects_non_numeric_bases() {
    for (base, amount, expected) in [
        ("18.2.3", vec![2], "16"),
        ("18.2.3", vec![0, 1], "18.1"),
        ("2.79.0", vec![0, 0, 1], "2.78"),
        ("0.0.0", vec![0, 0, 1], "0"),
        ("2.0.0", vec![0, 1], "1"),
    ] {
        assert_eq!(subtract(base, &amount).unwrap(), expected);
    }
    assert!(matches!(
        subtract("3.0.0-rc.1", &[1]),
        Err(ToolAcquireError::InvalidSubtraction { .. })
    ));
}

#[test]
fn remote_labels_and_list_sizes_are_bounded() {
    assert!(VersionListing::new(vec![("1.0.0".into(), false); 2], BTreeMap::new()).is_err());
    assert!(matches!(
        VersionListing::new(vec![("1".into(), false); MAX_VERSIONS + 1], BTreeMap::new()),
        Err(ToolAcquireError::VersionListLimit)
    ));
    assert!(VersionRequest::parse(&"x".repeat(MAX_REQUEST_BYTES + 1)).is_err());
}

#[test]
fn matching_uses_separator_boundaries_and_preserves_vendor_names() {
    for (query, version, expected) in [
        ("1.2", "1.20.0", false),
        ("1.2", "v1.2.3", true),
        ("v1.2", "1.2.3", true),
        ("1.2", "1.2+build", true),
        ("temurin-", "temurin-17.0.7+7", true),
        ("truffleruby", "truffleruby+graalvm", false),
        ("latest", "nightly", false),
        ("latest", "v2.0.0", true),
    ] {
        assert_eq!(fuzzy_matches(query, version), expected, "{query}/{version}");
    }
}

#[test]
fn installed_latest_can_select_a_named_release_and_falls_back_to_a_prerelease() {
    let latest = VersionRequest::parse("latest").unwrap();
    for (version, prerelease) in [("tip", false), ("2.0.0-rc.1", true)] {
        let listing =
            VersionListing::new(vec![(version.into(), prerelease)], BTreeMap::new()).unwrap();
        assert!(matches!(
            listing.resolve(&latest),
            Err(ToolAcquireError::VersionNotFound(_))
        ));
        assert_eq!(listing.select_installed(&latest).unwrap(), version);
    }
}
