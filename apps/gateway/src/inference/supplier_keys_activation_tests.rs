use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
fn policy() -> InferenceConfig {
    serde_json::from_value(serde_json::json!({
        "expires_at": (chrono::Utc::now()+chrono::Duration::hours(1)).to_rfc3339(),
        "tokenizer_revision": crate::config::INFERENCE_TOKENIZER_REVISION,
        "credentials": {}, "workers": {}, "guardrails": {},
        "routes": {"00000000-0000-4000-8000-000000000001": {
            "route_id":"00000000-0000-4000-8000-000000000001",
            "router":"fixture", "environment_id":"00000000-0000-4000-8000-000000000002", "policy_revision":1,
            "models":{"fixture":{
                "model_id":"00000000-0000-4000-8000-000000000003",
                "targets":[{
                    "target_id":"00000000-0000-4000-8000-000000000004", "service":"fixture",
                    "upstream_model":"internal", "priority":0, "weight":1, "api_key_env":"required"
                }]
            }}
        }}
    })).unwrap()
}
async fn server(response: Vec<u8>) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        loop {
            let mut buf = [0u8; 1024];
            let n = socket.read(&mut buf).await.unwrap();
            if n == 0 {
                break;
            }
            request.extend_from_slice(&buf[..n]);
            assert!(request.len() < 16384);
            if request.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }
        assert!(String::from_utf8_lossy(&request)
            .to_ascii_lowercase()
            .contains("authorization: bearer fixture-secret"));
        let _ = socket.write_all(&response).await;
    });
    url
}
#[tokio::test]
async fn keyless_routes_keep_serving_without_a_console_dependency() {
    let mut policy = policy();
    for target in policy
        .routes
        .values_mut()
        .flat_map(|r| r.models.values_mut())
        .flat_map(|m| m.targets.iter_mut())
    {
        target.api_key_env = None;
    }
    let config = GatewayConfig {
        inference: Some(policy),
        ..GatewayConfig::default()
    };
    assert!(load_supplier_credentials(&config).await.unwrap().is_empty());
}
#[tokio::test]
async fn keyed_routes_refuse_activation_without_a_console_origin() {
    let config = GatewayConfig {
        inference: Some(policy()),
        ..GatewayConfig::default()
    };
    assert!(load_supplier_credentials(&config).await.is_err());
}
#[tokio::test]
async fn failed_console_fetch_is_an_error_instead_of_empty_credentials() {
    let origin = server(
        b"HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
    )
    .await;
    assert!(fetch_supplier_credentials(&origin, "fixture-secret")
        .await
        .is_err());
}
#[tokio::test]
async fn an_authoritative_empty_list_is_distinct_from_a_failed_fetch() {
    let body = b"{\"success\":true,\"data\":[]}";
    let mut response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    response.extend_from_slice(body);
    let origin = server(response).await;
    assert!(fetch_supplier_credentials(&origin, "fixture-secret")
        .await
        .unwrap()
        .is_empty());
}
#[tokio::test]
async fn supplier_fetch_refuses_redirects_and_oversized_responses() {
    for response in [
        b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
        b"HTTP/1.1 200 OK\r\nContent-Length: 1048577\r\nConnection: close\r\n\r\n".to_vec(),
    ] {
        let origin=server(response).await;
        assert!(fetch_supplier_credentials(&origin,"fixture-secret").await.is_err());
    }
}
#[test]
fn all_four_platform_suppliers_use_the_same_console_secret_boundary() {
    let mut policy = policy();
    let route = policy.routes.values_mut().next().unwrap();
    let template = route.models.values().next().unwrap().targets[0].clone();
    route.models.clear();
    let mut targets = Vec::new();
    let mut suppliers = Vec::new();
    for (i, name) in ["ocr", "systemone", "embedding", "rerank"]
        .into_iter()
        .enumerate()
    {
        let mut target = template.clone();
        target.target_id = Uuid::from_u128(i as u128 + 100);
        target.service = name.into();
        suppliers.push(SupplierCredential {
            name: name.into(),
            api_key: "fixture-key".into(),
        });
        targets.push(target);
    }
    route.ocr_supplier = Some(targets[0].clone());
    route.decision_supplier = Some(targets[1].clone());
    route.embedding_supplier = Some(targets[2].clone());
    route.rerank_supplier = Some(targets[3].clone());
    let keys = resolve_supplier_keys(&policy, &suppliers);
    assert_eq!(keys.len(), 4);
    for target in targets {
        let value = keys.get(&target.target_id).unwrap();
        assert_eq!(value.to_str().unwrap(), "Bearer fixture-key");
        assert!(value.is_sensitive());
    }
}
