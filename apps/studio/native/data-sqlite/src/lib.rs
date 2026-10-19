//! Open a database with an a3s-orm driver and return one statement's grid.
//!
//! Admitted runtimes come from `Engine::profiles`. Other catalogued engines
//! parse and then refuse with the orm's missing-driver message.

use std::path::Path;

use a3s_orm::{Engine, PostgresExecutor, SqliteExecutor, StatementGrid, Value};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Grid {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
}

pub async fn query(engine: &str, target: &str, sql: &str) -> Result<Grid, String> {
    let sql = sql.trim();
    let target = target.trim();
    if sql.is_empty() {
        return Err("SQL statement is empty".into());
    }
    if target.is_empty() {
        return Err("Database target is empty".into());
    }
    let selected = if engine.trim().is_empty() {
        Engine::Sqlite
    } else {
        Engine::parse(engine).map_err(|error| error.to_string())?
    };
    if !selected.profile().runtime {
        return Err(selected.missing_driver_message());
    }
    let grid = match selected {
        Engine::Sqlite => {
            let executor = SqliteExecutor::open(Path::new(target))
                .await
                .map_err(|error| error.to_string())?;
            executor
                .query_statement(sql)
                .await
                .map_err(|error| error.to_string())?
        }
        Engine::Postgres => {
            let executor =
                PostgresExecutor::connect_no_tls(target, 1).map_err(|error| error.to_string())?;
            executor
                .query_statement(sql)
                .await
                .map_err(|error| error.to_string())?
        }
        _ => return Err(selected.missing_driver_message()),
    };
    Ok(grid_from(grid))
}

pub async fn query_path(path: &str, sql: &str) -> Result<Grid, String> {
    query("sqlite", path, sql).await
}

fn grid_from(grid: StatementGrid) -> Grid {
    Grid {
        columns: grid.columns,
        rows: grid
            .rows
            .into_iter()
            .map(|row| row.into_iter().map(cell).collect())
            .collect(),
    }
}

fn cell(value: Value) -> serde_json::Value {
    match value {
        Value::Null => serde_json::Value::Null,
        Value::Bool(value) => serde_json::Value::Bool(value),
        Value::I64(value) => serde_json::Value::from(value),
        Value::U64(value) => serde_json::Value::from(value),
        Value::F64(value) => serde_json::Number::from_f64(value)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        Value::String(value) => serde_json::Value::String(value),
        Value::Bytes(value) => serde_json::Value::Array(
            value
                .into_iter()
                .map(|byte| serde_json::Value::from(byte))
                .collect(),
        ),
        Value::Array(values) => serde_json::Value::Array(values.into_iter().map(cell).collect()),
        Value::Uuid(value) => serde_json::Value::String(value.to_string()),
        Value::Json(value) => serde_json::Value::String(value.to_string()),
        Value::Date(value) => serde_json::Value::String(value.to_string()),
        Value::Time(value) => serde_json::Value::String(value.to_string()),
        Value::DateTime(value) => serde_json::Value::String(value.to_string()),
        Value::DateTimeUtc(value) => serde_json::Value::String(value.to_rfc3339()),
        Value::Decimal(value) => serde_json::Value::String(value.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::query;

    #[tokio::test]
    async fn sqlite_round_trip_uses_the_orm_driver() {
        let path = std::env::temp_dir().join(format!(
            "a3s-data-orm-{}-{}.sqlite",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let path = path.to_string_lossy().into_owned();
        query(
            "sqlite",
            &path,
            "CREATE TABLE sales (sku TEXT, amount INTEGER)",
        )
        .await
        .expect("create");
        query(
            "sqlite",
            &path,
            "INSERT INTO sales (sku, amount) VALUES ('pen', 10), ('book', 32)",
        )
        .await
        .expect("insert");
        let grid = query(
            "sqlite",
            &path,
            "SELECT sku, amount FROM sales ORDER BY sku",
        )
        .await
        .expect("select");
        let _ = std::fs::remove_file(&path);
        assert_eq!(grid.columns, ["sku", "amount"]);
        assert_eq!(
            grid.rows,
            vec![
                vec![serde_json::json!("book"), serde_json::json!(32)],
                vec![serde_json::json!("pen"), serde_json::json!(10)],
            ]
        );
    }

    #[tokio::test]
    async fn mysql_is_refused_because_a3s_orm_has_no_driver() {
        let error = query("mysql", "127.0.0.1", "SELECT 1").await.unwrap_err();
        assert_eq!(error, "a3s-orm has no MySQL driver");
    }

    #[tokio::test]
    async fn an_unknown_engine_is_refused() {
        let error = query("redis", "127.0.0.1", "SELECT 1").await.unwrap_err();
        assert_eq!(error, "a3s-orm has no redis driver");
    }
}
