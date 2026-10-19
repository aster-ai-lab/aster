//! Member-scoped Preference storage. ACL is committed before a snapshot is published.

use std::collections::{BTreeMap, HashMap};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use a3s_acl::{Block, Document, Value as AclValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{Emitter, Manager};

#[path = "preference_keys.generated.rs"]
mod keys;

const LIMIT: u64 = 1024 * 1024;
const EVENT: &str = "studio-preferences-committed";

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Snapshot {
    pub scope: String,
    pub revision: u64,
    pub values: BTreeMap<String, Value>,
}

#[derive(Debug, thiserror::Error, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub(crate) enum PreferenceError {
    #[error("Preference access refused")]
    Denied,
    #[error("Preference store unavailable")]
    Unavailable,
    #[error("Invalid preference data")]
    Invalid,
    #[error("Invalid Harness recipe")]
    InvalidHarnessRecipe,
    #[error("Preference revision changed")]
    Conflict { snapshot: Snapshot },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Commit {
    pub scope: String,
    pub expected_revision: u64,
    pub updates: BTreeMap<String, Value>,
}

enum HarnessRecipe {
    Default,
    Lean,
    Host,
}

impl HarnessRecipe {
    fn parse(value: &str) -> Result<Self, PreferenceError> {
        match value {
            value if value == Self::Default.as_str() => Ok(Self::Default),
            value if value == Self::Lean.as_str() => Ok(Self::Lean),
            value if value == Self::Host.as_str() => Ok(Self::Host),
            _ => Err(PreferenceError::InvalidHarnessRecipe),
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Lean => "lean",
            Self::Host => "host",
        }
    }
}

pub(crate) struct PreferenceStore {
    root: PathBuf,
}

impl PreferenceStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn locked<T>(
        &self,
        scope: &str,
        action: impl FnOnce(&Path, Snapshot) -> Result<T, PreferenceError>,
    ) -> Result<T, PreferenceError> {
        let id = uuid::Uuid::parse_str(scope).map_err(|_| PreferenceError::Denied)?;
        if id.to_string() != scope {
            return Err(PreferenceError::Denied);
        }
        fs::create_dir_all(&self.root).map_err(|_| PreferenceError::Unavailable)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let lock = options
            .open(self.root.join(format!("{scope}.lock")))
            .map_err(|_| PreferenceError::Unavailable)?;
        lock.lock().map_err(|_| PreferenceError::Unavailable)?;
        let path = self.root.join(format!("{scope}.acl"));
        let snapshot = read_snapshot(&path, scope)?;
        action(&path, snapshot)
    }

    pub fn read(&self, scope: &str) -> Result<Snapshot, PreferenceError> {
        self.locked(scope, |_, snapshot| Ok(snapshot))
    }

    pub fn commit(&self, input: &Commit) -> Result<Snapshot, PreferenceError> {
        validate_updates(&input.updates)?;
        self.locked(&input.scope, |path, mut snapshot| {
            if snapshot.revision != input.expected_revision {
                return Err(PreferenceError::Conflict { snapshot });
            }
            if input
                .updates
                .iter()
                .all(|(key, value)| snapshot.values.get(key) == Some(value))
            {
                return Ok(snapshot);
            }
            snapshot.values.extend(input.updates.clone());
            snapshot.revision = snapshot
                .revision
                .checked_add(1)
                .ok_or(PreferenceError::Invalid)?;
            let text = encode(&snapshot)?;
            let mut temporary = tempfile::NamedTempFile::new_in(&self.root)
                .map_err(|_| PreferenceError::Unavailable)?;
            temporary
                .write_all(text.as_bytes())
                .map_err(|_| PreferenceError::Unavailable)?;
            temporary
                .as_file()
                .sync_all()
                .map_err(|_| PreferenceError::Unavailable)?;
            temporary
                .persist(path)
                .map_err(|_| PreferenceError::Unavailable)?;
            Ok(snapshot)
        })
    }
}

