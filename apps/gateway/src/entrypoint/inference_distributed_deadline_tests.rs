//! Exhausting the response budget before headers produces JSON 504, not a reset.
use super::*;
use crate::error::Result;
use crate::middleware::{Middleware, MiddlewareRegistry, RequestContext};
use async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};

struct ExhaustResponseBudget {
    responses: Arc<AtomicUsize>,
}

#[async_trait]
impl Middleware for ExhaustResponseBudget {
    async fn handle_request(
        &self,
        _req: &mut http::request::Parts,
        _ctx: &RequestContext,
    ) -> Result<Option<http::Response<Vec<u8>>>> {
        Ok(None)
    }

    async fn handle_response(&self, response: &mut http::response::Parts) -> Result<()> {
        if response.status == StatusCode::OK
            && response
                .headers
                .get(http::header::CONTENT_TYPE)
                .is_some_and(|value| value == "text/event-stream")
        {
            self.responses.fetch_add(1, Ordering::SeqCst);
            // Controlled fixture work consumes the entire configured 400ms
            // budget after Power is ready. No CPU/auth timing assumption is
            // required to prove that this response is expired before return.
            tokio::time::sleep(Duration::from_millis(401)).await;
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "exhaust-response-budget"
    }
}

#[tokio::test]
async fn expired_before_sse_headers_returns_504_aborts_workers_and_releases_admission() {
    let key = inference_key('s');
    let (prefill_address, decode_address, stream_started, prefill_abort, decode_abort) =
        spawn_holding_power_pair_for_client_cancel().await;
    let mut config = inference_config(decode_address, &key, Utc::now() + ChronoDuration::hours(1));
    enable_distributed_scheduling(&mut config, prefill_address, decode_address);
    let service = config.services.get_mut("model-service").unwrap();
    service.load_balancer.stream_idle_timeout = "2s".into();
    service.load_balancer.stream_total_timeout = "400ms".into();
    set_limits(
        &mut config,
        InferenceLimitsConfig {
            max_spend_micros: None,
            spend_window: None,
            spend_baseline_micros: None,
            baseline_epoch: None,
            max_concurrent_requests: 1,
            requests_per_minute: 600,
            request_burst: 10,
            tokens_per_minute: 10_000,
        },
    );
    config
        .routers
        .get_mut("test-router")
        .unwrap()
        .middlewares
        .push("exhaust-response-budget".into());
    let custom = std::collections::HashSet::from(["exhaust-response-budget".to_string()]);
    config.validate_with_custom_middlewares(&custom).unwrap();
    let responses = Arc::new(AtomicUsize::new(0));
    let mut registry = MiddlewareRegistry::new();
    registry
        .register(
            "exhaust-response-budget",
            ExhaustResponseBudget {
                responses: responses.clone(),
            },
        )
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let spool = Arc::new(
        UsageSpool::open(UsageSpoolOptions {
            directory: directory.path().join("usage-spool"),
            gateway_id: config.managed.gateway_id.unwrap(),
            max_bytes: 1024 * 1024,
        })
        .await
        .unwrap(),
    );
    let mut state =
        gateway_state_with_distributed_key_and_registry(&config, KEY_ENV, API_KEY, &registry);
    Arc::get_mut(&mut state).unwrap().usage_spool = Some(spool.clone());
    let (address, shutdown, task) = start_test_entrypoint(state).await;
    let client = reqwest::Client::new();
    // Deliberately cold credential: no warmup bypasses authentication work.
    let response = client
        .post(format!("http://{address}/v1/chat/completions"))
        .bearer_auth(&key)
        .header("content-type", "application/json")
        .body(r#"{"model":"allowed-model","messages":[],"stream":true}"#)
        .send()
        .await
        .expect("expired preheader response must be HTTP 504, never a transport reset");
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    assert_eq!(response.headers()["content-type"], "application/json");
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "distributed_inference_timeout"
    );
    assert_eq!(responses.load(Ordering::SeqCst), 1);
    tokio::time::timeout(Duration::from_secs(2), stream_started)
        .await
        .unwrap()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), prefill_abort)
        .await
        .expect("expired response must abort prefill")
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), decode_abort)
        .await
        .expect("expired response must abort decode")
        .unwrap();
    let recovered = client
        .get(format!("http://{address}/v1/models"))
        .bearer_auth(&key)
        .send()
        .await
        .unwrap();
    assert_eq!(
        recovered.status(),
        StatusCode::OK,
        "failed response must release admission"
    );
    let _ = recovered.bytes().await.unwrap();
    let events = lifecycle_events(&spool, 4).await;
    assert_eq!(
        events.len(),
        4,
        "attempt/request terminals must be recorded exactly once"
    );
    assert_eq!(events[2]["kind"], "attempt_terminal");
    assert_eq!(events[2]["outcome"], "failed");
    assert_eq!(events[3]["kind"], "request_terminal");
    assert_eq!(events[3]["outcome"], "failed");
    assert_eq!(
        spool.status().retained_records,
        4,
        "no duplicate attempt/request terminal may be appended beyond the read batch"
    );
    assert_eq!(spool.status().reserved_bytes, 0);
    stop_test_entrypoint(shutdown, task).await;
    spool.shutdown().await;
}
