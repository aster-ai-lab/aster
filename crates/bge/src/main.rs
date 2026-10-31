#[cfg(feature = "cli")]
#[tokio::main]
async fn main() -> std::process::ExitCode {
    let json = std::env::args().any(|argument| argument == "--json");
    match aster_use_bge::cli::run().await {
        Ok(output) => {
            if output.should_print && json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&output.json).unwrap_or_default()
                );
            } else if output.should_print && !output.human.is_empty() {
                println!("{}", output.human);
            }
            std::process::ExitCode::from(output.exit_code)
        }
        Err(error) => {
            if json {
                let output = serde_json::json!({
                    "schemaVersion": 1,
                    "ok": false,
                    "error": error,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&output).unwrap_or_default()
                );
            } else {
                eprintln!("aster-use-bge: {error}");
                if let Some(suggestion) = &error.suggestion {
                    eprintln!("suggestion: {suggestion}");
                }
            }
            std::process::ExitCode::from(1)
        }
    }
}

#[cfg(not(feature = "cli"))]
compile_error!("The aster-use-bge binary requires the cli feature");
