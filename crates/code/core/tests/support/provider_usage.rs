//! Bounded provider accounting probes through the real Core streaming clients.

use aster_core::llm::{http::ReqwestHttpClient, HttpClient, HttpResponse, StreamingHttpResponse};
use futures::StreamExt;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

#[derive(Default)]
struct Capture {
    status: Option<u16>,
    bytes: Vec<u8>,
    overflow: bool,
}
struct Transport {
    inner: ReqwestHttpClient,
    capture: Arc<Mutex<Capture>>,
}

#[async_trait::async_trait]
impl HttpClient for Transport {
    async fn post(
        &self,
        url: &str,
        headers: Vec<(&str, &str)>,
        body: &Value,
        cancel: CancellationToken,
    ) -> anyhow::Result<HttpResponse> {
        self.inner.post(url, headers, body, cancel).await
    }
    async fn post_streaming(
        &self,
        url: &str,
        headers: Vec<(&str, &str)>,
        body: &Value,
        cancel: CancellationToken,
    ) -> anyhow::Result<StreamingHttpResponse> {
        let mut response = self
            .inner
            .post_streaming(url, headers, body, cancel)
            .await?;
        self.capture.lock().unwrap().status = Some(response.status);
        let capture = self.capture.clone();
        response.byte_stream = Box::pin(response.byte_stream.map(move |chunk| {
            if let Ok(bytes) = &chunk {
                let mut capture = capture.lock().unwrap();
                if capture.bytes.len().saturating_add(bytes.len()) <= 262144 && !capture.overflow {
                    capture.bytes.extend_from_slice(bytes);
                } else {
                    capture.overflow = true;
                    capture.bytes.clear();
                }
            }
            chunk
        }));
        Ok(response)
    }
}

fn counters(
    bytes: &[u8],
) -> (
    Vec<Value>,
    Option<u64>,
    Option<u64>,
    Option<u64>,
    Option<u64>,
) {
    let Ok(source) = std::str::from_utf8(bytes) else {
        return (vec![], None, None, None, None);
    };
    let source = source.replace("\r\n", "\n");
    let mut rows = vec![];
    let (mut input, mut output, mut read, mut write) = (None, None, None, None);
    for block in source.split("\n\n") {
        let data = block
            .lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .map(str::trim_start)
            .collect::<Vec<_>>()
            .join("\n");
        let Ok(event) = serde_json::from_str::<Value>(&data) else {
            continue;
        };
        let native = matches!(
            event["type"].as_str(),
            Some("message_start" | "message_delta")
        );
        let Some(usage) = event
            .get("usage")
            .or_else(|| event.pointer("/message/usage"))
        else {
            continue;
        };
        if usage.is_null() {
            continue;
        }
        let fields: serde_json::Map<String, Value> = [
            "input_tokens",
            "output_tokens",
            "prompt_tokens",
            "completion_tokens",
            "total_tokens",
            "cache_read_input_tokens",
            "cache_creation_input_tokens",
        ]
        .into_iter()
        .filter_map(|key| {
            usage
                .get(key)
                .filter(|v| v.is_u64())
                .map(|v| (key.into(), v.clone()))
        })
        .collect();
        input = usage
            .get(if native {
                "input_tokens"
            } else {
                "prompt_tokens"
            })
            .and_then(Value::as_u64)
            .or(input);
        output = usage
            .get(if native {
                "output_tokens"
            } else {
                "completion_tokens"
            })
            .and_then(Value::as_u64)
            .or(output);
        read = usage
            .get("cache_read_input_tokens")
            .or_else(|| usage.pointer("/prompt_tokens_details/cached_tokens"))
            .and_then(Value::as_u64)
            .or(read);
        write = usage
            .get("cache_creation_input_tokens")
            .and_then(Value::as_u64)
            .or(write);
        rows.push(json!({"kind":if native{"messages_usage"}else{"chat_usage"},"counts":fields}));
    }
    (rows, input, output, read, write)
}

