//! Signed-in OCR and System One on the native inference route.

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
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use uuid::Uuid;

const SESSION: &str = "a3s_us_session-token";

#[path = "paperloom_gateway_tests.rs"]
mod paperloom_gateway_tests;

async fn spawn_auth_verifier() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let request = String::from_utf8(read_http_request(&mut stream).await).unwrap();
                let accepted = request
                    .to_ascii_lowercase()
                    .contains(&format!("authorization: bearer {SESSION}"));
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
    address
}

fn service(backend: &str) -> ServiceConfig {
    ServiceConfig {
        load_balancer: LoadBalancerConfig {
            strategy: Strategy::RoundRobin,
            request_timeout: "30s".into(),
            stream_idle_timeout: "5m".to_string(),
            stream_total_timeout: "60m".to_string(),
            connect_timeout: "2s".to_string(),
            servers: vec![ServerConfig {
                url: backend.into(),
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

fn model(service_name: &str, upstream_model: &str) -> InferenceModelConfig {
    InferenceModelConfig {
        engine_profile: None,
        routing: None,
        failover: Default::default(),
        pricing: None,
        latency_bias: false,
        prefix_affinity: false,
        prefix_parallelism: 1,
        model_id: Uuid::new_v4(),
        targets: vec![InferenceTargetConfig {
            target_id: Uuid::new_v4(),
            service: service_name.into(),
            upstream_model: upstream_model.into(),
            priority: 0,
            weight: 1,
            protocol: InferenceProtocol::Openai,
            api_key_env: None,
            translate: false,
        }],
        scheduling: None,
    }
}

fn supplier(
    service_name: &str,
    upstream_model: &str,
    protocol: InferenceProtocol,
) -> InferenceTargetConfig {
    InferenceTargetConfig {
        target_id: Uuid::new_v4(),
        service: service_name.into(),
        upstream_model: upstream_model.into(),
        priority: 0,
        weight: 1,
        protocol,
        api_key_env: None,
        translate: false,
    }
}

struct PlatformRoute {
    verifier: SocketAddr,
    chat: SocketAddr,
    embeddings: SocketAddr,
    ocr_supplier: Option<SocketAddr>,
    decision_supplier: Option<SocketAddr>,
    embedding_supplier: Option<SocketAddr>,
    rerank_supplier: Option<SocketAddr>,
}

fn platform_config(route: PlatformRoute) -> GatewayConfig {
    let mut config = GatewayConfig::default();
    config.routers.insert(
        "openai".into(),
        RouterConfig {
            rule: "PathPrefix(`/v1`)".into(),
            service: "chat-service".into(),
            entrypoints: vec!["web".into()],
            middlewares: vec![],
            priority: 0,
            parking: None,
        },
    );
    config.services.insert(
        "chat-service".into(),
        service(&format!("http://{}", route.chat)),
    );
    config.services.insert(
        "embeddings-service".into(),
        service(&format!("http://{}", route.embeddings)),
    );
    let ocr_supplier = route.ocr_supplier.map(|address| {
        config
            .services
            .insert("ocr-supplier".into(), service(&format!("http://{address}")));
        supplier("ocr-supplier", "supplier-ocr-model", InferenceProtocol::Ocr)
    });
    let decision_supplier = route.decision_supplier.map(|address| {
        config.services.insert(
            "decision-supplier".into(),
            service(&format!("http://{address}")),
        );
        supplier(
            "decision-supplier",
            "supplier-apofasi-model",
            InferenceProtocol::SystemOne,
        )
    });
    let embedding_supplier = route.embedding_supplier.map(|address| {
        config.services.insert(
            "embedding-supplier".into(),
            service(&format!("http://{address}")),
        );
        supplier(
            "embedding-supplier",
            "supplier-embed-model",
            InferenceProtocol::Openai,
        )
    });
    let rerank_supplier = route.rerank_supplier.map(|address| {
        config.services.insert(
            "rerank-supplier".into(),
            service(&format!("http://{address}")),
        );
        supplier(
            "rerank-supplier",
            "supplier-rerank-model",
            InferenceProtocol::Openai,
        )
    });
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
                models: HashMap::from([
                    ("qwen".into(), model("chat-service", "Qwen/Qwen3-8B")),
                    (
                        "embed".into(),
                        model("embeddings-service", "embed-upstream"),
                    ),
                ]),
                grants: HashMap::new(),
                listings: HashMap::new(),
                console_catalog: None,
                wire: InferenceWire::Openai,
                principal: Some(InferencePrincipalConfig {
                    verify_url: format!("http://{}/auth/verify", route.verifier),
                    subject_header: "x-a3s-subject".into(),
                    org_header: Some("x-a3s-org".into()),
                    cache_ttl_secs: 60,
                    models: vec!["*".into()],
                    endpoints: vec![
                        InferenceEndpoint::Models,
                        InferenceEndpoint::ChatCompletions,
                        InferenceEndpoint::Embeddings,
                        InferenceEndpoint::Ocr,
                        InferenceEndpoint::SystemOne,
                        InferenceEndpoint::Rerank,
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
                ocr_supplier,
                decision_supplier,
                embedding_supplier,
                rerank_supplier,
            },
        )]),
        guardrails: HashMap::new(),
        workers: HashMap::new(),
    });
    config
}

fn decision_body() -> serde_json::Value {
    serde_json::json!({
        "model": "caller-picked",
        "state": "Please refund my invoice today. I am very angry about the failed payout.",
        "questions": {
            "department": {
                "type": "choice",
                "instructions": "Which team should handle this refund?",
                "criteria": {
                    "billing": "Payments, invoicing, refunds",
                    "technical": "Bugs, outages, integrations"
                }
            },
            "frustration": {
                "type": "score",
                "instructions": "How frustrated is the customer?",
                "criteria": ["Calm", "Frustrated", "Very angry"]
            },
            "urgent": {
                "type": "noul",
                "instructions": "Does this convey urgency?",
                "criteria": {
                    "true": "Explicitly time-sensitive",
                    "false": "No urgency expressed"
                }
            }
        }
    })
}

/// Draw the fixture string with a real outline font.
///
/// The expected text is the string passed to the font rasterizer. A blocky
/// bitmap of the same letters is not an independent source: PP-OCRv6 read
/// that S as a 5.
fn draw_a3s_png() -> Vec<u8> {
    const TEXT: &str = "A3S";
    let font_bytes = fixture_font_bytes();
    let font = ab_glyph::FontRef::try_from_slice(&font_bytes).expect("fixture font");
    let scale = ab_glyph::PxScale::from(128.0);
    let (text_width, text_height) = imageproc::drawing::text_size(scale, &font, TEXT);
    let margin = 64u32;
    let width = text_width.saturating_add(margin.saturating_mul(2)).max(1);
    let height = text_height.saturating_add(margin.saturating_mul(2)).max(1);
    let mut image = image::RgbImage::from_pixel(width, height, image::Rgb([255, 255, 255]));
    imageproc::drawing::draw_text_mut(
        &mut image,
        image::Rgb([0, 0, 0]),
        margin as i32,
        margin as i32,
        scale,
        &font,
        TEXT,
    );
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}

fn fixture_font_bytes() -> Vec<u8> {
    const CANDIDATES: &[&str] = &[
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
    ];
    for path in CANDIDATES {
        if let Ok(bytes) = std::fs::read(path) {
            return bytes;
        }
    }
    panic!("no TrueType font available to draw the OCR fixture");
}

fn save_evidence(name: &str, body: &str) {
    let Some(dir) = std::env::var_os("A3S_PLATFORM_MODEL_EVIDENCE") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join(name), body);
}

async fn ensure_ocr_runtime() {
    if a3s_use_ocr::ocr_status().available {
        return;
    }
    if let Err(error) = a3s_use_ocr::install_ppocr_v6(false).await {
        let text = format!("{error:#}");
        save_evidence("ocr-runtime.txt", &text);
        panic!("PP-OCRv6 runtime is not available: {text}");
    }
}

fn assert_no_platform_identity(body: &str) {
    assert!(!body.contains("apofasi-lexical"), "{body}");
    assert!(!body.contains("pp-ocr-v6"), "{body}");
    assert!(!body.contains("PP-OCRv6_small"), "{body}");
}

fn assert_platform_decision(body: &serde_json::Value) {
    let model = body["model"].as_str().unwrap();
    assert!(model.starts_with("apofasi-lexical-"), "{body}");
    assert_ne!(model, "caller-picked");
    let choice = &body["answers"]["department"];
    assert_eq!(choice["type"], "choice");
    let picked = choice["choice"].as_str().unwrap();
    assert!(picked == "billing" || picked == "technical", "{choice}");
    let probabilities = choice["probabilities"].as_object().unwrap();
    assert!(probabilities.contains_key("billing"), "{choice}");
    assert!(probabilities.contains_key("technical"), "{choice}");
    let sum = probabilities
        .values()
        .map(|value| value.as_f64().unwrap())
        .sum::<f64>();
    // decode rounds each probability to 4 decimal places before it leaves the engine.
    assert!(
        (sum - 1.0).abs() < 5e-4,
        "choice probabilities sum to {sum}"
    );
    let score = body["answers"]["frustration"]["score"].as_f64().unwrap();
    assert!(
        (0.0..=3.0).contains(&score),
        "score {score} is outside the rubric"
    );
    let noul = body["answers"]["urgent"]["noul"].as_f64().unwrap();
    assert!((0.0..=1.0).contains(&noul), "noul {noul}");
}

fn assert_platform_ocr(body: &serde_json::Value) {
    assert_eq!(body["provider"], "pp-ocr-v6");
    assert_eq!(body["model"], "PP-OCRv6_small");
    let text = body["text"].as_str().unwrap();
    assert!(!text.trim().is_empty(), "{body}");
    let normalized: String = text
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    assert!(
        normalized.to_ascii_uppercase().contains("A3S"),
        "recognized text {text:?} does not contain the fixture string"
    );
    let blocks = body["blocks"].as_array().unwrap();
    assert!(
        blocks
            .iter()
            .any(|block| { block.get("boundingBox").is_some() || block.get("polygon").is_some() }),
        "{body}"
    );
}

async fn spawn_supplier(model: &'static str) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let hits = hits.clone();
            tokio::spawn(async move {
                let _ = read_http_request(&mut stream).await;
                hits.fetch_add(1, Ordering::SeqCst);
                let payload = serde_json::json!({
                    "model": model,
                    "provider": model,
                    "text": model,
                    "blocks": [{
                        "text": model,
                        "boundingBox": {"x": 1, "y": 2, "width": 3, "height": 4}
                    }]
                });
                let bytes = serde_json::to_vec(&payload).unwrap();
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    bytes.len()
                );
                let _ = stream.write_all(header.as_bytes()).await;
                let _ = stream.write_all(&bytes).await;
                let _ = stream.shutdown().await;
            });
        }
    });
    address
}

