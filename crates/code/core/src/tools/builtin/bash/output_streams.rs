//! Stream boundaries over the retained command text, without duplicating output.

use serde_json::{json, Value};

const SCHEMA: &str = "a3s.code.shell-output-streams.v1";

pub(crate) fn capture(stdout: &str, stderr: &str) -> Value {
    json!({
        "schema": SCHEMA,
        "stdout": {"text_bytes":stdout.len(), "sha256":sha256::digest(stdout)},
        "stderr": {"text_bytes":stderr.len(), "sha256":sha256::digest(stderr)},
    })
}

pub(crate) fn split<'a>(output: &'a str, streams: &Value) -> Option<(&'a str, &'a str)> {
    if streams["schema"].as_str()? != SCHEMA {
        return None;
    }
    let stdout_bytes = usize::try_from(streams["stdout"]["text_bytes"].as_u64()?).ok()?;
    let stderr_bytes = usize::try_from(streams["stderr"]["text_bytes"].as_u64()?).ok()?;
    let (stdout, stderr) = output.split_at_checked(stdout_bytes)?;
    if stderr.len() != stderr_bytes
        || sha256::digest(stdout) != streams["stdout"]["sha256"].as_str()?
        || sha256::digest(stderr) != streams["stderr"]["sha256"].as_str()?
    {
        return None;
    }
    Some((stdout, stderr))
}

#[cfg(test)]
#[path = "output_streams_tests.rs"]
mod tests;
