#![cfg(test)]

use super::*;
use serde_json::json;

#[test]
fn policy_is_typed_and_never_inferred_from_instructions() {
    assert!(Policy::admit_body(&json!({"turnContext":{"agentRuntime":{"runtime":"edge"},"workbench":{"reviewMode":true,"artifactTrace":false}}})).is_err());
    assert!(Policy::admit_body(&json!({"turnContext":{"agentRuntime":{"runtime":"edge"},"workbench":{"reviewMode":false,"artifactTrace":false}}})).is_ok());
    assert_eq!(
        Policy::from_body(&json!({"turnContext":{"directives":["自动审查：on"]}})).unwrap(),
        Policy::default()
    );
    assert!(
        Policy::from_body(
            &json!({"turnContext":{"workbench":{"reviewMode":"true","artifactTrace":false}}})
        )
        .is_err()
    );
    assert!(Policy::from_body(
        &json!({"turnContext":{"workbench":{"reviewMode":true,"artifactTrace":false,"other":true}}})
    )
    .is_err());
    for review in [false, true] {
        for trace in [false, true] {
            let policy = Policy::from_body(
                &json!({"turnContext":{"workbench":{"reviewMode":review,"artifactTrace":trace}}}),
            )
            .unwrap();
            assert_eq!(
                policy,
                Policy {
                    review_mode: review,
                    artifact_trace: trace
                }
            );
        }
    }
}

#[test]
fn proof_restores_exact_identity_and_refuses_unknown_state() {
    assert!(
        ReviewStatus::parse("guess")
            .unwrap_err()
            .contains("ReviewStatus")
    );
    assert_eq!(
        FindingStatus::parse("addressed").unwrap().as_str(),
        "addressed"
    );
    let proof = Proof::new(
        "session-A",
        "message-A",
        "run-A",
        "model-A",
        "user",
        Policy {
            review_mode: true,
            artifact_trace: false,
        },
    );
    let mut parts = vec![json!({"type":"data-workbench-proof","data":proof})];
    let restored = Proof::from_parts(&parts).unwrap();
    assert_eq!(
        (
            restored.session_id.as_str(),
            restored.message_id.as_str(),
            restored.run_id.as_str()
        ),
        ("session-A", "message-A", "run-A")
    );
    assert_eq!(restored.review.status, ReviewStatus::Pending);
    parts[0]["data"]["review"]["status"] = json!("guess");
    assert!(Proof::from_parts(&parts).is_none());
    assert!(Proof::from_parts(&[json!({"type":"text","text":"data-workbench-proof"})]).is_none());
}

#[test]
fn digest_is_of_bytes_including_unicode_and_line_endings() {
    assert_eq!(
        digest(b""),
        "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_ne!(digest("报告\r\n".as_bytes()), digest("报告\n".as_bytes()));
}