async fn post_ocr(client: &reqwest::Client, address: SocketAddr, png: &[u8]) -> reqwest::Response {
    client
        .post(format!("http://{address}/v1/ocr"))
        .bearer_auth(SESSION)
        .header(reqwest::header::CONTENT_TYPE, "image/png")
        .body(png.to_vec())
        .send()
        .await
        .unwrap()
}

async fn post_decision(client: &reqwest::Client, address: SocketAddr) -> reqwest::Response {
    client
        .post(format!("http://{address}/v1/systemone"))
        .bearer_auth(SESSION)
        .json(&decision_body())
        .send()
        .await
        .unwrap()
}

#[tokio::test]
async fn signed_in_callers_use_platform_ocr_and_apofasi() {
    ensure_ocr_runtime().await;
    let verifier = spawn_auth_verifier().await;
    let (chat, chat_request) = spawn_capturing_backend().await;
    let (embeddings, embeddings_request) = spawn_capturing_backend().await;
    let config = platform_config(PlatformRoute {
        verifier,
        chat,
        embeddings,
        ocr_supplier: None,
        decision_supplier: None,
        embedding_supplier: None,
        rerank_supplier: None,
    });
    config.validate().expect("platform route validates");
    let (address, shutdown_tx, handle) = start_test_entrypoint(gateway_state(&config)).await;
    let client = reqwest::Client::new();
    let png = draw_a3s_png();

    let denied = client
        .post(format!("http://{address}/v1/ocr"))
        .header(reqwest::header::CONTENT_TYPE, "image/png")
        .body(png.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), 401);
    let denied_body = denied.text().await.unwrap();
    assert_no_platform_identity(&denied_body);

    let denied = client
        .post(format!("http://{address}/v1/systemone"))
        .bearer_auth("a3s_us_forged")
        .json(&decision_body())
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), 401);
    assert_no_platform_identity(&denied.text().await.unwrap());

    let rejected = client
        .post(format!("http://{address}/v1/ocr"))
        .bearer_auth(SESSION)
        .json(&decision_body())
        .send()
        .await
        .unwrap();
    assert_eq!(rejected.status(), 415);

    let ocr = post_ocr(&client, address, &png).await;
    assert_eq!(ocr.status(), 200);
    let ocr_text = ocr.text().await.unwrap();
    save_evidence("ocr-response.json", &ocr_text);
    let ocr_body: serde_json::Value = serde_json::from_str(&ocr_text).unwrap();
    assert_platform_ocr(&ocr_body);

    let decision = post_decision(&client, address).await;
    assert_eq!(decision.status(), 200);
    let decision_text = decision.text().await.unwrap();
    save_evidence("systemone-response.json", &decision_text);
    let decision_body: serde_json::Value = serde_json::from_str(&decision_text).unwrap();
    assert_platform_decision(&decision_body);

    let chat_response = client
        .post(format!("http://{address}/v1/chat/completions"))
        .bearer_auth(SESSION)
        .json(&serde_json::json!({"model": "qwen", "messages": [{"role": "user", "content": "hello"}]}))
        .send()
        .await
        .unwrap();
    assert_eq!(chat_response.status(), 200);
    let chat_json: serde_json::Value = chat_response.json().await.unwrap();
    assert!(chat_json.get("answers").is_none(), "{chat_json}");
    assert!(chat_json.get("blocks").is_none(), "{chat_json}");
    let captured = String::from_utf8(chat_request.await.unwrap()).unwrap();
    let captured_lower = captured.to_ascii_lowercase();
    assert!(
        captured_lower.contains("post /v1/chat/completions"),
        "{captured}"
    );
    assert!(!captured_lower.contains("post /v1/ocr"), "{captured}");
    assert!(!captured_lower.contains("post /v1/systemone"), "{captured}");

    let embeddings_response = client
        .post(format!("http://{address}/v1/embeddings"))
        .bearer_auth(SESSION)
        .json(&serde_json::json!({"model": "embed", "input": "hello"}))
        .send()
        .await
        .unwrap();
    assert_eq!(embeddings_response.status(), 200);
    let embeddings_json: serde_json::Value = embeddings_response.json().await.unwrap();
    assert!(
        embeddings_json.get("answers").is_none(),
        "{embeddings_json}"
    );
    let captured = String::from_utf8(embeddings_request.await.unwrap()).unwrap();
    assert!(
        captured
            .to_ascii_lowercase()
            .contains("post /v1/embeddings"),
        "{captured}"
    );

    stop_test_entrypoint(shutdown_tx, handle).await;
}

