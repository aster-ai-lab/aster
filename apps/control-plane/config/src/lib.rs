//! Aster distributed config service.
//!
//! Postgres is the authority; Iggy topic `config_changed` fans out revisions.
//! Consumers (e.g. aster-api) use [`HttpConfigClient`] over `ASTER_CONFIG_URL`.

mod application;
pub mod bootstrap;
mod client;
mod config;
mod domain;
mod error;
mod iggy;
mod infrastructure;
mod interfaces;
mod migrations;
mod postgres;

use std::sync::Arc;

use aster_boot::{
    BootError, BoxFuture, HealthIndicator, HealthIndicatorResult, HealthModule, HealthStatus,
    Module, ModuleRef, ProviderDefinition, Result,
};

pub use application::{
    ConfigCenterService, ConfigChangeBus, ConfigChangedEvent, ConfigReader, PutConfigCommand,
};
pub use client::HttpConfigClient;
pub use config::ConfigServerConfig;
pub use domain::{ConfigEntry, ConfigKey, ConfigNamespace, ConfigValue, DomainError};
pub use error::{into_boot, map_app_error, AppError, AppResult};
pub use iggy::{IggyBus, IggyEndpoint, CONFIG_CHANGED_SUBJECT};
pub use infrastructure::{IggyConfigChangeBus, PostgresConfigRepository};
pub use interfaces::http::ConfigCenterController;
pub use postgres::Postgres;

use domain::ConfigRepository;

#[derive(Clone)]
pub struct ConfigCenterModule {
    service: Arc<ConfigCenterService>,
}

impl ConfigCenterModule {
    pub fn new(service: Arc<ConfigCenterService>) -> Self {
        Self { service }
    }
}

impl Module for ConfigCenterModule {
    fn name(&self) -> &'static str {
        "config_center"
    }

    fn providers(&self) -> Result<Vec<ProviderDefinition>> {
        Ok(vec![
            ProviderDefinition::from_arc(Arc::clone(&self.service)),
            ProviderDefinition::factory::<ConfigCenterController, _>(|module_ref| {
                Ok(ConfigCenterController {
                    configs: module_ref.get::<ConfigCenterService>()?,
                })
            }),
        ])
    }

    fn controllers(&self, module_ref: &ModuleRef) -> Result<Vec<aster_boot::ControllerDefinition>> {
        Ok(vec![module_ref
            .get::<ConfigCenterController>()?
            .controller()?])
    }
}

pub struct PostgresHealthIndicator {
    postgres: Arc<Postgres>,
}

impl PostgresHealthIndicator {
    pub fn new(postgres: Arc<Postgres>) -> Self {
        Self { postgres }
    }
}

impl HealthIndicator for PostgresHealthIndicator {
    fn check(&self) -> BoxFuture<'static, Result<HealthIndicatorResult>> {
        let postgres = Arc::clone(&self.postgres);
        Box::pin(async move {
            match postgres.ping().await {
                Ok(()) => Ok(HealthIndicatorResult::up()),
                Err(err) => Ok(HealthIndicatorResult::new(HealthStatus::Down)
                    .with_detail_value("error", err.to_string())),
            }
        })
    }
}

pub struct IggyHealthIndicator {
    iggy: Arc<IggyBus>,
}

impl IggyHealthIndicator {
    pub fn new(iggy: Arc<IggyBus>) -> Self {
        Self { iggy }
    }
}

impl HealthIndicator for IggyHealthIndicator {
    fn check(&self) -> BoxFuture<'static, Result<HealthIndicatorResult>> {
        let iggy = Arc::clone(&self.iggy);
        Box::pin(async move {
            match iggy.ping().await {
                Ok(()) => {
                    Ok(HealthIndicatorResult::up().with_detail_value("address", iggy.address()))
                }
                Err(err) => Ok(HealthIndicatorResult::new(HealthStatus::Down)
                    .with_detail_value("error", err.to_string())),
            }
        })
    }
}

/// Wire Postgres + Iggy adapters into [`ConfigCenterService`].
pub async fn bootstrap_config_center(
    postgres: Arc<Postgres>,
    iggy: Arc<IggyBus>,
    app_config: Arc<ConfigServerConfig>,
) -> AppResult<Arc<ConfigCenterService>> {
    let repo: Arc<dyn ConfigRepository> = Arc::new(PostgresConfigRepository::new(postgres));
    let bus = Arc::new(IggyConfigChangeBus::new(iggy));
    ConfigCenterService::start(repo, bus, app_config).await
}

/// Build the health module used by the binary.
pub fn health_module(postgres: Arc<Postgres>, iggy: Arc<IggyBus>) -> HealthModule {
    HealthModule::new("health")
        .indicator("postgres", PostgresHealthIndicator::new(postgres))
        .indicator("iggy", IggyHealthIndicator::new(iggy))
        .global()
}

pub fn map_boot(err: AppError) -> BootError {
    map_app_error(err)
}
