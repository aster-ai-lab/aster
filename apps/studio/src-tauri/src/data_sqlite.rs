//! Database statements for the data page. a3s-orm owns every bundled driver.

use a3s_studio_data_sqlite::Grid;

#[tauri::command]
pub async fn data_sqlite_query(
    path: String,
    sql: String,
    engine: Option<String>,
) -> Result<Grid, String> {
    let engine = engine.unwrap_or_else(|| "sqlite".to_owned());
    a3s_studio_data_sqlite::query(&engine, &path, &sql).await
}
