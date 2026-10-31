use serde::{Deserialize, Serialize};
use serde_json::Value;

use aster_boot::{BootError, Result, Validate, ValidationSchema};

use crate::domain::ConfigEntry;

#[derive(Debug, Clone, Deserialize, Serialize, ValidationSchema)]
pub struct PutConfigDto {
    pub value: Value,
    #[serde(default)]
    pub updated_by: String,
}

impl Validate for PutConfigDto {
    fn validate(&self) -> Result<()> {
        if self.updated_by.len() > 128 {
            return Err(BootError::BadRequest("updated_by is too long".into()));
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct ConfigEntryDto {
    pub namespace: String,
    pub key: String,
    pub value: Value,
    pub revision: i64,
    pub updated_at_epoch: i64,
    pub updated_by: String,
}

impl From<&ConfigEntry> for ConfigEntryDto {
    fn from(entry: &ConfigEntry) -> Self {
        Self {
            namespace: entry.namespace().as_str().to_string(),
            key: entry.key().as_str().to_string(),
            value: serde_json::from_str(entry.value().as_str()).unwrap_or(Value::Null),
            revision: entry.revision(),
            updated_at_epoch: entry.updated_at_epoch(),
            updated_by: entry.updated_by().to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ConfigStatusDto {
    pub revision: i64,
    pub publisher_id: String,
    pub subject: String,
}
