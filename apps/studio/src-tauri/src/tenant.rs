//! Launch-tenant identity for the Studio window.
//! The record is `apps/web/public/tenant/<slug>/INFO.md`. The desktop icon is
//! `icon.png` beside that file. The login wordmark is `mark-light.png`.
//! The application name is `product`. The application icon is the light-surface logo.

use std::fs;
use std::io;
use std::path::PathBuf;

#[derive(Clone)]
pub struct TenantBrand {
    pub slug: String,
    pub name: String,
    pub product: String,
    pub logo_path: PathBuf,
    pub logo_png: Vec<u8>,
    pub mark_png: Vec<u8>,
    pub icon_png: Vec<u8>,
    pub icon_path: PathBuf,
    pub theme_light: (u8, u8, u8),
}

/// Side process name from `A3S_STUDIO_INSTANCE`. Empty means the primary Studio.
/// An invalid value fails closed so a second window cannot take the primary lock.
pub fn instance_name() -> io::Result<Option<String>> {
    let Ok(raw) = std::env::var("A3S_STUDIO_INSTANCE") else {
        return Ok(None);
    };
    let name = raw.trim();
    if name.is_empty() {
        return Ok(None);
    }
    if !is_slug(name) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid studio instance: {name}"),
        ));
    }
    Ok(Some(name.to_owned()))
}

/// Single-instance lock id. The primary window uses the brand slug.
/// A side window uses `{brand}-{instance}`. Profile files use [`profile_dir_under`].
pub fn storage_slug(brand_slug: &str) -> io::Result<String> {
    let instance = instance_name()?;
    storage_slug_for(brand_slug, instance.as_deref())
}

pub fn storage_slug_for(brand_slug: &str, instance: Option<&str>) -> io::Result<String> {
    let Some(instance) = instance else {
        return Ok(brand_slug.to_owned());
    };
    if !is_slug(instance) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid studio instance: {instance}"),
        ));
    }
    let storage = format!("{brand_slug}-{instance}");
    if storage.len() > 64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("studio instance id is too long: {storage}"),
        ));
    }
    Ok(storage)
}

/// Profile directory for `storage`.
/// A new side instance is `<brand>/instances/<instance>`.
/// A flat directory that already exists stays, so a running window keeps its files.
pub fn profile_dir(storage: &str) -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let root = PathBuf::from(home).join("Library/Application Support/dev.a3s.studio");
    let instance = instance_name().ok().flatten();
    Some(profile_dir_under(&root, storage, instance.as_deref()))
}

pub fn profile_dir_under(root: &std::path::Path, storage: &str, instance: Option<&str>) -> PathBuf {
    let legacy = root.join(storage);
    if legacy.is_dir() {
        return legacy;
    }
    let Some(instance) = instance.filter(|name| is_slug(name)) else {
        return legacy;
    };
    let Some(brand) = storage.strip_suffix(&format!("-{instance}")) else {
        return legacy;
    };
    if brand.is_empty() || !is_slug(brand) {
        return legacy;
    }
    root.join(brand).join("instances").join(instance)
}

/// Bundle id for this profile. A new side window is `dev.a3s.studio.<brand>.<instance>`.
/// An existing flat directory keeps `dev.a3s.studio.<storage>`.
pub fn bundle_identifier(storage: &str, instance: Option<&str>, legacy_exists: bool) -> String {
    if !legacy_exists {
        if let Some(instance) = instance.filter(|name| is_slug(name)) {
            if let Some(brand) = storage.strip_suffix(&format!("-{instance}")) {
                if !brand.is_empty() && is_slug(brand) {
                    return format!("dev.a3s.studio.{brand}.{instance}");
                }
            }
        }
    }
    format!("dev.a3s.studio.{storage}")
}

/// `A3S_TENANT_SLUG` with no fallback. An empty value is the same as unset.
pub fn slug_from_env(raw: Option<&str>) -> io::Result<String> {
    let Some(slug) = raw.map(str::trim).filter(|slug| !slug.is_empty()) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "A3S_TENANT_SLUG is not set",
        ));
    };
    if !is_slug(slug) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid tenant slug: {slug}"),
        ));
    }
    Ok(slug.to_owned())
}

