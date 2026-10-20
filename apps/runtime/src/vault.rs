//! Scoped Runtime upstream vault (in-memory sealed + active; receipts on disk).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VaultProviderId {
    Boyue,
    Siliconflow,
    Worldmonitor,
}

impl VaultProviderId {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "boyue" => Some(Self::Boyue),
            "siliconflow" => Some(Self::Siliconflow),
            "worldmonitor" => Some(Self::Worldmonitor),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Boyue => "boyue",
            Self::Siliconflow => "siliconflow",
            Self::Worldmonitor => "worldmonitor",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultScope {
    pub organization_id: String,
    pub project_id: String,
    pub environment_id: String,
}

impl Default for VaultScope {
    fn default() -> Self {
        Self {
            organization_id: "org_local".into(),
            project_id: "proj_local".into(),
            environment_id: "env_local".into(),
        }
    }
}

fn scope_key(scope: &VaultScope) -> String {
    format!(
        "{}/{}/{}",
        scope.organization_id, scope.project_id, scope.environment_id
    )
}

fn default_base(provider: VaultProviderId) -> &'static str {
    match provider {
        VaultProviderId::Boyue => "http://35.220.164.252:3888/v1",
        VaultProviderId::Siliconflow => "https://api.siliconflow.cn/v1",
        VaultProviderId::Worldmonitor => "https://api.worldmonitor.app",
    }
}

fn normalize_base(raw: Option<&str>, provider: VaultProviderId) -> String {
    raw.map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| default_base(provider))
        .trim_end_matches('/')
        .to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultCredential {
    pub api_key: String,
    pub base_url: String,
    pub secret_ref: String,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultApplyReceipt {
    pub receipt_id: String,
    pub scope: VaultScope,
    pub provider: VaultProviderId,
    pub secret_ref: Option<String>,
    pub version: i64,
    pub status: String,
    pub applied_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Clone)]
struct SealedMaterial {
    secret_ref: String,
    version: i64,
    provider: VaultProviderId,
    api_key: String,
    base_url: String,
    #[allow(dead_code)]
    sealed_at: String,
}

#[derive(Clone)]
struct ActiveBinding {
    secret_ref: String,
    version: i64,
    api_key: String,
    base_url: String,
    applied_at: String,
    receipt_id: String,
}

#[derive(Serialize, Deserialize)]
struct DiskVaultFile {
    version: u32,
    receipts: Vec<VaultApplyReceipt>,
}

pub struct Vault {
    path: PathBuf,
    sealed: Vec<SealedMaterial>,
    active: HashMap<String, HashMap<VaultProviderId, ActiveBinding>>,
    receipts: Vec<VaultApplyReceipt>,
}

const RECEIPT_LIMIT: usize = 100;

impl Vault {
    pub fn open(data_dir: &Path) -> Self {
        let _ = std::fs::create_dir_all(data_dir);
        let path = data_dir.join("upstream-vault.json");
        let mut v = Self {
            path,
            sealed: Vec::new(),
            active: HashMap::new(),
            receipts: Vec::new(),
        };
        v.load_from_disk();
        v
    }

    fn load_from_disk(&mut self) {
        self.sealed.clear();
        self.active.clear();
        if let Ok(raw) = std::fs::read_to_string(&self.path) {
            if let Ok(parsed) = serde_json::from_str::<DiskVaultFile>(&raw) {
                self.receipts = parsed.receipts;
                if parsed.version != 2 {
                    self.persist();
                }
            }
        }
    }

    fn persist(&self) {
        let disk = DiskVaultFile {
            version: 2,
            receipts: self.receipts.clone(),
        };
        if let Ok(text) = serde_json::to_string_pretty(&disk) {
            let _ = std::fs::write(&self.path, format!("{text}\n"));
        }
    }

    pub fn materialize(
        &mut self,
        scope: VaultScope,
        provider: VaultProviderId,
        secret_ref: &str,
        version: i64,
        api_key: &str,
        base_url: Option<&str>,
    ) -> Result<String, String> {
        let _ = scope;
        let secret_ref = secret_ref.trim();
        let api_key = api_key.trim();
        if secret_ref.is_empty() {
            return Err("secret_ref required".into());
        }
        if api_key.is_empty() {
            return Err("apiKey required for materialize".into());
        }
        if version < 0 {
            return Err("version must be non-negative".into());
        }
        let sealed_at = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        self.sealed
            .retain(|r| !(r.secret_ref == secret_ref && r.version == version));
        self.sealed.push(SealedMaterial {
            secret_ref: secret_ref.to_string(),
            version,
            provider,
            api_key: api_key.to_string(),
            base_url: normalize_base(base_url, provider),
            sealed_at: sealed_at.clone(),
        });
        self.persist();
        Ok(sealed_at)
    }

