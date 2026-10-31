use async_trait::async_trait;

use super::{ConfigEntry, ConfigKey, ConfigNamespace, ConfigValue, DomainError};

#[async_trait]
pub trait ConfigRepository: Send + Sync {
    async fn list_namespace(
        &self,
        namespace: &ConfigNamespace,
    ) -> Result<Vec<ConfigEntry>, DomainError>;

    async fn list_all(&self) -> Result<Vec<ConfigEntry>, DomainError>;

    async fn find(
        &self,
        namespace: &ConfigNamespace,
        key: &ConfigKey,
    ) -> Result<Option<ConfigEntry>, DomainError>;

    async fn upsert(&self, entry: &ConfigEntry) -> Result<(), DomainError>;

    /// Allocate the next revision and write the entry in one local transaction.
    /// An older stored revision is left in place. The returned entry is the committed row.
    async fn commit_put(
        &self,
        namespace: &ConfigNamespace,
        key: &ConfigKey,
        value: &ConfigValue,
        updated_at_epoch: i64,
        updated_by: &str,
    ) -> Result<ConfigEntry, DomainError>;

    async fn delete(
        &self,
        namespace: &ConfigNamespace,
        key: &ConfigKey,
    ) -> Result<bool, DomainError>;

    /// Delete the entry and allocate the revision in one local transaction.
    /// `None` means the key was already absent and no revision was consumed.
    async fn commit_delete(
        &self,
        namespace: &ConfigNamespace,
        key: &ConfigKey,
    ) -> Result<Option<i64>, DomainError>;

    /// Atomically allocate the next global revision (Postgres singleton).
    async fn allocate_revision(&self) -> Result<i64, DomainError>;

    async fn max_revision(&self) -> Result<i64, DomainError>;

    /// Entries and the meta revision from one database snapshot.
    async fn read_snapshot(&self) -> Result<(Vec<ConfigEntry>, i64), DomainError>;

    /// Keys a committed delete removed. Startup seeding must not recreate them.
    async fn removed_keys(&self) -> Result<Vec<(String, String)>, DomainError>;
}