/// Name shown on the Dock and in the page. A fork keeps the brand product.
/// The homepage draws the ToWork wordmark only when this string is `ToWork`.
pub fn window_product(product: &str, _instance: Option<&str>) -> String {
    product.to_owned()
}

/// `A3S_STUDIO_INSTANCE=fork` asks the process to take the next free `forkN` id.
pub fn is_fork_request(instance: Option<&str>) -> bool {
    instance == Some("fork")
}

pub fn load() -> io::Result<TenantBrand> {
    load_slug(&selected_slug()?)
}

pub(crate) fn selected_slug() -> io::Result<String> {
    let from_env = std::env::var("A3S_TENANT_SLUG").ok();
    let installed = std::env::current_exe()
        .ok()
        .map(|exe| packaged_tenant_from(&exe))
        .transpose()?
        .flatten();
    if let Some(installed) = installed {
        if from_env.as_deref().is_some_and(|slug| slug != installed) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "packaged tenant mismatch",
            ));
        }
        return Ok(installed);
    }
    // Library tests share this process and do not set the variable.
    // The shipped binary refuses an unset slug.
    #[cfg(test)]
    let from_env = from_env.or_else(|| Some("toyou".to_owned()));
    slug_from_env(from_env.as_deref())
}

fn packaged_tenant_from(exe: &std::path::Path) -> io::Result<Option<String>> {
    use std::io::Read;
    if let Some(tenant) = crate::app_updater::installed_tenant(exe)? {
        return Ok(Some(tenant));
    }
    let Some(directory) = exe.parent() else {
        return Ok(None);
    };
    let marker = directory.join("../Resources/studio-tenant");
    let read = || -> io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        fs::File::open(&marker)?.take(34).read_to_end(&mut bytes)?;
        Ok(bytes)
    };
    match read() {
        Ok(bytes) if bytes.len() <= 33 => {
            let value = std::str::from_utf8(&bytes).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "invalid packaged tenant")
            })?;
            slug_from_env(Some(value.strip_suffix('\n').unwrap_or(value))).map(Some)
        }
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid packaged tenant",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
#[path = "tenant_delivery_tests.rs"]
mod delivery_tests;

fn load_slug(slug: &str) -> io::Result<TenantBrand> {
    if !is_slug(slug) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid tenant slug: {slug}"),
        ));
    }
    let directory = tenant_dir(slug)?;
    let info = fs::read_to_string(directory.join("INFO.md"))?;
    let parsed = parse_info(&info).map_err(|message| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("tenant {slug} INFO.md: {message}"),
        )
    })?;
    let logo_path = directory.join(&parsed.light_logo);
    let mark_path = directory.join("mark-light.png");
    let icon_path = directory.join("icon.png");
    let logo_png = fs::read(&logo_path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("tenant logo {}: {error}", logo_path.display()),
        )
    })?;
    let mark_png = fs::read(&mark_path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("tenant wordmark {}: {error}", mark_path.display()),
        )
    })?;
    let icon_png = fs::read(&icon_path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("tenant icon {}: {error}", icon_path.display()),
        )
    })?;
    if logo_png.is_empty() || mark_png.is_empty() || icon_png.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("tenant {slug} logo, wordmark, or icon is empty"),
        ));
    }
    Ok(TenantBrand {
        slug: slug.to_owned(),
        name: parsed.name,
        product: parsed.product,
        logo_path,
        logo_png,
        mark_png,
        icon_png,
        icon_path,
        theme_light: parsed.theme_light,
    })
}

struct ParsedInfo {
    name: String,
    product: String,
    light_logo: String,
    theme_light: (u8, u8, u8),
}

