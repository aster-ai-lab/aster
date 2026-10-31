//! Metering export contracts (`specs/005-os-convergence-execution` T029):
//! the batch endpoint publishes real records when the event spine is wired,
//! refuses closed with a named 501 when it is not, and reports a failed
//! publish as a named 503 — never an empty batch that reads as "nothing to
//! export".

use super::*;

use crate::usage_export::{UsageBatch, UsageExport, UsageExporter};
use async_trait::async_trait;
use axum::body::Body;
use axum::http::Request;
use tower::ServiceExt;

async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    key: Option<&str>,
    body: &str,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(key) = key {
        builder = builder.header("authorization", format!("Bearer {key}"));
    }
    let response = app
        .clone()
        .oneshot(builder.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1_000_000)
        .await
        .unwrap();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

fn temp_state() -> (std::path::PathBuf, AppState) {
    let dir = std::env::temp_dir().join(format!("runtime-export-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    (dir.clone(), AppState::new(dir))
}

/// In-memory transport double that retains every batch it receives.
#[derive(Default)]
struct RecordingExport {
    batches: std::sync::Mutex<Vec<UsageBatch>>,
}

#[async_trait]
impl UsageExport for RecordingExport {
    async fn publish(&self, batch: UsageBatch) -> Result<(), String> {
        self.batches.lock().unwrap().push(batch);
        Ok(())
    }
}

/// Transport double that always fails, standing in for a dead spine.
struct FailingExport;

#[async_trait]
impl UsageExport for FailingExport {
    async fn publish(&self, _batch: UsageBatch) -> Result<(), String> {
        Err("spine unreachable".to_string())
    }
}

/// Seed one denied-usage record without any upstream: a Tool invoke with a
/// sku outside the entitlement set is refused and recorded locally.
async fn seed_denied_record(app: &Router, key: &str) {
    let (status, body) = call(
        app,
        "POST",
        "/v1/api/services/tool/invoke/not-entitled-sku",
        Some(key),
        r#"{"input":{}}"#,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error_code"], "entitlement.denied");
}

#[tokio::test]
async fn batch_refuses_with_named_reason_without_a_spine() {
    let (dir, state) = temp_state();
    let app = router(state);
    let key = crate::auth::DEV_CAPABILITY_KEY;

    let (status, _) = call(&app, "POST", "/v1/api/usage/batch", None, "{}").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, body) = call(&app, "POST", "/v1/api/usage/batch", Some(key), "{}").await;
    assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
    assert_eq!(body["error_code"], "usage_export.not_wired");

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn batch_publishes_the_real_records_when_wired() {
    let (dir, state) = temp_state();
    let export = std::sync::Arc::new(RecordingExport::default());
    let state = state.with_usage_export(UsageExporter::with(export.clone()));
    let app = router(state);
    let key = crate::auth::DEV_CAPABILITY_KEY;

    seed_denied_record(&app, key).await;

    let (status, body) = call(&app, "POST", "/v1/api/usage/batch", Some(key), "{}").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["published"], true);
    let published = body["data"]["records"].as_u64().unwrap();
    assert!(published >= 1, "the flush must carry the real records");

    let batches = export.batches.lock().unwrap();
    // At least the per-record fire-and-forget attempt plus the awaited flush
    // crossed the transport; the flush carries every retained record.
    let flush = batches
        .iter()
        .max_by_key(|batch| batch.records.len())
        .expect("a batch crossed the spine");
    assert!(flush.records.len() as u64 >= published);
    assert_eq!(flush.gateway_id, "aster-runtime");
    assert!(flush
        .records
        .iter()
        .any(|record| record.sku == "not-entitled-sku"));

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn a_failed_publish_reports_a_named_503_not_an_empty_batch() {
    let (dir, state) = temp_state();
    let state = state.with_usage_export(UsageExporter::with(std::sync::Arc::new(FailingExport)));
    let app = router(state);
    let key = crate::auth::DEV_CAPABILITY_KEY;

    seed_denied_record(&app, key).await;

    let (status, body) = call(&app, "POST", "/v1/api/usage/batch", Some(key), "{}").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error_code"], "usage_export.publish_failed");
    assert!(body["message"]
        .as_str()
        .unwrap_or("")
        .contains("spine unreachable"));

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn usage_view_carries_real_records_and_no_lifecycle_advertisement() {
    let (dir, state) = temp_state();
    let app = router(state);
    let key = crate::auth::DEV_CAPABILITY_KEY;

    let (status, body) = call(&app, "GET", "/v1/api/usage", Some(key), "").await;
    assert_eq!(status, StatusCode::OK);
    let data = &body["data"];
    assert!(data["items"].is_array());
    assert!(data["spend_usd"].is_number());
    // The lifecycle export is not advertised as if it were live.
    assert!(data.get("lifecycle_events").is_none());
    assert!(data.get("lifecycle_schema").is_none());
    assert!(data.get("batch_schema").is_none());

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn analytics_refuses_with_named_reason_until_wired() {
    let (dir, state) = temp_state();
    let app = router(state);
    let key = crate::auth::DEV_CAPABILITY_KEY;

    let (status, _) = call(&app, "GET", "/v1/api/analytics", None, "").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, body) = call(&app, "GET", "/v1/api/analytics", Some(key), "").await;
    assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
    assert_eq!(body["error_code"], "analytics.not_wired");
    assert!(body["message"].as_str().unwrap_or("").contains("not wired"));

    let _ = std::fs::remove_dir_all(dir);
}
