use aster_boot::{BootError, Result};
use aster_orm::{Migrator, PostgresExecutor};

use crate::migrations;

#[derive(Clone)]
pub struct Postgres {
    executor: PostgresExecutor,
}

impl std::fmt::Debug for Postgres {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Postgres").finish_non_exhaustive()
    }
}

impl Postgres {
    pub fn connect(database_url: &str, pool_size: usize) -> Result<Self> {
        let executor = PostgresExecutor::connect_no_tls(database_url, pool_size)
            .map_err(|err| BootError::Internal(format!("postgres connect failed: {err}")))?;
        Ok(Self { executor })
    }

    pub fn executor(&self) -> &PostgresExecutor {
        &self.executor
    }

    pub async fn ensure_schema(&self) -> Result<()> {
        Migrator::new(self.executor.clone())
            .run(migrations::migrations())
            .await
            .map_err(|err| BootError::Internal(format!("aster-orm migrate failed: {err}")))?;
        Ok(())
    }

    pub async fn ping(&self) -> Result<()> {
        self.executor
            .health_check()
            .await
            .map_err(|err| BootError::Internal(format!("postgres health failed: {err}")))?;
        Ok(())
    }
}
