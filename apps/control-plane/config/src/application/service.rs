use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use crate::config::ConfigServerConfig;
use crate::error::{AppError, AppResult};

use super::super::domain::{
    ConfigEntry, ConfigKey, ConfigNamespace, ConfigRepository, ConfigValue, DomainError,
};
use super::catalog::{
    all_builtins, bootstrap_catalog, builtin, builtins_on, CatalogGroupDto, CatalogItemDto, Surface,
};
use super::ports::{ConfigChangeBus, ConfigReader};
use crate::bootstrap::BootstrapGroup;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ConfigOp {
    Put,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigChangedEvent {
    pub op: ConfigOp,
    pub namespace: String,
    pub key: String,
    pub revision: i64,
    #[serde(default)]
    pub value: Option<Value>,
    pub updated_at_epoch: i64,
    #[serde(default)]
    pub updated_by: String,
    pub publisher_id: String,
}

pub struct PutConfigCommand {
    pub namespace: String,
    pub key: String,
    pub value: Value,
    pub updated_by: String,
}

#[derive(Debug)]
struct CacheState {
    entries: HashMap<(String, String), ConfigEntry>,
    revision: i64,
    /// Revision through which `entries` is a complete database read.
    loaded: i64,
}

enum Publish {
    Snapshot {
        entries: Vec<ConfigEntry>,
        revision: i64,
    },
    Put {
        entry: ConfigEntry,
    },
    Delete {
        namespace: String,
        key: String,
        revision: i64,
    },
}

/// Apply one cache change. Both cursors live in `state` and neither moves backward.
/// Callers hold the cache write lock and must not await before releasing it.
/// Returns whether a snapshot was installed.
fn publish(state: &mut CacheState, revision_mirror: &AtomicI64, op: Publish) -> bool {
    let installed = matches!(
        &op,
        Publish::Snapshot { revision, .. } if *revision >= state.revision
    );
    match op {
        Publish::Snapshot { entries, revision } => {
            if revision >= state.revision {
                state.entries.clear();
                for entry in entries {
                    state.entries.insert(entry_key(&entry), entry);
                }
                state.revision = revision;
                state.loaded = revision;
            }
        }
        Publish::Put { entry } => {
            let revision = entry.revision();
            let map_key = entry_key(&entry);
            let absent = !state.entries.contains_key(&map_key);
            let cached_is_newer = state
                .entries
                .get(&map_key)
                .is_some_and(|current| current.revision() > revision);
            if !(absent && revision <= state.loaded) && !cached_is_newer {
                state.entries.insert(map_key, entry);
                if revision > state.revision {
                    state.revision = revision;
                }
                if revision == state.loaded.saturating_add(1) {
                    state.loaded = revision;
                }
            }
        }
        Publish::Delete {
            namespace,
            key,
            revision,
        } => {
            if revision >= state.revision {
                state.entries.remove(&(namespace, key));
                if revision > state.revision {
                    state.revision = revision;
                }
                if revision == state.loaded.saturating_add(1) {
                    state.loaded = revision;
                }
            }
        }
    }
    revision_mirror.store(state.revision, Ordering::Relaxed);
    installed
}

fn entry_key(entry: &ConfigEntry) -> (String, String) {
    (
        entry.namespace().as_str().to_string(),
        entry.key().as_str().to_string(),
    )
}

pub struct ConfigCenterService {
    repo: Arc<dyn ConfigRepository>,
    bus: Arc<dyn ConfigChangeBus>,
    app_config: Arc<ConfigServerConfig>,
    publisher_id: String,
    cache: Arc<RwLock<CacheState>>,
    /// Mirrors `CacheState.revision` for the synchronous reader.
    /// Fence decisions read the cursors inside the cache write guard.
    revision: AtomicI64,
}

impl std::fmt::Debug for ConfigCenterService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigCenterService")
            .field("publisher_id", &self.publisher_id)
            .finish_non_exhaustive()
    }
}

impl ConfigCenterService {
    /// Start with injected ports (composition root / infrastructure bootstrap).
    pub async fn start(
        repo: Arc<dyn ConfigRepository>,
        bus: Arc<dyn ConfigChangeBus>,
        app_config: Arc<ConfigServerConfig>,
    ) -> AppResult<Arc<Self>> {
        let service = Arc::new(Self {
            repo,
            bus,
            app_config,
            publisher_id: Uuid::new_v4().to_string(),
            cache: Arc::new(RwLock::new(CacheState {
                entries: HashMap::new(),
                revision: 0,
                loaded: 0,
            })),
            revision: AtomicI64::new(0),
        });
        service.reload_all().await?;
        service.ensure_runtime_builtins().await?;
        service.spawn_subscriber();
        Ok(service)
    }

    pub fn bootstrap_groups(&self) -> Vec<BootstrapGroup> {
        bootstrap_catalog(&self.app_config)
    }

    pub async fn runtime_builtin_groups(&self) -> AppResult<Vec<CatalogGroupDto>> {
        self.builtin_groups(Surface::Runtime).await
    }

    /// Builtins edited on one console page; secrets leave masked.
    pub async fn builtin_groups(&self, surface: Surface) -> AppResult<Vec<CatalogGroupDto>> {
        self.refresh_from_store().await?;
        let cache = self.cache.read().await;
        let mut groups: Vec<CatalogGroupDto> = Vec::new();
        for def in builtins_on(surface) {
            let stored = cache
                .entries
                .get(&(def.namespace.to_string(), def.key.to_string()));
            let item = CatalogItemDto {
                key: def.key.to_string(),
                label: def.label.to_string(),
                description: def.description.to_string(),
                value: def.schema.mask_secrets(
                    &stored
                        .map(|entry| published_json(entry.value()))
                        .unwrap_or_else(|| def.default.clone()),
                ),
                sensitive: def.sensitive,
                namespace: Some(def.namespace.to_string()),
                revision: stored.map(|entry| entry.revision()),
                updated_by: stored.map(|entry| entry.updated_by().to_string()),
                configured: Some(stored.is_some()),
                schema: Some(def.schema.clone()),
            };
            if let Some(group) = groups.iter_mut().find(|g| g.id == def.group) {
                group.items.push(item);
            } else {
                groups.push(CatalogGroupDto {
                    id: def.group.to_string(),
                    label: def.group_label.to_string(),
                    items: vec![item],
                });
            }
        }
        Ok(groups)
    }

