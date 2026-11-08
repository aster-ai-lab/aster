use super::*;
use std::collections::HashMap;
use std::io::{Cursor, Read, Write};

use serde_json::{json, Value};

use crate::remote::test_support::{
    extension_archive, TestRepository, TestServer, FUTURE, PACKAGE_VERSION,
};
use crate::remote::{
    prepare_cached_remote_package, prepare_remote_package, refresh_remote_registry, TrustedRegistry,
};
use crate::VerifiedTargetCachePolicy;

const TOKEN_PATH: &str =
    "/token?service=ghcr.io&scope=repository%3Aaster-ai-lab%2Fplugins%2Faster%2Fscience%3Apull";
const TOKEN: &str = "synthetic-ghcr-token-never-in-errors";

fn fixture() -> (Vec<u8>, PluginCatalogRecord, Vec<u8>, AixOciSource) {
    let tar = extension_archive(PACKAGE_VERSION);
    let baseline = TestRepository::new(tar.clone(), 31, FUTURE);
    let metadata: Value =
        serde_json::from_slice(baseline.routes.get("/metadata/targets.json").unwrap()).unwrap();
    let mut catalog = PluginCatalogRecord::from_json(
        &serde_json::to_vec(
            metadata
                .pointer("/signed/targets")
                .unwrap()
                .as_object()
                .unwrap()
                .values()
                .next()
                .unwrap()
                .pointer("/custom/aster")
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(tar.as_slice()));
    for entry in archive.entries().unwrap() {
        let mut entry = entry.unwrap();
        let path = entry
            .path()
            .unwrap()
            .strip_prefix("package")
            .unwrap()
            .to_path_buf();
        let name = path
            .iter()
            .map(|part| part.to_str().unwrap())
            .collect::<Vec<_>>()
            .join("/");
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        zip.write_all(&bytes).unwrap();
    }
    let bytes = zip.finish().unwrap().into_inner();
    let digest = hex_lower(&Sha256::digest(&bytes));
    let manifest = encode_aix_manifest(
        "science.aix".into(),
        format!("sha256:{digest}"),
        bytes.len() as u64,
    )
    .unwrap();
    catalog.archive.length = bytes.len() as u64;
    catalog.archive.sha256 = format!("sha256:{digest}");
    catalog.archive.target_name = format!(
        "extensions/aster/science/{PACKAGE_VERSION}/stable/{}/oci-ghcr-v1/{}/science.aix",
        catalog.target,
        hex_lower(&Sha256::digest(&manifest))
    );
    catalog.validate().unwrap();
    let source = AixOciSource::from_target(
        &TargetName::new(catalog.archive.target_name.clone()).unwrap(),
        &catalog,
    )
    .unwrap()
    .unwrap();
    (bytes, catalog, manifest, source)
}

fn routes(bytes: &[u8], manifest: &[u8], source: &AixOciSource) -> HashMap<String, Vec<u8>> {
    HashMap::from([
        (
            TOKEN_PATH.into(),
            serde_json::to_vec(&json!({"token":TOKEN})).unwrap(),
        ),
        (
            format!(
                "/v2/{}/manifests/sha256:{}",
                source.repository, source.manifest_sha256
            ),
            manifest.to_vec(),
        ),
        (
            format!(
                "/v2/{}/blobs/sha256:{}",
                source.repository,
                hex_lower(&Sha256::digest(bytes))
            ),
            bytes.to_vec(),
        ),
    ])
}

#[test]
fn oci_targets_require_a_signed_digest_and_native_aix_identity() {
    let (_, catalog, _, source) = fixture();
    assert_eq!(source.repository, "aster-ai-lab/plugins/aster/science");
    let mut recategorized = catalog.clone();
    recategorized.categories = vec!["development".into()];
    let recategorized_source = AixOciSource::from_target(
        &TargetName::new(recategorized.archive.target_name.clone()).unwrap(),
        &recategorized,
    )
    .unwrap()
    .unwrap();
    assert_eq!(source.repository, recategorized_source.repository);
    for changed in [
        catalog
            .archive
            .target_name
            .replace("oci-ghcr-v1/", "oci-ghcr-v1/research/"),
        catalog
            .archive
            .target_name
            .replace(&source.manifest_sha256, "not-a-digest"),
        catalog
            .archive
            .target_name
            .replace("science.aix", "other.aix"),
        catalog
            .archive
            .target_name
            .replace("science.aix", "science.zip"),
        catalog
            .archive
            .target_name
            .replace("science.aix", "nested/science.aix"),
    ] {
        assert_eq!(
            AixOciSource::from_target(&TargetName::new(changed).unwrap(), &catalog)
                .unwrap_err()
                .code,
            ERROR
        );
    }
    let http = TargetName::new(format!(
        "extensions/aster/science/{PACKAGE_VERSION}/stable/{}/science.aix",
        catalog.target
    ))
    .unwrap();
    assert!(AixOciSource::from_target(&http, &catalog)
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn signed_oci_identity_seals_native_aix_bytes_and_survives_cached_offline_preparation() {
    use aster_use_core::{
        LockedPluginPackage, PluginPackageLock, PluginPackageLockHost, PLUGIN_PACKAGE_LOCK_SCHEMA,
    };
    let (bytes, catalog, manifest, source) = fixture();
    let repository = TestRepository::with_target_metadata(
        bytes.clone(),
        catalog.archive.target_name.clone(),
        serde_json::to_value(&catalog).unwrap(),
        31,
        FUTURE,
    );
    let tuf_server = TestServer::start(repository.routes.clone());
    let ghcr_server = TestServer::start(routes(&bytes, &manifest, &source));
    let temporary = tempfile::tempdir().unwrap();
    let datastore = temporary.path().join("本地 cache with spaces");
    let trusted = TrustedRegistry::new(
        "fixture",
        tuf_server.base_url(),
        &repository.root_sha256,
        None,
        datastore.clone(),
    )
    .unwrap();
    refresh_remote_registry(&trusted).await.unwrap();
    let prepared = prepare_remote_package(&trusted, "aster/science", None, "stable", None)
        .await
        .unwrap();
    let lock = PluginPackageLock {
        schema: PLUGIN_PACKAGE_LOCK_SCHEMA.into(),
        root_package_id: "aster/science".into(),
        host: PluginPackageLockHost::new(
            crate::remote::host_target().unwrap(),
            env!("CARGO_PKG_VERSION"),
        )
        .unwrap(),
        packages: vec![LockedPluginPackage {
            catalog: prepared.verified_catalog().clone(),
            dependencies: Vec::new(),
        }],
    };
    let sealed = PluginPackageLock::from_json(&lock.canonical_bytes().unwrap()).unwrap();
    assert_eq!(
        sealed.packages[0].catalog.record.archive.target_name,
        catalog.archive.target_name
    );
    assert!(sealed.packages[0]
        .catalog
        .record
        .archive
        .target_name
        .contains(&source.manifest_sha256));
    let mut target = ResumableTarget::begin(
        &datastore,
        bytes.len() as u64,
        &hex_lower(&Sha256::digest(&bytes)),
        VerifiedTargetCachePolicy::default(),
    )
    .await
    .unwrap();
    let mut client = GhcrClient::new(RegistryNetworkPolicy::Standard).unwrap();
    client.endpoint = Url::parse(ghcr_server.base_url()).unwrap();
    client.download(&source, &mut target).await.unwrap();
    assert!(target.is_ready());
    drop(target);
    assert_eq!(ghcr_server.requests().len(), 3);
    assert!(ghcr_server.authorizations_for(TOKEN_PATH).is_empty());
    for path in ghcr_server
        .requests()
        .into_iter()
        .filter(|path| path.starts_with("/v2/"))
    {
        assert_eq!(
            ghcr_server.authorizations_for(&path),
            [format!("Bearer {TOKEN}")]
        );
    }
    ghcr_server.clear_requests();
    tuf_server.clear_requests();
    let downloaded = prepared.download().await.unwrap();
    assert_eq!(std::fs::read(downloaded.path()).unwrap(), bytes);
    let package = crate::ExtensionLifecyclePackage::prepare_remote("aster/science", downloaded)
        .await
        .unwrap();
    assert_eq!(package.manifest().skills[0].id, "science");
    let cached = prepare_cached_remote_package(
        &trusted,
        "aster/science",
        Some(PACKAGE_VERSION),
        "stable",
        None,
    )
    .await
    .unwrap()
    .download()
    .await
    .unwrap();
    assert_eq!(std::fs::read(cached.path()).unwrap(), bytes);
    assert!(ghcr_server.requests().is_empty());
    assert!(tuf_server.requests().is_empty());
}

#[tokio::test]
async fn oci_blob_redirects_preserve_range_and_do_not_forward_registry_authorization() {
    let (bytes, _, manifest, source) = fixture();
    let digest = hex_lower(&Sha256::digest(&bytes));
    let blob_path = format!("/v2/{}/blobs/sha256:{digest}", source.repository);
    let storage = TestServer::start(HashMap::from([("/blob.aix".into(), bytes.clone())]));
    let registry = TestServer::start(routes(&bytes, &manifest, &source));
    registry.redirect(&blob_path, &format!("{}blob.aix", storage.base_url()));
    let temporary = tempfile::tempdir().unwrap();
    let mut target = ResumableTarget::begin(
        temporary.path(),
        bytes.len() as u64,
        &digest,
        VerifiedTargetCachePolicy::default(),
    )
    .await
    .unwrap();
    target.append(&bytes[..17]).await.unwrap();
    target.checkpoint().await.unwrap();
    let mut client = GhcrClient::new(RegistryNetworkPolicy::Standard).unwrap();
    client.endpoint = Url::parse(registry.base_url()).unwrap();
    client.download(&source, &mut target).await.unwrap();
    assert!(target.is_ready());
    assert_eq!(
        registry.authorizations_for(&blob_path),
        [format!("Bearer {TOKEN}")]
    );
    assert!(storage.authorizations_for("/blob.aix").is_empty());
    assert_eq!(storage.ranges_for("/blob.aix"), ["bytes=17-"]);
}

#[tokio::test]
async fn bad_oci_manifest_or_metadata_limits_refuse_before_requesting_blob_bytes() {
    let (bytes, _, manifest, source) = fixture();
    for change in [
        "digest",
        "layer",
        "type",
        "source",
        "token-limit",
        "manifest-limit",
        "token-stream-limit",
        "manifest-stream-limit",
        "invalid-token",
    ] {
        let mut source = source.clone();
        let mut manifest_json: Value = serde_json::from_slice(&manifest).unwrap();
        match change {
            "layer" => {
                manifest_json["layers"][0]["digest"] = json!(format!("sha256:{}", "0".repeat(64)))
            }
            "type" => manifest_json["artifactType"] = json!("application/vnd.example.foreign"),
            "source" => {
                manifest_json["annotations"]["org.opencontainers.image.source"] =
                    json!("https://github.com/untrusted/plugins")
            }
            _ => {}
        }
        let mut changed_manifest = serde_json::to_vec(&manifest_json).unwrap();
        if matches!(change, "manifest-limit" | "manifest-stream-limit") {
            changed_manifest = vec![b' '; MAX_MANIFEST_BYTES as usize + 1];
        }
        if change != "digest" {
            source.manifest_sha256 = hex_lower(&Sha256::digest(&changed_manifest));
        } else {
            changed_manifest.push(b' ');
        }
        let mut responses = routes(&bytes, &changed_manifest, &source);
        if matches!(change, "token-limit" | "token-stream-limit") {
            responses.insert(TOKEN_PATH.into(), vec![b' '; MAX_TOKEN_BYTES as usize + 1]);
        }
        if change == "invalid-token" {
            responses.insert(
                TOKEN_PATH.into(),
                serde_json::to_vec(&json!({"token":format!("{TOKEN}\n")})).unwrap(),
            );
        }
        let server = TestServer::start(responses);
        if change == "token-stream-limit" {
            server.omit_content_length_for(TOKEN_PATH);
        }
        if change == "manifest-stream-limit" {
            server.omit_content_length_for(&format!(
                "/v2/{}/manifests/sha256:{}",
                source.repository, source.manifest_sha256
            ));
        }
        let temporary = tempfile::tempdir().unwrap();
        let mut target = ResumableTarget::begin(
            temporary.path(),
            bytes.len() as u64,
            &hex_lower(&Sha256::digest(&bytes)),
            VerifiedTargetCachePolicy::default(),
        )
        .await
        .unwrap();
        let mut client = GhcrClient::new(RegistryNetworkPolicy::Standard).unwrap();
        client.endpoint = Url::parse(server.base_url()).unwrap();
        let error = client.download(&source, &mut target).await.unwrap_err();
        assert_eq!(error.code, ERROR, "{change}");
        assert!(!format!("{error:?}").contains(TOKEN));
        assert!(!target.is_ready());
        assert!(
            server
                .requests()
                .iter()
                .all(|path| !path.contains("/blobs/")),
            "{change}"
        );
    }
}

#[tokio::test]
async fn altered_oci_blob_is_not_published_and_a_retry_uses_clean_bytes() {
    let (bytes, _, manifest, source) = fixture();
    let digest = hex_lower(&Sha256::digest(&bytes));
    let blob_path = format!("/v2/{}/blobs/sha256:{digest}", source.repository);
    let original = routes(&bytes, &manifest, &source);
    let mut altered = original.clone();
    let payload = altered.get_mut(&blob_path).unwrap();
    payload[0] ^= 0xff;
    let server = TestServer::start(altered);
    let temporary = tempfile::tempdir().unwrap();
    let mut client = GhcrClient::new(RegistryNetworkPolicy::Standard).unwrap();
    client.endpoint = Url::parse(server.base_url()).unwrap();
    let mut target = ResumableTarget::begin(
        temporary.path(),
        bytes.len() as u64,
        &digest,
        VerifiedTargetCachePolicy::default(),
    )
    .await
    .unwrap();
    assert_eq!(
        client
            .download(&source, &mut target)
            .await
            .unwrap_err()
            .code,
        ERROR
    );
    assert!(!target.is_ready());
    drop(target);
    let cache = temporary.path().join("verified-targets/sha256");
    assert!(!cache.join(&digest).exists());
    assert!(!cache.join(format!(".target-{digest}.part")).exists());
    server.replace_routes(original);
    let mut target = ResumableTarget::begin(
        temporary.path(),
        bytes.len() as u64,
        &digest,
        VerifiedTargetCachePolicy::default(),
    )
    .await
    .unwrap();
    assert_eq!(target.offset(), 0);
    client.download(&source, &mut target).await.unwrap();
    assert!(target.is_ready());
    assert_eq!(std::fs::read(cache.join(digest)).unwrap(), bytes);
}
