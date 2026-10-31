#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    Validation(String),
    NotFound(String),
    Conflict(String),
    Infrastructure(String),
}

impl DomainError {
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
