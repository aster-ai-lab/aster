//! Local Capability Key auth for the Function data-plane.

use axum::http::HeaderMap;

pub const DEV_CAPABILITY_KEY: &str = "a3s_sk_devlocalgateway00000000000001";

const ALLOWED: &[&str] = &[DEV_CAPABILITY_KEY, "a3s_sk_devlocalgateway00000000000002"];

#[derive(Debug, Clone)]
pub struct AuthOk {
    pub key: String,
    pub prefix: String,
}

#[derive(Debug, Clone)]
pub struct AuthErr {
    pub code: &'static str,
    pub message: String,
}

pub fn extract_bearer(headers: &HeaderMap) -> Option<String> {
    if let Some(auth) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
        let auth = auth.trim();
        if auth.len() > 7 && auth[..7].eq_ignore_ascii_case("bearer ") {
            let key = auth[7..].trim();
            if !key.is_empty() {
                return Some(key.to_string());
            }
        }
    }
    headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn authenticate(headers: &HeaderMap) -> Result<AuthOk, AuthErr> {
    let key = match extract_bearer(headers) {
        Some(k) if k.starts_with("a3s_sk_") => k,
        _ => {
            return Err(AuthErr {
                code: "auth.invalid_key",
                message: "missing or invalid Capability Key".into(),
            });
        }
    };
    if !ALLOWED.contains(&key.as_str()) {
        return Err(AuthErr {
            code: "auth.invalid_key",
            message: "Capability Key not recognized".into(),
        });
    }
    let prefix = key.chars().take(18).collect::<String>();
    Ok(AuthOk { key, prefix })
}
