use std::sync::Arc;

#[allow(unused_imports)]
use aster_boot::{body, controller, delete, get, param, put, query, validate, Result};

use crate::application::{CatalogGroupDto, ConfigCenterService, PutConfigCommand, Surface};
use crate::bootstrap::BootstrapGroup;
use crate::error::into_boot;
use crate::error::AppError;
use crate::CONFIG_CHANGED_SUBJECT;

use super::dto::{ConfigEntryDto, ConfigStatusDto, PutConfigDto};

#[derive(Debug)]
pub struct ConfigCenterController {
    pub configs: Arc<ConfigCenterService>,
}

#[controller("/configs")]
#[validate(whitelist, forbid_non_whitelisted)]
impl ConfigCenterController {
    #[get("/status")]
    async fn status(&self) -> Result<ConfigStatusDto> {
        Ok(ConfigStatusDto {
            revision: self.configs.current_revision(),
            publisher_id: self.configs.publisher_id().to_string(),
            subject: CONFIG_CHANGED_SUBJECT.to_string(),
        })
    }

    #[get("/bootstrap")]
    async fn bootstrap_catalog(&self) -> Result<Vec<BootstrapGroup>> {
        Ok(self.configs.bootstrap_groups())
    }

    #[get("/builtins")]
    async fn runtime_builtins(
        &self,
        #[query("surface")] surface: Option<String>,
    ) -> Result<Vec<CatalogGroupDto>> {
        let surface = Surface::parse(surface.as_deref())
            .ok_or_else(|| AppError::validation("surface must be runtime or login"));
        into_boot(match surface {
            Ok(surface) => self.configs.builtin_groups(surface).await,
            Err(err) => Err(err),
        })
    }

    #[get("/{namespace}")]
    async fn list(&self, #[param("namespace")] namespace: String) -> Result<Vec<ConfigEntryDto>> {
        let entries = into_boot(self.configs.list(&namespace).await)?;
        Ok(entries.iter().map(ConfigEntryDto::from).collect())
    }

    #[get("/{namespace}/{key}")]
    async fn get(
        &self,
        #[param("namespace")] namespace: String,
        #[param("key")] key: String,
    ) -> Result<ConfigEntryDto> {
        let entry = into_boot(self.configs.get(&namespace, &key).await)?;
        Ok(ConfigEntryDto::from(&entry))
    }

    #[put("/{namespace}/{key}")]
    async fn put(
        &self,
        #[param("namespace")] namespace: String,
        #[param("key")] key: String,
        #[body] dto: PutConfigDto,
    ) -> Result<ConfigEntryDto> {
        let entry = into_boot(
            self.configs
                .put(PutConfigCommand {
                    namespace,
                    key,
                    value: dto.value,
                    updated_by: dto.updated_by,
                })
                .await,
        )?;
        Ok(ConfigEntryDto::from(&entry))
    }

    #[delete("/{namespace}/{key}")]
    async fn delete(
        &self,
        #[param("namespace")] namespace: String,
        #[param("key")] key: String,
    ) -> Result<serde_json::Value> {
        into_boot(self.configs.delete(&namespace, &key).await)?;
        Ok(serde_json::json!({ "ok": true }))
    }
}
