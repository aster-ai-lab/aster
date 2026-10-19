//! One JSON request on stdin, one result grid on stdout.
//!
//! `{ "path": "...", "sql": "...", "engine": "sqlite" | "postgres" }`

use serde::Deserialize;

#[derive(Deserialize)]
struct Request {
    path: String,
    sql: String,
    #[serde(default)]
    engine: String,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let mut input = String::new();
    std::io::Read::read_to_string(&mut std::io::stdin(), &mut input)
        .map_err(|error| error.to_string())?;
    let request: Request = serde_json::from_str(&input).map_err(|error| error.to_string())?;
    let grid = a3s_studio_data_sqlite::query(&request.engine, &request.path, &request.sql).await?;
    println!(
        "{}",
        serde_json::to_string(&grid).map_err(|error| error.to_string())?
    );
    Ok(())
}
