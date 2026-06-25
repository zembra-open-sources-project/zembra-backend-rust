//! User configuration file initialization.

use std::fs;
use std::path::PathBuf;

/// Options accepted by `zembra-backend config init`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ConfigInitOptions {
    /// Whether an existing configuration file may be overwritten.
    pub force: bool,
}

/// Runtime inputs used to initialize the user configuration file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserConfigInit {
    /// Current user's home directory.
    pub home_dir: PathBuf,
}

/// Error returned by user configuration initialization.
#[derive(Debug, thiserror::Error)]
pub enum ConfigInitError {
    /// HOME is unavailable.
    #[error("HOME is not set")]
    MissingHome,
    /// File system operation failed.
    #[error("file operation failed at {path}: {source}")]
    Io {
        /// Path involved in the failed file operation.
        path: PathBuf,
        /// Original I/O error.
        source: std::io::Error,
    },
}

impl UserConfigInit {
    /// Builds config initialization inputs from the current process.
    ///
    /// # Returns
    ///
    /// Returns process-derived config initialization inputs.
    pub fn from_current_process() -> Result<Self, ConfigInitError> {
        let home_dir = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or(ConfigInitError::MissingHome)?;

        Ok(Self { home_dir })
    }
}

/// Initializes the current user's `~/.zembra.env` file.
///
/// # Arguments
///
/// * `config` - User config initialization inputs.
/// * `options` - Config initialization options.
///
/// # Returns
///
/// Returns the target config path.
pub fn init_user_config(
    config: &UserConfigInit,
    options: ConfigInitOptions,
) -> Result<PathBuf, ConfigInitError> {
    let path = config.home_dir.join(".zembra.env");

    if path.exists() && !options.force {
        return Ok(path);
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| ConfigInitError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    fs::write(&path, render_documented_user_config()).map_err(|source| ConfigInitError::Io {
        path: path.clone(),
        source,
    })?;

    Ok(path)
}

/// Renders the documented default user configuration.
///
/// # Returns
///
/// Returns TOML configuration content with one comment line before each field.
pub fn render_documented_user_config() -> String {
    [
        "[server]",
        "# HTTP server bind address.",
        "host = \"127.0.0.1\"",
        "# HTTP server bind port.",
        "port = 3000",
        "# Browser origins allowed to access the HTTP API through CORS.",
        "cors_allowed_origins = []",
        "",
        "[database]",
        "# SQLite database file path.",
        "path = \"data/zembra.db\"",
        "",
        "[logging]",
        "# Minimum log level written to console and log files.",
        "level = \"INFO\"",
        "# Directory where daily log files are written.",
        "path = \"logs\"",
        "",
        "[sync]",
        "# Whether background Supabase synchronization is enabled.",
        "enabled = false",
        "# Delay in seconds between background synchronization attempts.",
        "interval_seconds = 60",
        "# Supabase project URL used by the backend REST client.",
        "supabase_url = \"\"",
        "# Supabase secret key used only by the local backend.",
        "secret_key = \"\"",
        "# Supabase database password used only for remote schema migrations.",
        "remote_database_password = \"\"",
        "",
    ]
    .join("\n")
}
