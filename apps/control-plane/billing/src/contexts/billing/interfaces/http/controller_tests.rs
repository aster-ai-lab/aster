#![cfg(test)]
use super::{router, InternalGuard};
use crate::contexts::billing::application::service::BillingService;
use crate::open_memory;

const TOKEN: &str = "billing-test-token";

fn open_app(billing: Arc<BillingService>) -> axum::Router {
    router(billing, InternalGuard::from_secret(TOKEN)).unwrap()
}

fn authed(builder: http::request::Builder) -> http::request::Builder {
    builder.header("authorization", format!("Bearer {TOKEN}"))
}
use axum::body::Body;
use http::Request;
use serde_json::Value;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn usage_event_uses_the_api_envelope() {
    let billing = Arc::new(open_memory().await.unwrap());
    let app = open_app(billing);
    let request_id = Uuid::new_v4();
    let response = app
        .oneshot(
            authed(Request::builder())
                .method("POST")
                .uri("/v1/events")
                .header("content-type", "application/json")
                .header("x-request-id", request_id.to_string())
                .body(Body::from(
                    r#"{"transaction_id":"launch-1","external_subscription_id":"none","code":"tracked_pages","timestamp":1,"properties":{"pageviews":1}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
    assert_eq!(
        response
            .headers()
            .get("x-request-id")
            .unwrap()
            .to_str()
            .unwrap(),
        request_id.to_string()
    );
    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["success"], false);
    assert_eq!(body["code"], 40400);
    assert_eq!(body["message"], "unknown_subscription");
    assert!(body["data"].is_null());
    assert_eq!(body["request_id"], request_id.to_string());
    assert!(body["timestamp"].as_str().unwrap().ends_with('Z'));
}

#[tokio::test]
async fn member_subscription_then_accepts_a_token_event() {
    let billing = Arc::new(open_memory().await.unwrap());
    let app = open_app(Arc::clone(&billing));
    let org_id = Uuid::new_v4();
    let response = app
        .oneshot(
            authed(Request::builder())
                .method("POST")
                .uri("/v1/subscriptions")
                .header("content-type", "application/json")
                .body(Body::from(format!(r#"{{"org_id":"{org_id}"}}"#)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(
        body["data"]["external_subscription_id"],
        format!("org:{org_id}")
    );

    let app = open_app(Arc::clone(&billing));
    let response = app
        .oneshot(
            authed(Request::builder())
                .method("POST")
                .uri("/v1/events")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"transaction_id":"member-1","external_subscription_id":"org:{org_id}","code":"total_tokens","timestamp":1700000000,"properties":{{"tokens":4}}}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["data"]["duplicate"], false);

    let app = open_app(billing);
    let response = app
        .oneshot(
            authed(Request::builder())
                .method("POST")
                .uri("/v1/events")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"transaction_id":"member-1","external_subscription_id":"org:{org_id}","code":"total_tokens","timestamp":1700000000,"properties":{{"tokens":9}}}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 409);
    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["message"], "conflict");
}

#[tokio::test]
async fn a_write_without_the_internal_bearer_is_refused() {
    let billing = Arc::new(open_memory().await.unwrap());
    let app = router(billing, InternalGuard::closed()).unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/events")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"transaction_id":"x","external_subscription_id":"org:x","code":"total_tokens","properties":{"tokens":1}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["message"], "unauthorized");
}

#[tokio::test]
async fn an_unknown_event_field_is_refused() {
    let billing = Arc::new(open_memory().await.unwrap());
    let app = open_app(billing);
    let response = app
        .oneshot(
            authed(Request::builder())
                .method("POST")
                .uri("/v1/events")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"transaction_id":"x","external_subscription_id":"org:x","code":"total_tokens","extra":1}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn a_fractional_usage_property_is_refused() {
    let billing = Arc::new(open_memory().await.unwrap());
    let app = open_app(billing);
    let response = app
        .oneshot(
            authed(Request::builder())
                .method("POST")
                .uri("/v1/events")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"transaction_id":"x","external_subscription_id":"org:x","code":"total_tokens","properties":{"tokens":1.5}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["message"], "invalid_request");
}

#[tokio::test]
async fn ephemeral_store_is_not_ready() {
    let billing = Arc::new(open_memory().await.unwrap());
    let app = open_app(billing);
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 503);
}
