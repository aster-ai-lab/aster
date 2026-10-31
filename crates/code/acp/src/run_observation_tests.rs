#![cfg(test)]

use super::*;
use serde_json::json;

#[test]
fn observations_require_explicit_protocol_and_correlated_prompt_identity() {
    for value in [
        json!({}),
        json!({"promptId":"p"}),
        json!({"promptId":"p","asterExecutionObservation":2}),
        json!({"promptId":"","asterExecutionObservation":1}),
        json!({"promptId":"wrong id","asterExecutionObservation":1}),
        json!({"promptId":null,"asterExecutionObservation":1}),
    ] {
        assert!(Binding::from_meta(value.as_object()).is_none());
    }
    let value = json!({"promptId":"native-uuid","asterExecutionObservation":1});
    assert_eq!(
        Binding::from_meta(value.as_object()).unwrap().prompt_id,
        "native-uuid"
    );
}
