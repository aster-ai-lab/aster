use super::*;
use crate::inference::{
    DecisionError, DecisionPlane, DecisionRequest, DecisionResponse, DecisionService,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
struct GatedDecision {
    calls: AtomicUsize,
    release: tokio::sync::Semaphore,
}
#[async_trait::async_trait]
impl DecisionService for GatedDecision {
    async fn decide(&self, _: &DecisionRequest) -> Result<DecisionResponse, DecisionError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.release.acquire().await.unwrap().forget();
        Ok(serde_json::from_value(serde_json::json!({"decision":{"kind":"ordinal","probabilities":{"simple":0.95,"complex":0.05}}})).unwrap())
    }
}
fn fixture() -> (
    crate::config::InferenceModelConfig,
    crate::config::InferenceRoutingPlan,
    crate::inference::TurnIdentity,
    DecisionRequest,
) {
    let config = routed_config("127.0.0.1:1".parse().unwrap(), &inference_key('j'));
    let model = config
        .inference
        .unwrap()
        .routes
        .into_values()
        .next()
        .unwrap()
        .models
        .remove("allowed-model")
        .unwrap();
    let plan = model.routing.clone().unwrap();
    let document = serde_json::json!({"messages":[{"role":"user","content":"hello"}]})
        .as_object()
        .unwrap()
        .clone();
    let identity = crate::inference::turn_identity_from_document(&document, 1, "auto");
    let request = crate::inference::build_decision_request(&plan, &document).unwrap();
    (model, plan, identity, request)
}
async fn wait_calls(service: &GatedDecision, count: usize) {
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while service.calls.load(Ordering::SeqCst) < count {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn duplicate_turns_share_one_neural_decision() {
    let service = Arc::new(GatedDecision {
        calls: AtomicUsize::new(0),
        release: tokio::sync::Semaphore::new(0),
    });
    let plane = Arc::new(DecisionPlane::new(Some(service.clone())));
    let (model, plan, identity, request) = fixture();
    let mut tasks = Vec::new();
    for _ in 0..32 {
        let (plane, model, plan, request) =
            (plane.clone(), model.clone(), plan.clone(), request.clone());
        tasks.push(tokio::spawn(async move {
            plane
                .decide(&model, &plan, &identity, Some(&request), None)
                .await
        }));
    }
    wait_calls(&service, 1).await;
    for _ in 0..64 {
        tokio::task::yield_now().await;
    }
    assert_eq!(service.calls.load(Ordering::SeqCst), 1);
    service.release.add_permits(32);
    let mut fresh = 0;
    for task in tasks {
        let (decision, _, hit) = task.await.unwrap();
        assert_eq!(decision.group, crate::inference::ChosenGroup::Economy);
        if !hit {
            fresh += 1;
        }
    }
    assert_eq!(fresh, 1);
    assert_eq!(service.calls.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn unrelated_turns_do_not_serialize_neural_calls() {
    let service = Arc::new(GatedDecision {
        calls: AtomicUsize::new(0),
        release: tokio::sync::Semaphore::new(0),
    });
    let plane = Arc::new(DecisionPlane::new(Some(service.clone())));
    let (model, plan, identity, request) = fixture();
    let mut tasks = Vec::new();
    for delta in [0, 1] {
        let (plane, model, plan, request) =
            (plane.clone(), model.clone(), plan.clone(), request.clone());
        let identity = crate::inference::TurnIdentity {
            last_user_digest: identity.last_user_digest.wrapping_add(delta),
            ..identity
        };
        tasks.push(tokio::spawn(async move {
            plane
                .decide(&model, &plan, &identity, Some(&request), None)
                .await
        }));
    }
    wait_calls(&service, 2).await;
    service.release.add_permits(2);
    for task in tasks {
        task.await.unwrap();
    }
}
#[tokio::test]
async fn snapshot_policy_change_invalidates_frozen_choice() {
    let service = Arc::new(SwitchableDecision::armed(0.95));
    let plane = DecisionPlane::new(Some(service.clone()));
    let (mut model, mut plan, identity, request) = fixture();
    let first = plane
        .decide(&model, &plan, &identity, Some(&request), None)
        .await;
    assert_eq!(first.0.group, crate::inference::ChosenGroup::Economy);
    plan.simple_probability_threshold_bp = 9900;
    model.routing = Some(plan.clone());
    let next = plane
        .decide(&model, &plan, &identity, Some(&request), None)
        .await;
    assert_eq!(next.0.group, crate::inference::ChosenGroup::Primary);
    assert!(!next.2);
    assert_eq!(service.calls(), 2);
}
#[tokio::test]
async fn auto_alias_uses_frozen_typed_policy_for_tool_continuations() {
    let key = inference_key('m');
    let (backend, mut captures) = spawn_multi_capturing_backend(2).await;
    let mut config = routed_config(backend, &key);
    let route = config
        .inference
        .as_mut()
        .unwrap()
        .routes
        .values_mut()
        .next()
        .unwrap();
    let mut model = route.models.remove("allowed-model").unwrap();
    model.routing.as_mut().unwrap().decision_model = Some("auto".into());
    route.models.insert("auto".into(), model);
    for grant in route.grants.values_mut() {
        for model in &mut grant.models {
            if model == "allowed-model" {
                *model = "auto".into();
            }
        }
    }
    config.validate().unwrap();
    let service = Arc::new(SwitchableDecision::armed(0.95));
    let state =
        gateway_state_with_decision_plane(&config, DecisionPlane::new(Some(service.clone())));
    let (address, stop, handle) = start_test_entrypoint(state).await;
    for messages in [
        serde_json::json!([{"role":"user","content":"Hi 中文🦀","queuePosition":1,"queueTotal":2}]),
        serde_json::json!([{"role":"user","content":"Hi 中文🦀"},{"role":"assistant","content":null,"tool_calls":[{"id":"c1","type":"function","function":{"name":"lookup","arguments":"{}"}}]},{"role":"tool","tool_call_id":"c1","content":"result"}]),
    ] {
        let response = reqwest::Client::new()
            .post(format!("http://{address}/v1/chat/completions"))
            .bearer_auth(&key)
            .json(&serde_json::json!({"model":"auto","messages":messages}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let captured = captures.recv().await.unwrap();
        let body: serde_json::Value =
            serde_json::from_slice(body_after_headers(&captured)).unwrap();
        assert_eq!(body["model"], "internal-economy");
        for message in body["messages"].as_array().unwrap() {
            assert!(message.get("queuePosition").is_none());
            assert!(message.get("queueTotal").is_none());
        }
    }
    assert_eq!(service.calls(), 1);
    stop_test_entrypoint(stop, handle).await;
}

#[test]
fn a_large_leading_system_prompt_never_routes_to_economy() {
    let (_, plan, _, _) = fixture();
    let document=serde_json::json!({"messages":[{"role":"system","content":"x".repeat(65536)},{"role":"user","content":"Hi"}]}).as_object().unwrap().clone();
    assert!(crate::inference::build_decision_request(&plan, &document).is_err());
}
#[tokio::test]
async fn a_cached_simple_turn_cannot_bypass_the_observation_bound() {
    let service = Arc::new(SwitchableDecision::armed(0.95));
    let plane = DecisionPlane::new(Some(service.clone()));
    let (model, plan, identity, request) = fixture();
    assert_eq!(
        plane
            .decide(&model, &plan, &identity, Some(&request), None)
            .await
            .0
            .group,
        crate::inference::ChosenGroup::Economy
    );
    let next = plane.decide(&model, &plan, &identity, None, None).await;
    assert_eq!(next.0.group, crate::inference::ChosenGroup::Primary);
    assert!(!next.2);
    assert_eq!(service.calls(), 1);
}

#[tokio::test]
async fn direct_model_dispatch_filters_queue_metadata_and_keeps_tool_semantics() {
    let key = inference_key('k');
    let (backend, mut captures) = spawn_multi_capturing_backend(1).await;
    let config = inference_config(backend, &key, Utc::now() + ChronoDuration::hours(1));
    let (address, stop, handle) = start_test_entrypoint(gateway_state(&config)).await;
    let response=reqwest::Client::new().post(format!("http://{address}/v1/chat/completions"))
        .bearer_auth(&key).json(&serde_json::json!({
            "model":"allowed-model",
            "messages":[
                {"role":"system","content":"system text","queuePosition":1,"queueTotal":3},
                {"role":"user","content":"hi","agent":"ui","conversationRequestId":"ui-row","startsNewUserRequest":true},
                {"role":"assistant","content":null,"queuePosition":2,"queueTotal":3,"tool_calls":[{"id":"c1","type":"function","function":{"name":"lookup","arguments":"{}"}}]},
                {"role":"tool","tool_call_id":"c1","content":"tool result","queuePosition":3,"queueTotal":3}
            ]
        })).send().await.unwrap();
    assert_eq!(response.status(), 200);
    let captured = captures.recv().await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(body_after_headers(&captured)).unwrap();
    for message in body["messages"].as_array().unwrap() {
        for key in [
            "queuePosition",
            "queueTotal",
            "agent",
            "conversationRequestId",
            "startsNewUserRequest",
        ] {
            assert!(message.get(key).is_none(), "{message}");
        }
    }
    assert_eq!(
        body["messages"][2]["tool_calls"][0]["function"]["name"],
        "lookup"
    );
    assert_eq!(body["messages"][3]["tool_call_id"], "c1");
    assert_eq!(body["messages"][3]["content"], "tool result");
    stop_test_entrypoint(stop, handle).await;
}