pub async fn probe(mut config: aster_core::llm::LlmConfig, selected: &str) -> Value {
    use aster_core::llm::{create_client_with_config, Message, StreamEvent};
    let capture = Arc::new(Mutex::new(Capture::default()));
    let messages_protocol = config.protocol == aster_core::config::ProviderProtocol::Anthropic;
    config.max_tokens = Some(512);
    let mut retry = config.retry_config.take().unwrap_or_default();
    retry.max_retries = 0;
    config.retry_config = Some(retry);
    let client = create_client_with_config(config.with_http_client(Arc::new(Transport {
        inner: ReqwestHttpClient::without_redirects().unwrap(),
        capture: capture.clone(),
    })));
    let cancel = CancellationToken::new();
    let started = Instant::now();
    let result = tokio::time::timeout(Duration::from_secs(120), async {
        let mut rx = client
            .complete_streaming(
                &[Message::user("Reply with exactly pong. Do not use tools.")],
                None,
                &[],
                cancel.clone(),
            )
            .await?;
        while let Some(event) = rx.recv().await {
            if let StreamEvent::Done(response) = event {
                return Ok::<_, anyhow::Error>(response);
            }
        }
        anyhow::bail!("stream closed without completion")
    })
    .await;
    cancel.cancel();
    let mut captured = capture.lock().unwrap();
    let (raw, input, output, read, write) = counters(&captured.bytes);
    captured.bytes.clear();
    let normalized_input = input.map(|tokens| {
        if messages_protocol {
            tokens
                .saturating_add(read.unwrap_or(0))
                .saturating_add(write.unwrap_or(0))
        } else {
            tokens
        }
    });
    let mut report = json!({"model":selected,"http_status":captured.status,"elapsed_ms":started.elapsed().as_millis(),"raw_usage":raw,"raw_input":input,"raw_output":output,"raw_cache_read":read,"raw_cache_write":write,"capture_overflow":captured.overflow,"calls_limit":1,"output_limit":512,"retry_limit":0,"agent_created":false,"account_imports_activated":false,"response_text_exported":false});
    report["normalized_input_including_cache"] = normalized_input.into();
    report["input_bucket_semantics"] = if messages_protocol {
        "messages_disjoint"
    } else {
        "chat_cache_is_subset"
    }
    .into();
    match result {
        Ok(Ok(response)) => {
            let u = &response.usage;
            report["stream_completed"] = true.into();
            report["core_usage"] = serde_json::to_value(u).unwrap();
            report["input_matches_raw"] = input.map(|n| n == u.prompt_tokens as u64).into();
            report["normalized_input_matches_core"] =
                normalized_input.map(|n| n == u.prompt_tokens as u64).into();
            report["output_matches_raw"] = output.map(|n| n == u.completion_tokens as u64).into();
            report["cache_read_matches_raw"] = read
                .map(|n| Some(n) == u.cache_read_tokens.map(|n| n as u64))
                .into();
            report["cache_write_matches_raw"] = write
                .map(|n| Some(n) == u.cache_write_tokens.map(|n| n as u64))
                .into();
            let timing = response
                .meta
                .and_then(|m| m.first_token_ms.zip(m.duration_ms))
                .filter(|(first, last)| last > first);
            report["measured_output_tokens_per_second"] = timing
                .map(|(first, last)| u.completion_tokens as f64 * 1000.0 / (last - first) as f64)
                .into();
        }
        Ok(Err(error)) => {
            report["stream_completed"] = false.into();
            report["failure_details"] = json!({"retry_attempts":error.downcast_ref::<aster_core::llm::RetryExhaustedError>().map(aster_core::llm::RetryExhaustedError::attempts),"terminal_status":error.downcast_ref::<aster_core::llm::NonRetryableLlmError>().and_then(aster_core::llm::NonRetryableLlmError::status)});
        }
        Err(_) => {
            report["stream_completed"] = false.into();
            report["deadline_exceeded"] = true.into();
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn raw_usage_keeps_final_native_input_and_missing_chat_counters_distinct() {
        let wire = concat!(
            "data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":0,\"output_tokens\":0}}}\r\n\r\n",
            "data: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"private-output-must-not-be-exported\"}}\r\n\r\n",
            "data: {\"type\":\"message_delta\",\"usage\":{\"input_tokens\":16,\"output_tokens\":64,\"cache_read_input_tokens\":0}}\r\n\r\n"
        );
        let (rows, input, output, read, write) = counters(wire.as_bytes());
        assert_eq!(
            (input, output, read, write),
            (Some(16), Some(64), Some(0), None)
        );
        assert_eq!(rows.len(), 2);
        assert!(!serde_json::to_string(&rows)
            .unwrap()
            .contains("private-output"));
        let (_, input, output, read, write) =
            counters(b"data: {\"choices\":[],\"usage\":{\"total_tokens\":9}}\n\n");
        assert_eq!((input, output, read, write), (None, None, None, None));
        let (_,input,output,read,write)=counters(b"data: {\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":2,\"prompt_tokens_details\":{\"cached_tokens\":8}}}\n\n");
        assert_eq!(
            (input, output, read, write),
            (Some(10), Some(2), Some(8), None)
        );
    }
}
