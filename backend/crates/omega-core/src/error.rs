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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_io_error_display() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let omega_err = OmegaError::from(io_err);
        let display = format!("{omega_err}");
        assert!(
            display.contains("io error"),
            "expected 'io error' in display, got: {display}"
        );
        assert!(
            display.contains("file missing"),
            "expected 'file missing' in display, got: {display}"
        );
    }

    #[test]
    fn test_channel_error_display() {
        let err = OmegaError::Channel("test".into());
        let display = format!("{err}");
        assert_eq!(display, "channel error: test");
    }

    #[test]
    fn test_config_error_display() {
        let err = OmegaError::Config("test".into());
        let display = format!("{err}");
        assert_eq!(display, "config error: test");
    }
}
