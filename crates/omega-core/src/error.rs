use thiserror::Error;

/// Top-level error type for Omega.
#[derive(Debug, Error)]
pub enum OmegaError {
    /// Error from an AI provider.
    #[error("provider error: {0}")]
    Provider(String),

    /// Error from a messaging channel.
    #[error("channel error: {0}")]
    Channel(String),

    /// Configuration error.
    #[error("config error: {0}")]
    Config(String),

    /// Memory/storage error.
    #[error("memory error: {0}")]
    Memory(String),

    /// Sandbox execution error.
    #[error("sandbox error: {0}")]
    Sandbox(String),

    /// I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
