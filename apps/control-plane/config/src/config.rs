use std::env;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;

use aster_boot::{BootError, Result};

/// Process bootstrap for aster-config (not hot-reloadable).
#[derive(Debug, Clone)]
pub struct ConfigServerConfig {
    pub host: IpAddr,
    pub port: u16,
    /// `ASTER_CONFIG_LISTEN` override of `host:port`.
    pub listen_override: Option<String>,
    pub database_url: String,
    pub database_pool_size: usize,
    pub iggy_address: String,
    pub iggy_username: String,
    pub iggy_password: String,
}

impl ConfigServerConfig {
    pub fn from_env() -> Result<Self> {
        let host = env::var("ASTER_CONFIG_HOST")
            .or_else(|_| env::var("HOST"))
            .unwrap_or_else(|_| "127.0.0.1".into());
        let port = env::var("ASTER_CONFIG_PORT")
            .or_else(|_| env::var("PORT"))
            .unwrap_or_else(|_| "7652".into())
            .parse::<u16>()
            .map_err(|err| BootError::Internal(format!("invalid PORT: {err}")))?;
        let database_url = env::var("ASTER_CONFIG_DATABASE_URL")
            .or_else(|_| env::var("DATABASE_URL"))
            .map_err(|_| {
                BootError::Internal("ASTER_CONFIG_DATABASE_URL (or DATABASE_URL) is required".into())
            })?;
        let database_pool_size = env::var("ASTER_CONFIG_DATABASE_POOL_SIZE")
            .or_else(|_| env::var("DATABASE_POOL_SIZE"))
            .unwrap_or_else(|_| "8".into())
            .parse::<usize>()
            .map_err(|err| BootError::Internal(format!("invalid DATABASE_POOL_SIZE: {err}")))?;
        let iggy_address = env::var("ASTER_CONFIG_IGGY_ADDRESS")
            .or_else(|_| env::var("IGGY_ADDRESS"))
            .unwrap_or_else(|_| "127.0.0.1:5102".into());
        let iggy_username = env::var("ASTER_CONFIG_IGGY_USERNAME")
            .or_else(|_| env::var("IGGY_ROOT_USERNAME"))
            .unwrap_or_else(|_| "iggy".into());
        let iggy_password = env::var("ASTER_CONFIG_IGGY_PASSWORD")
            .or_else(|_| env::var("IGGY_ROOT_PASSWORD"))
            .unwrap_or_else(|_| "iggy".into());

        Ok(Self {
            host: IpAddr::from_str(&host)
                .map_err(|err| BootError::Internal(format!("invalid HOST: {err}")))?,
            port,
            listen_override: env::var("ASTER_CONFIG_LISTEN")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            database_url,
            database_pool_size: database_pool_size.max(1),
            iggy_address,
            iggy_username,
            iggy_password,
        })
    }

    pub fn listen_addr(&self) -> SocketAddr {
        SocketAddr::new(self.host, self.port)
    }

    /// The address the process binds: the override, else `host:port`.
    pub fn listen(&self) -> String {
        self.listen_override
            .clone()
            .unwrap_or_else(|| format!("{}:{}", self.host, self.port))
    }
}
