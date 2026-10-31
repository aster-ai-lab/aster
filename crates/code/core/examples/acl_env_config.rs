//! Release-only ACL env injection. Credentials never cross stdout or argv.

use aster_acl::{Block, Document, Value};
use aster_core::config::CodeConfig;
use anyhow::{anyhow, bail, ensure, Context, Result};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::Path,
    process::{Command, ExitCode},
};

#[path = "acl_env_config/tests.rs"]
#[cfg(test)]
mod tests;

const MAX_CONFIG_BYTES: u64 = 1024 * 1024;

enum Mode {
    Check,
    Run,
}

impl Mode {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "check" => Ok(Self::Check),
            "run" => Ok(Self::Run),
            _ => bail!("ACL helper mode must be check or run"),
        }
    }
    fn as_str(&self) -> &'static str {
        match self {
            Self::Check => "check",
            Self::Run => "run",
        }
    }
}

struct Prepared {
    document: Document,
    api_key: Option<String>,
    base_url: Option<String>,
    environment: BTreeMap<String, Option<String>>,
}

fn nonempty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

fn env_attribute(name: &str) -> Value {
    Value::Call("env".to_owned(), vec![Value::String(name.to_owned())])
}

fn rewrite_credentials(block: &mut Block, api_key: Option<&str>, base_url: Option<&str>) {
    for (name, alias, value) in [
        ("api_key", "apiKey", api_key),
        ("base_url", "baseUrl", base_url),
    ] {
        if let Some(value) = value {
            block.attributes.remove(alias);
            block
                .attributes
                .insert(name.to_owned(), Value::String(value.to_owned()));
        }
    }
}

fn bind_environment(
    document: &mut Document,
    environment: &mut BTreeMap<String, Option<String>>,
    lookup: &impl Fn(&str) -> Option<String>,
) {
    fn visit(
        value: &mut Value,
        lift: bool,
        vars: &mut BTreeMap<String, Option<String>>,
        lookup: &impl Fn(&str) -> Option<String>,
    ) {
        match value {
            Value::String(secret) if lift => {
                let mut index = vars.len();
                let name = loop {
                    let name = format!("ASTER_ACL_VALUE_{index}");
                    if !vars.contains_key(&name) {
                        break name;
                    }
                    index += 1;
                };
                vars.insert(name.clone(), Some(secret.clone()));
                *value = env_attribute(&name);
            }
            Value::Call(name, args) if name == "env" => {
                let first_is_name = matches!(args.first(), Some(Value::String(_)));
                if let Some(Value::String(name)) = args.first() {
                    vars.entry(name.clone()).or_insert_with(|| lookup(name));
                }
                for argument in args.iter_mut().skip(usize::from(first_is_name)) {
                    visit(argument, lift, vars, lookup);
                }
            }
            Value::List(values) | Value::Call(_, values) => {
                for value in values {
                    visit(value, lift, vars, lookup);
                }
            }
            Value::Object(values) => {
                for (_, value) in values {
                    visit(value, lift, vars, lookup);
                }
            }
            _ => {}
        }
    }
    fn visit_blocks(
        blocks: &mut [Block],
        vars: &mut BTreeMap<String, Option<String>>,
        lookup: &impl Fn(&str) -> Option<String>,
        lift: bool,
    ) {
        for block in blocks {
            for (key, value) in &mut block.attributes {
                let credential = matches!(
                    key.as_str(),
                    "api_key"
                        | "apiKey"
                        | "base_url"
                        | "baseUrl"
                        | "headers"
                        | "env"
                        | "client_secret"
                        | "clientSecret"
                        | "access_token"
                        | "accessToken"
                );
                visit(
                    value,
                    lift && (credential || matches!(block.name.as_str(), "headers" | "env")),
                    vars,
                    lookup,
                );
            }
            visit_blocks(&mut block.blocks, vars, lookup, lift);
        }
    }
    // Reserve existing env names before generating names for literal values.
    visit_blocks(&mut document.blocks, environment, lookup, false);
    visit_blocks(&mut document.blocks, environment, lookup, true);
}

