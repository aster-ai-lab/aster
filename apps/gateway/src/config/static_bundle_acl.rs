//! Strict consumption of the Cloud WEB0.1 static target projection.
//!
//! The wire format uses `sha256:` references; legacy standalone hex remains
//! accepted. Runtime digests stay canonical hex so object and ETag admission
//! use the same representation. Delivery policy is currently a fixed closed
//! contract, not a set of options silently ignored by the request path.

use super::{
    children, config_error, label_or_string_attr, required_string_attr, string_attr,
    string_list_attr, u64_attr,
};
use crate::config::{StaticBundleConfig, StaticBundleManifestConfig};
use crate::error::Result;
use crate::static_object::StaticObjectEntry;
use aster_acl::{Block, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub(super) fn parse_static_bundle_block(block: &Block) -> Result<StaticBundleConfig> {
    ensure_shape(
        block,
        &[
            "name",
            "release_digest",
            "object_namespace",
            "base_path",
            "spa_fallback",
            "provenance_digest",
            "local_digest_store",
        ],
        &["manifest", "delivery"],
    )?;
    ensure_one_identity(block, "name")?;
    let manifests = children(block, &["manifest"]);
    if manifests.len() != 1 {
        return Err(config_error(
            "static_bundles require exactly one manifest { ... } block",
        ));
    }
    let manifest = manifests[0];
    ensure_shape(manifest, &["entry_document"], &["entries", "entry"])?;
    ensure_unlabeled(manifest)?;

    let deliveries = children(block, &["delivery"]);
    if deliveries.len() > 1 {
        return Err(config_error(
            "static_bundles contain duplicate delivery blocks",
        ));
    }
    if let Some(delivery) = deliveries.first() {
        validate_delivery(delivery)?;
    }

    let mut entries = BTreeMap::new();
    for entry in children(manifest, &["entries", "entry"]) {
        ensure_shape(
            entry,
            &["path", "digest", "size", "media_type", "content_encoding"],
            &[],
        )?;
        ensure_one_identity(entry, "path")?;
        let path = label_or_string_attr(entry, &["path"])?;
        let value = StaticObjectEntry {
            digest: parse_digest(&required_string_attr(entry, &["digest"])?, "digest")?,
            size: u64_attr(entry, &["size"])?
                .ok_or_else(|| config_error("static_bundles manifest entries require size"))?,
            media_type: required_string_attr(entry, &["media_type"])?,
            content_encoding: string_attr(entry, &["content_encoding"])?,
        };
        if entries.insert(path.clone(), value).is_some() {
            return Err(config_error(format!(
                "Duplicate static_bundles manifest entry '{path}'"
            )));
        }
    }
    if entries.is_empty() {
        return Err(config_error(
            "static_bundles manifest requires at least one entries block",
        ));
    }

    Ok(StaticBundleConfig {
        release_digest: parse_digest(
            &required_string_attr(block, &["release_digest"])?,
            "release_digest",
        )?,
        object_namespace: required_string_attr(block, &["object_namespace"])?,
        base_path: string_attr(block, &["base_path"])?.unwrap_or_else(|| "/".into()),
        spa_fallback: string_attr(block, &["spa_fallback"])?,
        provenance_digest: string_attr(block, &["provenance_digest"])?
            .map(|value| parse_digest(&value, "provenance_digest"))
            .transpose()?,
        manifest: StaticBundleManifestConfig {
            entry_document: required_string_attr(manifest, &["entry_document"])?,
            entries,
        },
        local_digest_store: string_attr(block, &["local_digest_store"])?.map(PathBuf::from),
    })
}

fn parse_digest(value: &str, field: &str) -> Result<String> {
    let hex = value.strip_prefix("sha256:").unwrap_or(value);
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(config_error(format!(
            "static_bundles {field} requires a lowercase sha256 digest reference or legacy hex"
        )));
    }
    Ok(hex.to_owned())
}

fn validate_delivery(block: &Block) -> Result<()> {
    ensure_shape(
        block,
        &[
            "content_type_source",
            "x_content_type_options",
            "entry_document_cache",
            "asset_cache",
            "spa_excludes",
        ],
        &[],
    )?;
    ensure_unlabeled(block)?;
    for (field, expected) in [
        ("content_type_source", "manifest"),
        ("x_content_type_options", "nosniff"),
        ("entry_document_cache", "no-cache"),
        ("asset_cache", "immutable"),
    ] {
        if required_string_attr(block, &[field])? != expected {
            return Err(config_error(format!(
                "static_bundles delivery {field} must be '{expected}'"
            )));
        }
    }
    if !matches!(block.attributes.get("spa_excludes"), Some(Value::List(_)))
        || string_list_attr(block, &["spa_excludes"])? != ["/api"]
    {
        return Err(config_error(
            "static_bundles delivery spa_excludes must be exactly [\"/api\"]",
        ));
    }
    Ok(())
}

fn ensure_shape(block: &Block, attributes: &[&str], blocks: &[&str]) -> Result<()> {
    for field in block.attributes.keys() {
        if !attributes.contains(&field.as_str()) {
            return Err(config_error(format!(
                "Unknown static_bundles {} field '{field}'",
                block.name
            )));
        }
    }
    for child in &block.blocks {
        if !blocks.contains(&child.name.as_str()) {
            return Err(config_error(format!(
                "Unknown static_bundles {} block '{}'",
                block.name, child.name
            )));
        }
    }
    Ok(())
}

fn ensure_unlabeled(block: &Block) -> Result<()> {
    if !block.labels.is_empty() {
        return Err(config_error(format!(
            "static_bundles {} does not accept labels",
            block.name
        )));
    }
    Ok(())
}

fn ensure_one_identity(block: &Block, field: &str) -> Result<()> {
    let count = block.labels.len() + usize::from(block.attributes.contains_key(field));
    if count != 1 {
        return Err(config_error(format!(
            "static_bundles {} requires exactly one label or {field} identity",
            block.name
        )));
    }
    Ok(())
}
