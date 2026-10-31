//! HTTP client for remote aster-config (ConfigReader port).

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;

use crate::application::ConfigReader;
use crate::domain::{ConfigEntry, ConfigKey, ConfigNamespace, ConfigValue, DomainError};
use crate::error::{AppError, AppResult};

#[derive(Debug, Deserialize)]
struct EnvelopeData {
    #[serde(default)]
    data: Option<ConfigEntryDto>,
    #[serde(default)]
    error: Option<EnvelopeError>,
}

#[derive(Debug, Deserialize)]
struct EnvelopeError {
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConfigEntryDto {
    namespace: String,
    key: String,
    value: Value,
    revision: i64,
    updated_at_epoch: i64,
    updated_by: String,
}

/// Reads config entries from a running `aster-config` process.
#[derive(Clone)]
pub struct HttpConfigClient {
    base: String,
    http: reqwest::Client,
}

impl HttpConfigClient {
    pub fn new(base_url: impl Into<String>) -> AppResult<Self> {
        let base = base_url.into().trim_end_matches('/').to_string();
        if base.is_empty() {
            return Err(AppError::validation("config service URL is empty"));
        }
        Ok(Self {
            base,
            http: reqwest::Client::new(),
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base
    }

    fn map_domain(err: DomainError) -> AppError {
        match err {
            DomainError::Validation(message) => AppError::validation(message),
            DomainError::NotFound(message) => AppError::not_found(message),
            DomainError::Conflict(message) => AppError::conflict(message),
            DomainError::Infrastructure(message) => AppError::infrastructure(message),
        }
    }

    /// Maps a non-2xx config-service response to the matching `AppError`, keeping the
    /// service's own message (both `{message}` and `{error: {message}}` bodies).
    fn response_error(status: reqwest::StatusCode, body: &str) -> AppError {
        let parsed = serde_json::from_str::<Value>(body).ok();
        let message = parsed
            .as_ref()
            .and_then(|value| {
                value
                    .get("message")
                    .and_then(Value::as_str)
                    .or_else(|| value.pointer("/error/message").and_then(Value::as_str))
            })
            .map(str::to_string)
            .unwrap_or_else(|| format!("config service HTTP {status}: {body}"));
        match status.as_u16() {
            404 => AppError::not_found(message),
            409 => AppError::conflict(message),
            code if (400..500).contains(&code) => AppError::validation(message),
            _ => AppError::infrastructure(message),
        }
    }

    async fn decode_entry(&self, response: reqwest::Response) -> AppResult<ConfigEntry> {
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|err| AppError::infrastructure(format!("config client read body: {err}")))?;
        if !status.is_success() {
            return Err(Self::response_error(status, &body));
        }

        let envelope = envelope::<EnvelopeData>(&body);
        let wrapped = envelope
            .as_ref()
            .is_some_and(|env| env.data.is_some() || env.error.is_some());
        let dto = if wrapped {
            let env = envelope.expect("envelope checked");
            if let Some(err) = env.error {
                let message = err.message.unwrap_or_else(|| body.clone());
                return if status.as_u16() == 404 {
                    Err(AppError::not_found(message))
                } else if status.is_client_error() {
                    Err(AppError::validation(message))
                } else {
                    Err(AppError::infrastructure(message))
                };
            }
            env.data.ok_or_else(|| {
                AppError::infrastructure(format!(
                    "config client empty data (HTTP {status}): {body}"
                ))
            })?
        } else {
            serde_json::from_str::<ConfigEntryDto>(&body).map_err(|err| {
                AppError::infrastructure(format!(
                    "config client decode failed (HTTP {status}): {err}; body={body}"
                ))
            })?
        };

        ConfigEntry::new(
            ConfigNamespace::parse(dto.namespace).map_err(Self::map_domain)?,
            ConfigKey::parse(dto.key).map_err(Self::map_domain)?,
            ConfigValue::from_text(dto.value.to_string()),
            dto.revision,
            dto.updated_at_epoch,
            dto.updated_by,
        )
        .map_err(Self::map_domain)
    }

    /// Forward an admin HTTP call to the config service (for api proxy).
    pub async fn forward(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
    ) -> AppResult<(u16, Value)> {
        let url = format!("{}{}", self.base, path);
        let mut req = self.http.request(method, &url);
        if let Some(payload) = body {
            req = req.json(&payload);
        }
        let response = req
            .send()
            .await
            .map_err(|err| AppError::infrastructure(format!("config proxy: {err}")))?;
        let status = response.status().as_u16();
        let text = response
            .text()
            .await
            .map_err(|err| AppError::infrastructure(format!("config proxy body: {err}")))?;
        let value = if text.is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&text).unwrap_or(Value::String(text))
        };
        Ok((status, value))
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(&self, path: &str) -> AppResult<T> {
        let url = format!("{}{}", self.base, path);
        let response = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|err| AppError::infrastructure(format!("config client: {err}")))?;
        self.decode_json(response).await
    }