fn prepare(content: &str, env: impl Fn(&str) -> Option<String>) -> Result<Prepared> {
    // Do not include parser diagnostics: malformed input can contain secrets.
    let config = CodeConfig::from_acl(content).map_err(|_| anyhow!("invalid ACL configuration"))?;
    let (provider, model) = config
        .default_model_config()
        .ok_or_else(|| anyhow!("configured default provider/model is missing"))?;
    let mut document = aster_acl::parse(content).map_err(|_| anyhow!("invalid ACL configuration"))?;
    // This is an environment-variable spelling, never an entity classifier.
    let provider_env: String = provider
        .name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect();
    let api_key = nonempty(env(&format!("ASTER_{provider_env}_API_KEY")))
        .or_else(|| nonempty(provider.get_api_key(model).map(str::to_owned)))
        .or_else(|| nonempty(env("ASTER_OPENAI_API_KEY")));
    let base_url = nonempty(env(&format!("ASTER_{provider_env}_BASE_URL")))
        .or_else(|| nonempty(provider.get_base_url(model).map(str::to_owned)))
        .or_else(|| nonempty(env("ASTER_OPENAI_BASE_URL")));
    let mut matches = document
        .blocks
        .iter_mut()
        .filter(|block| block.name == "providers" && block.labels.first() == Some(&provider.name));
    let block = matches
        .next()
        .ok_or_else(|| anyhow!("default provider ACL block is missing"))?;
    ensure!(
        matches.next().is_none(),
        "default provider ACL block is ambiguous"
    );
    // Override only the selected model; siblings must retain their own credentials.
    let mut selected = 0;
    for child in &mut block.blocks {
        if child.name == "models" && child.labels.first() == Some(&model.id) {
            selected += 1;
            rewrite_credentials(child, api_key.as_deref(), base_url.as_deref());
        }
    }
    ensure!(
        selected == 1,
        "default model ACL block is missing or ambiguous"
    );
    let mut environment = BTreeMap::new();
    bind_environment(&mut document, &mut environment, &env);
    Ok(Prepared {
        document,
        api_key,
        base_url,
        environment,
    })
}

fn read_config(path: &Path) -> Result<String> {
    ensure!(
        path.extension().is_some_and(|extension| extension == "acl"),
        "configuration must use .acl"
    );
    let before = fs::symlink_metadata(path).context("cannot inspect ACL file")?;
    ensure!(before.is_file(), "ACL source must be a regular file");
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path).context("cannot open ACL file")?;
    let opened = file.metadata().context("cannot inspect opened ACL file")?;
    ensure!(opened.is_file(), "opened ACL source must be a regular file");
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        ensure!(
            before.dev() == opened.dev() && before.ino() == opened.ino(),
            "ACL source changed during open"
        );
    }
    let mut bytes = Vec::new();
    file.take(MAX_CONFIG_BYTES + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= MAX_CONFIG_BYTES,
        "ACL source exceeds 1 MiB"
    );
    String::from_utf8(bytes).map_err(|_| anyhow!("ACL source must be UTF-8"))
}

fn run() -> Result<i32> {
    let mut args = std::env::args_os().skip(1);
    let mode = args
        .next()
        .ok_or_else(|| anyhow!("ACL helper mode is required"))?;
    let mode = Mode::parse(mode.to_str().unwrap_or_default())?;
    let path = args.next().ok_or_else(|| anyhow!("ACL file is required"))?;
    let prepared = prepare(&read_config(Path::new(&path))?, |name| {
        std::env::var(name).ok()
    })?;
    match mode {
        Mode::Check => {
            ensure!(args.next().is_none(), "check accepts only an ACL file");
            println!(
                "{}",
                u8::from(prepared.api_key.is_some() && prepared.base_url.is_some())
            );
            Ok(0)
        }
        Mode::Run => {
            let command: OsString = args
                .next()
                .ok_or_else(|| anyhow!("run requires a child command"))?;
            let mut temporary = tempfile::Builder::new()
                .prefix("aster-env-")
                .suffix(".acl")
                .tempfile()?;
            // NamedTempFile is exclusively created with mode 0600 on Unix and
            // is retained until the child exits; errors and signals fail closed.
            temporary.write_all(aster_acl::generate(&prepared.document).as_bytes())?;
            temporary.flush()?;
            let mut child = Command::new(command);
            child
                .args(args)
                .env("ASTER_CONFIG_FILE", temporary.path())
                .env("ASTER_ACL_ENV_PREPARED", "1")
                .env(
                    "ASTER_ACL_ENV_NAMES",
                    serde_json::to_string(&prepared.environment.keys().collect::<Vec<_>>())?,
                );
            for (name, value) in prepared.environment {
                if let Some(value) = value {
                    child.env(name, value);
                } else {
                    child.env_remove(name);
                }
            }
            let status = child
                .status()
                .with_context(|| format!("{} child could not start", mode.as_str()))?;
            Ok(status.code().unwrap_or(1))
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        Err(error) => {
            eprintln!("ACL env preparation failed: {error}");
            ExitCode::FAILURE
        }
    }
}
