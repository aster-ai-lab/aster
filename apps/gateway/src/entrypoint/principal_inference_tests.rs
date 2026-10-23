//! Signed-in user access on a standalone, native-wire inference route.

use super::inference_tests::{
    gateway_state, read_http_request, spawn_capturing_backend, start_test_entrypoint,
    stop_test_entrypoint,
};
use crate::config::{
    GatewayConfig, InferenceConfig, InferenceEndpoint, InferenceLimitsConfig, InferenceModelConfig,
    InferencePrincipalConfig, InferenceProtocol, InferenceRouteConfig, InferenceTargetConfig,
    InferenceWire, LoadBalancerConfig, RouterConfig, ServerConfig, ServiceConfig, Strategy,
};
use chrono::{Duration as ChronoDuration, Utc};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use uuid::Uuid;

const SESSION: &str = "a3s_us_session-token";
const PERSONAL_KEY: &str = "a3s_sk_personal-key";

/// Forward-auth style verifier accepting two credentials for `user-1`.
async fn spawn_auth_verifier() -> (SocketAddr, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            counter.fetch_add(1, Ordering::SeqCst);
            tokio::spawn(async move {
                let request = String::from_utf8(read_http_request(&mut stream).await).unwrap();
                let lower = request.to_ascii_lowercase();
                let accepted = [SESSION, PERSONAL_KEY]
                    .iter()
                    .any(|token| lower.contains(&format!("authorization: bearer {token}")));
                let response = if accepted {
                    "HTTP/1.1 200 OK\r\nx-a3s-subject: user-1\r\nx-a3s-org: org-1\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}"
                } else {
                    "HTTP/1.1 401 Unauthorized\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}"
                };
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.shutdown().await;
            });
        }
    });
    (address, calls)
}

fn service(backend: SocketAddr) -> ServiceConfig {
    ServiceConfig {
        load_balancer: LoadBalancerConfig {
            strategy: Strategy::RoundRobin,
            request_timeout: "2s".into(),
            stream_idle_timeout: "5m".to_string(),
            stream_total_timeout: "60m".to_string(),
            connect_timeout: "2s".to_string(),
            servers: vec![ServerConfig {
                url: format!("http://{backend}"),
                weight: 1,
                target: None,
            }],
            health_check: None,
            sticky: None,
            tls_ca_file: None,
        },
        scaling: None,
        revisions: vec![],
        rollout: None,
        mirror: None,
        failover: None,
    }
}

fn target(
    service: &str,
    upstream_model: &str,
    protocol: InferenceProtocol,
    env: &str,
) -> InferenceTargetConfig {
    InferenceTargetConfig {
        target_id: Uuid::new_v4(),
        service: service.into(),
        upstream_model: upstream_model.into(),
        priority: 0,
        weight: 1,
        protocol,
        api_key_env: Some(env.into()),
        translate: false,
    }
}

fn principal_config(
    verifier: SocketAddr,
    openai_backend: SocketAddr,
    anthropic_backend: SocketAddr,
    openai_env: &str,
    anthropic_env: &str,
) -> GatewayConfig {
    let mut config = GatewayConfig::default();
    config.routers.insert(
        "openai".into(),
        RouterConfig {
            rule: "PathPrefix(`/v1`)".into(),
            service: "openai-service".into(),
            entrypoints: vec!["web".into()],
            middlewares: vec![],
            priority: 0,
            parking: None,
        },
    );
    config
        .services
        .insert("openai-service".into(), service(openai_backend));
    config
        .services
        .insert("anthropic-service".into(), service(anthropic_backend));
    let route_id = Uuid::new_v4();
    config.inference = Some(InferenceConfig {
        expires_at: Utc::now() + ChronoDuration::days(3650),
        tokenizer_revision: crate::config::INFERENCE_TOKENIZER_REVISION.into(),
        credentials: HashMap::new(),
        routes: HashMap::from([(
            route_id,
            InferenceRouteConfig {
                engine_profiles: Vec::new(),
                route_id,
                router: "openai".into(),
                environment_id: Uuid::new_v4(),
                policy_revision: 1,
                console_catalog: None,
                models: HashMap::from([
                    (
                        "qwen".to_string(),
                        InferenceModelConfig {
                            engine_profile: None,
                            routing: None,
                            failover: Default::default(),
                            pricing: None,
                            latency_bias: false,
                            prefix_affinity: false,
                            prefix_parallelism: 1,
                            model_id: Uuid::new_v4(),
                            targets: vec![target(
                                "openai-service",
                                "Qwen/Qwen3-8B",
                                InferenceProtocol::Openai,
                                openai_env,
                            )],
                            scheduling: None,
                        },
                    ),
                    (
                        "claude".to_string(),
                        InferenceModelConfig {
                            engine_profile: None,
                            routing: None,
                            failover: Default::default(),
                            pricing: None,
                            latency_bias: false,
                            prefix_affinity: false,
                            prefix_parallelism: 1,
                            model_id: Uuid::new_v4(),
                            targets: vec![target(
                                "anthropic-service",
                                "claude-sonnet-4-5",
                                InferenceProtocol::Anthropic,
                                anthropic_env,
                            )],
                            scheduling: None,
                        },
                    ),
                ]),
                grants: HashMap::new(),
                listings: HashMap::new(),
                wire: InferenceWire::Openai,
                principal: Some(InferencePrincipalConfig {
                    verify_url: format!("http://{verifier}/auth/verify"),
                    subject_header: "x-a3s-subject".into(),
                    org_header: Some("x-a3s-org".into()),
                    cache_ttl_secs: 60,
                    models: vec!["*".into()],
                    endpoints: vec![
                        InferenceEndpoint::Models,
                        InferenceEndpoint::ChatCompletions,
                        InferenceEndpoint::Messages,
                    ],
                    limits: InferenceLimitsConfig {
                        max_spend_micros: None,
                        spend_window: None,
                        spend_baseline_micros: None,
                        baseline_epoch: None,
                        max_concurrent_requests: 4,
                        requests_per_minute: 60,
                        request_burst: 10,
                        tokens_per_minute: 100_000,
                    },
                }),
                ocr_supplier: None,
                decision_supplier: None,
                embedding_supplier: None,
                rerank_supplier: None,
            },
        )]),
        guardrails: HashMap::new(),
        workers: HashMap::new(),
    });
    config
}