    async fn decode_json<T: for<'de> Deserialize<'de>>(
        &self,
        response: reqwest::Response,
    ) -> AppResult<T> {
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|err| AppError::infrastructure(format!("config client read body: {err}")))?;
        if !status.is_success() {
            return Err(Self::response_error(status, &body));
        }
        if let Some(env) = envelope::<EnvelopeValue>(&body) {
            if let Some(err) = env.error {
                let message = err.message.unwrap_or_else(|| body.clone());
                return if status.as_u16() == 404 {
                    Err(AppError::not_found(message))
                } else if status.is_client_error() {
                    Err(AppError::validation(message))
                } else {
                    Err(AppError::infrastructure(message))
                };
            }
            if let Some(data) = env.data {
                return serde_json::from_value(data).map_err(|err| {
                    AppError::infrastructure(format!("config client data decode: {err}"))
                });
            }
        }
        serde_json::from_str(&body).map_err(|err| {
            AppError::infrastructure(format!(
                "config client decode failed (HTTP {status}): {err}; body={body}"
            ))
        })
    }

    pub async fn status(&self) -> AppResult<Value> {
        self.get_json("/api/configs/status").await
    }

    pub async fn bootstrap_catalog(&self) -> AppResult<Value> {
        self.get_json("/api/configs/bootstrap").await
    }

    pub async fn runtime_builtins(&self) -> AppResult<Value> {
        self.get_json("/api/configs/builtins").await
    }

    /// Builtins for one console page (`runtime` or `login`).
    pub async fn builtins_on(&self, surface: &str) -> AppResult<Value> {
        self.get_json(&format!(
            "/api/configs/builtins?surface={}",
            urlencoding_lite(surface)
        ))
        .await
    }

    pub async fn list_namespace(&self, namespace: &str) -> AppResult<Value> {
        self.get_json(&format!("/api/configs/{}", urlencoding_lite(namespace)))
            .await
    }

    pub async fn put_entry(
        &self,
        namespace: &str,
        key: &str,
        value: Value,
        updated_by: String,
    ) -> AppResult<ConfigEntry> {
        let url = format!(
            "{}/api/configs/{}/{}",
            self.base,
            urlencoding_lite(namespace),
            urlencoding_lite(key)
        );
        let response = self
            .http
            .put(&url)
            .json(&serde_json::json!({ "value": value, "updated_by": updated_by }))
            .send()
            .await
            .map_err(|err| AppError::infrastructure(format!("config client put: {err}")))?;
        self.decode_entry(response).await
    }

    pub async fn delete_entry(&self, namespace: &str, key: &str) -> AppResult<()> {
        let url = format!(
            "{}/api/configs/{}/{}",
            self.base,
            urlencoding_lite(namespace),
            urlencoding_lite(key)
        );
        let response = self
            .http
            .delete(&url)
            .send()
            .await
            .map_err(|err| AppError::infrastructure(format!("config client delete: {err}")))?;
        let status = response.status();
        if status.is_success() || status.as_u16() == 404 {
            return Ok(());
        }
        let body = response.text().await.unwrap_or_default();
        Err(AppError::infrastructure(format!(
            "config client delete HTTP {status}: {body}"
        )))
    }
}

/// Serde fills a struct from a JSON array by position, so only an object can be an envelope.
fn envelope<T: for<'de> Deserialize<'de>>(body: &str) -> Option<T> {
    if !body.trim_start().starts_with('{') {
        return None;
    }
    serde_json::from_str(body).ok()
}

#[derive(Debug, Deserialize)]
struct EnvelopeValue {
    #[serde(default)]
    data: Option<Value>,
    #[serde(default)]
    error: Option<EnvelopeError>,
}

#[async_trait]
impl ConfigReader for HttpConfigClient {
    async fn get(&self, namespace: &str, key: &str) -> AppResult<ConfigEntry> {
        let url = format!(
            "{}/api/configs/{}/{}",
            self.base,
            urlencoding_lite(namespace),
            urlencoding_lite(key)
        );
        let response = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|err| AppError::infrastructure(format!("config client get: {err}")))?;
        if response.status().as_u16() == 404 {
            return Err(AppError::not_found(format!(
                "config {namespace}/{key} not found"
            )));
        }
        self.decode_entry(response).await
    }
}

fn urlencoding_lite(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::StatusCode;

    #[test]
    fn non_success_responses_keep_the_service_message() {
        let error = HttpConfigClient::response_error(
            StatusCode::BAD_REQUEST,
            r#"{"statusCode":400,"message":"limits.upload_mb: value must be between 1 and 10240","error":"Bad Request"}"#,
        );
        assert!(
            matches!(&error, AppError::Validation(m) if m == "limits.upload_mb: value must be between 1 and 10240")
        );
        let error = HttpConfigClient::response_error(
            StatusCode::NOT_FOUND,
            r#"{"error":{"message":"config default.x not found"}}"#,
        );
        assert!(matches!(&error, AppError::NotFound(m) if m == "config default.x not found"));
        let error = HttpConfigClient::response_error(StatusCode::BAD_GATEWAY, "upstream down");
        assert!(matches!(error, AppError::Infrastructure(_)));
    }

    #[test]
    fn a_one_item_list_is_not_mistaken_for_an_envelope() {
        assert!(envelope::<EnvelopeValue>(r#"[{"id":"wechat"}]"#).is_none());
        let wrapped = envelope::<EnvelopeValue>(r#"{"data":[{"id":"wechat"}]}"#).expect("envelope");
        assert_eq!(wrapped.data, Some(serde_json::json!([{ "id": "wechat" }])));
    }
}
