//! Applet products for the studio shell.
//!
//! A product follows the WeChat mini program release line: creating the
//! product, building an immutable code package (开发版), pinning that build as
//! the trial (体验版) or the release (线上版), and serving the release bytes.
//! Rolling back republishes an older build. The package digest is SHA-256 of
//! the exact JSON bytes the client caches.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const OFFICE: &[(&str, &str)] = &[
    ("approval", "审批"),
    ("leave", "请假"),
    ("travel", "出差"),
    ("expense", "报销"),
    ("meeting", "会议"),
    ("seal", "用印"),
    ("notice", "公告"),
    ("probation", "转正"),
    ("clock", "打卡"),
    ("correction", "补卡"),
    ("ai-resource", "AI 资源"),
];

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Build {
    version: String,
    digest: String,
    package: String,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Product {
    app_id: String,
    name: String,
    description: String,
    builds: Vec<Build>,
    trial: Option<String>,
    release: Option<String>,
}

#[derive(Deserialize)]
struct CreateBody {
    #[serde(rename = "appId")]
    app_id: String,
    name: String,
    #[serde(default)]
    description: String,
}

#[derive(Deserialize)]
struct FieldBody {
    name: String,
    kind: String,
    label: String,
    #[serde(default)]
    required: bool,
    max: u32,
}

#[derive(Clone)]
struct Field {
    name: String,
    kind: String,
    label: String,
    required: bool,
    max: u32,
}

#[derive(Deserialize)]
struct BuildBody {
    version: String,
    #[serde(rename = "navigationBarTitleText", default)]
    title: String,
    #[serde(default)]
    pages: Vec<String>,
    #[serde(default)]
    entry: String,
    #[serde(default)]
    fields: Vec<FieldBody>,
}

#[derive(Deserialize)]
struct PublishBody {
    version: String,
    channel: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct FileBody {
    products: Vec<Product>,
}

pub struct AppletProductStore {
    path: Option<PathBuf>,
    products: Mutex<Vec<Product>>,
}

impl AppletProductStore {
    pub fn open(slug: &str) -> Self {
        Self::open_at(products_path(slug))
    }

    pub fn open_at(path: Option<PathBuf>) -> Self {
        let mut products = path
            .as_ref()
            .and_then(|path| fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<FileBody>(&bytes).ok())
            .map(|file| file.products)
            .unwrap_or_default();
        let changed = ensure_office(&mut products);
        let store = Self {
            path,
            products: Mutex::new(products),
        };
        if changed {
            let _ = store.persist();
        }
        store
    }

    pub fn create(&self, body: &[u8]) -> Result<String, u16> {
        let parsed: CreateBody = serde_json::from_slice(body).map_err(|_| 400u16)?;
        let app_id = clean_app_id(&parsed.app_id)?;
        let name = clean_label(&parsed.name, 40)?;
        if name.is_empty() {
            return Err(400);
        }
        let description = clean_label(&parsed.description, 200)?;
        let created = app_id.clone();
        self.commit(|products| {
            if products.iter().any(|product| product.app_id == app_id) {
                return Err(409);
            }
            products.insert(
                0,
                Product {
                    app_id,
                    name,
                    description,
                    builds: Vec::new(),
                    trial: None,
                    release: None,
                },
            );
            Ok(())
        })?;
        self.metadata(&created)
    }

    pub fn build(&self, app_id: &str, body: &[u8]) -> Result<String, u16> {
        let app_id = clean_app_id(app_id)?;
        let parsed: BuildBody = serde_json::from_slice(body).map_err(|_| 400u16)?;
        let version = clean_version(&parsed.version)?;
        let pages = clean_pages(&parsed.pages)?;
        let entry = if parsed.entry.trim().is_empty() {
            pages[0].clone()
        } else {
            let entry = clean_page(&parsed.entry)?;
            if !pages.iter().any(|page| page == &entry) {
                return Err(400);
            }
            entry
        };
        self.commit(|products| {
            let product = products
                .iter_mut()
                .find(|product| product.app_id == app_id)
                .ok_or(404u16)?;
            if product.builds.iter().any(|build| build.version == version) {
                return Err(409);
            }
            let title = if parsed.title.trim().is_empty() {
                product.name.clone()
            } else {
                clean_label(&parsed.title, 40)?
            };
            if title.is_empty() {
                return Err(400);
            }
            let fields = clean_fields(parsed.fields)?;
            let package = render_package(&app_id, &version, &title, &pages, &entry, &fields);
            let digest = sha256_hex(package.as_bytes());
            product.builds.insert(
                0,
                Build {
                    version,
                    digest,
                    package,
                },
            );
            Ok(())
        })?;
        self.metadata(&app_id)
    }

    pub fn publish(&self, app_id: &str, body: &[u8]) -> Result<String, u16> {
        let app_id = clean_app_id(app_id)?;
        let parsed: PublishBody = serde_json::from_slice(body).map_err(|_| 400u16)?;
        let version = clean_version(&parsed.version)?;
        let channel = parsed.channel.as_str();
        if channel != "trial" && channel != "release" {
            return Err(400);
        }
        self.commit(|products| {
            let product = products
                .iter_mut()
                .find(|product| product.app_id == app_id)
                .ok_or(404u16)?;
            if !product.builds.iter().any(|build| build.version == version) {
                return Err(404);
            }
            match channel {
                "trial" => product.trial = Some(version),
                "release" => product.release = Some(version),
                _ => return Err(400),
            }
            Ok(())
        })?;
        self.metadata(&app_id)
    }

    pub fn metadata(&self, app_id: &str) -> Result<String, u16> {
        let app_id = clean_app_id(app_id)?;
        let products = self.products.lock().expect("applet products");
        let product = products
            .iter()
            .find(|product| product.app_id == app_id)
            .ok_or(404u16)?;
        Ok(metadata_json(product))
    }

    /// Exact package bytes for one channel. `development` is the newest build.
    pub fn package(&self, app_id: &str, channel: &str) -> Result<String, u16> {
        let app_id = clean_app_id(app_id)?;
        if !matches!(channel, "development" | "trial" | "release") {
            return Err(404);
        }
        let products = self.products.lock().expect("applet products");
        let product = products
            .iter()
            .find(|product| product.app_id == app_id)
            .ok_or(404u16)?;
        let version = match channel {
            "development" => product.builds.first().map(|build| build.version.as_str()),
            "trial" => product.trial.as_deref(),
            "release" => product.release.as_deref(),
            _ => None,
        };
        let version = version.ok_or(404u16)?;
        product
            .builds
            .iter()
            .find(|build| build.version == version)
            .map(|build| build.package.clone())
            .ok_or(404)
    }

    /// Exact bytes of one immutable build. The caller checks the digest.
    pub fn package_for_version(&self, app_id: &str, version: &str) -> Result<String, u16> {
        let app_id = clean_app_id(app_id)?;
        let version = clean_version(version)?;
        let products = self.products.lock().expect("applet products");
        let product = products
            .iter()
            .find(|product| product.app_id == app_id)
            .ok_or(404u16)?;
        product
            .builds
            .iter()
            .find(|build| build.version == version)
            .map(|build| build.package.clone())
            .ok_or(404)
    }

    fn commit(&self, change: impl FnOnce(&mut Vec<Product>) -> Result<(), u16>) -> Result<(), u16> {
        let mut guard = self.products.lock().expect("applet products");
        let mut next = guard.clone();
        change(&mut next)?;
        if let Some(path) = &self.path {
            write_atomic(
                path,
                &serde_json::to_vec(&FileBody {
                    products: next.clone(),
                })
                .map_err(|_| 500u16)?,
            )?;
        }
        *guard = next;
        Ok(())
    }

    fn persist(&self) -> Result<(), u16> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let products = self.products.lock().expect("applet products").clone();
        write_atomic(
            path,
            &serde_json::to_vec(&FileBody { products }).map_err(|_| 500u16)?,
        )
    }
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

pub(crate) fn applet_id_ok(value: &str) -> bool {
    clean_app_id(value).is_ok()
}

pub(crate) fn is_office_app(value: &str) -> bool {
    OFFICE.iter().any(|(app_id, _)| *app_id == value)
}

fn ensure_office(products: &mut Vec<Product>) -> bool {
    let mut changed = false;
    for (app_id, name) in OFFICE {
        if !products.iter().any(|product| product.app_id == *app_id) {
            products.push(office_product(app_id, name));
            changed = true;
        }
        if let Some(product) = products
            .iter_mut()
            .find(|product| product.app_id == *app_id)
        {
            if pin_office_fields(product, name) {
                changed = true;
            }
        }
    }
    changed
}

/// `1.0.0` stays byte-for-byte. A missing `1.1.0` is appended and pinned.
/// Once `1.1.0` exists, a later rollback to `1.0.0` is left alone.
fn pin_office_fields(product: &mut Product, name: &str) -> bool {
    if product.builds.iter().any(|build| build.version == "1.1.0") {
        return false;
    }
    let pages = vec!["pages/index/index".to_owned()];
    let fields = office_fields();
    let package = render_package(
        product.app_id.as_str(),
        "1.1.0",
        name,
        &pages,
        "pages/index/index",
        &fields,
    );
    let digest = sha256_hex(package.as_bytes());
    product.builds.insert(
        0,
        Build {
            version: "1.1.0".to_owned(),
            digest,
            package,
        },
    );
    product.release = Some("1.1.0".to_owned());
    true
}

fn office_fields() -> Vec<Field> {
    vec![
        Field {
            name: "title".to_owned(),
            kind: "text".to_owned(),
            label: "标题".to_owned(),
            required: true,
            max: 80,
        },
        Field {
            name: "detail".to_owned(),
            kind: "textarea".to_owned(),
            label: "说明".to_owned(),
            required: true,
            max: 2000,
        },
    ]
}

fn office_product(app_id: &str, name: &str) -> Product {
    let pages = vec!["pages/index/index".to_owned()];
    let package = render_package(app_id, "1.0.0", name, &pages, "pages/index/index", &[]);
    let digest = sha256_hex(package.as_bytes());
    Product {
        app_id: app_id.to_owned(),
        name: name.to_owned(),
        description: String::new(),
        builds: vec![Build {
            version: "1.0.0".to_owned(),
            digest,
            package,
        }],
        trial: None,
        release: Some("1.0.0".to_owned()),
    }
}

/// Compact JSON with a fixed key order. The digest is SHA-256 of these exact
/// bytes, so the encoding cannot follow serde_json's map order or `\u` escapes.
fn render_package(
    app_id: &str,
    version: &str,
    title: &str,
    pages: &[String],
    entry: &str,
    fields: &[Field],
) -> String {
    let mut page_list = String::new();
    for (index, page) in pages.iter().enumerate() {
        if index > 0 {
            page_list.push(',');
        }
        page_list.push_str(&json_string(page));
    }
    let mut package = format!(
        "{{\"appId\":{app},\"version\":{version},\"entry\":{entry},\"pages\":[{page_list}],\"window\":{{\"navigationBarTitleText\":{title}}}}}",
        app = json_string(app_id),
        version = json_string(version),
        entry = json_string(entry),
        title = json_string(title),
    );
    if fields.is_empty() {
        return package;
    }
    package.pop();
    package.push_str(",\"fields\":[");
    for (index, field) in fields.iter().enumerate() {
        if index > 0 {
            package.push(',');
        }
        package.push_str(&format!(
            "{{\"name\":{name},\"kind\":{kind},\"label\":{label},\"required\":{required},\"max\":{max}}}",
            name = json_string(&field.name),
            kind = json_string(&field.kind),
            label = json_string(&field.label),
            required = if field.required { "true" } else { "false" },
            max = field.max,
        ));
    }
    package.push_str("]}");
    package
}

fn clean_fields(fields: Vec<FieldBody>) -> Result<Vec<Field>, u16> {
    if fields.len() > 8 {
        return Err(400);
    }
    let mut cleaned = Vec::with_capacity(fields.len());
    for field in fields {
        let name = field.name.trim();
        let bytes = name.as_bytes();
        if bytes.is_empty()
            || bytes.len() > 32
            || !bytes[0].is_ascii_lowercase()
            || !bytes
                .iter()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        {
            return Err(400);
        }
        if field.kind != "text" && field.kind != "textarea" {
            return Err(400);
        }
        if field.max == 0 || field.max > 2000 {
            return Err(400);
        }
        let label = clean_label(&field.label, 40)?;
        if label.is_empty() {
            return Err(400);
        }
        if cleaned.iter().any(|existing: &Field| existing.name == name) {
            return Err(400);
        }
        cleaned.push(Field {
            name: name.to_owned(),
            kind: field.kind,
            label,
            required: field.required,
            max: field.max,
        });
    }
    Ok(cleaned)
}

pub(crate) struct PackageField {
    pub name: String,
    pub required: bool,
    pub max: usize,
}

/// Fields declared by a package. A package with no `fields` key uses the
/// office fallback so a rolled-back `1.0.0` build can still be submitted.
pub(crate) fn fields_of_package(package: &str) -> Result<Vec<PackageField>, u16> {
    let value: Value = serde_json::from_str(package).map_err(|_| 400u16)?;
    let Some(fields) = value.get("fields") else {
        return Ok(vec![
            PackageField {
                name: "title".to_owned(),
                required: true,
                max: 80,
            },
            PackageField {
                name: "detail".to_owned(),
                required: true,
                max: 2000,
            },
        ]);
    };
    let items = fields.as_array().ok_or(400u16)?;
    let parsed = items
        .iter()
        .map(|item| FieldBody {
            name: item
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            kind: item
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            label: item
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            required: item
                .get("required")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            max: item.get("max").and_then(Value::as_u64).unwrap_or(0) as u32,
        })
        .collect();
    Ok(clean_fields(parsed)?
        .into_iter()
        .map(|field| PackageField {
            name: field.name,
            required: field.required,
            max: field.max as usize,
        })
        .collect())
}

fn json_string(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn metadata_json(product: &Product) -> String {
    json!({
        "appId": product.app_id,
        "name": product.name,
        "description": product.description,
        "development": channel_json(product, product.builds.first().map(|build| build.version.as_str())),
        "trial": channel_json(product, product.trial.as_deref()),
        "release": channel_json(product, product.release.as_deref()),
    })
    .to_string()
}

fn channel_json(product: &Product, version: Option<&str>) -> Value {
    let Some(version) = version else {
        return Value::Null;
    };
    let Some(build) = product.builds.iter().find(|build| build.version == version) else {
        return Value::Null;
    };
    json!({ "version": build.version, "digest": build.digest })
}

fn clean_app_id(value: &str) -> Result<String, u16> {
    let trimmed = value.trim();
    let bytes = trimmed.as_bytes();
    if (1..=32).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
        && !trimmed.ends_with('-')
        && !trimmed.contains("--")
    {
        return Ok(trimmed.to_owned());
    }
    Err(400)
}

fn clean_version(value: &str) -> Result<String, u16> {
    let trimmed = value.trim();
    let parts: Vec<&str> = trimmed.split('.').collect();
    if parts.len() == 3
        && parts.iter().all(|part| {
            !part.is_empty()
                && part.len() <= 4
                && part.bytes().all(|byte| byte.is_ascii_digit())
                && (part.len() == 1 || !part.starts_with('0'))
        })
    {
        return Ok(trimmed.to_owned());
    }
    Err(400)
}

fn clean_pages(pages: &[String]) -> Result<Vec<String>, u16> {
    if pages.is_empty() {
        return Ok(vec!["pages/index/index".to_owned()]);
    }
    if pages.len() > 20 {
        return Err(400);
    }
    let mut cleaned = Vec::with_capacity(pages.len());
    for page in pages {
        let page = clean_page(page)?;
        if cleaned.iter().any(|existing| existing == &page) {
            return Err(400);
        }
        cleaned.push(page);
    }
    Ok(cleaned)
}

fn clean_page(value: &str) -> Result<String, u16> {
    let trimmed = value.trim();
    if trimmed.starts_with("pages/")
        && trimmed.len() <= 64
        && trimmed
            .bytes()
            .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-'))
        && !trimmed.contains("//")
        && !trimmed.ends_with('/')
    {
        return Ok(trimmed.to_owned());
    }
    Err(400)
}

fn clean_label(value: &str, max: usize) -> Result<String, u16> {
    let trimmed = value.trim();
    if trimmed.chars().count() > max || trimmed.chars().any(|ch| ch.is_control()) {
        return Err(400);
    }
    Ok(trimmed.to_owned())
}

fn products_path(slug: &str) -> Option<PathBuf> {
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
    Some(crate::tenant::profile_dir(slug)?.join("applet-products.json"))
}

fn write_atomic(path: &std::path::Path, body: &[u8]) -> Result<(), u16> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| 500u16)?;
    }
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, body).map_err(|_| 500u16)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600)).map_err(|_| 500u16)?;
    }
    fs::rename(&temporary, path).map_err(|_| 500u16)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{AppletProductStore, sha256_hex};

    #[test]
    fn sha256_matches_the_empty_and_abc_vectors() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn create_build_and_publish_pin_channels_without_a_token() {
        let store = AppletProductStore::open_at(None);
        let created = store
            .create(r#"{"appId":"widget","name":"挂件","description":"状态"}"#.as_bytes())
            .expect("create");
        assert!(!created.contains("token"));
        assert!(created.contains("\"development\":null"));
        assert!(
            store
                .create(r#"{"appId":"widget","name":"再次"}"#.as_bytes())
                .is_err()
        );

        let built = store
            .build(
                "widget",
                r#"{"version":"1.0.0","navigationBarTitleText":"挂件"}"#.as_bytes(),
            )
            .expect("build");
        let built_json: serde_json::Value = serde_json::from_str(&built).unwrap();
        assert_eq!(built_json["development"]["version"], "1.0.0");
        assert!(built_json["release"].is_null());
        let digest = built_json["development"]["digest"].as_str().unwrap();
        let package = store.package("widget", "development").unwrap();
        assert_eq!(sha256_hex(package.as_bytes()), digest);
        assert!(!package.contains("token"));
        assert!(package.contains("\"appId\":\"widget\""));
        assert!(store.package("widget", "release").is_err());
        assert!(
            store
                .build("widget", r#"{"version":"1.0.0"}"#.as_bytes())
                .is_err()
        );

        store
            .publish(
                "widget",
                r#"{"version":"1.0.0","channel":"trial"}"#.as_bytes(),
            )
            .unwrap();
        assert!(store.package("widget", "release").is_err());
        assert_eq!(
            store.package("widget", "trial").unwrap(),
            store.package("widget", "development").unwrap()
        );

        store
            .build("widget", r#"{"version":"1.1.0"}"#.as_bytes())
            .unwrap();
        store
            .publish(
                "widget",
                r#"{"version":"1.1.0","channel":"release"}"#.as_bytes(),
            )
            .unwrap();
        assert!(
            store
                .package("widget", "release")
                .unwrap()
                .contains("\"version\":\"1.1.0\"")
        );
        store
            .publish(
                "widget",
                r#"{"version":"1.0.0","channel":"release"}"#.as_bytes(),
            )
            .unwrap();
        let rolled = store.package("widget", "release").unwrap();
        assert!(rolled.contains("\"version\":\"1.0.0\""));
        let meta: serde_json::Value =
            serde_json::from_str(&store.metadata("widget").unwrap()).unwrap();
        assert_eq!(meta["development"]["version"], "1.1.0");
        assert_eq!(meta["release"]["version"], "1.0.0");
        assert!(
            store
                .publish(
                    "widget",
                    r#"{"version":"9.9.9","channel":"release"}"#.as_bytes()
                )
                .is_err()
        );
        assert!(
            store
                .publish(
                    "widget",
                    r#"{"version":"1.0.0","channel":"development"}"#.as_bytes()
                )
                .is_err()
        );
    }

    #[test]
    fn office_applets_ship_as_a_release_package() {
        let store = AppletProductStore::open_at(None);
        let meta: serde_json::Value =
            serde_json::from_str(&store.metadata("leave").unwrap()).unwrap();
        assert_eq!(meta["name"], "请假");
        assert_eq!(meta["release"]["version"], "1.1.0");
        assert_eq!(meta["development"]["version"], "1.1.0");
        assert!(meta["trial"].is_null());
        let package = store.package("leave", "release").unwrap();
        assert_eq!(
            sha256_hex(package.as_bytes()),
            meta["release"]["digest"].as_str().unwrap()
        );
        assert!(!package.contains("a3s_ms_"));
        assert!(package.contains("\"navigationBarTitleText\":\"请假\""));
        assert!(package.contains("\"name\":\"title\""));
        assert!(package.contains("\"label\":\"说明\""));
        store
            .publish(
                "leave",
                r#"{"version":"1.0.0","channel":"release"}"#.as_bytes(),
            )
            .unwrap();
        let rolled = store.package("leave", "release").unwrap();
        assert_eq!(
            rolled,
            "{\"appId\":\"leave\",\"version\":\"1.0.0\",\"entry\":\"pages/index/index\",\"pages\":[\"pages/index/index\"],\"window\":{\"navigationBarTitleText\":\"请假\"}}"
        );
        assert!(!rolled.contains("\"fields\""));
    }

    #[test]
    fn office_rollback_survives_a_new_store() {
        let dir = std::env::temp_dir().join(format!(
            "a3s-applet-rollback-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let path = dir.join("applet-products.json");
        let store = AppletProductStore::open_at(Some(path.clone()));
        store
            .publish(
                "leave",
                r#"{"version":"1.0.0","channel":"release"}"#.as_bytes(),
            )
            .unwrap();
        drop(store);
        let again = AppletProductStore::open_at(Some(path));
        let meta: serde_json::Value =
            serde_json::from_str(&again.metadata("leave").unwrap()).unwrap();
        assert_eq!(meta["release"]["version"], "1.0.0");
        assert_eq!(meta["development"]["version"], "1.1.0");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_built_product_survives_a_new_store() {
        let dir = std::env::temp_dir().join(format!(
            "a3s-applet-products-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let path = dir.join("applet-products.json");
        let store = AppletProductStore::open_at(Some(path.clone()));
        store
            .create(r#"{"appId":"widget","name":"挂件"}"#.as_bytes())
            .unwrap();
        store
            .build("widget", r#"{"version":"0.1.0"}"#.as_bytes())
            .unwrap();
        store
            .publish(
                "widget",
                r#"{"version":"0.1.0","channel":"release"}"#.as_bytes(),
            )
            .unwrap();
        let package = store.package("widget", "release").unwrap();
        drop(store);

        let again = AppletProductStore::open_at(Some(path.clone()));
        assert_eq!(again.package("widget", "release").unwrap(), package);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&again.metadata("leave").unwrap()).unwrap()["release"]
                ["version"],
            "1.1.0"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