    pub fn apply_job(
        &mut self,
        scope: VaultScope,
        provider: VaultProviderId,
        secret_ref: Option<&str>,
        version: i64,
        base_url: Option<&str>,
    ) -> VaultApplyReceipt {
        let applied_at = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let receipt_id = format!(
            "rcpt_{}_{}",
            Utc::now().timestamp_millis(),
            &Uuid::new_v4().to_string()[..6]
        );
        let key = scope_key(&scope);
        let entry = self.active.entry(key.clone()).or_default();

        if secret_ref.is_none() {
            entry.remove(&provider);
            let receipt = VaultApplyReceipt {
                receipt_id,
                scope,
                provider,
                secret_ref: None,
                version,
                status: "cleared".into(),
                applied_at,
                detail: None,
            };
            self.push_receipt(receipt.clone());
            self.persist();
            return receipt;
        }

        let secret_ref = secret_ref.unwrap();
        let material = self
            .sealed
            .iter()
            .find(|r| r.secret_ref == secret_ref && r.version == version && r.provider == provider);
        if material.is_none() {
            let receipt = VaultApplyReceipt {
                receipt_id,
                scope,
                provider,
                secret_ref: Some(secret_ref.to_string()),
                version,
                status: "failed".into(),
                applied_at,
                detail: Some("secret_ref not materialized".into()),
            };
            self.push_receipt(receipt.clone());
            self.persist();
            return receipt;
        }
        let material = material.unwrap().clone();
        entry.insert(
            provider,
            ActiveBinding {
                secret_ref: material.secret_ref.clone(),
                version: material.version,
                api_key: material.api_key,
                base_url: base_url
                    .map(|b| normalize_base(Some(b), provider))
                    .unwrap_or(material.base_url),
                applied_at: applied_at.clone(),
                receipt_id: receipt_id.clone(),
            },
        );
        let receipt = VaultApplyReceipt {
            receipt_id,
            scope,
            provider,
            secret_ref: Some(material.secret_ref),
            version: material.version,
            status: "applied".into(),
            applied_at,
            detail: None,
        };
        self.push_receipt(receipt.clone());
        self.persist();
        receipt
    }

    fn push_receipt(&mut self, receipt: VaultApplyReceipt) {
        self.receipts.insert(0, receipt);
        if self.receipts.len() > RECEIPT_LIMIT {
            self.receipts.truncate(RECEIPT_LIMIT);
        }
    }

    pub fn get_credential(
        &self,
        provider: VaultProviderId,
        scope: Option<&VaultScope>,
    ) -> Option<VaultCredential> {
        let default = VaultScope::default();
        let scope = scope.unwrap_or(&default);
        let binding = self.active.get(&scope_key(scope))?.get(&provider)?;
        if binding.api_key.is_empty() {
            return None;
        }
        Some(VaultCredential {
            api_key: binding.api_key.clone(),
            base_url: binding.base_url.clone(),
            secret_ref: binding.secret_ref.clone(),
            version: binding.version,
        })
    }

    pub fn list_receipts(&self, limit: usize) -> Vec<VaultApplyReceipt> {
        self.receipts.iter().take(limit).cloned().collect()
    }

    pub fn status(&self, scope: Option<&VaultScope>) -> Value {
        let default = VaultScope::default();
        let scope = scope.unwrap_or(&default);
        let binding = self.active.get(&scope_key(scope));
        let mut providers = serde_json::Map::new();
        for provider in [
            VaultProviderId::Boyue,
            VaultProviderId::Siliconflow,
            VaultProviderId::Worldmonitor,
        ] {
            let row = binding.and_then(|b| b.get(&provider));
            let val = if let Some(row) = row {
                serde_json::json!({
                    "present": true,
                    "secret_ref": row.secret_ref,
                    "version": row.version,
                    "applied_at": row.applied_at,
                    "receipt_id": row.receipt_id,
                })
            } else {
                serde_json::json!({ "present": false })
            };
            providers.insert(provider.as_str().to_string(), val);
        }
        serde_json::json!({
            "scope": scope,
            "providers": providers,
        })
    }

    pub fn apply_upstream_secrets_patch(
        &mut self,
        body: &Value,
    ) -> Result<(bool, bool, bool), String> {
        let obj = body
            .as_object()
            .ok_or_else(|| "body must be a JSON object".to_string())?;
        let scope = VaultScope::default();
        let now = Utc::now().timestamp_millis();

        let mut handle = |provider: VaultProviderId, value: &Value| -> Result<(), String> {
            if value.is_null() {
                self.apply_job(scope.clone(), provider, None, 0, None);
                return Ok(());
            }
            let entry = value.as_object().ok_or_else(|| {
                format!(
                    "{} must be {{ apiKey, baseUrl }} or null",
                    provider.as_str()
                )
            })?;
            let api_key = entry
                .get("apiKey")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if api_key.is_empty() {
                self.apply_job(scope.clone(), provider, None, 0, None);
                return Ok(());
            }
            let secret_ref = format!("inline://{}/{now}", provider.as_str());
            let version = 1i64;
            let base = entry.get("baseUrl").and_then(|v| v.as_str());
            self.materialize(scope.clone(), provider, &secret_ref, version, api_key, base)?;
            self.apply_job(scope.clone(), provider, Some(&secret_ref), version, base);
            Ok(())
        };

        if let Some(v) = obj.get("boyue") {
            handle(VaultProviderId::Boyue, v)?;
        }
        if let Some(v) = obj.get("siliconflow") {
            handle(VaultProviderId::Siliconflow, v)?;
        }
        if let Some(v) = obj.get("worldmonitor") {
            handle(VaultProviderId::Worldmonitor, v)?;
        }

        Ok((
            self.get_credential(VaultProviderId::Boyue, None).is_some(),
            self.get_credential(VaultProviderId::Siliconflow, None)
                .is_some(),
            self.get_credential(VaultProviderId::Worldmonitor, None)
                .is_some(),
        ))
    }
}

pub fn is_loopback_host(host: &str) -> bool {
    let host = host.split(':').next().unwrap_or(host);
    host == "127.0.0.1" || host == "localhost" || host == "::1" || host == "[::1]"
}
