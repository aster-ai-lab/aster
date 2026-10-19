use std::io::{self, Read};
use std::path::PathBuf;

use a3s_oci_runtime::{MacosHvfHostService, MacosHvfHostServiceConfig};
use a3s_oci_sdk::{Error, ErrorCode, Result};
use tokio::signal::unix::{signal, SignalKind};

#[cfg(test)]
#[path = "macos_hvf_service_tests.rs"]
mod tests;

pub(crate) async fn run(
    root: PathBuf,
    shim: PathBuf,
    system_image_manifest: PathBuf,
    exit_on_stdin_eof: bool,
) -> Result<()> {
    let mut interrupt =
        signal(SignalKind::interrupt()).map_err(|error| signal_error("SIGINT", error))?;
    let mut terminate =
        signal(SignalKind::terminate()).map_err(|error| signal_error("SIGTERM", error))?;
    let config = MacosHvfHostServiceConfig::new(root, shim, system_image_manifest)?;
    let service = MacosHvfHostService::bind(config).await?;
    let mut stdin_result = Ok(());
    service
        .serve_until(async {
            tokio::select! {
                _ = interrupt.recv() => {}
                _ = terminate.recv() => {}
                result = wait_for_stdin_eof(io::stdin()), if exit_on_stdin_eof => {
                    stdin_result = result;
                }
            }
        })
        .await?;
    stdin_result.map_err(|error| {
        Error::new(
            ErrorCode::Unavailable,
            format!("macOS HVF owner lifetime pipe failed: {error}"),
        )
        .for_operation("watch-macos-hvf-host-owner")
    })
}

async fn wait_for_stdin_eof(input: impl Read + Send + 'static) -> io::Result<()> {
    let (closed, receiver) = tokio::sync::oneshot::channel();
    // A signal can cancel this wait; Tokio's blocking pool must still shut down.
    std::thread::Builder::new()
        .name("aster-oci-owner-stdin".into())
        .spawn(move || {
            let mut input = input;
            let mut bytes = [0_u8; 64];
            let result = loop {
                match input.read(&mut bytes) {
                    Ok(0) => break Ok(()),
                    Ok(_) => {}
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                    Err(error) => break Err(error),
                }
            };
            let _ = closed.send(result);
        })?;
    receiver
        .await
        .map_err(|_| io::Error::other("owner lifetime reader stopped without a receipt"))?
}

fn signal_error(signal_name: &'static str, error: std::io::Error) -> Error {
    Error::new(
        ErrorCode::Unavailable,
        format!("failed to register macOS HVF host service {signal_name} handler: {error}"),
    )
    .for_operation("start-macos-hvf-host-service")
}
