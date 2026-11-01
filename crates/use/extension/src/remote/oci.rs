//! Digest-pinned native AIX artifacts in the official GitHub Container Registry namespace.

use std::collections::BTreeMap;
use std::time::Duration;

use aster_use_core::{PluginCatalogRecord, UseError, UseResult};
use reqwest::header::{HeaderValue, ACCEPT, ACCEPT_ENCODING, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tough::TargetName;
use url::Url;

use super::target_cache::ResumableTarget;
use super::{hex_lower, network, resumable_http, RegistryNetworkPolicy};

pub(super) const TARGET_PREFIX: &str = "oci-ghcr-v1/";
pub(crate) const REPOSITORY: &str = "https://github.com/aster-ai-lab/plugins";
pub(super) const ARTIFACT_TYPE: &str = "application/vnd.aster.aix.v1";
pub(super) const PAYLOAD_TYPE: &str = "application/vnd.aster.aix.v1+zip";
const MANIFEST_TYPE: &str = "application/vnd.oci.image.manifest.v1+json";
const ERROR: &str = "use.extension.registry_oci_invalid";
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_TOKEN_BYTES: u64 = 64 * 1024;

pub(crate) fn encode_aix_manifest(
    filename: String,
    archive_sha256: String,
    archive_length: u64,
) -> UseResult<Vec<u8>> {
    let manifest = Manifest {
        schema_version: 2,
        media_type: MANIFEST_TYPE.into(),
        artifact_type: ARTIFACT_TYPE.into(),
        config: Layer {
            media_type: "application/vnd.oci.empty.v1+json".into(),
            digest: format!("sha256:{}", hex_lower(&Sha256::digest(b"{}"))),
            size: 2,
            annotations: BTreeMap::new(),
            urls: Vec::new(),
            data: Some("e30=".into()),
        },
        layers: vec![Layer {
            media_type: PAYLOAD_TYPE.into(),
            digest: archive_sha256,
            size: archive_length,
            annotations: BTreeMap::from([("org.opencontainers.image.title".into(), filename)]),
            urls: Vec::new(),
            data: None,
        }],
        annotations: BTreeMap::from([(
            "org.opencontainers.image.source".into(),
            REPOSITORY.into(),
        )]),
    };
    serde_json::to_vec(&manifest)
        .map_err(|_| invalid("Failed to encode the native AIX OCI manifest."))
}

#[derive(Debug, Clone)]
pub(super) struct AixOciSource {
    repository: String,
    manifest_sha256: String,
    filename: String,
}

impl AixOciSource {
    pub(super) fn from_target(
        target: &TargetName,
        catalog: &PluginCatalogRecord,
    ) -> UseResult<Option<Self>> {
        let prefix = format!(
            "extensions/{}/{}/{}/{}/",
            catalog.package_id,
            catalog.version,
            catalog.channel.as_str(),
            catalog.target
        );
        let Some(oci) = target
            .raw()
            .strip_prefix(&prefix)
            .and_then(|path| path.strip_prefix(TARGET_PREFIX))
        else {
            return Ok(None);
        };
        let parts = oci.split('/').collect::<Vec<_>>();
        let [domain, manifest_sha256, filename] = parts.as_slice() else {
            return Err(invalid(
                "The AIX OCI target requires domain, manifest SHA-256 and archive name.",
            ));
        };
        let name = catalog.package_id.rsplit('/').next().unwrap_or_default();
        if !crate::valid_segment(domain)
            || !catalog
                .categories
                .iter()
                .any(|category| category == *domain)
            || manifest_sha256.len() != 64
            || !manifest_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            || *filename != format!("{name}.aix")
        {
            return Err(invalid("The AIX OCI target domain, immutable digest or native archive identity is invalid."));
        }
        Ok(Some(Self {
            repository: format!("aster-ai-lab/plugins/{domain}/{}", catalog.package_id),
            manifest_sha256: (*manifest_sha256).into(),
            filename: (*filename).into(),
        }))
    }

    fn verify_manifest(&self, bytes: &[u8], target: &ResumableTarget) -> UseResult<()> {
        if bytes.len() as u64 > MAX_MANIFEST_BYTES
            || hex_lower(&Sha256::digest(bytes)) != self.manifest_sha256
        {
            return Err(invalid(
                "The OCI manifest does not match its signed digest.",
            ));
        }
        let manifest: Manifest = serde_json::from_slice(bytes)
            .map_err(|_| invalid("The OCI manifest is not valid AIX artifact metadata."))?;
        let [layer] = manifest.layers.as_slice() else {
            return Err(invalid(
                "An AIX OCI artifact must contain exactly one native package layer.",
            ));
        };
        if manifest.schema_version != 2
            || manifest.media_type != MANIFEST_TYPE
            || manifest.artifact_type != ARTIFACT_TYPE
            || manifest.config.media_type != "application/vnd.oci.empty.v1+json"
            || manifest.config.digest != format!("sha256:{}", hex_lower(&Sha256::digest(b"{}")))
            || manifest.config.size != 2
            || !manifest.config.urls.is_empty()
            || manifest
                .config
                .data
                .as_deref()
                .is_some_and(|data| data != "e30=")
            || manifest
                .annotations
                .get("org.opencontainers.image.source")
                .map(String::as_str)
                != Some(REPOSITORY)
            || layer.media_type != PAYLOAD_TYPE
            || layer.digest != format!("sha256:{}", target.expected_sha256())
            || layer.size != target.expected_length()
            || layer.annotations.get("org.opencontainers.image.title") != Some(&self.filename)
            || !layer.urls.is_empty()
            || layer.data.is_some()
        {
            return Err(invalid(
                "The OCI artifact does not match the signed native AIX package.",
            ));
        }
        Ok(())
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    schema_version: u32,
    media_type: String,
    artifact_type: String,
    config: Layer,
    layers: Vec<Layer>,
    #[serde(default)]
    annotations: BTreeMap<String, String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Layer {
    media_type: String,
    digest: String,
    size: u64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    annotations: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    urls: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<String>,
}

pub(super) struct GhcrClient {
    endpoint: Url,
    policy: RegistryNetworkPolicy,
    client: Option<reqwest::Client>,
}

impl GhcrClient {
    pub(super) fn new(policy: RegistryNetworkPolicy) -> UseResult<Self> {
        let client = match policy {
            RegistryNetworkPolicy::Standard => Some(
                reqwest::Client::builder()
                    .user_agent("aster-use-extension/0.3")
                    .connect_timeout(Duration::from_secs(15))
                    .timeout(Duration::from_secs(60))
                    .redirect(reqwest::redirect::Policy::none())
                    .build()
                    .map_err(|_| invalid("Failed to create the GHCR metadata client."))?,
            ),
            RegistryNetworkPolicy::PublicInternet => None,
        };
        Ok(Self {
            endpoint: Url::parse("https://ghcr.io/")
                .map_err(|_| invalid("The GHCR endpoint is invalid."))?,
            policy,
            client,
        })
    }

    pub(super) async fn download(
        &self,
        source: &AixOciSource,
        target: &mut ResumableTarget,
    ) -> UseResult<()> {
        if target.is_ready() {
            return Ok(());
        }
        let mut token_url = self
            .endpoint
            .join("token")
            .map_err(|_| invalid("The GHCR token endpoint is invalid."))?;
        token_url
            .query_pairs_mut()
            .append_pair("service", "ghcr.io")
            .append_pair("scope", &format!("repository:{}:pull", source.repository));
        let token_bytes = self
            .get_bounded(&token_url, None, MAX_TOKEN_BYTES, "application/json")
            .await?;
        let token: Token = serde_json::from_slice(&token_bytes)
            .map_err(|_| invalid("The GHCR authorization response is invalid."))?;
        let token = match (token.token, token.access_token) {
            (Some(token), None) | (None, Some(token)) if !token.is_empty() => token,
            (Some(token), Some(access)) if !token.is_empty() && token == access => token,
            _ => return Err(invalid("GHCR did not authorize anonymous package reads.")),
        };
        let mut authorization = HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|_| invalid("The GHCR authorization header is invalid."))?;
        authorization.set_sensitive(true);

        let manifest_url = self
            .endpoint
            .join(&format!(
                "v2/{}/manifests/sha256:{}",
                source.repository, source.manifest_sha256
            ))
            .map_err(|_| invalid("The GHCR manifest endpoint is invalid."))?;
        let manifest = self
            .get_bounded(
                &manifest_url,
                Some(&authorization),
                MAX_MANIFEST_BYTES,
                MANIFEST_TYPE,
            )
            .await?;
        source.verify_manifest(&manifest, target)?;
        let blob_url = self
            .endpoint
            .join(&format!(
                "v2/{}/blobs/sha256:{}",
                source.repository,
                target.expected_sha256()
            ))
            .map_err(|_| invalid("The GHCR blob endpoint is invalid."))?;
        resumable_http::download(target, &blob_url, self.policy, ERROR, Some(&authorization)).await
    }

    async fn get_bounded(
        &self,
        url: &Url,
        authorization: Option<&HeaderValue>,
        limit: u64,
        accept: &'static str,
    ) -> UseResult<Vec<u8>> {
        let client = match self.policy {
            RegistryNetworkPolicy::Standard => self
                .client
                .clone()
                .ok_or_else(|| invalid("The GHCR metadata client is unavailable."))?,
            RegistryNetworkPolicy::PublicInternet => {
                network::public_internet_client(
                    url,
                    Duration::from_secs(15),
                    Duration::from_secs(60),
                )
                .await?
            }
        };
        let mut request = client
            .get(url.clone())
            .header(ACCEPT, accept)
            .header(ACCEPT_ENCODING, "identity");
        if let Some(authorization) = authorization {
            request = request.header(AUTHORIZATION, authorization);
        }
        let mut response = request.send().await.map_err(|error| {
            invalid(format!(
                "GHCR metadata request failed: {}",
                error.without_url()
            ))
        })?;
        if !response.status().is_success() {
            return Err(invalid(format!(
                "GHCR metadata returned HTTP {}.",
                response.status()
            )));
        }
        if response
            .content_length()
            .is_some_and(|length| length > limit)
        {
            return Err(invalid("GHCR metadata exceeds its byte limit."));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|error| {
            invalid(format!(
                "GHCR metadata read failed: {}",
                error.without_url()
            ))
        })? {
            if (bytes.len() as u64)
                .checked_add(chunk.len() as u64)
                .is_none_or(|length| length > limit)
            {
                return Err(invalid("GHCR metadata exceeds its byte limit."));
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
}

#[derive(Deserialize)]
struct Token {
    token: Option<String>,
    access_token: Option<String>,
}

fn invalid(message: impl Into<String>) -> UseError {
    UseError::new(ERROR, message)
}

#[cfg(test)]
mod tests;
