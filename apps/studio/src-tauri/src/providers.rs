//! Provider setup reached from the login shell.
//! The catalog is the shipped provider registry. Saved keys and models live
//! beside the tenant bundle. Tests keep that store in memory.
//! Model sync uses curl's config stdin so the API key is not a process argument.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::Deserialize;
use serde_json::{Value, json};

#[cfg(test)]
#[path = "providers_default_alignment_tests.rs"]
mod default_alignment_tests;

const CATALOG: &str = include_str!("../../packages/provider-registry/data/providers.json");

#[derive(Clone)]
struct CatalogProvider {
    id: String,
    name: String,
    base_url: String,
    api_key_url: String,
    auth_optional: bool,
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
struct SavedModel {
    id: String,
    name: String,
    enabled: bool,
    chat: bool,
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
struct SavedProvider {
    enabled: bool,
    #[serde(default, rename = "apiKey")]
    api_key: String,
    #[serde(default, rename = "baseUrl")]
    base_url: String,
    #[serde(default)]
    models: Vec<SavedModel>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct ModelRef {
    #[serde(rename = "providerId")]
    provider_id: String,
    #[serde(rename = "modelId")]
    model_id: String,
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
struct ModelDefaults {
    #[serde(default, rename = "defaultModel")]
    default_model: Option<ModelRef>,
    #[serde(default, rename = "quickModel")]
    quick_model: Option<ModelRef>,
    #[serde(default, rename = "translateModel")]
    translate_model: Option<ModelRef>,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Disk {
    #[serde(default)]
    providers: HashMap<String, SavedProvider>,
    #[serde(default)]
    defaults: ModelDefaults,
}

#[derive(Deserialize)]
struct UpdateBody {
    enabled: Option<bool>,
    #[serde(default, rename = "apiKey")]
    api_key: Option<String>,
    #[serde(default, rename = "baseUrl")]
    base_url: Option<String>,
}

#[derive(Deserialize)]
struct ModelBody {
    id: String,
    name: Option<String>,
    enabled: Option<bool>,
}

#[derive(Deserialize)]
struct DefaultsBody {
    #[serde(rename = "defaultModel")]
    default_model: ModelRef,
    #[serde(rename = "quickModel")]
    quick_model: ModelRef,
    #[serde(rename = "translateModel")]
    translate_model: ModelRef,
}

#[derive(Debug)]
enum Fail {
    Bad(&'static str),
    Missing,
    Sync(&'static str),
}

impl Fail {
    fn into_response(self) -> (u16, &'static str, &'static str, Vec<u8>) {
        let (status, reason, code) = match self {
            Self::Bad(code) => (400, "Bad Request", code),
            Self::Missing => (404, "Not Found", "not_found"),
            Self::Sync(code) => (502, "Bad Gateway", code),
        };
        (
            status,
            reason,
            "application/json",
            format!("{{\"error\":\"{code}\"}}").into_bytes(),
        )
    }
}

/// Endpoint and key an agent runtime needs to call a provider directly.
pub struct ProviderCredentials {
    pub base_url: String,
    pub api_key: String,
}

pub struct ProviderStore {
    path: Option<PathBuf>,
    catalog: Vec<CatalogProvider>,
    saved: HashMap<String, SavedProvider>,
    defaults: ModelDefaults,
}

impl ProviderStore {
    pub fn open(slug: &str) -> Self {
        Self::open_at(store_path(slug))
    }

    fn open_at(path: Option<PathBuf>) -> Self {
        let disk = path
            .as_ref()
            .and_then(|path| fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<Disk>(&bytes).ok())
            .unwrap_or_default();
        Self {
            path,
            catalog: parse_catalog(),
            saved: disk.providers,
            defaults: disk.defaults,
        }
    }

    pub fn list(&self) -> String {
        let providers: Vec<_> = self
            .catalog
            .iter()
            .map(|item| {
                let saved = self.saved.get(&item.id);
                let enabled = saved.is_some_and(|item| item.enabled);
                let chat_models = saved
                    .filter(|item| item.enabled)
                    .map(chat_model_summaries)
                    .unwrap_or_default();
                json!({
                    "id": item.id,
                    "name": item.name,
                    "baseUrl": effective_base(item, saved),
                    "apiKeyUrl": item.api_key_url,
                    "authOptional": item.auth_optional,
                    "enabled": enabled,
                    "hasApiKey": saved.is_some_and(|item| !item.api_key.is_empty()),
                    "enabledChatModels": chat_models.len(),
                    "chatModels": chat_models,
                })
            })
            .collect();
        json!({ "providers": providers, "defaults": self.defaults }).to_string()
    }

    fn detail(&self, id: &str) -> Result<String, Fail> {
        let item = self.catalog_item(id)?;
        Ok(detail_json(item, self.saved.get(id)).to_string())
    }

    fn update(&mut self, id: &str, body: &[u8]) -> Result<String, Fail> {
        let item = self.catalog_item(id)?.clone();
        let patch: UpdateBody =
            serde_json::from_slice(body).map_err(|_| Fail::Bad("bad_request"))?;
        if patch.api_key.is_some() || patch.base_url.is_some() {
            return Err(Fail::Bad("managed_gateway"));
        }
        let saved = self.saved.entry(item.id.clone()).or_default();
        if let Some(enabled) = patch.enabled {
            saved.enabled = enabled;
        }
        self.persist()?;
        Ok(detail_json(&item, self.saved.get(&item.id)).to_string())
    }

    fn upsert_model(&mut self, id: &str, body: &[u8]) -> Result<String, Fail> {
        let item = self.catalog_item(id)?.clone();
        let patch: ModelBody =
            serde_json::from_slice(body).map_err(|_| Fail::Bad("bad_request"))?;
        let model_id = patch.id.trim();
        if !valid_model_id(model_id) {
            return Err(Fail::Bad("bad_request"));
        }
        let model_id = model_id.to_owned();
        let name = match patch
            .name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
        {
            Some(name) if valid_model_name(name) => name.to_owned(),
            Some(_) => return Err(Fail::Bad("bad_request")),
            None => model_id.clone(),
        };
        let saved = self.saved.entry(item.id.clone()).or_default();
        if let Some(existing) = saved.models.iter_mut().find(|model| model.id == model_id) {
            if patch
                .name
                .as_ref()
                .is_some_and(|value| !value.trim().is_empty())
            {
                existing.name = name;
            }
            if let Some(enabled) = patch.enabled {
                existing.enabled = enabled;
            }
            existing.chat = counts_as_chat(&existing.id);
        } else {
            saved.models.push(SavedModel {
                chat: counts_as_chat(&model_id),
                enabled: patch.enabled.unwrap_or(true),
                id: model_id,
                name,
            });
        }
        self.persist()?;
        Ok(detail_json(&item, self.saved.get(&item.id)).to_string())
    }

    fn sync(&mut self, id: &str) -> Result<String, Fail> {
        let item = self.catalog_item(id)?.clone();
        let saved = self.saved.get(&item.id).cloned().unwrap_or_default();
        let base = if saved.base_url.is_empty() {
            item.base_url.clone()
        } else {
            saved.base_url.clone()
        };
        if !valid_base_url(&base) {
            return Err(Fail::Bad("missing_base_url"));
        }
        if saved.api_key.is_empty() && !item.auth_optional {
            return Err(Fail::Bad("missing_api_key"));
        }
        let ids = fetch_model_ids(&models_url(&base), &saved.api_key)?;
        let saved = self.saved.entry(item.id.clone()).or_default();
        for model_id in ids {
            if !valid_model_id(&model_id) || saved.models.iter().any(|model| model.id == model_id) {
                continue;
            }
            saved.models.push(SavedModel {
                name: model_id.clone(),
                chat: counts_as_chat(&model_id),
                enabled: true,
                id: model_id,
            });
        }
        self.persist()?;
        Ok(detail_json(&item, self.saved.get(&item.id)).to_string())
    }

    fn set_defaults(&mut self, body: &[u8]) -> Result<String, Fail> {
        let patch: DefaultsBody =
            serde_json::from_slice(body).map_err(|_| Fail::Bad("bad_request"))?;
        if !self.eligible(&patch.default_model)
            || !self.eligible(&patch.quick_model)
            || !self.eligible(&patch.translate_model)
        {
            return Err(Fail::Bad("missing_model"));
        }
        let updates = [
            ("chat.default_model_id", &patch.default_model),
            ("feature.quick_assistant.model_id", &patch.quick_model),
            ("feature.translate.model_id", &patch.translate_model),
        ]
        .into_iter()
        .map(|(key, model)| {
            (
                key.to_owned(),
                json!(format!("{}::{}", model.provider_id, model.model_id)),
            )
        })
        .collect();
        let previous = std::mem::replace(
            &mut self.defaults,
            ModelDefaults {
                default_model: Some(patch.default_model),
                quick_model: Some(patch.quick_model),
                translate_model: Some(patch.translate_model),
            },
        );
        if let Err(error) = self.persist() {
            self.defaults = previous;
            return Err(error);
        }
        // The composer reads all three device defaults, including after restart.
        // shortcut: separate stores are not crash-atomic; unify storage if crash recovery is required.
        if let Some(root) = self.path.as_ref().and_then(|path| path.parent()) {
            if crate::preferences::PreferenceStore::new(root.join("preferences"))
                .set_device(updates)
                .is_err()
            {
                self.defaults = previous;
                self.persist()?;
                return Err(Fail::Bad("preferences_unavailable"));
            }
        }
        Ok(json!(self.defaults).to_string())
    }

    fn catalog_item(&self, id: &str) -> Result<&CatalogProvider, Fail> {
        self.catalog
            .iter()
            .find(|item| item.id == id)
            .ok_or(Fail::Missing)
    }

    fn eligible(&self, model: &ModelRef) -> bool {
        if is_managed(&model.provider_id) {
            return false;
        }
        self.saved.get(&model.provider_id).is_some_and(|saved| {
            saved.enabled
                && saved
                    .models
                    .iter()
                    .any(|item| item.id == model.model_id && item.enabled && item.chat)
        })
    }

    fn persist(&self) -> Result<(), Fail> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|_| Fail::Bad("bad_request"))?;
        }
        let body = serde_json::to_vec(&Disk {
            providers: self.saved.clone(),
            defaults: self.defaults.clone(),
        })
        .map_err(|_| Fail::Bad("bad_request"))?;
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, body).map_err(|_| Fail::Bad("bad_request"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600));
        }
        fs::rename(&temporary, path).map_err(|_| Fail::Bad("bad_request"))?;
        Ok(())
    }
}

/// Local data-plane key accepted by the loopback runtime and gateway.
/// Same value as `apps/runtime` `DEV_CAPABILITY_KEY`. It never leaves this process.
const LOCAL_DATA_PLANE_KEY: &str = "a3s_sk_devlocalgateway00000000000001";

pub(crate) fn gateway_origin() -> String {
    std::env::var("A3S_GATEWAY_ORIGIN")
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_owned())
        .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        .unwrap_or_else(|| "http://127.0.0.1:8080".to_owned())
}

/// Credentials a3s-code uses for the signed-in gateway. The token is the member
/// session, and the base is the gateway's OpenAI-compatible root.
/// Origin and bearer the ACP child uses for `POST /v1/systemone`.
/// The member session is preferred. The loopback data-plane key is the
/// fallback when nobody is signed in.
pub(crate) fn gateway_decision_env() -> (String, Option<String>) {
    let origin = gateway_origin();
    let Some(scope) = signed_in_session_slug() else {
        return (origin, None);
    };
    let member = crate::member_session::MemberSessionStore::open(&scope).token();
    let key = member.filter(|token| valid_api_key(token)).or_else(|| {
        let configured = std::env::var("A3S_GATEWAY_KEY").ok();
        resolve_service_key(&origin, configured.as_deref())
    });
    (origin, key)
}

pub fn gateway_credentials(token: &str) -> ProviderCredentials {
    ProviderCredentials {
        base_url: format!("{}/v1", gateway_origin()),
        api_key: token.to_owned(),
    }
}

/// `a3s` is the only provider a signed-in client may call. Every other id
/// is refused so a saved host or key cannot leave the machine.
pub fn signed_in_gateway_credentials(
    provider_id: &str,
    token: Option<&str>,
) -> Option<ProviderCredentials> {
    if provider_id != "a3s" {
        return None;
    }
    let token = token.filter(|token| !token.is_empty())?;
    Some(gateway_credentials(token))
}

fn loopback_origin(origin: &str) -> bool {
    let Some(rest) = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
    else {
        return false;
    };
    let host = rest
        .split('/')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("");
    host == "127.0.0.1" || host == "localhost" || host == "[::1]"
}

fn resolve_service_key(origin: &str, configured: Option<&str>) -> Option<String> {
    if let Some(key) = configured.map(str::trim).filter(|key| valid_api_key(key)) {
        return Some(key.to_owned());
    }
    if loopback_origin(origin) {
        return Some(LOCAL_DATA_PLANE_KEY.to_owned());
    }
    None
}

fn service_model_ids(body: &[u8]) -> Option<Vec<String>> {
    if let Some(ids) = parse_model_ids(body) {
        return Some(ids);
    }
    let value: Value = serde_json::from_slice(body).ok()?;
    let data = value.get("data")?;
    if !data.is_object() {
        return None;
    }
    let bytes = serde_json::to_vec(data).ok()?;
    parse_model_ids(&bytes)
}

fn fetch_service_models() -> Vec<String> {
    let origin = gateway_origin();
    let configured = std::env::var("A3S_GATEWAY_KEY").ok();
    let Some(key) = resolve_service_key(&origin, configured.as_deref()) else {
        return Vec::new();
    };
    let url = format!("{origin}/v1/models");
    let Some(body) = curl_get(&url, &key, 20) else {
        return Vec::new();
    };
    service_model_ids(&body).unwrap_or_default()
}

/// First chat model on the signed-in gateway, as `a3s::{id}`.
///
/// The member session is the credential the gateway accepts. The loopback
/// data-plane key is only a fallback when nobody is signed in.
pub(crate) fn first_signed_in_model() -> Option<String> {
    let origin = gateway_origin();
    let scope = signed_in_session_slug()?;
    let member = crate::member_session::MemberSessionStore::open(&scope).token();
    let key = member.filter(|token| valid_api_key(token)).or_else(|| {
        let configured = std::env::var("A3S_GATEWAY_KEY").ok();
        resolve_service_key(&origin, configured.as_deref())
    })?;
    chat_model_for_key(&key, 2)
}

/// First chat model the gateway lists for this key, as `a3s::{id}`.
///
/// Library tests keep `MemberSessionStore::open` in memory, so a live task
/// row passes the signed-in session key here instead.
pub(crate) fn first_chat_model_for_key(key: &str) -> Option<String> {
    chat_model_for_key(key, 15)
}

fn chat_model_for_key(key: &str, timeout_secs: u64) -> Option<String> {
    let url = format!("{}/v1/models", gateway_origin());
    let body = curl_get(&url, key, timeout_secs)?;
    service_model_ids(&body)?
        .iter()
        .find_map(|id| admit_chat_model(id))
}

fn admit_chat_model(id: &str) -> Option<String> {
    if counts_as_chat(id)
        && !id.is_empty()
        && !id.contains(':')
        && !id.contains('/')
        && !id.contains(' ')
    {
        Some(format!("a3s::{id}"))
    } else {
        None
    }
}

fn curl_get(url: &str, api_key: &str, max_time: u64) -> Option<Vec<u8>> {
    let mut child = std::process::Command::new("curl")
        .arg("--config")
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    {
        let Some(stdin) = child.stdin.as_mut() else {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        };
        if std::io::Write::write_all(
            stdin,
            curl_config_timeout(url, api_key, max_time).as_bytes(),
        )
        .is_err()
        {
            drop(child.stdin.take());
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
    }
    let output = child.wait_with_output().ok()?;
    output.status.success().then_some(output.stdout)
}

fn service_models_json(ids: Vec<String>) -> String {
    let models: Vec<_> = ids
        .into_iter()
        .map(|id| json!({ "id": id, "name": id, "enabled": true }))
        .collect();
    json!({ "models": models }).to_string()
}

pub fn route(
    method: &str,
    parts: &[&str],
    body: &[u8],
    store: &Mutex<ProviderStore>,
) -> Option<(u16, &'static str, &'static str, Vec<u8>)> {
    let result = match (method, parts) {
        ("GET", ["api", "model-service", "models"]) => {
            Ok(service_models_json(fetch_service_models()))
        }
        ("GET", ["api", "providers"]) => Ok(store.lock().expect("providers").list()),
        ("GET", ["api", "providers", id]) if is_provider_id(id) => {
            store.lock().expect("providers").detail(id)
        }
        ("POST", ["api", "providers", "defaults"]) => {
            store.lock().expect("providers").set_defaults(body)
        }
        ("POST", ["api", "providers", id]) if is_provider_id(id) => {
            store.lock().expect("providers").update(id, body)
        }
        ("POST", ["api", "providers", id, "models"]) if is_provider_id(id) => {
            store.lock().expect("providers").upsert_model(id, body)
        }
        ("POST", ["api", "providers", id, "sync"]) if is_provider_id(id) => {
            store.lock().expect("providers").sync(id)
        }
        _ => return None,
    };
    Some(match result {
        Ok(text) => (200, "OK", "application/json", text.into_bytes()),
        Err(fail) => fail.into_response(),
    })
}

fn store_path(slug: &str) -> Option<PathBuf> {
    Some(tenant_data_dir(slug)?.join("providers.json"))
}

/// Profile directory for `slug`. `None` under test.
pub fn tenant_data_dir(slug: &str) -> Option<PathBuf> {
    // Library tests call the live listener. Keeping them in memory avoids
    // writing the tenant file under Application Support.
    if cfg!(test) {
        return None;
    }
    if slug.is_empty()
        || slug.len() > 64
        || !slug
            .bytes()
            .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-'))
    {
        return None;
    }
    crate::tenant::profile_dir(slug)
}

fn parse_catalog() -> Vec<CatalogProvider> {
    let value: Value = serde_json::from_str(CATALOG).expect("provider catalog");
    let Some(list) = value.get("providers").and_then(Value::as_array) else {
        return Vec::new();
    };
    list.iter().filter_map(read_catalog_provider).collect()
}

fn read_catalog_provider(value: &Value) -> Option<CatalogProvider> {
    let id = value.get("id")?.as_str()?.to_owned();
    if !is_provider_id(&id) {
        return None;
    }
    let name = value.get("name")?.as_str()?.to_owned();
    let endpoints = value.get("endpointConfigs").and_then(Value::as_object);
    let default_endpoint = value
        .get("defaultChatEndpoint")
        .and_then(Value::as_str)
        .unwrap_or("");
    let base_url = endpoint_base(endpoints, default_endpoint)
        .or_else(|| {
            endpoints.and_then(|endpoints| {
                endpoints
                    .values()
                    .find_map(|item| item.get("baseUrl").and_then(Value::as_str))
                    .and_then(|url| normalize_base_url(url))
            })
        })
        .unwrap_or_default();
    let api_key_url = value
        .pointer("/metadata/website/apiKey")
        .and_then(Value::as_str)
        .filter(|url| url.starts_with("https://"))
        .unwrap_or("")
        .to_owned();
    let auth_optional = value
        .get("authOptional")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Some(CatalogProvider {
        id,
        name,
        base_url,
        api_key_url,
        auth_optional,
    })
}

fn endpoint_base(endpoints: Option<&serde_json::Map<String, Value>>, name: &str) -> Option<String> {
    let endpoints = endpoints?;
    if name.is_empty() {
        return None;
    }
    normalize_base_url(endpoints.get(name)?.get("baseUrl")?.as_str()?)
}

fn normalize_base_url(url: &str) -> Option<String> {
    valid_base_url(url).then(|| url.trim().trim_end_matches('/').to_owned())
}

fn effective_base(item: &CatalogProvider, saved: Option<&SavedProvider>) -> String {
    saved
        .and_then(|saved| {
            if saved.base_url.is_empty() {
                None
            } else {
                Some(saved.base_url.clone())
            }
        })
        .unwrap_or_else(|| item.base_url.clone())
}

fn detail_json(item: &CatalogProvider, saved: Option<&SavedProvider>) -> Value {
    let empty = SavedProvider::default();
    let saved = saved.unwrap_or(&empty);
    json!({
        "id": item.id,
        "name": item.name,
        "baseUrl": if saved.base_url.is_empty() { item.base_url.clone() } else { saved.base_url.clone() },
        "catalogBaseUrl": item.base_url,
        "apiKeyUrl": item.api_key_url,
        "authOptional": item.auth_optional,
        "enabled": saved.enabled,
        "apiKey": saved.api_key,
        "models": saved.models,
    })
}

fn chat_model_summaries(saved: &SavedProvider) -> Vec<Value> {
    saved
        .models
        .iter()
        .filter(|model| model.enabled && model.chat)
        .map(|model| json!({ "id": model.id, "name": model.name }))
        .collect()
}

fn is_provider_id(value: &str) -> bool {
    let mut bytes = value.bytes();
    match bytes.next() {
        Some(b'a'..=b'z' | b'0'..=b'9') => {}
        _ => return false,
    }
    value.len() <= 64 && bytes.all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'-'))
}