fn split_request(request: Vec<u8>) -> (String, serde_json::Value) {
    let text = String::from_utf8(request).unwrap();
    let offset = text.find("\r\n\r\n").unwrap();
    let body = serde_json::from_str(&text[offset + 4..]).unwrap();
    (text[..offset].to_ascii_lowercase(), body)
}

#[tokio::test]
async fn signed_in_users_reach_providers_on_the_native_wire() {
    let openai_env = format!("A3S_TEST_OPENAI_KEY_{}", Uuid::new_v4().simple());
    let anthropic_env = format!("A3S_TEST_ANTHROPIC_KEY_{}", Uuid::new_v4().simple());
    std::env::set_var(&openai_env, "sk-upstream-openai");
    std::env::set_var(&anthropic_env, "sk-upstream-anthropic");
    let (verifier, verify_calls) = spawn_auth_verifier().await;
    let (openai_backend, openai_request) = spawn_capturing_backend().await;
    let (anthropic_backend, anthropic_request) = spawn_capturing_backend().await;
    let config = principal_config(
        verifier,
        openai_backend,
        anthropic_backend,
        &openai_env,
        &anthropic_env,
    );
    config
        .validate()
        .expect("standalone principal policy validates");
    let (address, shutdown_tx, handle) = start_test_entrypoint(gateway_state(&config)).await;
    let client = reqwest::Client::new();

    let response = client
        .get(format!("http://{address}/v1/models"))
        .bearer_auth("a3s_us_forged")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["error"]["type"], "authentication_error");
    assert!(body.get("success").is_none());

    let response = client
        .get(format!("http://{address}/v1/models"))
        .bearer_auth(SESSION)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["object"], "list");
    let mut ids = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|model| model["id"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    ids.sort();
    assert_eq!(ids, ["claude", "qwen"]);

    let response = client
        .post(format!("http://{address}/v1/chat/completions"))
        .bearer_auth(SESSION)
        .header("cookie", "a3s_us=browser-session")
        .json(&serde_json::json!({"model": "qwen", "stream": true, "messages": []}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.text().await.unwrap(), "{}");
    let (headers, body) = split_request(
        tokio::time::timeout(Duration::from_secs(2), openai_request)
            .await
            .unwrap()
            .unwrap(),
    );
    assert!(headers.contains("authorization: bearer sk-upstream-openai"));
    assert!(headers.contains(&format!("host: {openai_backend}")));
    assert!(!headers.contains(&SESSION.to_ascii_lowercase()));
    assert!(!headers.contains("cookie:"));
    assert_eq!(body["model"], "Qwen/Qwen3-8B");
    assert_eq!(body["stream_options"]["include_usage"], true);
    assert_eq!(
        verify_calls.load(Ordering::SeqCst),
        2,
        "the session token is verified once and then served from cache"
    );

    let response = client
        .post(format!("http://{address}/v1/messages"))
        .header("x-api-key", PERSONAL_KEY)
        .header("anthropic-version", "2023-06-01")
        .json(&serde_json::json!({
            "model": "claude",
            "max_tokens": 16,
            "messages": [{"role": "user", "content": "hi"}]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["error"], "standard_not_admitted");
    assert!(
        tokio::time::timeout(Duration::from_millis(200), anthropic_request)
            .await
            .is_err(),
        "a reserved Anthropic path must not reach the upstream"
    );

    stop_test_entrypoint(shutdown_tx, handle).await;
    std::env::remove_var(&openai_env);
    std::env::remove_var(&anthropic_env);
}

#[tokio::test]
async fn targets_without_their_provider_key_are_never_called() {
    let (verifier, _) = spawn_auth_verifier().await;
    let unset = format!("A3S_TEST_UNSET_KEY_{}", Uuid::new_v4().simple());
    let blackhole = SocketAddr::from(([127, 0, 0, 1], 9));
    let config = principal_config(verifier, blackhole, blackhole, &unset, &unset);
    config.validate().unwrap();
    let (address, shutdown_tx, handle) = start_test_entrypoint(gateway_state(&config)).await;

    let response = reqwest::Client::new()
        .post(format!("http://{address}/v1/chat/completions"))
        .bearer_auth(SESSION)
        .json(&serde_json::json!({"model": "qwen", "messages": []}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 503);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["error"]["type"], "api_error");

    stop_test_entrypoint(shutdown_tx, handle).await;
}

#[test]
fn principal_policies_still_require_valid_shape() {
    let blackhole = SocketAddr::from(([127, 0, 0, 1], 9));
    let mut config = principal_config(blackhole, blackhole, blackhole, "A", "B");
    let route = config
        .inference
        .as_mut()
        .unwrap()
        .routes
        .values_mut()
        .next()
        .unwrap();
    route.principal.as_mut().unwrap().verify_url = "not a url".into();
    assert!(config.validate().is_err());

    let mut config = principal_config(blackhole, blackhole, blackhole, "A", "B");
    let route = config
        .inference
        .as_mut()
        .unwrap()
        .routes
        .values_mut()
        .next()
        .unwrap();
    route.principal = None;
    let error = config.validate().unwrap_err().to_string();
    assert!(error.contains("cloud-managed"), "{error}");
}
