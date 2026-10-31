//! Independent distributed config process (`aster-config`).
//!
//! `ASTER_CONFIG_LISTEN` / `ASTER_CONFIG_HOST`+`ASTER_CONFIG_PORT` default to `:7652`.
//! `ASTER_CONFIG_DATABASE_URL` (or `DATABASE_URL`) is required.
//! `ASTER_CONFIG_IGGY_ADDRESS` (or `IGGY_ADDRESS`) defaults to `127.0.0.1:5102`.

use std::sync::Arc;

use aster_boot::{
    AxumAdapter, BootApplication, BootFactory, MiddlewareRoute, OpenApiInfo, ValidationOptions,
};
use aster_config::{
    bootstrap_config_center, health_module, ConfigCenterModule, ConfigServerConfig, IggyBus,
    IggyEndpoint, Postgres,
};

const API_PREFIX: &str = "/api";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();

    let config = Arc::new(ConfigServerConfig::from_env()?);
    let listen = config.listen();
    let addr: std::net::SocketAddr = listen
        .parse()
        .map_err(|err| format!("invalid ASTER_CONFIG_LISTEN={listen}: {err}"))?;

    let postgres = Arc::new(Postgres::connect(
        &config.database_url,
        config.database_pool_size,
    )?);
    postgres.ensure_schema().await?;

    let iggy = Arc::new(
        IggyBus::connect(&IggyEndpoint {
            address: config.iggy_address.clone(),
            username: config.iggy_username.clone(),
            password: config.iggy_password.clone(),
        })
        .await?,
    );
    iggy.ping().await?;

    let service = bootstrap_config_center(
        Arc::clone(&postgres),
        Arc::clone(&iggy),
        Arc::clone(&config),
    )
    .await
    .map_err(aster_config::map_boot)?;

    let health = health_module(postgres, iggy);

    let port = addr.port();
    let _discovery = aster_boot::try_register_from_env("aster-config", port).await?;
    if _discovery.is_some() {
        eprintln!("aster-config registered with etcd discovery");
    }

    let mut app = BootFactory::create_with_builder(
        BootApplication::builder()
            .global_prefix(API_PREFIX)
            .exclude_global_prefix([
                MiddlewareRoute::any("/health")?,
                MiddlewareRoute::any("/live")?,
            ])
            .serve_openapi(
                "/docs/openapi.json",
                OpenApiInfo::new("aster-config", env!("CARGO_PKG_VERSION"))
                    .with_description(
                        "Distributed config service. Postgres authority + Iggy config_changed.",
                    )
                    .with_server_description("/", "Current host"),
            )
            .use_global_validation_options(
                ValidationOptions::new()
                    .whitelist(true)
                    .forbid_non_whitelisted(true)
                    .transform(true),
            )
            .import(health)
            .import(ConfigCenterModule::new(service)),
    )?;

    eprintln!("aster-config listening on {addr}");
    app.listen_with(&AxumAdapter::new(), addr).await?;
    Ok(())
}
