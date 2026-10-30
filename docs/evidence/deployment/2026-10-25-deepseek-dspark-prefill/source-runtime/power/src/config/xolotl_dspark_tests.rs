use super::*;

fn valid() -> XolotlDSparkConfig {
    XolotlDSparkConfig {
        sidecar: "/models/draft.gguf".into(),
        sidecar_sha256: "a".repeat(64),
        depth: 4,
        confidence_bp: 6000,
        evict_prompt_tokens: 4096,
    }
}
#[test]
fn typed_draft_emits_only_the_closed_canonical_environment() {
    let mut env = vec![];
    valid().append_env(&mut env).unwrap();
    assert_eq!(
        env,
        vec![
            ("STRATA_SERVE_DSPARK".into(), "/models/draft.gguf".into()),
            ("STRATA_DSPARK_DEPTH".into(), "4".into()),
            ("STRATA_DSPARK_CONF".into(), "0.6000".into()),
            ("STRATA_SERVE_DSPARK_EVICT".into(), "4096".into())
        ]
    );
}
#[test]
fn rejects_invalid_paths_digests_and_bounds() {
    let mut cases = vec![];
    let mut c = valid();
    c.sidecar = "relative.gguf".into();
    cases.push(c);
    let mut c = valid();
    c.sidecar = "/model/bad\npath".into();
    cases.push(c);
    let mut c = valid();
    c.sidecar_sha256 = "A".repeat(64);
    cases.push(c);
    let mut c = valid();
    c.sidecar_sha256 = "a".repeat(63);
    cases.push(c);
    for n in [0, 6] {
        let mut c = valid();
        c.depth = n;
        cases.push(c);
    }
    let mut c = valid();
    c.confidence_bp = 10001;
    cases.push(c);
    for n in [1, 1048704] {
        let mut c = valid();
        c.evict_prompt_tokens = n;
        cases.push(c);
    }
    assert!(cases.iter().all(|c| c.validate().is_err()));
}
#[test]
fn acl_roundtrip_and_unknown_or_incomplete_draft_are_closed() {
    let source = "xolotl { dspark = { sidecar = \"/models/draft.gguf\", sidecar_sha256 = \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\", depth = 4, confidence_bp = 6000, evict_prompt_tokens = 4096 } }";
    let c = super::super::acl::deserialize(source).unwrap();
    assert_eq!(c.xolotl.dspark.as_ref(), Some(&valid()));
    let encoded = super::super::acl::serialize(&c).unwrap();
    assert_eq!(
        super::super::acl::deserialize(&encoded)
            .unwrap()
            .xolotl
            .dspark,
        Some(valid())
    );
    assert!(super::super::acl::deserialize("xolotl { dspark = { depth = 4 } }").is_err());
    assert!(super::super::acl::deserialize(
        &source.replace("depth = 4", "depth = 4, arbitrary_env = true")
    )
    .is_err());
}