fn is_managed(id: &str) -> bool {
    id == "cherryai" || id == "cherryai-subscription"
}

fn valid_base_url(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 2048
        || value.contains('@')
        || value.contains('"')
        || value.contains('\\')
        || value.chars().any(char::is_whitespace)
    {
        return false;
    }
    let Some(rest) = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
    else {
        return false;
    };
    !rest.is_empty() && !rest.starts_with('/')
}

/// Member file for this process. A fork instance does not share the primary window's session.
fn signed_in_session_slug() -> Option<String> {
    session_scope_from(
        crate::tenant::selected_slug(),
        crate::tenant::instance_name(),
    )
}

fn session_scope_from(
    brand: std::io::Result<String>,
    instance: std::io::Result<Option<String>>,
) -> Option<String> {
    let brand = brand.ok()?;
    let instance = instance.ok()?;
    session_slug_for(&brand, instance.as_deref())
}

fn session_slug_for(brand: &str, instance: Option<&str>) -> Option<String> {
    crate::tenant::storage_slug_for(brand, instance).ok()
}

fn valid_api_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 4096
        && value
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && byte != b'"' && byte != b'\\')
}

fn valid_model_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && !value.contains("..")
        && !value.starts_with('/')
        && value.bytes().all(|byte| {
            matches!(
                byte,
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'-' | b':' | b'/' | b'@'
            )
        })
}

