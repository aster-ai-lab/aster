//! Deliver a platform-owned model through the API that owns the deployment.
//! Customers still call the gateway; this hop only runs after that entry.

use serde_json::{json, Value};

use super::ProviderErr;

#[derive(Debug, Clone)]
pub struct PlatformDelivery {
    pub listed: bool,
    pub available: bool,
    pub body: String,
    pub reason: String,
}

fn api_origin() -> String {
    std::env::var("A3S_API_ORIGIN")
        .unwrap_or_else(|_| "http://127.0.0.1:3000".into())
        .trim_end_matches('/')
        .to_string()
}

pub async fn deliver_platform_model(
    http: &reqwest::Client,
    capability_key: &str,
    sku: &str,
    transaction_id: &str,
    body: &Value,
) -> Result<PlatformDelivery, ProviderErr> {
    let url = format!("{}/v1/api/admin/inference/deliveries", api_origin());
    let payload = json!({
        "sku": sku,
        "transactionId": transaction_id,
        "body": body.to_string(),
    });
    let response = http
        .post(url)
        .bearer_auth(capability_key)
        .header("x-transaction-id", transaction_id)
        .json(&payload)
        .send()
        .await
        .map_err(|error| ProviderErr::upstream(error.to_string()))?;
    let status = response.status();
    let value: Value = response
        .json()
        .await
        .map_err(|error| ProviderErr::upstream(error.to_string()))?;
    if !status.is_success() {
        let message = value
            .get("message")
            .and_then(|item| item.as_str())
            .unwrap_or("platform delivery failed");
        return Err(ProviderErr::Status {
            status: status.as_u16(),
            error_code: "upstream.exhausted".into(),
            message: message.to_string(),
        });
    }
    let data = value.get("data").unwrap_or(&value);
    Ok(PlatformDelivery {
        listed: data
            .get("listed")
            .and_then(|item| item.as_bool())
            .unwrap_or(false),
        available: data
            .get("available")
            .and_then(|item| item.as_bool())
            .unwrap_or(false),
        body: data
            .get("body")
            .and_then(|item| item.as_str())
            .unwrap_or("")
            .to_string(),
        reason: data
            .get("reason")
            .and_then(|item| item.as_str())
            .unwrap_or("")
            .to_string(),
    })
}