fn validate_updates(values: &BTreeMap<String, Value>) -> Result<(), PreferenceError> {
    for (key, value) in values {
        if keys::PREFERENCE_KEYS.binary_search(&key.as_str()).is_err() {
            return Err(PreferenceError::Invalid);
        }
        if key == "app.harness.assemble" {
            HarnessRecipe::parse(
                value
                    .as_str()
                    .ok_or(PreferenceError::InvalidHarnessRecipe)?,
            )?;
        }
        to_acl(value, 0)?;
    }
    Ok(())
}

fn read_snapshot(path: &Path, scope: &str) -> Result<Snapshot, PreferenceError> {
    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Snapshot {
                scope: scope.to_owned(),
                revision: 0,
                values: BTreeMap::new(),
            });
        }
        Err(_) => return Err(PreferenceError::Unavailable),
    };
    let mut text = String::new();
    Read::by_ref(&mut file)
        .take(LIMIT + 1)
        .read_to_string(&mut text)
        .map_err(|_| PreferenceError::Invalid)?;
    if text.len() as u64 > LIMIT {
        return Err(PreferenceError::Invalid);
    }
    let document = a3s_acl::parse(&text).map_err(|_| PreferenceError::Invalid)?;
    let [block] = document.blocks.as_slice() else {
        return Err(PreferenceError::Invalid);
    };
    if block.name != "preferences" || block.labels != [scope] || block.attributes.len() != 1 {
        return Err(PreferenceError::Invalid);
    }
    let revision = block
        .attributes
        .get("revision")
        .and_then(AclValue::as_str)
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(PreferenceError::Invalid)?;
    let mut values = BTreeMap::new();
    for item in &block.blocks {
        let [key] = item.labels.as_slice() else {
            return Err(PreferenceError::Invalid);
        };
        if item.name != "preference" || !item.blocks.is_empty() || item.attributes.len() != 1 {
            return Err(PreferenceError::Invalid);
        }
        let value = from_acl(
            item.attributes
                .get("value")
                .ok_or(PreferenceError::Invalid)?,
            0,
        )?;
        if values.insert(key.clone(), value).is_some() {
            return Err(PreferenceError::Invalid);
        }
    }
    validate_updates(&values)?;
    Ok(Snapshot {
        scope: scope.to_owned(),
        revision,
        values,
    })
}

fn encode(snapshot: &Snapshot) -> Result<String, PreferenceError> {
    let mut blocks = Vec::new();
    for (key, value) in &snapshot.values {
        blocks.push(Block {
            name: "preference".to_owned(),
            labels: vec![key.clone()],
            blocks: vec![],
            attributes: HashMap::from([("value".to_owned(), to_acl(value, 0)?)]),
        });
    }
    let text = a3s_acl::generate(&Document {
        blocks: vec![Block {
            name: "preferences".to_owned(),
            labels: vec![snapshot.scope.clone()],
            blocks,
            attributes: HashMap::from([(
                "revision".to_owned(),
                AclValue::String(snapshot.revision.to_string()),
            )]),
        }],
    });
    if text.len() as u64 > LIMIT {
        return Err(PreferenceError::Invalid);
    }
    Ok(text)
}

// Object entries keep arbitrary user map keys out of ACL identifiers; numeric text preserves precision.
fn to_acl(value: &Value, depth: usize) -> Result<AclValue, PreferenceError> {
    if depth > 64 {
        return Err(PreferenceError::Invalid);
    }
    Ok(match value {
        Value::Null => AclValue::Null,
        Value::Bool(value) => AclValue::Bool(*value),
        Value::String(value) => AclValue::String(value.clone()),
        Value::Number(value) => AclValue::Object(vec![(
            "number".to_owned(),
            AclValue::String(value.to_string()),
        )]),
        Value::Array(values) => AclValue::List(
            values
                .iter()
                .map(|item| to_acl(item, depth + 1))
                .collect::<Result<_, _>>()?,
        ),
        Value::Object(values) => AclValue::Object(vec![(
            "entries".to_owned(),
            AclValue::List(
                values
                    .iter()
                    .map(|(key, value)| {
                        Ok(AclValue::List(vec![
                            AclValue::String(key.clone()),
                            to_acl(value, depth + 1)?,
                        ]))
                    })
                    .collect::<Result<_, PreferenceError>>()?,
            ),
        )]),
    })
}

