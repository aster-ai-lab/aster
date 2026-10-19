use a3s_boot::BootError;

#[derive(Debug)]
pub enum PlaneError {
    NotFound(String),
    BadRequest(String),
    Conflict(String),
    Internal(String),
}

impl PlaneError {
    pub fn bad(message: impl Into<String>) -> Self {
        Self::BadRequest(message.into())
    }
}

impl From<std::io::Error> for PlaneError {
    fn from(error: std::io::Error) -> Self {
        Self::Internal(error.to_string())
    }
}

impl From<PlaneError> for BootError {
    fn from(error: PlaneError) -> Self {
        match error {
            PlaneError::NotFound(message) => BootError::NotFound(message),
            PlaneError::BadRequest(message) => BootError::BadRequest(message),
            PlaneError::Conflict(message) => BootError::HttpException {
                status: 409,
                message,
            },
            PlaneError::Internal(message) => BootError::HttpException {
                status: 500,
                message,
            },
        }
    }
}
