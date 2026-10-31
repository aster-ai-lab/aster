pub mod platform;

use serde_json::Value;

#[derive(Debug)]
pub enum ProviderErr {
    Status {
        status: u16,
        error_code: String,
        message: String,
    },
}

impl ProviderErr {
    pub fn credential_missing() -> Self {
        Self::Status {
            status: 503,
            error_code: crate::SUPPLIER_VAULT_ENV_MISSING_CODE.into(),
            message: crate::SUPPLIER_VAULT_ENV_MISSING_MESSAGE.into(),
        }
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::Status {
            status: 400,
            error_code: "request.invalid".into(),
            message: message.into(),
        }
    }

    pub fn upstream(message: impl Into<String>) -> Self {
        Self::Status {
            status: 502,
            error_code: "upstream.exhausted".into(),
            message: message.into(),
        }
    }
}

pub fn token_total(usage: &Value) -> f64 {
    let Some(obj) = usage.as_object() else {
        return 0.0;
    };
    if let Some(t) = obj.get("total_tokens").and_then(|v| v.as_f64()) {
        if t > 0.0 {
            return t.round();
        }
    }
    let prompt = obj
        .get("prompt_tokens")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let completion = obj
        .get("completion_tokens")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let sum = prompt + completion;
    if sum > 0.0 {
        sum.round()
    } else {
        0.0
    }
}
