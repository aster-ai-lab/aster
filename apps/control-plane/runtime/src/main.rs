//! aster-runtime binary — Gateway `/v1` data-plane.

use std::net::SocketAddr;

use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8787);
    let addr: SocketAddr = format!("{host}:{port}").parse()?;

    // An empty allowlist refuses every request, which looks exactly like an outage from the
    // client side. Say so once at boot instead of letting callers decode 403s.
    if aster_gateway_runtime::entitlement::denies_everything() {
        tracing::warn!(
            "entitlement allowlists are empty: every model, tool and API request will be refused \
             with entitlement.denied. Set ASTER_ENTITLED_MODELS / ASTER_ENTITLED_APIS / \
             ASTER_ENTITLED_TOOLS (comma separated, or * for all)."
        );
    }

    let data_dir = aster_gateway_runtime::usage::data_dir();
    let mut state = aster_gateway_runtime::AppState::new(data_dir);
    // Wire the metering event spine from the env; a missing origin keeps
    // the exporter refused closed and a dead server degrades to disabled
    // with a named error rather than taking the data plane down.
    match aster_gateway_runtime::usage_export::NatsUsageExport::connect_from_env().await {
        Some(Ok(export)) => {
            state = state.with_usage_export(
                aster_gateway_runtime::usage_export::UsageExporter::with(std::sync::Arc::new(export)),
            );
            tracing::info!("usage event spine wired (events.runtime.usage)");
        }
        Some(Err(error)) => {
            tracing::error!(%error, "usage event spine connection failed; export stays refused closed");
        }
        None => {}
    }
    let app = aster_gateway_runtime::http::router(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("aster-runtime listening on http://{addr}");
    tracing::info!(
        "paths: POST /v1/api/services/{{kind}}/invoke (sku in additional parameters) · /v1/api/services/{{kind}}/invoke/{{sku}} · /v1/api/models · /v1/api/usage · /v1/api/events/recent"
    );
    axum::serve(listener, app).await?;
    Ok(())
}