fn from_acl(value: &AclValue, depth: usize) -> Result<Value, PreferenceError> {
    if depth > 64 {
        return Err(PreferenceError::Invalid);
    }
    match value {
        AclValue::Null => Ok(Value::Null),
        AclValue::Bool(value) => Ok(Value::Bool(*value)),
        AclValue::String(value) => Ok(Value::String(value.clone())),
        AclValue::List(values) => values
            .iter()
            .map(|item| from_acl(item, depth + 1))
            .collect::<Result<_, _>>()
            .map(Value::Array),
        AclValue::Object(fields) => {
            let [(tag, data)] = fields.as_slice() else {
                return Err(PreferenceError::Invalid);
            };
            match (tag.as_str(), data) {
                ("number", AclValue::String(text)) => text
                    .parse()
                    .map(Value::Number)
                    .map_err(|_| PreferenceError::Invalid),
                ("entries", AclValue::List(entries)) => {
                    let mut map = serde_json::Map::new();
                    for entry in entries {
                        let AclValue::List(pair) = entry else {
                            return Err(PreferenceError::Invalid);
                        };
                        let [AclValue::String(key), value] = pair.as_slice() else {
                            return Err(PreferenceError::Invalid);
                        };
                        if map
                            .insert(key.clone(), from_acl(value, depth + 1)?)
                            .is_some()
                        {
                            return Err(PreferenceError::Invalid);
                        }
                    }
                    Ok(Value::Object(map))
                }
                _ => Err(PreferenceError::Invalid),
            }
        }
        _ => Err(PreferenceError::Invalid),
    }
}

pub(crate) struct PreferenceHost {
    store: Arc<PreferenceStore>,
    member: Arc<crate::member_session::MemberSessionStore>,
    token: String,
}

impl PreferenceHost {
    pub fn new(
        root: PathBuf,
        member: Arc<crate::member_session::MemberSessionStore>,
        token: String,
    ) -> Self {
        Self {
            store: Arc::new(PreferenceStore::new(root)),
            member,
            token,
        }
    }
}

fn check_window(
    window: &str,
    label: &str,
    url: &str,
    presented: &str,
    expected: &str,
) -> Result<(), PreferenceError> {
    if window != "main"
        || label != "main"
        || !crate::is_main_renderer_document(url)
        || expected.is_empty()
        || !crate::authority::tokens_match(presented, expected)
    {
        return Err(PreferenceError::Denied);
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn preference_read<R: tauri::Runtime>(
    webview: tauri::Webview<R>,
    state: tauri::State<'_, PreferenceHost>,
    host_token: String,
) -> Result<Snapshot, PreferenceError> {
    check_window(
        webview.window().label(),
        webview.label(),
        webview.url().map_err(|_| PreferenceError::Denied)?.as_str(),
        &host_token,
        &state.token,
    )?;
    let store = Arc::clone(&state.store);
    let member = Arc::clone(&state.member);
    tokio::task::spawn_blocking(move || member.with_preference_scope(|scope| store.read(scope)))
        .await
        .map_err(|_| PreferenceError::Unavailable)?
}

#[tauri::command]
pub(crate) async fn preference_commit<R: tauri::Runtime>(
    webview: tauri::Webview<R>,
    state: tauri::State<'_, PreferenceHost>,
    input: Commit,
    host_token: String,
) -> Result<Snapshot, PreferenceError> {
    check_window(
        webview.window().label(),
        webview.label(),
        webview.url().map_err(|_| PreferenceError::Denied)?.as_str(),
        &host_token,
        &state.token,
    )?;
    let store = Arc::clone(&state.store);
    let member = Arc::clone(&state.member);
    let app = webview.app_handle().clone();
    tokio::task::spawn_blocking(move || {
        member.with_preference_scope(|scope| {
            if scope != input.scope {
                return Err(PreferenceError::Denied);
            }
            let snapshot = store.commit(&input)?;
            // A missed event is repaired by the next read; a committed write must never appear failed.
            let _ = app.emit_to(tauri::EventTarget::webview_window("main"), EVENT, ());
            Ok(snapshot)
        })
    })
    .await
    .map_err(|_| PreferenceError::Unavailable)?
}

#[cfg(test)]
#[path = "preferences_tests.rs"]
mod tests;
