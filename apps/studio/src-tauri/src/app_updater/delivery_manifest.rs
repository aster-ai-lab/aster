//! Resource generation sealed by Studio's final application signature.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::*;

const MARKER: &str = "Contents/Resources/studio-delivery.json";
const HOST: &str = "Contents/MacOS/Studio";
const REQUIRED: &[&str] = &[
    "Contents/Info.plist",
    "Contents/Resources/agent/a3s-code-acp",
    "Contents/Resources/agent/a3s-office",
    "Contents/Resources/studio-frontend/windows/main/index.html",
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Delivery {
    schema: u32,
    product: String,
    version: String,
    tenant: String,
    files: BTreeMap<String, DeliveryFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeliveryFile {
    sha256: String,
    size: u64,
}

fn read_delivery(bundle: &Path) -> Result<Delivery, UpdateFail> {
    regular_file(bundle, Path::new(MARKER), true)?;
    let mut bytes = Vec::new();
    File::open(bundle.join(MARKER))
        .map_err(|_| UpdateFail::InvalidPackage)?
        .take(MAX_JSON as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| UpdateFail::InvalidPackage)?;
    if bytes.len() > MAX_JSON {
        return Err(UpdateFail::InvalidPackage);
    }
    serde_json::from_slice(&bytes).map_err(|_| UpdateFail::InvalidPackage)
}

pub(super) fn validate(bundle: &Path, version: &str, tenant: &str) -> Result<(), UpdateFail> {
    let delivery = read_delivery(bundle)?;
    if delivery.schema != 1
        || delivery.product != "a3s-studio"
        || delivery.version != version
        || delivery.tenant != tenant
    {
        return Err(UpdateFail::ProductMismatch);
    }
    if delivery.files.len() > MAX_ENTRIES
        || REQUIRED.iter().any(|path| {
            !delivery
                .files
                .get(*path)
                .is_some_and(|entry| entry.size > 0)
        })
        || !delivery
            .files
            .contains_key(&format!("Contents/Resources/tenant/{tenant}/INFO.md"))
    {
        return Err(UpdateFail::InvalidPackage);
    }
    regular_file(bundle, Path::new(HOST), true)?;
    let mut total = 0u64;
    for (name, entry) in delivery.files {
        let relative = Path::new(&name);
        if name.is_empty()
            || !relative
                .components()
                .all(|part| matches!(part, Component::Normal(_)))
            || relative.to_str() != Some(name.as_str())
            || !name.starts_with("Contents/")
            || name == MARKER
            || entry.size > MAX_BYTES
        {
            return Err(UpdateFail::InvalidPackage);
        }
        total = total
            .checked_add(entry.size)
            .ok_or(UpdateFail::InvalidPackage)?;
        if total > MAX_UNPACKED {
            return Err(UpdateFail::InvalidPackage);
        }
        regular_file(bundle, relative, false)?;
        hash_file(
            &bundle.join(relative),
            &entry.sha256,
            entry.size,
            &mut |_| {},
        )
        .map_err(|_| UpdateFail::IntegrityFailed)?;
    }
    Ok(())
}

fn regular_file(bundle: &Path, relative: &Path, nonempty: bool) -> Result<(), UpdateFail> {
    let mut path = bundle.to_path_buf();
    for part in relative.components() {
        path.push(part);
        if fs::symlink_metadata(&path)
            .map_err(|_| UpdateFail::InvalidPackage)?
            .is_symlink()
        {
            return Err(UpdateFail::InvalidPackage);
        }
    }
    let metadata = fs::metadata(path).map_err(|_| UpdateFail::InvalidPackage)?;
    if !metadata.is_file() || (nonempty && metadata.len() == 0) {
        return Err(UpdateFail::InvalidPackage);
    }
    Ok(())
}

pub(super) fn installed_tenant(exe: &Path) -> Result<Option<String>, UpdateFail> {
    // The existing debug relaunch copies the host and uses checkout resources.
    #[cfg(debug_assertions)]
    if is_development_relaunch(true, std::env::var("A3S_STUDIO_APP_BUNDLE").ok().as_deref()) {
        return Ok(None);
    }
    let Some(macos) = exe
        .parent()
        .filter(|path| path.file_name().is_some_and(|name| name == "MacOS"))
    else {
        return Ok(None);
    };
    let Some(contents) = macos
        .parent()
        .filter(|path| path.file_name().is_some_and(|name| name == "Contents"))
    else {
        return Ok(None);
    };
    let bundle = contents.parent().ok_or(UpdateFail::InvalidPackage)?;
    let delivery = read_delivery(bundle)?;
    let tenant = crate::tenant::slug_from_env(Some(&delivery.tenant))
        .map_err(|_| UpdateFail::ProductMismatch)?;
    validate(bundle, env!("CARGO_PKG_VERSION"), &tenant)?;
    super::verify_signature(bundle, env!("CARGO_PKG_VERSION"))?;
    Ok(Some(tenant))
}

#[cfg(any(debug_assertions, test))]
fn is_development_relaunch(debug_build: bool, flag: Option<&str>) -> bool {
    debug_build && flag == Some("1")
}

#[cfg(test)]
#[path = "delivery_manifest_tests.rs"]
mod tests;
