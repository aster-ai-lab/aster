use std::ffi::OsString;
use std::path::PathBuf;

use aster_use_core::{UseError, UseResult};
use clap::error::ErrorKind;
use clap::{Parser, Subcommand};
use serde::Serialize;

use crate::{OcrClient, OcrMcpServer, OcrRequest, PpOcrV6Profile, PpOcrV6Provider};

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

    fn text(value: String) -> Self {
        Self {
            human: value.clone(),
            json: serde_json::json!({
                "schemaVersion": 1,
                "ok": true,
                "data": { "text": value },
            }),
            exit_code: 0,
            should_print: true,
        }
    }

    fn silent() -> Self {
        Self {
            human: String::new(),
            json: serde_json::Value::Null,
            exit_code: 0,
            should_print: false,
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "aster-use-ocr",
    version,
    about = "Provider-oriented OCR for Aster (PP-OCRv6 is the default provider)",
    arg_required_else_help = true
)]
struct Cli {
    /// Emit one versioned JSON document.
    #[arg(long, global = true)]
    json: bool,

    /// Detection bounds; document matches Studio's 1536-pixel profile.
    #[arg(long, global = true, value_enum, default_value = "general")]
    profile: PpOcrV6Profile,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Inspect the configured OCR provider without reading an image.
    Doctor,
    /// Extract text and layout evidence from one local image.
    Extract { path: PathBuf },
    /// Run an extension protocol surface.
    Serve {
        /// Serve standard MCP over stdin/stdout.
        #[arg(long)]
        mcp: bool,
    },
}

pub async fn run(args: Vec<String>) -> UseResult<CommandOutput> {
    run_os(args.into_iter().map(OsString::from).collect()).await
}

/// Parse native process arguments without normalizing filesystem values.
pub async fn run_os(args: Vec<OsString>) -> UseResult<CommandOutput> {
    let mut argv = vec![OsString::from("aster-use-ocr")];
    argv.extend(args);
    let cli = match Cli::try_parse_from(argv) {
        Ok(cli) => cli,
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            return Ok(CommandOutput::text(error.to_string()));
        }
        Err(error) => return Err(usage_error(error.to_string())),
    };

    if let Command::Serve { mcp } = &cli.command {
        if !mcp {
            return Err(usage_error("serve requires --mcp"));
        }
        if cli.json {
            return Err(usage_error("--json cannot be combined with serve --mcp"));
        }
        OcrMcpServer::new(client_for_profile(cli.profile).await?)
            .serve_stdio()
            .await?;
        return Ok(CommandOutput::silent());
    }

    let client = client_for_profile(cli.profile).await?;
    match cli.command {
        Command::Doctor => CommandOutput::data(client.diagnostic()),
        Command::Extract { path } => {
            CommandOutput::data(client.extract(OcrRequest { path }).await?)
        }
        Command::Serve { .. } => Err(UseError::new(
            "use.ocr.command_invalid",
            "OCR MCP command dispatch reached an invalid state.",
        )),
    }
}

async fn client_for_profile(profile: PpOcrV6Profile) -> UseResult<OcrClient> {
    tokio::task::spawn_blocking(move || {
        let executable = std::env::current_exe()
            .map_err(|error| UseError::new("use.ocr.executable_missing", error.to_string()))?;
        let root = executable.parent().ok_or_else(|| {
            UseError::new(
                "use.ocr.executable_missing",
                "OCR executable has no parent directory.",
            )
        })?;
        let models = root.join("ocr-models").join(crate::config::MODEL_FAMILY);
        if models
            .try_exists()
            .map_err(|error| UseError::new("use.ocr.model_invalid", error.to_string()))?
        {
            let runtime = if cfg!(target_os = "macos") {
                "libonnxruntime.dylib"
            } else if cfg!(windows) {
                "onnxruntime.dll"
            } else if cfg!(target_os = "linux") {
                "libonnxruntime.so"
            } else {
                return Err(UseError::new(
                    "use.ocr.platform_unsupported",
                    "Unsupported OCR platform.",
                ));
            };
            return OcrClient::with_provider(PpOcrV6Provider::from_paths_with_profile(
                &models,
                &root.join(runtime),
                profile,
            )?);
        }
        if profile == PpOcrV6Profile::Document {
            return Err(UseError::new(
                "use.ocr.model_missing",
                "The OCR plugin's packaged models are missing.",
            ));
        }
        OcrClient::from_env()
    })
    .await
    .map_err(|error| UseError::new("use.ocr.runtime_failed", error.to_string()))?
}

fn output_error(error: serde_json::Error) -> UseError {
    UseError::new(
        "use.ocr.output_invalid",
        format!("Failed to encode OCR command output: {error}"),
    )
}

fn usage_error(message: impl Into<String>) -> UseError {
    UseError::new("use.ocr.usage_invalid", message).with_suggestion("Run 'aster-use-ocr --help'.")
}

#[cfg(test)]
mod tests;
