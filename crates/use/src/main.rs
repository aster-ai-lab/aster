use std::process::ExitCode;

fn main() -> ExitCode {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let json = args.iter().any(|argument| argument == "--json");
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(aster_use::cli::CLI_WORKER_STACK_SIZE)
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            return startup_error(json, format!("could not start async runtime: {error}"))
        }
    };

    // Windows gives the process entry thread a smaller stack than Unix. Poll
    // the schema-v3 command graph on an explicitly sized Tokio worker while
    // the entry thread waits only on the small join handle.
    let command = runtime.spawn(aster_use::cli::run_os(args));
    match runtime.block_on(command) {
        Ok(result) => render_result(json, result),
        Err(error) => startup_error(json, format!("async command worker failed: {error}")),
    }
}

fn render_result(
    json: bool,
    result: aster_use_core::UseResult<aster_use::cli::CommandOutput>,
) -> ExitCode {
    match aster_use::cli::write_result(
        json,
        result,
        &mut std::io::stdout().lock(),
        &mut std::io::stderr().lock(),
    ) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("aster-use: could not write command output: {error}");
            ExitCode::from(1)
        }
    }
}

fn startup_error(json: bool, message: String) -> ExitCode {
    if json {
        let output = serde_json::json!({
            "schemaVersion": 1,
            "ok": false,
            "error": {
                "code": "use.runtime_start_failed",
                "message": message,
            },
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&output).unwrap_or_default()
        );
    } else {
        eprintln!("aster-use: {message}");
    }
    ExitCode::from(1)
}
