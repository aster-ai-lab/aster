//! Independent billing process.
//!
//! `ASTER_BILLING_LISTEN` defaults to `127.0.0.1:7651`.
//! `ASTER_BILLING_DATABASE_URL` selects Postgres through aster-orm. An unset or
//! empty URL refuses startup before the process listens or registers. It does
//! not read the Cloud usage ledger.
//! `ASTER_BILLING_INTERNAL_TOKEN` admits usage, invoice, and subscription writes.
//! An unset token keeps those routes closed. A non-loopback listen without the
//! token refuses to start.
//! `ASTER_ETCD_ENDPOINTS` optional — registers into Boot discovery when set.

use std::sync::Arc;

use aster_billing::{open_postgres, serve, InternalGuard};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listen = std::env::var("ASTER_BILLING_LISTEN").unwrap_or_else(|_| "127.0.0.1:7651".into());
    let port = listen
        .rsplit_once(':')
        .and_then(|(_, port)| port.parse::<u16>().ok())
        .unwrap_or(7651);
    let internal_token = std::env::var("ASTER_BILLING_INTERNAL_TOKEN")
        .ok()
        .filter(|value| !value.is_empty());
    if !loopback_listen(&listen) && internal_token.is_none() {
        return Err(
            "ASTER_BILLING_INTERNAL_TOKEN is required when ASTER_BILLING_LISTEN is not loopback".into(),
        );
    }
    let internal = match &internal_token {
        Some(token) => InternalGuard::from_secret(token.clone()),
        None => {
            eprintln!(
                "aster-billing: ASTER_BILLING_INTERNAL_TOKEN is unset; usage, invoice, and subscription writes stay closed"
            );
            InternalGuard::closed()
        }
    };
    let configured = std::env::var("ASTER_BILLING_DATABASE_URL").ok();
    let database_url = require_database_url(configured.as_deref())?;
    let billing = open_postgres(database_url).await?;

    let _discovery = aster_boot::try_register_from_env("aster-billing", port).await?;
    if _discovery.is_some() {
        eprintln!("aster-billing registered with etcd discovery");
    }

    let listener = tokio::net::TcpListener::bind(&listen).await?;
    let bound = listener.local_addr()?;
    eprintln!("aster-billing listening on {bound}");
    serve(listener, Arc::new(billing), internal).await?;
    Ok(())
}

/// Postgres is the invoice ledger. An empty URL is the same refusal as an
/// unset URL: the process must not listen.
fn require_database_url(raw: Option<&str>) -> Result<&str, String> {
    match raw.map(str::trim).filter(|url| !url.is_empty()) {
        Some(url) => Ok(url),
        None => {
            Err("ASTER_BILLING_DATABASE_URL is required; billing stores invoices in Postgres".into())
        }
    }
}

fn loopback_listen(listen: &str) -> bool {
    let Some((host, _)) = listen.rsplit_once(':') else {
        return false;
    };
    let host = host.trim_matches(['[', ']']);
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