fn parse_info(text: &str) -> Result<ParsedInfo, String> {
    let body = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))
        .ok_or("missing front matter")?;
    let end = body.find("\n---").ok_or("front matter is not closed")?;
    let mut section = "";
    let mut name_zh = None;
    let mut name_en = None;
    let mut product = None;
    let mut light_logo = None;
    let mut theme_light = None;
    let mut in_theme = false;
    for raw in body[..end].lines() {
        let line = raw.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim();
        if indent == 0 {
            in_theme = false;
            if trimmed.starts_with("product:") {
                product = Some(display_text(field_value(trimmed, "product:")?, "product")?);
                section = "";
                continue;
            }
            section = match trimmed {
                "name:" => "name",
                "logos:" => "logos",
                "theme_color:" => {
                    in_theme = true;
                    ""
                }
                _ => "",
            };
            continue;
        }
        if in_theme && indent >= 2 && trimmed.starts_with("light:") {
            theme_light = Some(parse_color(field_value(trimmed, "light:")?.as_str())?);
            continue;
        }
        if indent < 2 {
            continue;
        }
        match section {
            "name" if trimmed.starts_with("zh:") => {
                name_zh = Some(field_value(trimmed, "zh:")?);
            }
            "name" if trimmed.starts_with("en:") => {
                name_en = Some(field_value(trimmed, "en:")?);
            }
            "logos" if trimmed.starts_with("light_surface:") => {
                light_logo = Some(logo_file(field_value(trimmed, "light_surface:")?)?);
            }
            _ => {}
        }
    }
    let name = display_text(
        name_zh.or(name_en).ok_or("tenant name is empty")?,
        "tenant name",
    )?;
    let product = product.ok_or("product is missing")?;
    let light_logo = light_logo.ok_or("light surface logo is missing")?;
    Ok(ParsedInfo {
        name,
        product,
        light_logo,
        theme_light: theme_light.unwrap_or((0xf6, 0xf6, 0xf6)),
    })
}

fn display_text(value: String, label: &str) -> Result<String, String> {
    if value.chars().count() > 80 || value.contains(['\n', '\r']) {
        return Err(format!("{label} is invalid"));
    }
    Ok(value)
}

fn field_value(line: &str, key: &str) -> Result<String, String> {
    let value = unquote(line[key.len()..].trim());
    if value.is_empty() {
        return Err(format!("{key} is empty"));
    }
    Ok(value)
}

fn unquote(value: &str) -> String {
    let bytes = value.as_bytes();
    if bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''))
    {
        return value[1..value.len() - 1].to_owned();
    }
    value.to_owned()
}

fn logo_file(name: String) -> Result<String, String> {
    let ok = name.len() <= 64
        && name.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-' || byte == b'_'
        })
        && name.ends_with(".png")
        && !name.contains("..");
    if !ok {
        return Err(format!("light surface logo is invalid: {name}"));
    }
    Ok(name)
}

fn parse_color(value: &str) -> Result<(u8, u8, u8), String> {
    let hex = value.strip_prefix('#').unwrap_or(value);
    if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("theme color is invalid: {value}"));
    }
    let red = u8::from_str_radix(&hex[0..2], 16).map_err(|error| error.to_string())?;
    let green = u8::from_str_radix(&hex[2..4], 16).map_err(|error| error.to_string())?;
    let blue = u8::from_str_radix(&hex[4..6], 16).map_err(|error| error.to_string())?;
    Ok((red, green, blue))
}

fn is_slug(slug: &str) -> bool {
    let mut chars = slug.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() => {}
        _ => return false,
    }
    slug.len() <= 32
        && chars.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