#[tokio::test]
async fn supplier_binding_replaces_the_platform_model_until_removed() {
    ensure_ocr_runtime().await;
    let verifier = spawn_auth_verifier().await;
    let (chat, _chat_request) = spawn_capturing_backend().await;
    let (embeddings, _embeddings_request) = spawn_capturing_backend().await;
    let ocr_supplier = spawn_supplier("supplier-ocr-model").await;
    let decision_supplier = spawn_supplier("supplier-apofasi-model").await;
    let client = reqwest::Client::new();
    let png = draw_a3s_png();

    let unreachable = platform_config(PlatformRoute {
        verifier,
        chat,
        embeddings,
        ocr_supplier: Some("127.0.0.1:1".parse().unwrap()),
        decision_supplier: Some("127.0.0.1:1".parse().unwrap()),
        embedding_supplier: None,
        rerank_supplier: None,
    });
    unreachable
        .validate()
        .expect("unreachable supplier still validates");
    let (address, shutdown_tx, handle) = start_test_entrypoint(gateway_state(&unreachable)).await;
    let ocr = post_ocr(&client, address, &png).await;
    assert_eq!(ocr.status(), 503);
    assert_no_platform_identity(&ocr.text().await.unwrap());
    let decision = post_decision(&client, address).await;
    assert_eq!(decision.status(), 503);
    assert_no_platform_identity(&decision.text().await.unwrap());
    stop_test_entrypoint(shutdown_tx, handle).await;

    let bound = platform_config(PlatformRoute {
        verifier,
        chat,
        embeddings,
        ocr_supplier: Some(ocr_supplier),
        decision_supplier: Some(decision_supplier),
        embedding_supplier: None,
        rerank_supplier: None,
    });
    bound.validate().expect("supplier route validates");
    let (address, shutdown_tx, handle) = start_test_entrypoint(gateway_state(&bound)).await;
    let ocr = post_ocr(&client, address, &png).await;
    assert_eq!(ocr.status(), 200);
    let ocr_body: serde_json::Value = ocr.json().await.unwrap();
    assert_eq!(ocr_body["model"], "supplier-ocr-model");
    save_evidence("supplier-ocr-response.json", &ocr_body.to_string());
    let decision = post_decision(&client, address).await;
    assert_eq!(decision.status(), 200);
    let decision_body: serde_json::Value = decision.json().await.unwrap();
    assert_eq!(decision_body["model"], "supplier-apofasi-model");
    save_evidence(
        "supplier-systemone-response.json",
        &decision_body.to_string(),
    );
    stop_test_entrypoint(shutdown_tx, handle).await;

    let unbound = platform_config(PlatformRoute {
        verifier,
        chat,
        embeddings,
        ocr_supplier: None,
        decision_supplier: None,
        embedding_supplier: None,
        rerank_supplier: None,
    });
    let (address, shutdown_tx, handle) = start_test_entrypoint(gateway_state(&unbound)).await;
    let ocr = post_ocr(&client, address, &png).await;
    assert_eq!(ocr.status(), 200);
    let ocr_body: serde_json::Value = ocr.json().await.unwrap();
    assert_platform_ocr(&ocr_body);
    let decision = post_decision(&client, address).await;
    assert_eq!(decision.status(), 200);
    let decision_body: serde_json::Value = decision.json().await.unwrap();
    assert_platform_decision(&decision_body);
    stop_test_entrypoint(shutdown_tx, handle).await;
}

