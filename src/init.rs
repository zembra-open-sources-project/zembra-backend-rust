//! Global user initialization for configuration and SQLite database.

use std::path::PathBuf;

use crate::config_init::{ConfigInitError, UserConfigInit, init_user_config_with_database_path};
use crate::repositories::database::Database;

/// Runtime inputs used to initialize the current user's local state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalInitConfig {
    /// Current user's home directory.
    pub home_dir: PathBuf,
}

/// Result of a global initialization run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalInit {
    /// Initialization created or completed local state.
    Initialized,
    /// Both default database and config file already existed.
    Skipped,
}

/// Error returned by global initialization.
#[derive(Debug, thiserror::Error)]
pub enum GlobalInitError {
    /// HOME is unavailable.
    #[error("HOME is not set")]
    MissingHome,
    /// User configuration initialization failed.
    #[error("{0}")]
    Config(#[from] ConfigInitError),
    /// SQLite database creation or migration failed.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl GlobalInitConfig {
    /// Builds global initialization inputs from the current process.
    ///
    /// # Returns
    ///
    /// Returns process-derived global initialization inputs.
    pub fn from_current_process() -> Result<Self, GlobalInitError> {
        let home_dir = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or(GlobalInitError::MissingHome)?;

        Ok(Self { home_dir })
    }

    /// Returns the default SQLite database file path.
    ///
    /// # Returns
    ///
    /// Returns `~/.zembra/zembra.sqlite3`.
    pub fn default_database_path(&self) -> PathBuf {
        self.home_dir.join(".zembra/zembra.sqlite3")
    }

    /// Returns the current user's configuration file path.
    ///
    /// # Returns
    ///
    /// Returns `~/.zembra.env`.
    pub fn config_path(&self) -> PathBuf {
        self.home_dir.join(".zembra.env")
    }
}

/// Initializes the default SQLite database and user configuration.
///
/// # Arguments
///
/// * `config` - Global initialization inputs.
///
/// # Returns
///
/// Returns whether initialization ran or was skipped.
pub async fn init_global(config: &GlobalInitConfig) -> Result<GlobalInit, GlobalInitError> {
    let database_path = config.default_database_path();
    let config_path = config.config_path();

    if database_path.exists() && config_path.exists() {
        return Ok(GlobalInit::Skipped);
    }

    let database_url = format!("sqlite://{}", database_path.display());
    Database::connect(&database_url).await?;

    let user_config = UserConfigInit {
        home_dir: config.home_dir.clone(),
    };
    init_user_config_with_database_path(&user_config, &database_path)?;

    Ok(GlobalInit::Initialized)
}
