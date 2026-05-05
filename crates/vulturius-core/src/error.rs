use thiserror::Error;

/// Core error types for Vulturius.
#[derive(Debug, Error)]
pub enum VulturiusError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Scan error: {0}")]
    Scan(String),

    #[error("Analysis error: {0}")]
    Analysis(String),

    #[error("Cleanup error: {0}")]
    Cleanup(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("LLM error: {0}")]
    Llm(String),

    #[error("Permission denied: {path}")]
    PermissionDenied { path: String },

    #[error("Path blocked (system protection): {path}")]
    PathBlocked { path: String },

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("Platform not supported: {0}")]
    PlatformNotSupported(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Config parse error: {0}")]
    ConfigParse(#[from] config::ConfigError),

    #[error("Database error: {0}")]
    DbError(#[from] sqlx::Error),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Cancelled by user")]
    Cancelled,
}

impl VulturiusError {
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            VulturiusError::Io(_)
                | VulturiusError::Scan(_)
                | VulturiusError::Timeout(_)
                | VulturiusError::Llm(_)
        )
    }

    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            VulturiusError::Database(_)
                | VulturiusError::Config(_)
                | VulturiusError::PlatformNotSupported(_)
        )
    }
}

/// Result type alias for Vulturius operations.
pub type Result<T> = std::result::Result<T, VulturiusError>;
