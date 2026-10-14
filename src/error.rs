#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Tool(String),
    #[error("{0}")]
    Internal(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::Invalid(message.into())
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }

    /// Prefixes the message with what was being attempted.
    pub fn context(self, prefix: &str) -> Self {
        match self {
            Self::Invalid(message) => Self::Invalid(format!("{prefix}: {message}")),
            other => Self::Internal(format!("{prefix}: {other}")),
        }
    }
}
