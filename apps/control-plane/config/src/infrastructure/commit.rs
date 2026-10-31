//! One local transaction owns a config revision and its entry.
//!
//! The publish to other replicas happens after this commit returns.

use super::super::domain::{ConfigEntry, ConfigKey, ConfigNamespace, ConfigValue, DomainError};
use aster_orm::{sql_query, Dialect, Executor, FromRow, Query, Row};

#[derive(Debug)]
pub(super) struct CommitError(String);

impl std::fmt::Display for CommitError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for CommitError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CommittedConfig {
    pub revision: i64,
    pub value: String,
    pub updated_at_epoch: i64,
    pub updated_by: String,
}

pub(super) async fn commit_put_in<D, E>(
    dialect: &D,
    tx: &E,
    namespace: &str,
    key: &str,
    value: &str,
    updated_at_epoch: i64,
    updated_by: &str,
) -> Result<CommittedConfig, CommitError>
where
    D: Dialect,
    E: Executor,
    E::Row: Row,
{
    let revision = one_i64(
        dialect,
        tx,
        sql_query::<i64>(
            "update config_meta set revision = revision + 1 where id = 1 returning revision",
        ),
    )
    .await?;
    let stored = affected(
        dialect,
        tx,
        fenced_write(
            namespace,
            key,
            value,
            revision,
            updated_at_epoch,
            updated_by,
        ),
    )
    .await?;
    if stored == 0 {
        return read_entry(dialect, tx, namespace, key).await;
    }
    Ok(CommittedConfig {
        revision,
        value: value.to_owned(),
        updated_at_epoch,
        updated_by: updated_by.to_owned(),
    })
}

pub(super) async fn commit_delete_in<D, E>(
    dialect: &D,
    tx: &E,
    namespace: &str,
    key: &str,
) -> Result<Option<i64>, CommitError>
where
    D: Dialect,
    E: Executor,
    E::Row: Row,
{
    let removed = affected(
        dialect,
        tx,
        sql_query::<()>("delete from config_entries where namespace = ")
            .bind(namespace)
            .append(" and key = ")
            .bind(key),
    )
    .await?;
    if removed == 0 {
        return Ok(None);
    }
    let revision = one_i64(
        dialect,
        tx,
        sql_query::<i64>(
            "update config_meta set revision = revision + 1 where id = 1 returning revision",
        ),
    )
    .await?;
    affected(
        dialect,
        tx,
        sql_query::<()>(
            "insert into config_removals (namespace, key, revision) values (",
        )
        .bind(namespace)
        .append(", ")
        .bind(key)
        .append(", ")
        .bind(revision)
        .append(
            ") on conflict (namespace, key) do update set revision = excluded.revision where config_removals.revision < excluded.revision",
        ),
    )
    .await?;
    Ok(Some(revision))
}

pub(super) async fn upsert_fenced<D, E>(
    dialect: &D,
    executor: &E,
    namespace: &str,
    key: &str,
    value: &str,
    revision: i64,
    updated_at_epoch: i64,
    updated_by: &str,
) -> Result<u64, CommitError>
where
    D: Dialect,
    E: Executor,
    E::Row: Row,
{
    affected(
        dialect,
        executor,
        fenced_write(
            namespace,
            key,
            value,
            revision,
            updated_at_epoch,
            updated_by,
        ),
    )
    .await
}

fn fenced_write(
    namespace: &str,
    key: &str,
    value: &str,
    revision: i64,
    updated_at_epoch: i64,
    updated_by: &str,
) -> aster_orm::SqlQuery<()> {
    sql_query::<()>(
        "insert into config_entries (namespace, key, value, revision, updated_at_epoch, updated_by) select ",
    )
    .bind(namespace)
    .append(", ")
    .bind(key)
    .append(", ")
    .bind(value)
    .append(", ")
    .bind(revision)
    .append(", ")
    .bind(updated_at_epoch)
    .append(", ")
    .bind(updated_by)
    .append(" where not exists (select 1 from config_removals where namespace = ")
    .bind(namespace)
    .append(" and key = ")
    .bind(key)
    .append(" and revision >= ")
    .bind(revision)
    .append(
        ") on conflict (namespace, key) do update set value = excluded.value, revision = excluded.revision, updated_at_epoch = excluded.updated_at_epoch, updated_by = excluded.updated_by where config_entries.revision < excluded.revision",
    )
}

pub(super) fn entry_from_committed(
    namespace: &str,
    key: &str,
    committed: CommittedConfig,
) -> Result<ConfigEntry, DomainError> {
    ConfigEntry::new(
        ConfigNamespace::parse(namespace)?,
        ConfigKey::parse(key)?,
        ConfigValue::from_text(committed.value),
        committed.revision,
        committed.updated_at_epoch,
        committed.updated_by,
    )
}

async fn read_entry<D, E>(
    dialect: &D,
    tx: &E,
    namespace: &str,
    key: &str,
) -> Result<CommittedConfig, CommitError>
where
    D: Dialect,
    E: Executor,
    E::Row: Row,
{
    let row = one_row(
        dialect,
        tx,
        sql_query::<(String, i64, i64, String)>(
            "select value, revision, updated_at_epoch, updated_by from config_entries where namespace = ",
        )
        .bind(namespace)
        .append(" and key = ")
        .bind(key),
    )
    .await?;
    let (value, revision, updated_at_epoch, updated_by) = row;
    Ok(CommittedConfig {
        revision,
        value,
        updated_at_epoch,
        updated_by,
    })
}

async fn affected<D, E>(dialect: &D, tx: &E, query: impl Query) -> Result<u64, CommitError>
where
    D: Dialect,
    E: Executor,
{
    let compiled = query
        .compile(dialect)
        .map_err(|err| CommitError(err.to_string()))?;
    let result = tx
        .execute(&compiled)
        .await
        .map_err(|err| CommitError(err.to_string()))?;
    Ok(result.rows_affected)
}

async fn one_i64<D, E>(
    dialect: &D,
    tx: &E,
    query: aster_orm::SqlQuery<i64>,
) -> Result<i64, CommitError>
where
    D: Dialect,
    E: Executor,
    E::Row: Row,
{
    let compiled = query
        .compile(dialect)
        .map_err(|err| CommitError(err.to_string()))?;
    let result = tx
        .fetch_all(&compiled)
        .await
        .map_err(|err| CommitError(err.to_string()))?;
    match result.rows.as_slice() {
        [row] => i64::from_row(row).map_err(|err| CommitError(err.to_string())),
        _ => Err(CommitError("expected one row".into())),
    }
}

async fn one_row<D, E, T>(
    dialect: &D,
    tx: &E,
    query: aster_orm::SqlQuery<T>,
) -> Result<T, CommitError>
where
    D: Dialect,
    E: Executor,
    E::Row: Row,
    T: FromRow,
{
    let compiled = query
        .compile(dialect)
        .map_err(|err| CommitError(err.to_string()))?;
    let result = tx
        .fetch_all(&compiled)
        .await
        .map_err(|err| CommitError(err.to_string()))?;
    match result.rows.as_slice() {
        [row] => T::from_row(row).map_err(|err| CommitError(err.to_string())),
        [] => Err(CommitError("expected one row".into())),
        _ => Err(CommitError("expected one row".into())),
    }
}
