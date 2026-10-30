#![cfg(test)]

use super::*;

const POLICY: &str = r#"
local_models {
  route "daily" {
    name = "日常开发"
    strategy = "ordered"
    candidate { provider_id = "provider-a" model_id = "model/one" }
    candidate { provider_id = "provider-b" model_id = "model-two" }
  }
}
"#;

#[test]
fn acl_round_trip_preserves_candidate_order_and_existing_model_identity() {
    let policy = LocalModelPolicy::from_acl(POLICY).unwrap();
    let candidates = policy
        .candidates(&ModelTarget::Route {
            route_id: "daily".into(),
        })
        .unwrap();
    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.provider_id.as_str())
            .collect::<Vec<_>>(),
        ["provider-a", "provider-b"]
    );
    assert_eq!(candidates[0].model_id, "model/one");
    assert_eq!(policy.routes[0].name, "日常开发");
    assert_eq!(
        LocalModelPolicy::from_acl(&policy.to_acl().unwrap()).unwrap(),
        policy
    );
}

#[test]
fn empty_policy_is_explicit_and_does_not_invent_a_default_model() {
    let policy = LocalModelPolicy::from_acl("local_models {}").unwrap();
    assert!(policy.routes.is_empty());
}

#[test]
fn unknown_attributes_and_second_default_binding_sources_are_refused() {
    for invalid in [
        POLICY.replace("strategy = \"ordered\"", "strategy = \"smart\""),
        POLICY.replace(
            "strategy = \"ordered\"",
            "strategy = \"ordered\" typo = true",
        ),
        POLICY.replace("candidate {", "unknown {"),
        "local_models { binding \"default\" { kind = \"route\" route_id = \"daily\" } }".to_owned(),
        "local_models {} local_models {}".to_owned(),
    ] {
        assert!(
            LocalModelPolicy::from_acl(&invalid).is_err(),
            "accepted: {invalid}"
        );
    }
}

#[test]
fn invalid_and_duplicate_candidates_are_refused_before_activation() {
    for invalid in [
        POLICY.replace("strategy = \"ordered\"", "strategy = \"fixed\""),
        POLICY.replace(
            "provider_id = \"provider-b\" model_id = \"model-two\"",
            "provider_id = \"provider-a\" model_id = \"model/one\"",
        ),
        POLICY.replace(
            "provider_id = \"provider-a\"",
            "provider_id = \"bad::provider\"",
        ),
        POLICY.replace("model_id = \"model/one\"", "model_id = \"\""),
    ] {
        assert!(
            LocalModelPolicy::from_acl(&invalid).is_err(),
            "accepted: {invalid}"
        );
    }
}

#[test]
fn fixed_routes_admit_one_candidate_and_duplicate_route_ids_refuse() {
    let mut policy = LocalModelPolicy::from_acl(POLICY).unwrap();
    policy.routes[0].strategy = RoutingStrategy::Fixed;
    policy.routes[0].candidates.truncate(1);
    assert_eq!(
        LocalModelPolicy::from_acl(&policy.to_acl().unwrap()).unwrap(),
        policy
    );
    policy.routes.push(policy.routes[0].clone());
    assert!(policy.validate().is_err());
}

#[test]
fn model_targets_are_typed_without_overloading_real_model_ids() {
    let policy = LocalModelPolicy::from_acl(POLICY).unwrap();
    let target: ModelTarget = serde_json::from_str(
        r#"{"kind":"model","providerId":"provider-a","modelId":"route/daily"}"#,
    )
    .unwrap();
    let candidates = policy.candidates(&target).unwrap();
    assert_eq!(candidates[0].model_id, "route/daily");
    assert!(serde_json::from_str::<ModelTarget>(
        r#"{"kind":"route","routeId":"daily","modelId":"other"}"#
    )
    .is_err());
    assert!(policy
        .candidates(&ModelTarget::Route {
            route_id: "missing".into()
        })
        .is_err());
    assert!(ModelTarget::parse(
        "model",
        Some("provider-a"),
        Some("model-one"),
        Some("daily")
    )
    .is_err());
    assert!(ModelTarget::parse("route", Some("provider-a"), None, Some("daily")).is_err());
    assert!(ModelTarget::parse("route", None, None, None).is_err());
    assert!(ModelTarget::parse("guess", None, None, None).is_err());
    assert!(ModelTarget::parse("route", None, None, Some(" ")).is_err());
    assert!(ModelTarget::parse("model", Some("provider-a"), Some(""), None).is_err());
    assert!(policy
        .candidates(&ModelTarget::Model {
            provider_id: " ".into(),
            model_id: "model".into()
        })
        .is_err());
}

#[test]
fn closed_strategy_and_use_sets_round_trip() {
    for value in ["fixed", "ordered"] {
        assert_eq!(RoutingStrategy::parse(value).unwrap().as_str(), value);
    }
    for value in ["default", "quick", "translate"] {
        assert_eq!(ModelUse::parse(value).unwrap().as_str(), value);
    }
    assert!(RoutingStrategy::parse("rotate").is_err());
    assert!(ModelUse::parse("coding").is_err());
}
