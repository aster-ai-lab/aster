use super::*;

#[test]
fn fresh_scope_binds_exact_objective_session_policy_and_completed_dependent_stages() {
    let view = json!({"work_policy_digest":"policy","definition":{
        "state":"completed","goal":{"description":"original objective"},
        "plan_accepted":true,"plan_fact_sequence":3,
        "binding":{"session_id":"original-session","work_policy_digest":"policy","scope_fact_sequences":[]},
        "plan":{"steps":[{"id":"repair","status":"completed","dependencies":[]},{"id":"verify","status":"completed","dependencies":["repair"]}]}
    }});
    assert!(fresh_goal_complete(
        &view,
        "original-session",
        "original objective"
    ));
    assert!(!fresh_goal_complete(
        &view,
        "foreign-session",
        "original objective"
    ));
    assert!(!fresh_goal_complete(
        &view,
        "original-session",
        "rewritten objective"
    ));
    for pointer in [
        "/definition/plan_fact_sequence",
        "/definition/binding/work_policy_digest",
        "/definition/binding/scope_fact_sequences",
    ] {
        let mut bad = view.clone();
        *bad.pointer_mut(pointer).unwrap() = Value::Null;
        assert!(!fresh_goal_complete(
            &bad,
            "original-session",
            "original objective"
        ));
    }
    for (pointer, value) in [
        ("/definition/state", json!("active")),
        ("/definition/binding/scope_fact_sequences", json!([1])),
        ("/definition/plan/steps/1/status", json!("pending")),
        ("/definition/plan/steps/1/dependencies", json!([])),
    ] {
        let mut bad = view.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(!fresh_goal_complete(
            &bad,
            "original-session",
            "original objective"
        ));
    }
}
