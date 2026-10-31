use std::sync::Arc;

use aster_orm::{orm_table, select_from, sql_query, Database, OrderDirection, PostgresDialect};

use super::commit::{
    commit_delete_in, commit_put_in, entry_from_committed, upsert_fenced, CommittedConfig,
};
use crate::Postgres;
use async_trait::async_trait;

use super::super::domain::{
    ConfigEntry, ConfigKey, ConfigNamespace, ConfigRepository, ConfigValue, DomainError,
};

orm_table! {
    struct ConfigEntryRow => "config_entries" {
        namespace: String => "namespace",
        key: String => "key",
        value: String => "value",
        revision: i64 => "revision",
        updated_at_epoch: i64 => "updated_at_epoch",
        updated_by: String => "updated_by",
    }
}

#[derive(Clone)]
pub struct PostgresConfigRepository {
    postgres: Arc<Postgres>,
}

impl PostgresConfigRepository {
    pub fn new(postgres: Arc<Postgres>) -> Self {
        Self { postgres }
    }

    fn db(&self) -> Database<PostgresDialect, aster_orm::PostgresExecutor> {
        Database::new(PostgresDialect, self.postgres.executor().clone())
    }
}

#[async_trait]
impl ConfigRepository for PostgresConfigRepository {
    async fn list_namespace(
        &self,
        namespace: &ConfigNamespace,
    ) -> Result<Vec<ConfigEntry>, DomainError> {
        let rows = self
            .db()
            .fetch_all_as(
                select_from::<ConfigEntryRow>()
                    .select((
                        ConfigEntryRow::namespace(),
                        ConfigEntryRow::key(),
                        ConfigEntryRow::value(),
                        ConfigEntryRow::revision(),
                        ConfigEntryRow::updated_at_epoch(),
                        ConfigEntryRow::updated_by(),
                    ))
                    .filter(ConfigEntryRow::namespace().eq(namespace.as_str().to_string()))
                    .order_by(ConfigEntryRow::key(), OrderDirection::Asc),
            )
            .await
            .map_err(|err| DomainError::infrastructure(format!("list config failed: {err}")))?;

        rows.rows.into_iter().map(map_row).collect()
    }

    async fn list_all(&self) -> Result<Vec<ConfigEntry>, DomainError> {
        let rows = self
            .db()
            .fetch_all_as(
                select_from::<ConfigEntryRow>()
                    .select((
                        ConfigEntryRow::namespace(),
                        ConfigEntryRow::key(),
                        ConfigEntryRow::value(),
                        ConfigEntryRow::revision(),
                        ConfigEntryRow::updated_at_epoch(),
                        ConfigEntryRow::updated_by(),
                    ))
                    .order_by(ConfigEntryRow::namespace(), OrderDirection::Asc),
            )
            .await
            .map_err(|err| DomainError::infrastructure(format!("list all config failed: {err}")))?;

        rows.rows.into_iter().map(map_row).collect()
    }

    async fn find(
        &self,
        namespace: &ConfigNamespace,
        key: &ConfigKey,
    ) -> Result<Option<ConfigEntry>, DomainError> {
        let row = self
            .db()
            .fetch_optional_as(
                select_from::<ConfigEntryRow>()
                    .select((
                        ConfigEntryRow::namespace(),
                        ConfigEntryRow::key(),
                        ConfigEntryRow::value(),
                        ConfigEntryRow::revision(),
                        ConfigEntryRow::updated_at_epoch(),
                        ConfigEntryRow::updated_by(),
                    ))
                    .filter(ConfigEntryRow::namespace().eq(namespace.as_str().to_string()))
                    .filter(ConfigEntryRow::key().eq(key.as_str().to_string())),
            )
            .await
            .map_err(|err| DomainError::infrastructure(format!("load config failed: {err}")))?;

        row.map(map_row).transpose()
    }

    async fn upsert(&self, entry: &ConfigEntry) -> Result<(), DomainError> {
        let value = entry.value().as_str().to_owned();
        upsert_fenced(
            &PostgresDialect,
            self.postgres.executor(),
            entry.namespace().as_str(),
            entry.key().as_str(),
            &value,
            entry.revision(),
            entry.updated_at_epoch(),
            entry.updated_by(),
        )
        .await
        .map_err(|err| DomainError::infrastructure(format!("upsert config failed: {err}")))?;
        Ok(())
    }

