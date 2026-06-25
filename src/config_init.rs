//! User configuration file initialization.

use std::fs;
use std::path::{Path, PathBuf};

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

    fs::write(
        &path,
        render_documented_user_config_for_home(&config.home_dir),
    )
    .map_err(|source| ConfigInitError::Io {
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
    let home_dir = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));

    render_documented_user_config_for_home(&home_dir)
}

/// Renders the documented default user configuration for a home directory.
///
/// # Arguments
///
/// * `home_dir` - Current user's home directory.
///
/// # Returns
///
/// Returns TOML configuration content with absolute local paths.
fn render_documented_user_config_for_home(home_dir: &Path) -> String {
    let database_path = home_dir.join(".local/share/zembra/zembra.db");

    vec![
        "[server]".to_string(),
        "# HTTP server bind address.".to_string(),
        "host = \"127.0.0.1\"".to_string(),
        "# HTTP server bind port.".to_string(),
        "port = 3000".to_string(),
        "# Browser origins allowed to access the HTTP API through CORS.".to_string(),
        "cors_allowed_origins = []".to_string(),
        "".to_string(),
        "[database]".to_string(),
        "# SQLite database file path.".to_string(),
        format!("path = \"{}\"", database_path.display()),
        "".to_string(),
        "[logging]".to_string(),
        "# Minimum log level written to console and log files.".to_string(),
        "level = \"INFO\"".to_string(),
        "# Directory where daily log files are written.".to_string(),
        "path = \"logs\"".to_string(),
        "".to_string(),
        "[sync]".to_string(),
        "# Whether background Supabase synchronization is enabled.".to_string(),
        "enabled = false".to_string(),
        "# Delay in seconds between background synchronization attempts.".to_string(),
        "interval_seconds = 60".to_string(),
        "# Supabase project URL used by the backend REST client.".to_string(),
        "supabase_url = \"\"".to_string(),
        "# Supabase secret key used only by the local backend.".to_string(),
        "secret_key = \"\"".to_string(),
        "# Supabase database password used only for remote schema migrations.".to_string(),
        "remote_database_password = \"\"".to_string(),
        "".to_string(),
    ]
    .join("\n")
}
