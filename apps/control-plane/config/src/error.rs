use aster_boot::{BootError, Result};

/// Application-facing error before HTTP mapping.
#[derive(Debug, Clone)]
pub enum AppError {
    Validation(String),
    NotFound(String),
    Conflict(String),
    Infrastructure(String),
}

impl AppError {
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound(message.into())
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::Conflict(message.into())
    }

    pub fn infrastructure(message: impl Into<String>) -> Self {
        Self::Infrastructure(message.into())
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation(message)
            | Self::NotFound(message)
            | Self::Conflict(message)
            | Self::Infrastructure(message) => write!(f, "{message}"),
        }
    }
}

pub fn map_app_error(error: AppError) -> BootError {
    match error {
        AppError::Validation(message) => BootError::BadRequest(message),
        AppError::NotFound(message) => BootError::NotFound(message),
        AppError::Conflict(message) => BootError::Conflict(message),
        AppError::Infrastructure(message) => BootError::Internal(message),
    }
}

pub type AppResult<T> = std::result::Result<T, AppError>;

pub fn into_boot<T>(result: AppResult<T>) -> Result<T> {
    result.map_err(map_app_error)
}
