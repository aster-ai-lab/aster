//! The `aster-use-bge` binary: manifest-driven install against the platform
//! model supply, plus status and uninstall.

use std::time::Duration;

use aster_use_core::{UseError, UseResult};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

use crate::assets::{bge_status, Family};
use crate::install::{install_family, uninstall_family, InstallManifest, ManifestFile};

#[derive(Debug)]
pub struct CommandOutput {
    pub human: String,
    pub json: serde_json::Value,
    pub exit_code: u8,
    pub should_print: bool,
}

impl CommandOutput {
    fn data<T>(value: T) -> UseResult<Self>
    where
        T: Serialize,
    {
        let data = serde_json::to_value(value).map_err(output_error)?;
        let human = serde_json::to_string_pretty(&data).map_err(output_error)?;
        Ok(Self {
            human,
            json: serde_json::json!({
                "schemaVersion": 1,
                "ok": true,
                "data": data,
            }),
            exit_code: 0,
            should_print: true,
        })
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "aster-use-bge",
    version,
    about = "Local BGE embedding and reranker engines for Aster",
    arg_required_else_help = true
)]
struct Cli {
    /// Emit one versioned JSON document.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Install one family from the platform model supply.
    Install {
        /// bge-m3 or bge-reranker-v2-m3.
        family: String,
        /// aster-api origin, e.g. http://127.0.0.1:3000.
        #[arg(long)]
        api: String,
        /// The model supply model id (uuid) holding the imported files.
        #[arg(long)]
        model: String,
        /// Manifest path of the ONNX weight, e.g. onnx/model_quantized.onnx.
        #[arg(long)]
        model_file: String,
        /// Manifest path of the tokenizer file.
        #[arg(long, default_value = "tokenizer.json")]
        tokenizer_file: String,
        /// Bearer token for the admin API; defaults to ASTER_MODEL_SUPPLY_TOKEN.
        #[arg(long)]
        token: Option<String>,
        /// Replace a usable install instead of skipping it.
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    /// Report readiness for one family (both when omitted).
    Status {
        /// bge-m3 or bge-reranker-v2-m3.
        family: Option<String>,
    },
    /// Remove a managed family install.
    Uninstall {
        /// bge-m3 or bge-reranker-v2-m3.
        family: String,
    },
}

pub async fn run() -> UseResult<CommandOutput> {
    let cli = Cli::parse();
    match cli.command {
        Command::Install {
            family,
            api,
            model,
            model_file,
            tokenizer_file,
            token,
            force,
        } => run_install(family, api, model, model_file, tokenizer_file, token, force).await,
        Command::Status { family } => run_status(family).await,
        Command::Uninstall { family } => run_uninstall(family).await,
    }
}

async fn run_install(
    family: String,
    api: String,
    model: String,
    model_file: String,
    tokenizer_file: String,
    token: Option<String>,
    force: bool,
) -> UseResult<CommandOutput> {
    let family = Family::parse(&family)?;
    let token = token
        .or_else(|| std::env::var("ASTER_MODEL_SUPPLY_TOKEN").ok())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            UseError::new(
                "use.bge.token_missing",
                "Pass --token or set ASTER_MODEL_SUPPLY_TOKEN for the admin API.",
            )
        })?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|error| {
            UseError::new(
                "use.bge.download_failed",
                format!("Failed to create the model supply client: {error}"),
            )
        })?;
    let manifest = build_manifest(
        &client,
        family,
        &api,
        &model,
        &model_file,
        &tokenizer_file,
        &token,
    )
    .await?;
    let status = install_family(manifest, force).await?;
    CommandOutput::data(status)
}

async fn run_status(family: Option<String>) -> UseResult<CommandOutput> {
    match family {
        Some(value) => CommandOutput::data(bge_status(Family::parse(&value)?)),
        None => CommandOutput::data([
            bge_status(Family::BgeM3),
            bge_status(Family::BgeRerankerV2M3),
        ]),
    }
}

async fn run_uninstall(family: String) -> UseResult<CommandOutput> {
    let removed = uninstall_family(Family::parse(&family)?).await?;
    CommandOutput::data(serde_json::json!({ "family": family, "removed": removed }))
}

#[derive(Debug, Deserialize)]
struct ApiEnvelope<T> {
    #[serde(default = "default_true")]
    success: bool,
    data: Option<T>,
}

