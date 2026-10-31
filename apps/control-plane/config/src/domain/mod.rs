mod entry;
mod error;
mod repository;

pub use entry::{ConfigEntry, ConfigKey, ConfigNamespace, ConfigValue};
pub use error::DomainError;
pub use repository::ConfigRepository;
