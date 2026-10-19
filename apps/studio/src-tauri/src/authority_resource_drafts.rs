//! Durable authoring recovery, separate from revocable view authority and UI cache.

use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::PathBuf,
    sync::Mutex,
};

use a3s_acl::{self as acl, Block, Document, Value};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub(super) const MAX_DRAFT_BYTES: usize = 64 * 1024 * 1024;
const MAX_FILE_BYTES: u64 = 6 * MAX_DRAFT_BYTES as u64 + 4096;
const MAX_DRAFTS: usize = 32;
const MAX_REVISION: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, Default, Serialize)]
pub(super) struct Snapshot {
    revision: u64,
    content: Option<String>,
    #[serde(skip)]
    operation: Option<Uuid>,
    #[serde(skip)]
    expected: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Commit {
    expected_revision: u64,
    operation_id: Uuid,
    content: Option<String>,
}

pub(super) struct DraftStore {
    root: Option<PathBuf>,
    memory: Mutex<BTreeMap<(String, String), Snapshot>>,
}

fn invalid() -> io::Error {
    io::ErrorKind::InvalidInput.into()
}
fn conflict() -> io::Error {
    io::ErrorKind::AlreadyExists.into()
}

impl DraftStore {
    pub fn new(root: Option<PathBuf>) -> Self {
        Self {
            root,
            memory: Mutex::new(BTreeMap::new()),
        }
    }

    pub fn scope(member: &str, organization: Uuid, package: &str, surface: &str) -> String {
        let mut digest = Sha256::new();
        for value in [member, &organization.to_string(), package, surface] {
            digest.update((value.len() as u64).to_be_bytes());
            digest.update(value.as_bytes());
        }
        format!("{:x}", digest.finalize())
    }