fn valid_model_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && !value.chars().any(|character| character.is_control())
}

fn counts_as_chat(id: &str) -> bool {
    let id = id.to_ascii_lowercase();
    const BLOCKED: &[&str] = &[
        "embed",
        "tts",
        "whisper",
        "dall-e",
        "dalle",
        "rerank",
        "moderation",
        "transcri",
        "speech",
    ];
    !BLOCKED.iter().any(|part| id.contains(part))
}

fn models_url(base: &str) -> String {
    let base = base.trim().trim_end_matches('/');
    if base.ends_with("/v1") || base.ends_with("/v3") {
        format!("{base}/models")
    } else {
        format!("{base}/v1/models")
    }
}

fn fetch_model_ids(url: &str, api_key: &str) -> Result<Vec<String>, Fail> {
    let mut child = std::process::Command::new("curl")
        .arg("--config")
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|_| Fail::Sync("sync_unavailable"))?;
    {
        let stdin = child.stdin.as_mut().ok_or(Fail::Sync("sync_unavailable"))?;
        std::io::Write::write_all(stdin, curl_config(url, api_key).as_bytes())
            .map_err(|_| Fail::Sync("sync_unavailable"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|_| Fail::Sync("sync_failed"))?;
    if !output.status.success() {
        return Err(Fail::Sync("sync_failed"));
    }
    parse_model_ids(&output.stdout).ok_or(Fail::Sync("sync_failed"))
}

fn curl_config(url: &str, api_key: &str) -> String {
    curl_config_timeout(url, api_key, 20)
}

fn curl_config_timeout(url: &str, api_key: &str, max_time: u64) -> String {
    let mut config = format!(
        "silent\nshow-error\nfail\nmax-time = {max_time}\nheader = \"Accept: application/json\"\nurl = \"{url}\"\n"
    );
    if !api_key.is_empty() {
        config.push_str(&format!("header = \"Authorization: Bearer {api_key}\"\n"));
    }
    config
}

fn parse_model_ids(body: &[u8]) -> Option<Vec<String>> {
    let value: Value = serde_json::from_slice(body).ok()?;
    let list = value
        .get("data")
        .and_then(Value::as_array)
        .or_else(|| value.get("models").and_then(Value::as_array))?;
    let mut ids = Vec::new();
    for item in list {
        let Some(id) = item
            .get("id")
            .and_then(Value::as_str)
            .or_else(|| item.get("name").and_then(Value::as_str))
        else {
            continue;
        };
        if !id.is_empty() {
            ids.push(id.to_owned());
        }
    }
    Some(ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_includes_openai_and_ollama_can_skip_a_key() {
        let store = ProviderStore::open_at(None);
        let list: Value = serde_json::from_str(&store.list()).unwrap();
        let providers = list["providers"].as_array().unwrap();
        let openai = providers
            .iter()
            .find(|item| item["id"] == "openai")
            .unwrap();
        assert_eq!(openai["name"], "OpenAI");
        assert_eq!(openai["baseUrl"], "https://api.openai.com");
        assert_eq!(openai["enabled"], false);
        assert!(openai.get("apiKey").is_none());
        let ollama = providers
            .iter()
            .find(|item| item["id"] == "ollama")
            .unwrap();
        assert_eq!(ollama["authOptional"], true);
        assert!(
            ollama["baseUrl"]
                .as_str()
                .unwrap()
                .starts_with("http://localhost:11434")
        );
    }

    #[test]
    fn embedding_models_do_not_satisfy_the_chat_gate() {
        let mut store = ProviderStore::open_at(None);
        let detail = store
            .upsert_model("openai", br#"{"id":"text-embedding-3-small"}"#)
            .unwrap();
        let json: Value = serde_json::from_str(&detail).unwrap();
        assert_eq!(json["models"][0]["chat"], false);
        store.update("openai", br#"{"enabled":true}"#).unwrap();
        store
            .upsert_model("openai", br#"{"id":"gpt-4o","name":"GPT-4o"}"#)
            .unwrap();
        let list: Value = serde_json::from_str(&store.list()).unwrap();
        let openai = list["providers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["id"] == "openai")
            .unwrap();
        assert_eq!(openai["enabledChatModels"], 1);
        assert_eq!(openai["chatModels"][0]["id"], "gpt-4o");
        assert!(!store.list().contains("sk-test"));
        let saved = store
            .set_defaults(
                br#"{"defaultModel":{"providerId":"openai","modelId":"gpt-4o"},"quickModel":{"providerId":"openai","modelId":"gpt-4o"},"translateModel":{"providerId":"openai","modelId":"gpt-4o"}}"#,
            )
            .unwrap();
        assert!(saved.contains("gpt-4o"));
        let rejected = store.set_defaults(
            br#"{"defaultModel":{"providerId":"openai","modelId":"text-embedding-3-small"},"quickModel":{"providerId":"openai","modelId":"gpt-4o"},"translateModel":{"providerId":"openai","modelId":"gpt-4o"}}"#,
        );
        assert!(rejected.is_err());
    }

    #[test]
    fn model_list_urls_follow_the_catalog_base() {
        assert_eq!(
            models_url("https://api.openai.com"),
            "https://api.openai.com/v1/models"
        );
        assert_eq!(
            models_url("https://api.siliconflow.cn/v1"),
            "https://api.siliconflow.cn/v1/models"
        );
        assert_eq!(
            models_url("http://localhost:8000/v3/"),
            "http://localhost:8000/v3/models"
        );
        let parsed = parse_model_ids(br#"{"data":[{"id":"gpt-4o"},{"id":""}]}"#).unwrap();
        assert_eq!(parsed, vec!["gpt-4o".to_owned()]);
        let named = parse_model_ids(br#"{"models":[{"name":"llama3"}]}"#).unwrap();
        assert_eq!(named, vec!["llama3".to_owned()]);
    }

    #[test]
    fn reopens_a_saved_key_from_disk() {
        let directory = std::env::temp_dir().join(format!(
            "a3s-studio-providers-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("providers.json");
        {
            let mut store = ProviderStore::open_at(Some(path.clone()));
            let rejected = store.update(
                "deepseek",
                br#"{"enabled":true,"apiKey":"sk-disk-secret","baseUrl":"https://api.deepseek.com"}"#,
            );
            assert!(rejected.is_err());
        }
        let store = ProviderStore::open_at(Some(path));
        let detail = store.detail("deepseek").unwrap();
        assert!(!detail.contains("sk-disk-secret"));
        assert!(detail.contains("\"enabled\":false"));
        assert!(!store.list().contains("sk-disk-secret"));
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn fork_instance_session_is_not_the_primary_slug() {
        assert_eq!(
            session_slug_for("towork-cua", Some("fork1")),
            Some("towork-cua-fork1".to_owned())
        );
        assert_eq!(
            session_slug_for("towork-cua", None),
            Some("towork-cua".to_owned())
        );
    }

    #[test]
    fn signed_in_calls_use_the_gateway_and_the_member_session() {
        let token = "a3s_ms_member.session.token";
        let credentials = signed_in_gateway_credentials("a3s", Some(token)).unwrap();
        assert_eq!(credentials.base_url, format!("{}/v1", gateway_origin()));
        assert_eq!(credentials.api_key, token);
        assert!(signed_in_gateway_credentials("openai", Some(token)).is_none());
        assert!(signed_in_gateway_credentials("a3s", None).is_none());
        assert!(signed_in_gateway_credentials("a3s", Some("")).is_none());
    }

    #[test]
    fn service_models_unwrap_the_gateway_envelope() {
        let body = br#"{"success":true,"code":0,"data":{"object":"list","data":[{"id":"boyue.openai.chat","object":"model"},{"id":"siliconflow.openai.chat"}]}}"#;
        assert_eq!(
            service_model_ids(body).unwrap(),
            vec![
                "boyue.openai.chat".to_owned(),
                "siliconflow.openai.chat".to_owned()
            ]
        );
        let openai = br#"{"object":"list","data":[{"id":"deepseek-v4-flash"}]}"#;
        assert_eq!(
            service_model_ids(openai).unwrap(),
            vec!["deepseek-v4-flash".to_owned()]
        );
    }

    #[test]
    fn service_key_stays_on_the_loopback_gateway() {
        assert!(resolve_service_key("http://127.0.0.1:8080", None).is_some());
        assert!(resolve_service_key("http://localhost:8080", None).is_some());
        assert!(resolve_service_key("https://gateway.example", None).is_none());
        assert_eq!(
            resolve_service_key("https://gateway.example", Some("a3s_sk_configured")),
            Some("a3s_sk_configured".to_owned())
        );
        let listed = service_models_json(vec!["boyue.openai.chat".to_owned()]);
        assert!(listed.contains("boyue.openai.chat"));
        assert!(!listed.contains("apiKey"));
    }
}
