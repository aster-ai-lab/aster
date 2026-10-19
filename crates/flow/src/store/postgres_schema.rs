use a3s_orm::{
    sql_query, Database, DatabaseError, MigrationReport, Migrator, PostgresDialect, PostgresError,
    PostgresExecutor, Value,
};
use sha2::{Digest, Sha256};

use crate::error::{FlowError, Result};

use super::postgres_migrations;

use super::migrations::canonical_flow_migration_version;

/// Apply the canonical Flow PostgreSQL schema with a dedicated migration
/// executor.
///
/// Production hosts should terminate this operation before starting serving
/// workers, then construct stores and queues through their `*_verified`
/// constructors with a role that does not have DDL authority.
pub async fn migrate_postgres_flow(executor: &PostgresExecutor) -> Result<MigrationReport> {
    adopt_historical_flow_migration_ids(executor).await?;
    Migrator::new(executor.clone())
        .run(postgres_migrations())
        .await
        .map_err(|error| FlowError::Store(format!("PostgreSQL Flow migration failed: {error}")))
}

/// Journals created before the Flow migration ids were renamed still record
/// `kense-flow-*`. The SQL checksum is unchanged, so the ledger version is
/// rewritten to `a3s-flow-*` before admission. A checksum that does not
/// match the current script is left untouched and fails closed.
async fn adopt_historical_flow_migration_ids(executor: &PostgresExecutor) -> Result<()> {
    let database = Database::new(PostgresDialect, executor.clone());
    let applied = match database
        .fetch_all(sql_query::<()>(
            "select version, checksum from a3s_orm_migrations where version like 'kense-flow-%' order by version",
        ))
        .await
    {
        Ok(result) => result.rows,
        Err(error) if missing_migration_ledger(&error) => return Ok(()),
        Err(error) => {
            return Err(FlowError::Store(format!(
                "PostgreSQL Flow migration ledger could not be read: {error}"
            )))
        }
    };
    let source = postgres_migrations();
    for row in applied {
        let version = ledger_text(&row, 0)?;
        let checksum = ledger_text(&row, 1)?;
        let Some(renamed) = canonical_flow_migration_version(&version) else {
            continue;
        };
        let Some(migration) = source
            .iter()
            .find(|migration| migration.version() == renamed)
        else {
            continue;
        };
        let source_checksum = format!("{:x}", Sha256::digest(migration.up_sql().as_bytes()));
        if source_checksum != checksum {
            return Err(FlowError::Store(format!(
                "historical Flow migration {version} checksum {checksum} does not match {renamed} ({source_checksum})"
            )));
        }
        database
            .execute(
                sql_query::<()>("update a3s_orm_migrations set version = ")
                    .bind(renamed)
                    .append(" where version = ")
                    .bind(version),
            )
            .await
            .map_err(|error| {
                FlowError::Store(format!(
                    "PostgreSQL Flow migration ledger could not be updated: {error}"
                ))
            })?;
    }
    Ok(())
}

fn ledger_text(row: &a3s_orm::PostgresRow, index: usize) -> Result<String> {
    match row.get(index) {
        Some(Value::String(value)) => Ok(value.clone()),
        _ => Err(FlowError::Store(
            "PostgreSQL Flow migration ledger returned a non-text column".to_string(),
        )),
    }
}

fn missing_migration_ledger(error: &DatabaseError<PostgresError>) -> bool {
    let DatabaseError::Execute(PostgresError::Database(source)) = error else {
        return false;
    };
    source.code().is_some_and(|code| code.code() == "42P01")
}

pub(crate) async fn verify_postgres_flow(executor: &PostgresExecutor) -> Result<()> {
    Migrator::new(executor.clone())
        .verify_required(postgres_migrations())
        .await
        .map_err(|error| {
            FlowError::Store(format!(
                "PostgreSQL Flow schema admission failed: {error}; run migrate_postgres_flow before serving"
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::super::migrations::canonical_flow_migration_version;

    #[test]
    fn historical_flow_migration_ids_rename_only_the_prefix() {
        assert_eq!(
            canonical_flow_migration_version("kense-flow-0001-events").as_deref(),
            Some("a3s-flow-0001-events")
        );
        assert_eq!(
            canonical_flow_migration_version("a3s-flow-0001-events"),
            None
        );
        assert_eq!(canonical_flow_migration_version("kense-flow-"), None);
    }
}

// NOTE: `kense-flow-` prefixed migration versions are historical records.
// The canonical_flow_migration_version function maps them to the
// a3s-flow namespace for lookup. Do not rewrite historical records.
