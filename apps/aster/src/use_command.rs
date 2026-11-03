//! Package commands reuse the standalone SDK's parser and output contract.

use anyhow::{Context, Result};
use aster_pager::env_manager_cmd::UseForwardArgs;
use aster_use::core::UseError;

pub(super) async fn run(args: UseForwardArgs) -> Result<()> {
    if std::env::var_os("ASTER_USE_BIN").is_some() {
        return aster_pager::env_manager_cmd::run_use_forward(args).await;
    }
    let json = args.args.iter().any(|argument| argument == "--json");
    let result =
        match tokio::spawn(async move { aster_use::cli::run_embedded_os(args.args).await }).await {
            Ok(result) => result,
            Err(error) => Err(UseError::new(
                "use.runtime_start_failed",
                format!("async command worker failed: {error}"),
            )),
        };
    let code = aster_use::cli::write_result(
        json,
        result,
        &mut std::io::stdout().lock(),
        &mut std::io::stderr().lock(),
    )
    .context("write package command output")?;
    if code != 0 {
        std::process::exit(i32::from(code));
    }
    Ok(())
}
