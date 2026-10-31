//! Read-only view of a process's startup environment for the Console.
//!
//! Each process describes the variables it reads with their effective values. Secrets never
//! leave the process (only whether they are set), and URLs lose their password.

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub struct BootstrapGroup {
    pub id: String,
    pub label: String,
    pub items: Vec<BootstrapItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BootstrapItem {
    /// Primary environment variable.
    pub key: String,
    pub label: String,
    pub description: String,
    /// Effective value after defaults; `null` when unset and optional. Redacted per `sensitive`.
    pub value: Value,
    pub sensitive: bool,
    /// True when the environment sets it; false when the built-in default applies.
    pub configured: bool,
    /// Fallback variables read when the primary one is unset.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub required: bool,
}

pub const SECRET_MASK: &str = "••••••••";

impl BootstrapGroup {
    pub fn new(id: &str, label: &str, items: Vec<BootstrapItem>) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            items,
        }
    }
}

impl BootstrapItem {
    /// `keys[0]` is the primary variable, the rest are fallbacks in read order.
    pub fn env(keys: &[&str], label: &str, description: &str, value: impl Into<Value>) -> Self {
        Self {
            key: keys[0].to_string(),
            label: label.to_string(),
            description: description.to_string(),
            value: value.into(),
            sensitive: false,
            configured: env_is_set(keys),
            aliases: keys[1..].iter().map(|key| key.to_string()).collect(),
            required: false,
        }
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// Replaces a non-empty value with a fixed mask.
    pub fn secret(mut self) -> Self {
        self.sensitive = true;
        self.value = match &self.value {
            Value::String(text) if text.is_empty() => Value::String(String::new()),
            Value::Null => Value::Null,
            _ => Value::String(SECRET_MASK.into()),
        };
        self
    }

    /// Masks the password in a URL's userinfo.
    pub fn url(mut self) -> Self {
        if let Value::String(text) = &self.value {
            self.value = Value::String(redact_url(text));
        }
        self
    }
}

/// etcd registration read by `aster_boot::try_register_from_env`; shared by every Aster process.
pub fn discovery_group(service_name: &str, default_port: u16) -> BootstrapGroup {
    let endpoints = std::env::var("ASTER_ETCD_ENDPOINTS")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let enabled = endpoints.is_some();
    let read = |key: &str| {
        std::env::var(key)
            .ok()
            .filter(|value| !value.trim().is_empty())
    };
    BootstrapGroup::new(
        "discovery",
        "服务发现（etcd）",
        vec![
            BootstrapItem::env(
                &["ASTER_ETCD_ENDPOINTS"],
                "etcd 地址",
                "逗号分隔；未设置时不注册服务发现",
                endpoints.map_or(Value::Null, Value::from),
            ),
            BootstrapItem::env(
                &["ASTER_SERVICE_NAME"],
                "服务名",
                "注册到 etcd 的服务名",
                service_name.to_string(),
            ),
            BootstrapItem::env(
                &["ASTER_SERVICE_ID"],
                "实例 ID",
                "默认 <服务名>-<进程号>",
                read("ASTER_SERVICE_ID")
                    .unwrap_or_else(|| format!("{service_name}-{}", std::process::id())),
            ),
            BootstrapItem::env(
                &["ASTER_SERVICE_HOST"],
                "通告主机",
                "其他服务连接本实例的主机",
                read("ASTER_SERVICE_HOST").unwrap_or_else(|| "127.0.0.1".into()),
            ),
            BootstrapItem::env(
                &["ASTER_SERVICE_PORT"],
                "通告端口",
                if enabled {
                    "其他服务连接本实例的端口"
                } else {
                    "其他服务连接本实例的端口（服务发现未启用）"
                },
                read("ASTER_SERVICE_PORT")
                    .and_then(|value| value.parse::<u16>().ok())
                    .unwrap_or(default_port),
            ),
        ],
    )
}

/// True when any of `keys` is set to a non-blank value.
pub fn env_is_set(keys: &[&str]) -> bool {
    keys.iter().any(|key| {
        std::env::var(key)
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false)
    })
}

/// `postgres://user:pass@host/db` → `postgres://user:••••••••@host/db`.
pub fn redact_url(url: &str) -> String {
    let Some(scheme_end) = url.find("://") else {
        return url.to_string();
    };
    let rest_start = scheme_end + 3;
    let authority_end = url[rest_start..]
        .find(['/', '?', '#'])
        .map_or(url.len(), |offset| rest_start + offset);
    let Some(at) = url[rest_start..authority_end].rfind('@') else {
        return url.to_string();
    };
    let userinfo = &url[rest_start..rest_start + at];
    let Some(colon) = userinfo.find(':') else {
        return url.to_string();
    };
    format!(
        "{}{}:{}{}",
        &url[..rest_start],
        &userinfo[..colon],
        SECRET_MASK,
        &url[rest_start + at..]
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn urls_lose_only_their_password() {
        assert_eq!(
            redact_url("postgres://aster:secret@127.0.0.1:5432/aster?sslmode=disable"),
            "postgres://aster:••••••••@127.0.0.1:5432/aster?sslmode=disable"
        );
        assert_eq!(
            redact_url("redis://:pw@redis:6379/0"),
            "redis://:••••••••@redis:6379/0"
        );
        assert_eq!(redact_url("http://user@host/x"), "http://user@host/x");
        assert_eq!(redact_url("http://host/a@b"), "http://host/a@b");
        assert_eq!(redact_url("127.0.0.1:5102"), "127.0.0.1:5102");
    }

    #[test]
    fn secrets_report_presence_only() {
        let set = BootstrapItem::env(&["ASTER_TEST_UNSET_SECRET"], "", "", "hunter2").secret();
        assert_eq!(set.value, json!(SECRET_MASK));
        assert!(set.sensitive);
        let empty = BootstrapItem::env(&["ASTER_TEST_UNSET_SECRET"], "", "", "").secret();
        assert_eq!(empty.value, json!(""));
        assert!(!empty.configured);
    }

    #[test]
    fn primary_key_and_aliases_keep_read_order() {
        let item = BootstrapItem::env(&["ASTER_X", "X"], "x", "", 1).required();
        assert_eq!(item.key, "ASTER_X");
        assert_eq!(item.aliases, vec!["X".to_string()]);
        let json = serde_json::to_value(&item).unwrap();
        assert_eq!(json["required"], json!(true));
        assert_eq!(json["aliases"], json!(["X"]));
    }
}