    async fn ensure_runtime_builtins(&self) -> AppResult<()> {
        let removed: HashSet<(String, String)> = self
            .repo
            .removed_keys()
            .await
            .map_err(map_domain)?
            .into_iter()
            .collect();
        for def in all_builtins() {
            let map_key = (def.namespace.to_string(), def.key.to_string());
            // A committed removal stays absent across process start. A missing
            // key with no removal is the first seed of that builtin.
            if removed.contains(&map_key) {
                continue;
            }
            let exists = {
                let cache = self.cache.read().await;
                cache.entries.contains_key(&map_key)
            };
            if exists {
                continue;
            }
            self.put(PutConfigCommand {
                namespace: def.namespace.to_string(),
                key: def.key.to_string(),
                value: def.default.clone(),
                updated_by: "system".into(),
            })
            .await?;
        }
        Ok(())
    }

    pub fn publisher_id(&self) -> &str {
        &self.publisher_id
    }

    pub fn current_revision(&self) -> i64 {
        self.revision.load(Ordering::Relaxed)
    }

    pub async fn list(&self, namespace: &str) -> AppResult<Vec<ConfigEntry>> {
        let namespace = ConfigNamespace::parse(namespace).map_err(map_domain)?;
        self.refresh_from_store().await?;
        let cache = self.cache.read().await;
        let mut entries: Vec<_> = cache
            .entries
            .values()
            .filter(|entry| entry.namespace().as_str() == namespace.as_str())
            .cloned()
            .collect();
        entries.sort_by(|a, b| a.key().as_str().cmp(b.key().as_str()));
        Ok(entries)
    }

    pub async fn get(&self, namespace: &str, key: &str) -> AppResult<ConfigEntry> {
        let namespace = ConfigNamespace::parse(namespace).map_err(map_domain)?;
        let key = ConfigKey::parse(key).map_err(map_domain)?;
        self.refresh_from_store().await?;
        if let Some(entry) = self.cached(&namespace, &key).await {
            return Ok(entry);
        }
        match self.repo.find(&namespace, &key).await.map_err(map_domain)? {
            Some(entry) => {
                let mut state = self.cache.write().await;
                let revision = entry.revision();
                let map_key = (namespace.as_str().to_string(), key.as_str().to_string());
                publish(&mut state, &self.revision, Publish::Put { entry });
                match state
                    .entries
                    .get(&map_key)
                    .filter(|cached| cached.revision() >= revision)
                {
                    Some(cached) => Ok(cached.clone()),
                    None => Err(AppError::not_found(format!(
                        "config {}.{} not found",
                        namespace.as_str(),
                        key.as_str()
                    ))),
                }
            }
            None => Err(AppError::not_found(format!(
                "config {}.{} not found",
                namespace.as_str(),
                key.as_str()
            ))),
        }
    }

    pub async fn put(&self, command: PutConfigCommand) -> AppResult<ConfigEntry> {
        let namespace = ConfigNamespace::parse(&command.namespace).map_err(map_domain)?;
        let key = ConfigKey::parse(&command.key).map_err(map_domain)?;
        let updated_by = command.updated_by.trim().to_string();
        if updated_by.len() > 128 {
            return Err(AppError::validation("updated_by is too long"));
        }
        let mut value = command.value;
        if let Some(def) = builtin(namespace.as_str(), key.as_str()) {
            let stored = {
                let cache = self.cache.read().await;
                cache
                    .entries
                    .get(&(namespace.as_str().to_string(), key.as_str().to_string()))
                    .map(|entry| published_json(entry.value()))
            };
            def.schema.restore_secrets(&mut value, stored.as_ref());
            def.schema
                .check(&value)
                .map_err(|message| AppError::validation(format!("{}: {message}", key.as_str())))?;
        }
        let updated_at_epoch = now_epoch();
        let entry = self
            .repo
            .commit_put(
                &namespace,
                &key,
                &ConfigValue::from_text(value.to_string()),
                updated_at_epoch,
                &updated_by,
            )
            .await
            .map_err(map_domain)?;
        let revision = entry.revision();
        {
            let mut state = self.cache.write().await;
            publish(
                &mut state,
                &self.revision,
                Publish::Put {
                    entry: entry.clone(),
                },
            );
        }

        let event = ConfigChangedEvent {
            op: ConfigOp::Put,
            namespace: namespace.as_str().to_string(),
            key: key.as_str().to_string(),
            revision,
            value: Some(published_json(entry.value())),
            updated_at_epoch: entry.updated_at_epoch(),
            updated_by: entry.updated_by().to_string(),
            publisher_id: self.publisher_id.clone(),
        };
        // The database commit already finished. A failed publish must not undo it.
        if let Err(err) = self.bus.publish(&event).await {
            eprintln!("config center publish failed after commit: {err}");
        }
        Ok(entry)
    }

    pub async fn delete(&self, namespace: &str, key: &str) -> AppResult<()> {
        let namespace = ConfigNamespace::parse(namespace).map_err(map_domain)?;
        let key = ConfigKey::parse(key).map_err(map_domain)?;
        let Some(revision) = self
            .repo
            .commit_delete(&namespace, &key)
            .await
            .map_err(map_domain)?
        else {
            return Err(AppError::not_found(format!(
                "config {}.{} not found",
                namespace.as_str(),
                key.as_str()
            )));
        };
        {
            let mut state = self.cache.write().await;
            publish(
                &mut state,
                &self.revision,
                Publish::Delete {
                    namespace: namespace.as_str().to_string(),
                    key: key.as_str().to_string(),
                    revision,
                },
            );
        }

        let event = ConfigChangedEvent {
            op: ConfigOp::Delete,
            namespace: namespace.as_str().to_string(),
            key: key.as_str().to_string(),
            revision,
            value: None,
            updated_at_epoch: now_epoch(),
            updated_by: String::new(),
            publisher_id: self.publisher_id.clone(),
        };
        if let Err(err) = self.bus.publish(&event).await {
            eprintln!("config center publish failed after commit: {err}");
        }
        Ok(())
    }

