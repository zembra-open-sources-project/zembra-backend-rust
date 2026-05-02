/// Application-level error type used by executable entrypoints.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// Configuration loading or parsing failed.
    #[error("configuration error: {0}")]
    Config(#[from] config::ConfigError),
    /// Database connection or migration failed.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    /// TCP socket binding failed.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
