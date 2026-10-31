//! Explicit ACL and private credential lease for the native composition root.
use crate::{prompt::ThinkingMode, replicas::XqReplicaLayout};
use aster_acl::{AttributeSchema, Document, ParseLimits, Schema, Value, ValueSchema};
use aster_power::{
    error::{PowerError, Result},
    server::auth::{ApiKeyAuth, AuthProvider},
};
use std::{
    io::Read,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const ACL_LIMITS: ParseLimits = ParseLimits {
    max_document_bytes: 16384,
    max_nesting_depth: 2,
    max_collection_items: 32,
    max_token_bytes: 4096,
    max_diagnostics: 8,
};

pub struct NativeServingConfig {
    pub payload_root: PathBuf,
    pub official_blob_root: PathBuf,
    pub tokenizer_file: String,
    pub tokenizer_bytes: u64,
    pub auth_root: PathBuf,
    pub layout: XqReplicaLayout,
    pub context_tokens: usize,
    pub max_generated_tokens: usize,
    pub capacity_per_replica: usize,
    pub load_seconds: u64,
    pub mode: ThinkingMode,
    pub effort: u8,
}
impl NativeServingConfig {
    pub fn from_acl(input: &str) -> Result<Self> {
        let doc = document(
            input,
            &[
                "schema",
                "model",
                "revision",
                "payload_root",
                "official_blob_root",
                "tokenizer_file",
                "auth_root",
                "layout",
                "thinking_mode",
            ],
            &[
                "tokenizer_bytes",
                "context_tokens",
                "max_generated_tokens",
                "capacity_per_replica",
                "load_seconds",
                "reasoning_effort",
            ],
        )?;
        let model = crate::model::DeepSeekModel::V41Flash1017Xq;
        if string(&doc, "schema")? != "aster.deepseek.xq.serving.v1"
            || string(&doc, "model")? != model.as_str()
            || string(&doc, "revision")? != model.revision()
        {
            return Err(config_error(
                "native serving requires the exact XQ schema/model/revision",
            ));
        }
        let path = |name| -> Result<PathBuf> {
            let value = PathBuf::from(string(&doc, name)?);
            if !value.is_absolute() {
                return Err(config_error("native hosting paths must be absolute"));
            }
            Ok(value)
        };
        let config = Self {
            payload_root: path("payload_root")?,
            official_blob_root: path("official_blob_root")?,
            auth_root: path("auth_root")?,
            tokenizer_file: string(&doc, "tokenizer_file")?.into(),
            tokenizer_bytes: number(&doc, "tokenizer_bytes", 1, 16 * 1024 * 1024)?,
            layout: XqReplicaLayout::parse(string(&doc, "layout")?)?,
            context_tokens: number(&doc, "context_tokens", 8, 256)? as usize,
            max_generated_tokens: number(&doc, "max_generated_tokens", 1, 32)? as usize,
            capacity_per_replica: number(&doc, "capacity_per_replica", 1, 4)? as usize,
            load_seconds: number(&doc, "load_seconds", 1, 3600)?,
            mode: ThinkingMode::parse(string(&doc, "thinking_mode")?)?,
            effort: number(&doc, "reasoning_effort", 1, 100)? as u8,
        };
        if config.max_generated_tokens >= config.context_tokens {
            return Err(config_error(
                "native generation bound must leave room for a nonempty prompt",
            ));
        }
        Ok(config)
    }
}

fn document(input: &str, strings: &[&str], numbers: &[&str]) -> Result<Document> {
    let document = aster_acl::parse_with_limits(input, ACL_LIMITS)
        .map_err(|error| PowerError::Acl(error.to_string()))?;
    let mut schema = Schema::new();
    for &name in strings {
        schema = schema.attribute(name, AttributeSchema::required(ValueSchema::string()));
    }
    for &name in numbers {
        schema = schema.attribute(name, AttributeSchema::required(ValueSchema::number()));
    }
    let report = aster_acl::validate_document_with_limits(&document, &schema, ACL_LIMITS);
    if !report.is_empty() {
        return Err(config_error("hosting ACL violates its closed schema"));
    }
    Ok(document)
}
fn field<'a>(document: &'a Document, name: &str) -> Result<&'a Value> {
    document
        .blocks
        .iter()
        .find(|block| block.name == name)
        .and_then(|block| block.attributes.get(name))
        .ok_or_else(|| config_error("required hosting ACL field is missing"))
}
fn string<'a>(document: &'a Document, name: &str) -> Result<&'a str> {
    field(document, name)?
        .as_str()
        .ok_or_else(|| config_error("hosting ACL requires a string"))
}
fn number(document: &Document, name: &str, minimum: u64, maximum: u64) -> Result<u64> {
    let Value::Number(value) = field(document, name)? else {
        return Err(config_error("hosting ACL requires an integer"));
    };
    if !value.is_finite()
        || value.fract() != 0.0
        || *value < minimum as f64
        || *value > maximum as f64
    {
        return Err(config_error("hosting ACL integer exceeds its bound"));
    }
    Ok(*value as u64)
}