impl<T> ApiEnvelope<T> {
    /// Unwraps the aster-api envelope, refusing an explicit failure.
    fn into_data(self, what: &str) -> UseResult<T> {
        if !self.success {
            return Err(UseError::new(
                "use.bge.manifest_invalid",
                format!("The model supply refused the {what} request."),
            ));
        }
        self.data.ok_or_else(|| {
            UseError::new(
                "use.bge.manifest_invalid",
                format!("The model supply {what} response carried no data envelope."),
            )
        })
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
struct ModelDetail {
    #[serde(default)]
    manifest: Vec<SupplyFile>,
}

#[derive(Debug, Deserialize)]
struct SupplyFile {
    path: String,
    size_bytes: i64,
    sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PresignedFile {
    url: String,
}

async fn build_manifest(
    client: &reqwest::Client,
    family: Family,
    api: &str,
    model: &str,
    model_file: &str,
    tokenizer_file: &str,
    token: &str,
) -> UseResult<InstallManifest> {
    let detail_url = format!(
        "{}/v1/api/admin/models/{}",
        api.trim_end_matches('/'),
        model
    );
    let detail: ApiEnvelope<ModelDetail> = get_json(client, &detail_url, token).await?;
    let supply = detail.into_data("model detail")?.manifest;
    if supply.is_empty() {
        return Err(UseError::new(
            "use.bge.manifest_invalid",
            "The model supply reports no imported files for this model.",
        ));
    }

    // Required roles plus the weight/tokenizer sidecars an ONNX export
    // commonly splits out. Anything else in the import is skipped.
    let model_data_file = format!("{model_file}_data");
    let wanted: Vec<String> = {
        let mut wanted = vec![
            model_file.to_string(),
            tokenizer_file.to_string(),
            model_data_file,
            "config.json".to_string(),
            "tokenizer_config.json".to_string(),
            "special_tokens_map.json".to_string(),
            "sentencepiece.bpe.model".to_string(),
        ];
        wanted.sort();
        wanted.dedup();
        wanted
    };
    let mut files = Vec::new();
    for file in &supply {
        if !wanted.contains(&file.path) {
            continue;
        }
        let presign_base = format!(
            "{}/v1/api/admin/models/{}/files",
            api.trim_end_matches('/'),
            model
        );
        let presign_url = reqwest::Url::parse_with_params(&presign_base, &[("path", &file.path)])
            .map_err(|error| {
            UseError::new(
                "use.bge.manifest_invalid",
                format!("Failed to build the presign URL: {error}"),
            )
        })?;
        let presigned: ApiEnvelope<PresignedFile> =
            get_json(client, presign_url.as_str(), token).await?;
        let url = presigned.into_data("presign")?.url;
        let url = (!url.trim().is_empty()).then_some(url).ok_or_else(|| {
            UseError::new(
                "use.bge.manifest_invalid",
                format!("No presigned URL was issued for {:?}.", file.path),
            )
        })?;
        let bytes = u64::try_from(file.size_bytes).unwrap_or(0);
        if bytes == 0 {
            return Err(UseError::new(
                "use.bge.manifest_invalid",
                format!(
                    "The supply reports a non-positive size for {:?}.",
                    file.path
                ),
            ));
        }
        files.push(ManifestFile {
            path: file.path.clone(),
            url,
            bytes,
            sha256: file.sha256.to_ascii_lowercase(),
        });
    }
    for role in [model_file, tokenizer_file] {
        if !files.iter().any(|file| file.path == role) {
            return Err(UseError::new(
                "use.bge.manifest_invalid",
                format!("The import does not contain the required file {role:?}."),
            ));
        }
    }
    InstallManifest::new(family, model, api, model_file, tokenizer_file, files)
}

async fn get_json<T: for<'de> Deserialize<'de>>(
    client: &reqwest::Client,
    url: &str,
    token: &str,
) -> UseResult<T> {
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|error| {
            UseError::new(
                "use.bge.download_failed",
                format!("The model supply request failed: {error}"),
            )
        })?;
    let status = response.status();
    if !status.is_success() {
        return Err(UseError::new(
            "use.bge.download_failed",
            format!("The model supply returned HTTP {status} for {url}."),
        ));
    }
    response.json::<T>().await.map_err(|error| {
        UseError::new(
            "use.bge.manifest_invalid",
            format!("The model supply response was not the expected JSON: {error}"),
        )
    })
}

fn output_error(error: serde_json::Error) -> UseError {
    UseError::new(
        "use.bge.output_invalid",
        format!("Failed to encode the command output: {error}"),
    )
}