#[tokio::test]
async fn platform_aliases_fail_closed_without_weights_but_proxy_aliases_still_route() {
    let verifier = spawn_auth_verifier().await;
    let (chat, _chat_request) = spawn_capturing_backend().await;
    let (embeddings, embeddings_request) = spawn_capturing_backend().await;
    let config = platform_config(PlatformRoute {
        verifier,
        chat,
        embeddings,
        ocr_supplier: None,
        decision_supplier: None,
        embedding_supplier: None,
        rerank_supplier: None,
    });
    config.validate().expect("platform route validates");
    let (address, shutdown_tx, handle) = start_test_entrypoint(gateway_state(&config)).await;
    let client = reqwest::Client::new();

    // An ordinary alias keeps its managed proxy path untouched.
    let proxied = client
        .post(format!("http://{address}/v1/embeddings"))
        .bearer_auth(SESSION)
        .json(&serde_json::json!({"model": "embed", "input": "hello"}))
        .send()
        .await
        .unwrap();
    assert_eq!(proxied.status(), 200);
    let captured = String::from_utf8(embeddings_request.await.unwrap()).unwrap();
    assert!(captured.contains("POST /v1/embeddings"), "{captured}");

    // Without installed weights the reserved aliases refuse by name and
    // carry the install suggestion — never a fallthrough to another alias.
    // Machines with a default-home install would serve instead; this suite
    // asserts the uninstalled contract the way CI runs it.
    let refused = client
        .post(format!("http://{address}/v1/embeddings"))
        .bearer_auth(SESSION)
        .json(&serde_json::json!({"model": "platform/bge-m3", "input": "hello"}))
        .send()
        .await
        .unwrap();
    assert_eq!(refused.status(), 503);
    let body = refused.text().await.unwrap();
    assert!(body.contains("a3s-use-bge install"), "{body}");

    let refused = client
        .post(format!("http://{address}/v1/rerank"))
        .bearer_auth(SESSION)
        .json(&serde_json::json!({
            "model": "platform/bge-reranker-v2-m3",
            "query": "q",
            "documents": ["a"]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(refused.status(), 404);
    let body: serde_json::Value = refused.json().await.unwrap();
    assert_eq!(body["error"], "standard_not_admitted");

    // The reserved alias cannot be hijacked through the grant either: an
    // unknown ordinary alias stays a plain model-not-granted refusal.
    let unknown = client
        .post(format!("http://{address}/v1/embeddings"))
        .bearer_auth(SESSION)
        .json(&serde_json::json!({"model": "not-a-model", "input": "hello"}))
        .send()
        .await
        .unwrap();
    assert_eq!(unknown.status(), 403);

    let listed = client
        .get(format!("http://{address}/v1/models"))
        .bearer_auth(SESSION)
        .send()
        .await
        .unwrap();
    assert_eq!(listed.status(), 200);
    let models: serde_json::Value = listed.json().await.unwrap();
    let ids = models["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["id"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    for alias in [
        "platform/pp-ocr-v6",
        "platform/apofasi",
        "platform/bge-m3",
        "platform/bge-reranker-v2-m3",
        "embed",
        "qwen",
    ] {
        assert!(ids.iter().any(|id| id == alias), "missing {alias}: {ids:?}");
    }

    stop_test_entrypoint(shutdown_tx, handle).await;
}

#[tokio::test]
async fn embedding_and_rerank_suppliers_replace_the_platform_model() {
    let verifier = spawn_auth_verifier().await;
    let (chat, _chat_request) = spawn_capturing_backend().await;
    let (embeddings, _embeddings_request) = spawn_capturing_backend().await;
    let embedding_supplier = spawn_supplier("supplier-embed-model").await;
    let rerank_supplier = spawn_supplier("supplier-rerank-model").await;
    let config = platform_config(PlatformRoute {
        verifier,
        chat,
        embeddings,
        ocr_supplier: None,
        decision_supplier: None,
        embedding_supplier: Some(embedding_supplier),
        rerank_supplier: Some(rerank_supplier),
    });
    config.validate().expect("platform route validates");
    let (address, shutdown_tx, handle) = start_test_entrypoint(gateway_state(&config)).await;
    let client = reqwest::Client::new();

    let served = client
        .post(format!("http://{address}/v1/embeddings"))
        .bearer_auth(SESSION)
        .json(&serde_json::json!({"model": "platform/bge-m3", "input": "hello"}))
        .send()
        .await
        .unwrap();
    assert_eq!(served.status(), 200);
    let body: serde_json::Value = served.json().await.unwrap();
    assert_eq!(body["model"], "supplier-embed-model", "{body}");

    let served = client
        .post(format!("http://{address}/v1/rerank"))
        .bearer_auth(SESSION)
        .json(&serde_json::json!({
            "model": "platform/bge-reranker-v2-m3",
            "query": "q",
            "documents": ["a", "b"],
            "top_n": 1
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(served.status(), 404);
    let body: serde_json::Value = served.json().await.unwrap();
    assert_eq!(body["error"], "standard_not_admitted");

    stop_test_entrypoint(shutdown_tx, handle).await;

    // A bound but unreachable supplier is a 503 that never falls through to
    // the local engine — the OCR and System One contract.
    let verifier = spawn_auth_verifier().await;
    let (chat, _chat_request) = spawn_capturing_backend().await;
    let (embeddings, _embeddings_request) = spawn_capturing_backend().await;
    let ghost = {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        address
    };
    let config = platform_config(PlatformRoute {
        verifier,
        chat,
        embeddings,
        ocr_supplier: None,
        decision_supplier: None,
        embedding_supplier: Some(ghost),
        rerank_supplier: Some(ghost),
    });
    config.validate().expect("platform route validates");
    let (address, shutdown_tx, handle) = start_test_entrypoint(gateway_state(&config)).await;
    let refused = client
        .post(format!("http://{address}/v1/embeddings"))
        .bearer_auth(SESSION)
        .json(&serde_json::json!({"model": "platform/bge-m3", "input": "hello"}))
        .send()
        .await
        .unwrap();
    assert_eq!(refused.status(), 503);
    stop_test_entrypoint(shutdown_tx, handle).await;
}
