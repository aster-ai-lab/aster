//! `aster-runner`: claims pipeline jobs and runs each step in a fresh
//! aster-box MicroVM.
//!
//! Exit codes: 0 after shutdown (or after the job with `--once`), 1 on a
//! setup or job error, 2 when the API refuses the runner's credential.

mod api;
mod checkout;
mod config;
mod executor;
mod job;
mod protocol;
mod runner;
mod state;
#[cfg(all(test, unix))]
mod test_support;

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use tokio::sync::watch;

use crate::api::HttpApi;
use crate::checkout::GitCheckout;
use crate::executor::{resolve_box_cli, BoxExecutor, BoxSettings};
use crate::runner::{ensure_credential, note, Runner, RunnerError};
use crate::state::REGISTRATION_TOKEN_ENV;

const USAGE: &str = "usage: aster-runner --config <runner.acl> [--once]";

struct Arguments {
    config: PathBuf,
    once: bool,
}

fn parse_arguments(args: impl Iterator<Item = String>) -> Result<Arguments, String> {
    let mut config = None;
    let mut once = false;
    let mut args = args;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => config = Some(args.next().ok_or(USAGE)?),
            "--once" => once = true,
            _ => return Err(USAGE.into()),
        }
    }
    Ok(Arguments {
        config: PathBuf::from(config.ok_or(USAGE)?),
        once,
    })
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error @ RunnerError::Revoked(_)) => {
            note(&error.to_string());
            ExitCode::from(2)
        }
        Err(error) => {
            note(&error.to_string());
            ExitCode::from(1)
        }
    }
}

async fn run() -> Result<(), RunnerError> {
    let arguments = parse_arguments(std::env::args().skip(1)).map_err(RunnerError::Setup)?;
    let text = std::fs::read_to_string(&arguments.config)
        .map_err(|error| RunnerError::Setup(format!("{}: {error}", arguments.config.display())))?;
    let config = config::parse_config(&text).map_err(RunnerError::Setup)?;
    let cli = resolve_box_cli(config.box_cli.as_deref()).map_err(RunnerError::Setup)?;
    let api = Arc::new(HttpApi::new(&config.api_url).map_err(RunnerError::Setup)?);
    let registration = std::env::var(REGISTRATION_TOKEN_ENV).ok();
    let state = ensure_credential(api.as_ref(), &config, registration).await?;
    let (shutdown_tx, shutdown) = watch::channel(false);
    tokio::spawn(async move {
        wait_for_signal().await;
        note("shutting down");
        let _ = shutdown_tx.send(true);
    });
    let executor = BoxExecutor::new(
        cli,
        BoxSettings {
            image: config.image.clone(),
            cpus: config.cpus,
            memory: config.memory.clone(),
        },
    );
    note(&format!(
        "runner {} polling {}",
        state.runner_id, config.api_url
    ));
    Runner {
        api,
        checkout: GitCheckout,
        executor,
        config,
        state,
    }
    .run(arguments.once, shutdown)
    .await
}

#[cfg(unix)]
async fn wait_for_signal() {
    use tokio::signal::unix::{signal, SignalKind};
    match signal(SignalKind::terminate()) {
        Ok(mut terminate) => {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = terminate.recv() => {}
            }
        }
        Err(_) => {
            let _ = tokio::signal::ctrl_c().await;
        }
    }
}

#[cfg(not(unix))]
async fn wait_for_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
