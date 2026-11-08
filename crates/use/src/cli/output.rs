use std::io::{self, Write};

use aster_use_core::UseResult;

use super::CommandOutput;

/// Render the standalone and embedded CLI wire contract and return its exit code.
/// Output failures preserve their I/O or serialization source. No process exits.
pub fn write_result(
    json: bool,
    result: UseResult<CommandOutput>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> io::Result<u8> {
    match result {
        Ok(output) => {
            if output.should_print && json {
                let text = serde_json::to_string_pretty(&output.json).map_err(io::Error::other)?;
                writeln!(stdout, "{text}")?;
            } else if output.should_print && !output.human.is_empty() {
                writeln!(stdout, "{}", output.human)?;
            }
            Ok(output.exit_code)
        }
        Err(error) => {
            if json {
                let output = serde_json::json!({"schemaVersion": 1, "ok": false, "error": error});
                let text = serde_json::to_string_pretty(&output).map_err(io::Error::other)?;
                writeln!(stdout, "{text}")?;
            } else {
                writeln!(stderr, "aster-use: {error}")?;
                if let Some(suggestion) = &error.suggestion {
                    writeln!(stderr, "suggestion: {suggestion}")?;
                }
            }
            Ok(1)
        }
    }
}

#[cfg(test)]
mod tests;