struct CredentialLease {
    hash: String,
    starts: u64,
    expires: u64,
}
impl CredentialLease {
    fn from_acl(input: &str) -> Result<Self> {
        let doc = document(
            input,
            &["schema", "api_key_sha256"],
            &["starts_unix", "expires_unix"],
        )?;
        let hash = string(&doc, "api_key_sha256")?;
        let starts = number(&doc, "starts_unix", 1, 9_007_199_254_740_991)?;
        let expires = number(&doc, "expires_unix", 1, 9_007_199_254_740_991)?;
        if string(&doc, "schema")? != "aster.deepseek.credential-lease.v1"
            || hash.len() != 64
            || !hash
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || expires <= starts
            || expires - starts > 7200
        {
            return Err(config_error("credential lease requires a lowercase hash and an absolute window no longer than two hours"));
        }
        Ok(Self {
            hash: hash.into(),
            starts,
            expires,
        })
    }
    fn active(&self, now: u64) -> bool {
        self.starts <= now && now < self.expires
    }
}

/// The host atomically publishes credential.acl; restart never resets its
/// absolute window. Creating `revoked` in this same private directory revokes
/// the lease immediately. Missing/malformed/insecure files deny authentication.
pub struct FileLeaseAuth {
    root: PathBuf,
}
impl FileLeaseAuth {
    pub fn new(root: PathBuf) -> Result<Self> {
        private_directory(&root)?;
        Ok(Self { root })
    }
    fn current(&self) -> Option<ApiKeyAuth> {
        private_directory(&self.root).ok()?;
        match std::fs::symlink_metadata(self.root.join("revoked")) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            _ => return None,
        }
        let input = read_private_acl(&self.root.join("credential.acl"), 4096).ok()?;
        let lease = CredentialLease::from_acl(&input).ok()?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
        if !lease.active(now) {
            return None;
        }
        Some(ApiKeyAuth::new(&[lease.hash]))
    }
}
impl AuthProvider for FileLeaseAuth {
    fn authenticate(&self, token: &str) -> bool {
        self.current().is_some_and(|auth| auth.authenticate(token))
    }
    fn identify(&self, token: &str) -> Option<String> {
        self.current()?.identify(token)
    }
}

pub fn read_private_acl(path: &Path, maximum: u64) -> Result<String> {
    if maximum == 0 || maximum > 16384 {
        return Err(config_error(
            "private ACL read bound must be 1..16384 bytes",
        ));
    }
    let metadata = std::fs::symlink_metadata(path)?;
    private_metadata(path, &metadata)?;
    if !metadata.is_file() || metadata.len() > maximum {
        return Err(config_error("private ACL must be a bounded regular file"));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(maximum + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum {
        return Err(config_error("private ACL grew beyond its read bound"));
    }
    String::from_utf8(bytes).map_err(|_| config_error("private ACL must be UTF-8"))
}
fn private_directory(path: &Path) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    private_metadata(path, &metadata)?;
    if !metadata.is_dir() {
        return Err(config_error("credential root must be a private directory"));
    }
    Ok(())
}
#[cfg(unix)]
fn private_metadata(path: &Path, metadata: &std::fs::Metadata) -> Result<()> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let owner = std::fs::metadata("/proc/self")?.uid();
    if path.canonicalize()? != path
        || metadata.file_type().is_symlink()
        || metadata.uid() != owner
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(config_error(
            "hosting artifact requires its canonical path, process UID and private permissions",
        ));
    }
    Ok(())
}
#[cfg(not(unix))]
fn private_metadata(_path: &Path, _metadata: &std::fs::Metadata) -> Result<()> {
    Err(config_error(
        "native H200 hosting requires Unix ownership enforcement",
    ))
}
fn config_error(message: &str) -> PowerError {
    PowerError::Config(message.into())
}

#[cfg(test)]
#[path = "hosting/tests.rs"]
mod tests;