    async fn reload_all(&self) -> AppResult<bool> {
        let (entries, revision) = self.repo.read_snapshot().await.map_err(map_domain)?;
        let mut state = self.cache.write().await;
        Ok(publish(
            &mut state,
            &self.revision,
            Publish::Snapshot { entries, revision },
        ))
    }

    /// Install a database snapshot, or leave a newer published cache in place.
    /// A snapshot that loses to a revision published while it was being read is
    /// not the end of this refresh: read again until one is installed or `loaded`
    /// already covers the revision observed at the start.
    async fn refresh_from_store(&self) -> AppResult<()> {
        let target = self.repo.max_revision().await.map_err(map_domain)?;
        loop {
            if self.cache.read().await.loaded >= target {
                return Ok(());
            }
            if self.reload_all().await? {
                return Ok(());
            }
        }
    }

    async fn cached(&self, namespace: &ConfigNamespace, key: &ConfigKey) -> Option<ConfigEntry> {
        self.cache
            .read()
            .await
            .entries
            .get(&(namespace.as_str().to_string(), key.as_str().to_string()))
            .cloned()
    }

    fn spawn_subscriber(self: &Arc<Self>) {
        let (tx, mut rx) = mpsc::channel::<ConfigChangedEvent>(64);
        self.bus.spawn_listen(tx);
        let service = Arc::clone(self);
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                if let Err(err) = service.on_remote_event(event).await {
                    eprintln!("config center sync failed: {err}");
                }
            }
        });
    }

    async fn on_remote_event(&self, event: ConfigChangedEvent) -> AppResult<()> {
        if event.publisher_id == self.publisher_id {
            return Ok(());
        }

        match event.op {
            ConfigOp::Put => {
                let namespace = ConfigNamespace::parse(&event.namespace).map_err(map_domain)?;
                let key = ConfigKey::parse(&event.key).map_err(map_domain)?;
                let entry = match event.value {
                    Some(value) => ConfigEntry::new(
                        namespace,
                        key,
                        ConfigValue::from_text(value.to_string()),
                        event.revision,
                        event.updated_at_epoch,
                        event.updated_by,
                    )
                    .map_err(map_domain)?,
                    None => match self.repo.find(&namespace, &key).await.map_err(map_domain)? {
                        Some(entry) => entry,
                        None => return Ok(()),
                    },
                };
                let mut state = self.cache.write().await;
                publish(&mut state, &self.revision, Publish::Put { entry });
            }
            ConfigOp::Delete => {
                let mut state = self.cache.write().await;
                publish(
                    &mut state,
                    &self.revision,
                    Publish::Delete {
                        namespace: event.namespace,
                        key: event.key,
                        revision: event.revision,
                    },
                );
            }
        }
        Ok(())
    }
}

fn published_json(value: &ConfigValue) -> Value {
    serde_json::from_str(value.as_str()).unwrap_or(Value::Null)
}

fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[async_trait::async_trait]
impl ConfigReader for ConfigCenterService {
    async fn get(&self, namespace: &str, key: &str) -> AppResult<ConfigEntry> {
        ConfigCenterService::get(self, namespace, key).await
    }
}