    async fn commit_put(
        &self,
        namespace: &ConfigNamespace,
        key: &ConfigKey,
        value: &ConfigValue,
        updated_at_epoch: i64,
        updated_by: &str,
    ) -> Result<ConfigEntry, DomainError> {
        if updated_by.len() > 128 {
            return Err(DomainError::validation("updated_by is too long"));
        }
        let value = value.as_str().to_owned();
        let dialect = PostgresDialect;
        let executor = self.postgres.executor().clone();
        let namespace = namespace.as_str().to_owned();
        let key = key.as_str().to_owned();
        let updated_by = updated_by.to_owned();
        let namespace_for_tx = namespace.clone();
        let key_for_tx = key.clone();
        let committed = executor
            .transaction(move |tx| {
                Box::pin(async move {
                    commit_put_in(
                        &dialect,
                        tx,
                        &namespace_for_tx,
                        &key_for_tx,
                        &value,
                        updated_at_epoch,
                        &updated_by,
                    )
                    .await
                })
            })
            .await
            .map_err(|err| DomainError::infrastructure(format!("commit config failed: {err}")))?;
        entry_from_committed(&namespace, &key, committed)
    }

    async fn delete(
        &self,
        namespace: &ConfigNamespace,
        key: &ConfigKey,
    ) -> Result<bool, DomainError> {
        Ok(self.commit_delete(namespace, key).await?.is_some())
    }

    async fn commit_delete(
        &self,
        namespace: &ConfigNamespace,
        key: &ConfigKey,
    ) -> Result<Option<i64>, DomainError> {
        let dialect = PostgresDialect;
        let executor = self.postgres.executor().clone();
        let namespace = namespace.as_str().to_owned();
        let key = key.as_str().to_owned();
        executor
            .transaction(move |tx| {
                Box::pin(async move { commit_delete_in(&dialect, tx, &namespace, &key).await })
            })
            .await
            .map_err(|err| {
                DomainError::infrastructure(format!("commit config delete failed: {err}"))
            })
    }

    async fn allocate_revision(&self) -> Result<i64, DomainError> {
        let revision: i64 = self
            .db()
            .fetch_one_as(sql_query::<i64>(
                "update config_meta set revision = revision + 1 where id = 1 \
                     returning revision",
            ))
            .await
            .map_err(|err| {
                DomainError::infrastructure(format!("allocate config revision failed: {err}"))
            })?;
        Ok(revision)
    }

    async fn read_snapshot(&self) -> Result<(Vec<ConfigEntry>, i64), DomainError> {
        // One statement, so the meta revision and the entry rows share a snapshot.
        let rows = self
            .db()
            .fetch_all_as(sql_query::<(
                Option<String>,
                Option<String>,
                Option<String>,
                Option<i64>,
                Option<i64>,
                Option<String>,
                i64,
            )>(
                "select e.namespace, e.key, e.value, e.revision, e.updated_at_epoch, e.updated_by, m.revision \
                 from config_meta m left join config_entries e on true where m.id = 1",
            ))
            .await
            .map_err(|err| DomainError::infrastructure(format!("load config snapshot failed: {err}")))?;
        let mut revision = 0;
        let mut entries = Vec::new();
        for (namespace, key, value, entry_revision, updated_at, updated_by, meta_revision) in
            rows.rows
        {
            revision = meta_revision;
            let (Some(namespace), Some(key), Some(value)) = (namespace, key, value) else {
                continue;
            };
            entries.push(entry_from_committed(
                &namespace,
                &key,
                CommittedConfig {
                    revision: entry_revision.unwrap_or(0),
                    value,
                    updated_at_epoch: updated_at.unwrap_or(0),
                    updated_by: updated_by.unwrap_or_default(),
                },
            )?);
        }
        Ok((entries, revision))
    }

    async fn max_revision(&self) -> Result<i64, DomainError> {
        let revision: i64 = self
            .db()
            .fetch_one_as(sql_query::<i64>(
                "select revision::bigint from config_meta where id = 1",
            ))
            .await
            .map_err(|err| {
                DomainError::infrastructure(format!("load config revision failed: {err}"))
            })?;
        Ok(revision.max(0))
    }

    async fn removed_keys(&self) -> Result<Vec<(String, String)>, DomainError> {
        let rows = self
            .db()
            .fetch_all_as(sql_query::<(String, String)>(
                "select namespace, key from config_removals",
            ))
            .await
            .map_err(|err| {
                DomainError::infrastructure(format!("load config removals failed: {err}"))
            })?;
        Ok(rows.rows)
    }
}

fn map_row(row: (String, String, String, i64, i64, String)) -> Result<ConfigEntry, DomainError> {
    let (namespace, key, value, revision, updated_at_epoch, updated_by) = row;
    let namespace = ConfigNamespace::parse(namespace)?;
    let key = ConfigKey::parse(key)?;
    ConfigEntry::new(
        namespace,
        key,
        ConfigValue::from_text(value),
        revision,
        updated_at_epoch,
        updated_by,
    )
}
