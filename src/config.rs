use serde::Deserialize;
use std::path::{Path, PathBuf};
use tracing::warn;

/// Runtime settings loaded from files and environment variables.
#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    /// HTTP server settings.
    pub server: ServerSettings,
    /// SQLite database settings.
    pub database: DatabaseSettings,
    /// Logging settings.
    #[serde(default)]
    pub logging: LoggingSettings,
}

/// HTTP server binding settings.
#[derive(Debug, Clone, Deserialize)]
pub struct ServerSettings {
    /// IPv4 host octets used to bind the HTTP server.
    pub host: [u8; 4],
    /// TCP port used to bind the HTTP server.
    pub port: u16,
}

/// SQLite database connection settings.
#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseSettings {
    /// SQLite database file path.
    pub path: String,
}

/// Runtime logging settings.
#[derive(Debug, Clone, Deserialize)]
pub struct LoggingSettings {
    /// Minimum log level displayed by console and file subscribers.
    #[serde(default = "default_log_level")]
    pub level: String,
    /// Directory where daily log files are written.
    #[serde(default = "default_log_path")]
    pub path: String,
}

impl Default for LoggingSettings {
    /// Creates default logging settings.
    ///
    /// # Returns
    ///
    /// Returns `INFO` as the displayed log level and `logs` as the log directory.
    fn default() -> Self {
        Self {
            level: default_log_level(),
            path: default_log_path(),
        }
    }
}

impl Settings {
    /// Loads service settings from `config/default.toml` and `~/.zembra.env`.
    ///
    /// # Returns
    ///
    /// Returns parsed settings on success, or a configuration error when required
    /// fields are missing or invalid.
    pub fn load() -> Result<Self, config::ConfigError> {
        let mut builder =
            config::Config::builder().add_source(config::File::with_name("config/default"));

        if let Some(user_config_path) = user_config_path() {
            if user_config_path.exists() {
                builder = builder.add_source(
                    config::File::from(user_config_path).format(config::FileFormat::Toml),
                );
            } else {
                warn!(
                    path = %user_config_path.display(),
                    "user configuration file not found; continuing with remaining sources"
                );
            }
        }

        builder.build()?.try_deserialize()
    }
}

impl DatabaseSettings {
    /// Converts the configured SQLite file path into a SQLx-compatible URL.
    ///
    /// # Returns
    ///
    /// Returns a SQLite connection URL derived from `database.path`.
    pub fn sqlite_url(&self) -> String {
        sqlite_url_from_path(&self.path)
    }
}

/// Builds the expected user configuration path from the current home directory.
///
/// # Returns
///
/// Returns `Some(path)` when the `HOME` environment variable is available, or
/// `None` after logging a warning when the home directory cannot be resolved.
fn user_config_path() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(|home| Path::new(&home).join(".zembra.env"))
        .or_else(|| {
            warn!("HOME is not set; skipping user configuration file lookup");
            None
        })
}

/// Converts a SQLite filesystem path into a SQLx connection URL.
///
/// # Returns
///
/// Returns `sqlite://{path}` for both relative and absolute database paths.
fn sqlite_url_from_path(path: &str) -> String {
    format!("sqlite://{path}")
}

/// Provides the default displayed log level.
///
/// # Returns
///
/// Returns `INFO`.
fn default_log_level() -> String {
    "INFO".to_string()
}

/// Provides the default log directory.
///
/// # Returns
///
/// Returns `logs`.
fn default_log_path() -> String {
    "logs".to_string()
}

#[cfg(test)]
mod tests {
    use super::{DatabaseSettings, Settings, sqlite_url_from_path};

    #[test]
    fn user_config_env_name_is_parsed_as_toml() {
        let settings: Settings = config::Config::builder()
            .add_source(
                config::File::from_str(
                    r#"
                    [server]
                    host = [127, 0, 0, 1]
                    port = 3010

                    [database]
                    path = "data/custom-zembra.db"
                    "#,
                    config::FileFormat::Toml,
                )
                .format(config::FileFormat::Toml),
            )
            .build()
            .expect("test TOML config should build")
            .try_deserialize()
            .expect("test TOML config should deserialize");

        assert_eq!(settings.server.port, 3010);
        assert_eq!(settings.database.path, "data/custom-zembra.db");
        assert_eq!(settings.logging.level, "INFO");
        assert_eq!(settings.logging.path, "logs");
    }

    #[test]
    fn logging_settings_are_loaded_from_toml() {
        let settings: Settings = config::Config::builder()
            .add_source(
                config::File::from_str(
                    r#"
                    [server]
                    host = [127, 0, 0, 1]
                    port = 3010

                    [database]
                    path = "data/custom-zembra.db"

                    [logging]
                    level = "DEBUG"
                    path = "tmp/logs"
                    "#,
                    config::FileFormat::Toml,
                )
                .format(config::FileFormat::Toml),
            )
            .build()
            .expect("test TOML config should build")
            .try_deserialize()
            .expect("test TOML config should deserialize");

        assert_eq!(settings.logging.level, "DEBUG");
        assert_eq!(settings.logging.path, "tmp/logs");
    }

    #[test]
    fn sqlite_url_preserves_relative_database_paths() {
        let settings = DatabaseSettings {
            path: "data/zembra.db".to_string(),
        };

        assert_eq!(settings.sqlite_url(), "sqlite://data/zembra.db");
    }

    #[test]
    fn sqlite_url_preserves_absolute_database_paths() {
        assert_eq!(
            sqlite_url_from_path("/path/to/zembra.sqlite3"),
            "sqlite:///path/to/zembra.sqlite3"
        );
    }
}