fn tenant_dir(slug: &str) -> io::Result<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(directory) = exe.parent() {
            candidates.push(directory.join("../Resources/tenant").join(slug));
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("apps/web/public/tenant").join(slug));
        candidates.push(cwd.join("../web/public/tenant").join(slug));
        candidates.push(cwd.join("../../web/public/tenant").join(slug));
        candidates.push(cwd.join("web/public/tenant").join(slug));
    }
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../web/public/tenant")
            .join(slug),
    );
    for candidate in candidates {
        let info = candidate.join("INFO.md");
        if info.is_file() {
            return Ok(fs::canonicalize(candidate)?);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("tenant directory for {slug} was not found"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_tenant_name_and_light_logo() {
        let parsed = parse_info(
            "---\nname:\n  zh: \"通友科技\"\n  en: \"Toyou AI Labs\"\nproduct: \"ToWork\"\nlogos:\n  light_surface: logo-light.png\ntheme_color:\n  light: \"#f6f6f6\"\n---\n",
        )
        .unwrap();
        assert_eq!(parsed.name, "通友科技");
        assert_eq!(parsed.product, "ToWork");
        assert_eq!(parsed.light_logo, "logo-light.png");
        assert_eq!(parsed.theme_light, (0xf6, 0xf6, 0xf6));
    }

    #[test]
    fn towork_cua_is_an_isolated_towork_brand() {
        let directory = tenant_dir("towork-cua").unwrap();
        assert_eq!(
            directory.file_name().and_then(|name| name.to_str()),
            Some("towork-cua")
        );
        let parsed = parse_info(&fs::read_to_string(directory.join("INFO.md")).unwrap()).unwrap();
        assert_eq!(parsed.product, "ToWork");
        assert_eq!(parsed.name, "通友科技");
        for name in ["logo-light.png", "mark-light.png", "icon.png", "icon.icns"] {
            let bytes = fs::read(directory.join(name)).unwrap();
            assert!(!bytes.is_empty(), "{name}");
        }
        assert!(directory.join("icon.png").metadata().unwrap().len() > 8);
        assert!(
            fs::read(directory.join("icon.png"))
                .unwrap()
                .starts_with(b"\x89PNG")
        );
        assert!(!directory.join("model-market.json").exists());
    }

    #[test]
    fn loads_the_selected_tenant_files() {
        let brand = load_slug("toyou").unwrap();
        assert!(!brand.slug.is_empty());
        assert!(!brand.name.is_empty());
        assert!(!brand.product.is_empty());
        assert!(brand.logo_png.starts_with(b"\x89PNG"));
        assert!(brand.mark_png.starts_with(b"\x89PNG"));
        assert!(brand.icon_png.starts_with(b"\x89PNG"));
        assert_ne!(brand.mark_png, brand.logo_png);
        assert_eq!(
            brand.logo_path.extension().and_then(|ext| ext.to_str()),
            Some("png")
        );
        assert!(brand.icon_path.ends_with("icon.png"));
    }

    #[test]
    fn a_side_instance_keeps_the_brand_and_uses_its_own_storage_id() {
        assert_eq!(storage_slug_for("toyou", None).unwrap(), "toyou");
        assert_eq!(
            storage_slug_for("toyou", Some("fork1")).unwrap(),
            "toyou-fork1"
        );
        assert_eq!(
            storage_slug_for("toyou", Some("fork2")).unwrap(),
            "toyou-fork2"
        );
        assert!(is_fork_request(Some("fork")));
        assert!(!is_fork_request(Some("fork1")));
        assert!(!is_fork_request(None));
        assert_eq!(window_product("ToWork", None), "ToWork");
        assert_eq!(window_product("ToWork", Some("fork1")), "ToWork");
        assert!(storage_slug_for("toyou", Some("../toyou")).is_err());
        assert!(storage_slug_for(&"a".repeat(32), Some(&"b".repeat(32))).is_err());
    }

    #[test]
    fn a_new_side_profile_nests_and_an_existing_flat_directory_stays() {
        let root = tempfile::tempdir().expect("tempdir");
        let root = root.path();
        assert_eq!(profile_dir_under(root, "acme", None), root.join("acme"));
        assert_eq!(
            bundle_identifier("acme", None, false),
            "dev.a3s.studio.acme"
        );
        let nested = root.join("acme").join("instances").join("side1");
        assert_eq!(profile_dir_under(root, "acme-side1", Some("side1")), nested);
        assert_eq!(
            bundle_identifier("acme-side1", Some("side1"), false),
            "dev.a3s.studio.acme.side1"
        );
        // The instance name is the only suffix that may split. A shorter tail stays flat.
        assert_eq!(
            profile_dir_under(root, "acme-side1", Some("1")),
            root.join("acme-side1")
        );
        fs::create_dir_all(root.join("acme-side1")).expect("legacy dir");
        assert_eq!(
            profile_dir_under(root, "acme-side1", Some("side1")),
            root.join("acme-side1")
        );
        assert_eq!(
            bundle_identifier("acme-side1", Some("side1"), true),
            "dev.a3s.studio.acme-side1"
        );
        assert!(slug_from_env(None).is_err());
        assert!(slug_from_env(Some("")).is_err());
        assert!(slug_from_env(Some("  ")).is_err());
        assert_eq!(slug_from_env(Some("acme")).unwrap(), "acme");
    }
}