fn map_domain(error: DomainError) -> AppError {
    match error {
        DomainError::Validation(message) => AppError::validation(message),
        DomainError::NotFound(message) => AppError::not_found(message),
        DomainError::Conflict(message) => AppError::conflict(message),
        DomainError::Infrastructure(message) => AppError::infrastructure(message),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::net::IpAddr;
    use std::str::FromStr;
    use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
    use std::sync::{Arc, Mutex, Weak};

    use serde_json::json;
    use tokio::sync::mpsc;

    use super::super::ports::ConfigChangeBus;
    use super::{publish, CacheState, ConfigCenterService, Publish, PutConfigCommand};
    use crate::config::ConfigServerConfig;
    use crate::domain::{
        ConfigEntry, ConfigKey, ConfigNamespace, ConfigRepository, ConfigValue, DomainError,
    };
    use crate::infrastructure::PostgresConfigRepository;
    use crate::Postgres;

    struct DeadBus {
        publishes: AtomicU64,
    }

    #[async_trait::async_trait]
    impl ConfigChangeBus for DeadBus {
        async fn publish(&self, _event: &super::ConfigChangedEvent) -> Result<(), String> {
            self.publishes.fetch_add(1, Ordering::SeqCst);
            Err("bus dropped".into())
        }

        fn spawn_listen(&self, _tx: mpsc::Sender<super::ConfigChangedEvent>) {}
    }

    fn app_config() -> Arc<ConfigServerConfig> {
        Arc::new(ConfigServerConfig {
            host: IpAddr::from_str("127.0.0.1").expect("host"),
            port: 0,
            listen_override: None,
            database_url: String::new(),
            database_pool_size: 1,
            iggy_address: String::new(),
            iggy_username: String::new(),
            iggy_password: String::new(),
        })
    }

    fn entry(namespace: &str, key: &str, value: serde_json::Value, revision: i64) -> ConfigEntry {
        ConfigEntry::new(
            ConfigNamespace::parse(namespace).unwrap(),
            ConfigKey::parse(key).unwrap(),
            ConfigValue::from_text(value.to_string()),
            revision,
            1,
            "writer".into(),
        )
        .unwrap()
    }

    fn fresh_cache() -> (CacheState, AtomicI64) {
        (
            CacheState {
                entries: HashMap::new(),
                revision: 0,
                loaded: 0,
            },
            AtomicI64::new(0),
        )
    }

    #[test]
    fn publish_snapshot_keeps_a_newer_put() {
        let (mut state, mirror) = fresh_cache();
        let n = 4;
        publish(
            &mut state,
            &mirror,
            Publish::Snapshot {
                entries: vec![entry("fence", "owner", json!("old"), n)],
                revision: n,
            },
        );
        publish(
            &mut state,
            &mirror,
            Publish::Put {
                entry: entry("fence", "owner", json!("newer"), n + 1),
            },
        );
        publish(
            &mut state,
            &mirror,
            Publish::Snapshot {
                entries: vec![entry("fence", "owner", json!("old"), n)],
                revision: n,
            },
        );
        let kept = state
            .entries
            .get(&("fence".into(), "owner".into()))
            .expect("newer put stays");
        assert_eq!(kept.value().as_str(), json!("newer").to_string());
        assert_eq!(kept.revision(), n + 1);
        assert_eq!(state.revision, n + 1);
        assert_eq!(state.loaded, n + 1);
        assert_eq!(mirror.load(Ordering::Relaxed), n + 1);
    }

    #[test]
    fn publish_snapshot_keeps_a_removed_key_absent() {
        let (mut state, mirror) = fresh_cache();
        let n = 4;
        publish(
            &mut state,
            &mirror,
            Publish::Snapshot {
                entries: vec![entry("fence", "owner", json!("old"), n)],
                revision: n,
            },
        );
        publish(
            &mut state,
            &mirror,
            Publish::Delete {
                namespace: "fence".into(),
                key: "owner".into(),
                revision: n + 1,
            },
        );
        publish(
            &mut state,
            &mirror,
            Publish::Snapshot {
                entries: vec![entry("fence", "owner", json!("old"), n)],
                revision: n,
            },
        );
        assert!(!state
            .entries
            .contains_key(&("fence".into(), "owner".into())));
        assert_eq!(state.revision, n + 1);
        assert_eq!(state.loaded, n + 1);
        assert_eq!(mirror.load(Ordering::Relaxed), n + 1);
    }

    #[test]
    fn publish_delete_from_the_previous_loaded_cursor_rejects_the_older_put() {
        let (mut state, mirror) = fresh_cache();
        let d = 6;
        publish(
            &mut state,
            &mirror,
            Publish::Snapshot {
                entries: vec![entry(
                    "default",
                    "feature.maintenance",
                    json!({ "enabled": true, "message": "from-a" }),
                    d - 1,
                )],
                revision: d - 1,
            },
        );
        assert_eq!(state.loaded, d - 1);
        publish(
            &mut state,
            &mirror,
            Publish::Delete {
                namespace: "default".into(),
                key: "feature.maintenance".into(),
                revision: d,
            },
        );
        assert!(!state
            .entries
            .contains_key(&("default".into(), "feature.maintenance".into())));
        assert_eq!(state.loaded, d);
        assert_eq!(state.revision, d);
        publish(
            &mut state,
            &mirror,
            Publish::Put {
                entry: entry(
                    "default",
                    "feature.maintenance",
                    json!({ "enabled": true, "message": "from-a" }),
                    d - 1,
                ),
            },
        );
        assert!(!state
            .entries
            .contains_key(&("default".into(), "feature.maintenance".into())));
        assert_eq!(state.loaded, d);
        assert_eq!(state.revision, d);
        assert_eq!(mirror.load(Ordering::Relaxed), d);
    }

    #[test]
    fn publish_put_at_or_below_loaded_does_not_insert_an_absent_key() {
        let mirror = AtomicI64::new(5);
        let mut state = CacheState {
            entries: HashMap::new(),
            revision: 5,
            loaded: 5,
        };
        for revision in [5, 4] {
            publish(
                &mut state,
                &mirror,
                Publish::Put {
                    entry: entry(
                        "default",
                        "feature.maintenance",
                        json!({ "enabled": true, "message": "from-a" }),
                        revision,
                    ),
                },
            );
            assert!(
                state.entries.is_empty(),
                "revision {revision} must not recreate an absent key"
            );
            assert_eq!(state.revision, 5);
            assert_eq!(state.loaded, 5);
        }
        assert_eq!(mirror.load(Ordering::Relaxed), 5);
    }

    #[test]
    fn publish_delete_below_revision_does_not_remove_or_rewind() {
        let (mut state, mirror) = fresh_cache();
        publish(
            &mut state,
            &mirror,
            Publish::Snapshot {
                entries: vec![entry("fence", "owner", json!("kept"), 10)],
                revision: 10,
            },
        );
        publish(
            &mut state,
            &mirror,
            Publish::Delete {
                namespace: "fence".into(),
                key: "owner".into(),
                revision: 9,
            },
        );
        let kept = state
            .entries
            .get(&("fence".into(), "owner".into()))
            .expect("older delete leaves the key");
        assert_eq!(kept.value().as_str(), json!("kept").to_string());
        assert_eq!(state.revision, 10);
        assert_eq!(state.loaded, 10);
        assert_eq!(mirror.load(Ordering::Relaxed), 10);
    }

    fn put(
        namespace: &str,
        key: &str,
        value: serde_json::Value,
        updated_by: &str,
    ) -> PutConfigCommand {
        PutConfigCommand {
            namespace: namespace.into(),
            key: key.into(),
            value,
            updated_by: updated_by.into(),
        }
    }

    #[tokio::test]
    async fn two_instances_read_the_committed_row_when_the_bus_is_dropped() {
        let db_name = format!("aster_cfg_{}", uuid::Uuid::new_v4().simple());
        admin_sql("aster_api", &format!("create database {db_name}"));
        let url = format!("postgres://aster_api:aster_api@127.0.0.1:5432/{db_name}");
        let pg_a = Arc::new(Postgres::connect(&url, 4).expect("connect config database"));
        pg_a.ensure_schema().await.expect("config schema");
        let pg_b = Arc::new(Postgres::connect(&url, 4).expect("second connection"));
        admin_sql(
            &db_name,
            "create function config_entry_reject_rollback() returns trigger language plpgsql as $$ begin if new.key = 'rollback' then raise exception 'config entry write failed'; end if; return new; end $$; create trigger config_entry_rollback before insert or update on config_entries for each row execute function config_entry_reject_rollback();",
        );
        let repo_a = Arc::new(PostgresConfigRepository::new(pg_a));
        let repo_b = Arc::new(PostgresConfigRepository::new(pg_b));
        let bus_a = Arc::new(DeadBus {
            publishes: AtomicU64::new(0),
        });
        let bus_b = Arc::new(DeadBus {
            publishes: AtomicU64::new(0),
        });
        let config = app_config();
        let service_a =
            ConfigCenterService::start(repo_a.clone(), bus_a.clone(), Arc::clone(&config))
                .await
                .expect("service a");
        let service_b = ConfigCenterService::start(repo_b.clone(), bus_b.clone(), config)
            .await
            .expect("service b");

        let before = bus_a.publishes.load(Ordering::SeqCst);
        let first = service_a
            .put(put("fence", "owner", json!("first"), "replica-a"))
            .await
            .expect("first put commits even though publish fails");
        assert!(bus_a.publishes.load(Ordering::SeqCst) > before);
        let seen = service_b
            .get("fence", "owner")
            .await
            .expect("replica that missed the bus reads postgres");
        assert_eq!(seen.value(), first.value());
        assert_eq!(seen.revision(), first.revision());
        let row = repo_b
            .find(
                &ConfigNamespace::parse("fence").unwrap(),
                &ConfigKey::parse("owner").unwrap(),
            )
            .await
            .expect("row")
            .expect("stored entry");
        assert_eq!(row.revision(), first.revision());
        assert_eq!(row.value(), first.value());

        let second = service_b
            .put(put("fence", "owner", json!("second"), "replica-b"))
            .await
            .expect("second put");
        assert!(second.revision() > first.revision());
        let latest = service_a
            .get("fence", "owner")
            .await
            .expect("writer cache is not the authority");
        assert_eq!(latest.value(), second.value());
        assert_eq!(latest.revision(), second.revision());
        let listed = service_a.list("fence").await.expect("list");
        assert!(listed
            .iter()
            .any(|entry| { entry.key().as_str() == "owner" && entry.value() == second.value() }));

        let stale = ConfigEntry::new(
            ConfigNamespace::parse("fence").unwrap(),
            ConfigKey::parse("owner").unwrap(),
            ConfigValue::from_text(json!("stale").to_string()),
            first.revision(),
            1,
            "stale".into(),
        )
        .unwrap();
        repo_a.upsert(&stale).await.expect("older revision write");
        let kept = repo_a
            .find(
                &ConfigNamespace::parse("fence").unwrap(),
                &ConfigKey::parse("owner").unwrap(),
            )
            .await
            .expect("row")
            .expect("entry");
        assert_eq!(kept.revision(), second.revision());
        assert_eq!(kept.value(), second.value());
        let from_b = service_b.get("fence", "owner").await.expect("read");
        assert_eq!(from_b.revision(), second.revision());
        assert_eq!(from_b.value(), second.value());

        let (left, right) = tokio::join!(
            service_a.put(put("fence", "race", json!("left"), "replica-a")),
            service_b.put(put("fence", "race", json!("right"), "replica-b")),
        );
        let left = left.expect("left put");
        let right = right.expect("right put");
        assert_ne!(left.revision(), right.revision());
        let winner = if left.revision() > right.revision() {
            left
        } else {
            right
        };
        let raced_a = service_a.get("fence", "race").await.expect("race a");
        let raced_b = service_b.get("fence", "race").await.expect("race b");
        assert_eq!(raced_a.value(), winner.value());
        assert_eq!(raced_a.revision(), winner.revision());
        assert_eq!(raced_b.value(), raced_a.value());
        assert_eq!(raced_b.revision(), raced_a.revision());
        let raced_row = repo_b
            .find(
                &ConfigNamespace::parse("fence").unwrap(),
                &ConfigKey::parse("race").unwrap(),
            )
            .await
            .expect("race row")
            .expect("entry");
        assert_eq!(raced_row.revision(), winner.revision());
        assert_eq!(raced_row.value(), winner.value());

        assert!(service_a.get("fence", "missing").await.is_err());
        assert!(service_b.get("fence", "missing").await.is_err());

        let before_failure = repo_a
            .max_revision()
            .await
            .expect("revision before failed write");
        let failed = service_a
            .put(put("fence", "rollback", json!("no"), "replica-a"))
            .await;
        assert!(failed.is_err(), "entry write failure must surface");
        assert_eq!(
            repo_b
                .max_revision()
                .await
                .expect("revision after failed write"),
            before_failure
        );
        assert!(repo_a
            .find(
                &ConfigNamespace::parse("fence").unwrap(),
                &ConfigKey::parse("rollback").unwrap(),
            )
            .await
            .expect("rollback lookup")
            .is_none());

        service_a.delete("fence", "owner").await.expect("delete");
        assert!(service_b.get("fence", "owner").await.is_err());
        repo_a
            .upsert(
                &ConfigEntry::new(
                    ConfigNamespace::parse("fence").unwrap(),
                    ConfigKey::parse("owner").unwrap(),
                    second.value().clone(),
                    second.revision(),
                    second.updated_at_epoch(),
                    second.updated_by().to_string(),
                )
                .unwrap(),
            )
            .await
            .expect("older revision after removal");
        assert!(repo_b
            .find(
                &ConfigNamespace::parse("fence").unwrap(),
                &ConfigKey::parse("owner").unwrap(),
            )
            .await
            .expect("removed lookup")
            .is_none());
        assert!(service_a.get("fence", "owner").await.is_err());
        assert!(service_b.get("fence", "owner").await.is_err());

        drop(service_a);
        drop(service_b);
        drop(repo_a);
        drop(repo_b);
        admin_sql("aster_api", &format!("drop database {db_name} with (force)"));
    }

    fn maintenance_item(
        groups: &[super::super::catalog::CatalogGroupDto],
    ) -> super::super::catalog::CatalogItemDto {
        groups
            .iter()
            .flat_map(|group| group.items.iter())
            .find(|item| item.key == "feature.maintenance")
            .expect("maintenance builtin")
            .clone()
    }

    #[tokio::test]
    async fn builtin_catalog_follows_the_committed_row_when_the_bus_is_dead() {
        let db_name = format!("aster_cfg_{}", uuid::Uuid::new_v4().simple());
        admin_sql("aster_api", &format!("create database {db_name}"));
        let url = format!("postgres://aster_api:aster_api@127.0.0.1:5432/{db_name}");
        let pg_a = Arc::new(Postgres::connect(&url, 4).expect("connect"));
        pg_a.ensure_schema().await.expect("schema");
        let pg_b = Arc::new(Postgres::connect(&url, 4).expect("second"));
        let repo_a = Arc::new(PostgresConfigRepository::new(pg_a));
        let repo_b = Arc::new(PostgresConfigRepository::new(pg_b));
        let config = app_config();
        let service_a = ConfigCenterService::start(
            repo_a,
            Arc::new(DeadBus {
                publishes: AtomicU64::new(0),
            }),
            Arc::clone(&config),
        )
        .await
        .expect("service a");
        let service_b = ConfigCenterService::start(
            repo_b,
            Arc::new(DeadBus {
                publishes: AtomicU64::new(0),
            }),
            config,
        )
        .await
        .expect("service b");

        let written = service_a
            .put(put(
                "default",
                "feature.maintenance",
                json!({ "enabled": true, "message": "from-a" }),
                "replica-a",
            ))
            .await
            .expect("committed builtin");
        let shown = maintenance_item(
            &service_b
                .runtime_builtin_groups()
                .await
                .expect("catalog without a prior get"),
        );
        assert_eq!(shown.value, json!({ "enabled": true, "message": "from-a" }));
        assert_eq!(shown.configured, Some(true));
        assert_eq!(shown.revision, Some(written.revision()));

        service_a
            .delete("default", "feature.maintenance")
            .await
            .expect("delete builtin");
        let after = maintenance_item(
            &service_b
                .runtime_builtin_groups()
                .await
                .expect("catalog after removal"),
        );
        assert_eq!(after.configured, Some(false));
        assert_eq!(after.revision, None);
        assert_eq!(after.value, json!({ "enabled": false, "message": "" }));

        drop(service_a);
        drop(service_b);
        admin_sql("aster_api", &format!("drop database {db_name} with (force)"));
    }

    #[tokio::test]
    async fn restarted_service_does_not_recreate_a_removed_builtin() {
        let db_name = format!("aster_cfg_{}", uuid::Uuid::new_v4().simple());
        admin_sql("aster_api", &format!("create database {db_name}"));
        let url = format!("postgres://aster_api:aster_api@127.0.0.1:5432/{db_name}");
        let pg_a = Arc::new(Postgres::connect(&url, 4).expect("connect"));
        pg_a.ensure_schema().await.expect("schema");
        let pg_b = Arc::new(Postgres::connect(&url, 4).expect("second"));
        let repo_a = Arc::new(PostgresConfigRepository::new(pg_a));
        let repo_b = Arc::new(PostgresConfigRepository::new(Arc::clone(&pg_b)));
        let config = app_config();
        let service_a = ConfigCenterService::start(
            repo_a,
            Arc::new(DeadBus {
                publishes: AtomicU64::new(0),
            }),
            Arc::clone(&config),
        )
        .await
        .expect("first service");
        service_a
            .delete("default", "feature.maintenance")
            .await
            .expect("delete builtin");

        let service_b = ConfigCenterService::start(
            repo_b.clone(),
            Arc::new(DeadBus {
                publishes: AtomicU64::new(0),
            }),
            config,
        )
        .await
        .expect("service started after the delete");

        assert!(service_b
            .get("default", "feature.maintenance")
            .await
            .is_err());
        let shown = maintenance_item(
            &service_b
                .runtime_builtin_groups()
                .await
                .expect("catalog after restart"),
        );
        assert_eq!(shown.configured, Some(false));
        assert_eq!(shown.revision, None);
        assert_eq!(shown.value, json!({ "enabled": false, "message": "" }));
        assert!(repo_b
            .find(
                &ConfigNamespace::parse("default").unwrap(),
                &ConfigKey::parse("feature.maintenance").unwrap(),
            )
            .await
            .expect("row")
            .is_none());

        drop(service_a);
        drop(service_b);
        drop(repo_b);
        drop(pg_b);
        admin_sql("aster_api", &format!("drop database {db_name} with (force)"));
    }

    #[tokio::test]
    async fn delayed_put_does_not_recreate_a_removed_builtin() {
        let db_name = format!("aster_cfg_{}", uuid::Uuid::new_v4().simple());
        admin_sql("aster_api", &format!("create database {db_name}"));
        let url = format!("postgres://aster_api:aster_api@127.0.0.1:5432/{db_name}");
        let pg = Arc::new(Postgres::connect(&url, 4).expect("connect"));
        pg.ensure_schema().await.expect("schema");
        let repo = Arc::new(PostgresConfigRepository::new(Arc::clone(&pg)));
        let service = ConfigCenterService::start(
            repo.clone(),
            Arc::new(DeadBus {
                publishes: AtomicU64::new(0),
            }),
            app_config(),
        )
        .await
        .expect("service");
        let written = service
            .put(put(
                "default",
                "feature.maintenance",
                json!({ "enabled": true, "message": "from-a" }),
                "replica-a",
            ))
            .await
            .expect("put builtin");
        service
            .delete("default", "feature.maintenance")
            .await
            .expect("delete builtin");
        assert!(service.get("default", "feature.maintenance").await.is_err());

        service
            .on_remote_event(super::ConfigChangedEvent {
                op: super::ConfigOp::Put,
                namespace: "default".into(),
                key: "feature.maintenance".into(),
                revision: written.revision(),
                value: Some(serde_json::from_str(written.value().as_str()).unwrap()),
                updated_at_epoch: written.updated_at_epoch(),
                updated_by: written.updated_by().to_string(),
                publisher_id: "other-replica".into(),
            })
            .await
            .expect("deliver older put");

        assert!(service.get("default", "feature.maintenance").await.is_err());
        let listed = service.list("default").await.expect("list");
        assert!(listed
            .iter()
            .all(|entry| entry.key().as_str() != "feature.maintenance"));
        let shown = maintenance_item(&service.runtime_builtin_groups().await.expect("catalog"));
        assert_eq!(shown.configured, Some(false));
        assert_eq!(shown.revision, None);
        assert_eq!(shown.value, json!({ "enabled": false, "message": "" }));
        assert!(repo
            .find(
                &ConfigNamespace::parse("default").unwrap(),
                &ConfigKey::parse("feature.maintenance").unwrap(),
            )
            .await
            .expect("row")
            .is_none());

        drop(service);
        drop(repo);
        drop(pg);
        admin_sql("aster_api", &format!("drop database {db_name} with (force)"));
    }

    /// If a reload observed entries at revision N and the counter at N+1, the next get
    /// must not keep serving N.
    struct SplitRepo {
        inner: Arc<PostgresConfigRepository>,
        writer: Arc<PostgresConfigRepository>,
        split: AtomicBool,
    }

    impl SplitRepo {
        fn arm(&self) {
            self.split.store(true, Ordering::SeqCst);
        }

        async fn commit_later_generation(&self) -> Result<(), DomainError> {
            if !self.split.swap(false, Ordering::SeqCst) {
                return Ok(());
            }
            let namespace = ConfigNamespace::parse("fence").expect("namespace");
            let key = ConfigKey::parse("snap").expect("key");
            self.writer
                .commit_put(
                    &namespace,
                    &key,
                    &ConfigValue::from_text(json!("generation-2").to_string()),
                    1,
                    "split",
                )
                .await?;
            Ok(())
        }
    }

    #[async_trait::async_trait]
    impl ConfigRepository for SplitRepo {
        async fn list_namespace(
            &self,
            namespace: &ConfigNamespace,
        ) -> Result<Vec<ConfigEntry>, DomainError> {
            self.inner.list_namespace(namespace).await
        }

        async fn list_all(&self) -> Result<Vec<ConfigEntry>, DomainError> {
            let entries = self.inner.list_all().await?;
            self.commit_later_generation().await?;
            Ok(entries)
        }

        async fn find(
            &self,
            namespace: &ConfigNamespace,
            key: &ConfigKey,
        ) -> Result<Option<ConfigEntry>, DomainError> {
            self.inner.find(namespace, key).await
        }

        async fn upsert(&self, entry: &ConfigEntry) -> Result<(), DomainError> {
            self.inner.upsert(entry).await
        }

        async fn commit_put(
            &self,
            namespace: &ConfigNamespace,
            key: &ConfigKey,
            value: &ConfigValue,
            updated_at_epoch: i64,
            updated_by: &str,
        ) -> Result<ConfigEntry, DomainError> {
            self.inner
                .commit_put(namespace, key, value, updated_at_epoch, updated_by)
                .await
        }

        async fn delete(
            &self,
            namespace: &ConfigNamespace,
            key: &ConfigKey,
        ) -> Result<bool, DomainError> {
            self.inner.delete(namespace, key).await
        }

        async fn commit_delete(
            &self,
            namespace: &ConfigNamespace,
            key: &ConfigKey,
        ) -> Result<Option<i64>, DomainError> {
            self.inner.commit_delete(namespace, key).await
        }

        async fn allocate_revision(&self) -> Result<i64, DomainError> {
            self.inner.allocate_revision().await
        }

        async fn max_revision(&self) -> Result<i64, DomainError> {
            self.inner.max_revision().await
        }

        async fn removed_keys(&self) -> Result<Vec<(String, String)>, DomainError> {
            self.inner.removed_keys().await
        }

        async fn read_snapshot(&self) -> Result<(Vec<ConfigEntry>, i64), DomainError> {
            let snapshot = self.inner.read_snapshot().await?;
            self.commit_later_generation().await?;
            Ok(snapshot)
        }
    }

    #[tokio::test]
    async fn reload_does_not_keep_an_older_snapshot_when_the_revision_moves() {
        let db_name = format!("aster_cfg_{}", uuid::Uuid::new_v4().simple());
        admin_sql("aster_api", &format!("create database {db_name}"));
        let url = format!("postgres://aster_api:aster_api@127.0.0.1:5432/{db_name}");
        let pg_a = Arc::new(Postgres::connect(&url, 4).expect("connect"));
        pg_a.ensure_schema().await.expect("schema");
        let pg_b = Arc::new(Postgres::connect(&url, 4).expect("reader"));
        let pg_w = Arc::new(Postgres::connect(&url, 4).expect("writer"));
        let repo_a = Arc::new(PostgresConfigRepository::new(pg_a));
        let split = Arc::new(SplitRepo {
            inner: Arc::new(PostgresConfigRepository::new(pg_b)),
            writer: Arc::new(PostgresConfigRepository::new(pg_w)),
            split: AtomicBool::new(false),
        });
        let config = app_config();
        let service_a = ConfigCenterService::start(
            repo_a.clone(),
            Arc::new(DeadBus {
                publishes: AtomicU64::new(0),
            }),
            Arc::clone(&config),
        )
        .await
        .expect("writer service");
        let service_b = ConfigCenterService::start(
            split.clone(),
            Arc::new(DeadBus {
                publishes: AtomicU64::new(0),
            }),
            config,
        )
        .await
        .expect("reader service");

        service_a
            .put(put("fence", "snap", json!("generation-0"), "replica-a"))
            .await
            .expect("generation 0");
        let synced = service_b.get("fence", "snap").await.expect("sync");
        assert_eq!(synced.value().as_str(), json!("generation-0").to_string());
        service_a
            .put(put("fence", "snap", json!("generation-1"), "replica-a"))
            .await
            .expect("generation 1");
        split.arm();
        let during = service_b.get("fence", "snap").await.expect("reload");
        assert_eq!(during.value().as_str(), json!("generation-1").to_string());
        let followed = service_b.get("fence", "snap").await.expect("following get");
        assert_eq!(followed.value().as_str(), json!("generation-2").to_string());
        let stored = repo_a
            .find(
                &ConfigNamespace::parse("fence").unwrap(),
                &ConfigKey::parse("snap").unwrap(),
            )
            .await
            .expect("row")
            .expect("entry");
        assert_eq!(stored.value(), followed.value());
        assert_eq!(stored.revision(), followed.revision());

        drop(service_a);
        drop(service_b);
        drop(split);
        drop(repo_a);
        admin_sql("aster_api", &format!("drop database {db_name} with (force)"));
    }

    /// One `read_snapshot` returns a real snapshot taken before this process
    /// publishes another key, so the cache revision moves past that snapshot.
    struct SkippedSnapshotRepo {
        inner: Arc<PostgresConfigRepository>,
        raise: AtomicBool,
        service: Mutex<Weak<ConfigCenterService>>,
    }

    impl SkippedSnapshotRepo {
        fn arm(&self, service: &Arc<ConfigCenterService>) {
            *self.service.lock().expect("service hook") = Arc::downgrade(service);
            self.raise.store(true, Ordering::SeqCst);
        }
    }

    #[async_trait::async_trait]
    impl ConfigRepository for SkippedSnapshotRepo {
        async fn list_namespace(
            &self,
            namespace: &ConfigNamespace,
        ) -> Result<Vec<ConfigEntry>, DomainError> {
            self.inner.list_namespace(namespace).await
        }

        async fn list_all(&self) -> Result<Vec<ConfigEntry>, DomainError> {
            self.inner.list_all().await
        }

        async fn find(
            &self,
            namespace: &ConfigNamespace,
            key: &ConfigKey,
        ) -> Result<Option<ConfigEntry>, DomainError> {
            self.inner.find(namespace, key).await
        }

        async fn upsert(&self, entry: &ConfigEntry) -> Result<(), DomainError> {
            self.inner.upsert(entry).await
        }

        async fn commit_put(
            &self,
            namespace: &ConfigNamespace,
            key: &ConfigKey,
            value: &ConfigValue,
            updated_at_epoch: i64,
            updated_by: &str,
        ) -> Result<ConfigEntry, DomainError> {
            self.inner
                .commit_put(namespace, key, value, updated_at_epoch, updated_by)
                .await
        }

        async fn delete(
            &self,
            namespace: &ConfigNamespace,
            key: &ConfigKey,
        ) -> Result<bool, DomainError> {
            self.inner.delete(namespace, key).await
        }

        async fn commit_delete(
            &self,
            namespace: &ConfigNamespace,
            key: &ConfigKey,
        ) -> Result<Option<i64>, DomainError> {
            self.inner.commit_delete(namespace, key).await
        }

        async fn allocate_revision(&self) -> Result<i64, DomainError> {
            self.inner.allocate_revision().await
        }

        async fn max_revision(&self) -> Result<i64, DomainError> {
            self.inner.max_revision().await
        }

        async fn removed_keys(&self) -> Result<Vec<(String, String)>, DomainError> {
            self.inner.removed_keys().await
        }

        async fn read_snapshot(&self) -> Result<(Vec<ConfigEntry>, i64), DomainError> {
            if !self.raise.swap(false, Ordering::SeqCst) {
                return self.inner.read_snapshot().await;
            }
            let snapshot = self.inner.read_snapshot().await?;
            let service = self
                .service
                .lock()
                .expect("service hook")
                .upgrade()
                .expect("config service");
            service
                .put(put("fence", "peer", json!("peer"), "raise"))
                .await
                .expect("peer publish");
            Ok(snapshot)
        }
    }

    #[tokio::test]
    async fn skipped_snapshot_retries_until_the_committed_value_is_visible() {
        let db_name = format!("aster_cfg_{}", uuid::Uuid::new_v4().simple());
        admin_sql("aster_api", &format!("create database {db_name}"));
        let url = format!("postgres://aster_api:aster_api@127.0.0.1:5432/{db_name}");
        let pg_a = Arc::new(Postgres::connect(&url, 4).expect("connect"));
        pg_a.ensure_schema().await.expect("schema");
        let pg_b = Arc::new(Postgres::connect(&url, 4).expect("reader"));
        let repo_a = Arc::new(PostgresConfigRepository::new(pg_a));
        let skipped = Arc::new(SkippedSnapshotRepo {
            inner: Arc::new(PostgresConfigRepository::new(pg_b)),
            raise: AtomicBool::new(false),
            service: Mutex::new(Weak::new()),
        });
        let config = app_config();
        let service_a = ConfigCenterService::start(
            repo_a.clone(),
            Arc::new(DeadBus {
                publishes: AtomicU64::new(0),
            }),
            Arc::clone(&config),
        )
        .await
        .expect("writer service");
        let service_b = ConfigCenterService::start(
            skipped.clone(),
            Arc::new(DeadBus {
                publishes: AtomicU64::new(0),
            }),
            config,
        )
        .await
        .expect("reader service");

        let cached = service_a
            .put(put("fence", "owner", json!("cached"), "replica-a"))
            .await
            .expect("cached generation");
        let synced = service_b.get("fence", "owner").await.expect("sync");
        assert_eq!(synced.value(), cached.value());
        assert_eq!(synced.revision(), cached.revision());

        let committed = service_a
            .put(put("fence", "owner", json!("committed"), "replica-a"))
            .await
            .expect("committed generation");
        assert!(committed.revision() > cached.revision());
        skipped.arm(&service_b);

        let seen = service_b.get("fence", "owner").await.expect("refresh");
        assert_eq!(seen.value().as_str(), json!("committed").to_string());
        assert_eq!(seen.revision(), committed.revision());
        let peer = service_b.get("fence", "peer").await.expect("peer");
        assert_eq!(peer.value().as_str(), json!("peer").to_string());
        let stored = repo_a
            .find(
                &ConfigNamespace::parse("fence").unwrap(),
                &ConfigKey::parse("owner").unwrap(),
            )
            .await
            .expect("row")
            .expect("entry");
        assert_eq!(stored.value(), seen.value());
        assert_eq!(stored.revision(), seen.revision());

        drop(service_a);
        drop(service_b);
        drop(skipped);
        drop(repo_a);
        admin_sql("aster_api", &format!("drop database {db_name} with (force)"));
    }

    fn admin_sql(database: &str, sql: &str) {
        let output = std::process::Command::new("docker")
            .args([
                "exec",
                "aster-postgres",
                "psql",
                "-U",
                "aster_api",
                "-d",
                database,
                "-v",
                "ON_ERROR_STOP=1",
                "-c",
                sql,
            ])
            .output()
            .expect("docker exec psql");
        assert!(
            output.status.success(),
            "psql failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
