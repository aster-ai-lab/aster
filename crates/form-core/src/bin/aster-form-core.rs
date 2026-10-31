use std::env;
use std::io::{self, Read, Write};
use std::process::ExitCode;

use aster_form_core::{
    canonicalize_json, compile_bytes, digest_document_json, evaluate_bytes,
    json_render_component_catalog, ABSOLUTE_MAX_REQUEST_BYTES,
};

type BoxError = Box<dyn std::error::Error + Send + Sync>;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("aster-form-core: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), BoxError> {
    let mut arguments = env::args();
    let _program = arguments.next();
    let command = arguments.next().ok_or(
        "missing command; expected `canonicalize`, `digest-document`, `compile`, `evaluate`, `json-render-catalog`",
    )?;
    if arguments.next().is_some() {
        return Err("unexpected command arguments".into());
    }

    let mut input = Vec::new();
    io::stdin()
        .take((ABSOLUTE_MAX_REQUEST_BYTES + 1) as u64)
        .read_to_end(&mut input)?;

    let output = match command.as_str() {
        "canonicalize" => canonicalize_json(&input)?,
        "compile" => compile_bytes(&input)?,
        "evaluate" => evaluate_bytes(&input)?,
        "json-render-catalog" => serde_json::to_vec_pretty(&json_render_component_catalog())?,
        "digest-document" => {
            let mut digest = digest_document_json(&input)?.into_bytes();
            digest.push(b'\n');
            digest
        }
        _ => return Err(format!("unsupported command {command:?}").into()),
    };
    io::stdout().write_all(&output)?;
    Ok(())
}