    fn locked<T>(
        &self,
        scope: &str,
        key: &str,
        action: impl FnOnce(
            Snapshot,
            Option<PathBuf>,
            &mut BTreeMap<(String, String), Snapshot>,
        ) -> io::Result<T>,
    ) -> io::Result<T> {
        if scope.len() != 64
            || !scope.bytes().all(|byte| byte.is_ascii_hexdigit())
            || !super::key_ok(key)
        {
            return Err(invalid());
        }
        let mut memory = self
            .memory
            .lock()
            .map_err(|_| io::Error::other("draft lock unavailable"))?;
        let Some(root) = &self.root else {
            if !cfg!(test) {
                return Err(io::ErrorKind::NotFound.into());
            }
            let current = memory
                .get(&(scope.into(), key.into()))
                .cloned()
                .unwrap_or_default();
            return action(current, None, &mut memory);
        };
        let directory = root.join(scope);
        fs::create_dir_all(&directory)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let lock = options.open(directory.join("drafts.lock"))?;
        lock.lock()?;
        let path = directory.join(format!("{:x}.acl", Sha256::digest(key.as_bytes())));
        let current = match fs::File::open(&path) {
            Ok(file) => {
                let mut content = String::new();
                file.take(MAX_FILE_BYTES + 1).read_to_string(&mut content)?;
                decode(&content)?
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Snapshot::default(),
            Err(error) => return Err(error),
        };
        let files = fs::read_dir(&directory)?.collect::<io::Result<Vec<_>>>()?;
        if !path.exists()
            && files
                .iter()
                .filter(|entry| entry.path().extension().is_some_and(|value| value == "acl"))
                .count()
                >= MAX_DRAFTS
        {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        action(current, Some(path), &mut memory)
    }

    pub fn read(&self, scope: &str, key: &str) -> io::Result<Snapshot> {
        self.locked(scope, key, |snapshot, _, _| Ok(snapshot))
    }

    pub fn commit(&self, scope: &str, key: &str, input: Commit) -> io::Result<Snapshot> {
        if input.operation_id.is_nil()
            || input.expected_revision >= MAX_REVISION
            || input
                .content
                .as_ref()
                .is_some_and(|text| text.len() > MAX_DRAFT_BYTES)
        {
            return Err(invalid());
        }
        self.locked(scope, key, |current, path, memory| {
            if current.operation == Some(input.operation_id) {
                return if current.expected == input.expected_revision
                    && current.content == input.content
                {
                    Ok(current)
                } else {
                    Err(conflict())
                };
            }
            if current.revision != input.expected_revision {
                return Err(conflict());
            }
            let next = Snapshot {
                revision: current.revision + 1,
                content: input.content,
                operation: Some(input.operation_id),
                expected: input.expected_revision,
            };
            if let Some(path) = path {
                let directory = path.parent().ok_or_else(invalid)?;
                let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
                temporary.write_all(encode(&next).as_bytes())?;
                temporary.as_file().sync_all()?;
                temporary.persist(&path).map_err(|error| error.error)?;
                #[cfg(unix)]
                fs::File::open(directory)?.sync_all()?;
            } else {
                if !memory.contains_key(&(scope.into(), key.into()))
                    && memory.keys().filter(|(owner, _)| owner == scope).count() >= MAX_DRAFTS
                {
                    return Err(io::ErrorKind::WouldBlock.into());
                }
                memory.insert((scope.into(), key.into()), next.clone());
            }
            Ok(next)
        })
    }
}

fn encode(snapshot: &Snapshot) -> String {
    acl::generate_acl(&Document {
        blocks: vec![Block {
            name: "authoring_draft".into(),
            labels: vec![],
            blocks: vec![],
            attributes: [
                ("schema_version".into(), Value::Number(1.0)),
                (
                    "revision".into(),
                    Value::String(snapshot.revision.to_string()),
                ),
                (
                    "expected_revision".into(),
                    Value::String(snapshot.expected.to_string()),
                ),
                (
                    "operation_id".into(),
                    Value::String(
                        snapshot
                            .operation
                            .map(|id| id.to_string())
                            .unwrap_or_default(),
                    ),
                ),
                (
                    "content".into(),
                    snapshot
                        .content
                        .clone()
                        .map(Value::String)
                        .unwrap_or(Value::Null),
                ),
            ]
            .into(),
        }],
    })
}

fn decode(text: &str) -> io::Result<Snapshot> {
    let document = acl::parse_with_limits(
        text,
        acl::ParseLimits {
            max_document_bytes: MAX_FILE_BYTES as usize,
            max_token_bytes: MAX_FILE_BYTES as usize,
            max_nesting_depth: 2,
            max_collection_items: 8,
            ..Default::default()
        },
    )
    .map_err(|_| invalid())?;
    let [block] = document.blocks.as_slice() else {
        return Err(invalid());
    };
    if block.name != "authoring_draft"
        || !block.labels.is_empty()
        || !block.blocks.is_empty()
        || block.attributes.len() != 5
        || block
            .attributes
            .get("schema_version")
            .and_then(Value::as_number)
            != Some(1.0)
    {
        return Err(invalid());
    }
    let integer = |key| {
        block
            .attributes
            .get(key)
            .and_then(Value::as_str)
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| *value <= MAX_REVISION)
            .ok_or_else(invalid)
    };
    let revision = integer("revision")?;
    let expected = integer("expected_revision")?;
    let operation = block
        .attributes
        .get("operation_id")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .filter(|id| !id.is_nil())
        .ok_or_else(invalid)?;
    let content = match block.attributes.get("content") {
        Some(Value::Null) => None,
        Some(Value::String(value)) if value.len() <= MAX_DRAFT_BYTES => Some(value.clone()),
        _ => return Err(invalid()),
    };
    if revision != expected + 1 {
        return Err(invalid());
    }
    Ok(Snapshot {
        revision,
        content,
        operation: Some(operation),
        expected,
    })
}

#[cfg(test)]
#[path = "authority_resource_drafts_tests.rs"]
mod tests;
