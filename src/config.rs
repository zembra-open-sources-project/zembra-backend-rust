use axum::http::HeaderValue;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
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
    /// Background synchronization settings.
    #[serde(default)]
    pub sync: SyncSettings,
}

/// HTTP server binding settings.
#[derive(Debug, Clone, Deserialize)]
pub struct ServerSettings {
    /// IPv4 address string used to bind the HTTP server.
    pub host: String,
    /// TCP port used to bind the HTTP server.
    pub port: u16,
    /// Browser origins allowed to access the HTTP API through CORS.
    #[serde(default)]
    pub cors_allowed_origins: Vec<String>,
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

/// Runtime background synchronization settings.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SyncSettings {
    /// Whether the background Supabase synchronization worker is enabled.
    #[serde(default)]
    pub enabled: bool,
    /// Delay in seconds between background synchronization attempts.
    #[serde(default = "default_sync_interval_seconds")]
    pub interval_seconds: u64,
    /// Supabase project URL used by the backend REST client.
    #[serde(default)]
    pub supabase_url: String,
    /// Supabase service role key used only by the local backend.
    #[serde(default)]
    pub service_role_key: String,
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

impl Default for SyncSettings {
    /// Creates default synchronization settings.
    ///
    /// # Returns
    ///
    /// Returns disabled synchronization with a 60 second polling interval.
    fn default() -> Self {
        Self {
            enabled: false,
            interval_seconds: default_sync_interval_seconds(),
            supabase_url: String::new(),
            service_role_key: String::new(),
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

        let settings = builder.build()?.try_deserialize::<Self>()?;
        settings.validate_sync()?;

        Ok(settings)
    }

    /// Validates synchronization settings that depend on multiple fields.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` when sync settings are internally consistent.
    fn validate_sync(&self) -> Result<(), config::ConfigError> {
        self.sync.validate()
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

impl ServerSettings {
    /// Parses the configured host string into an IPv4 bind address.
    ///
    /// # Returns
    ///
    /// Returns a parsed `Ipv4Addr`, or a configuration error when `server.host`
    /// is not a valid IPv4 address string.
    pub fn host_addr(&self) -> Result<Ipv4Addr, config::ConfigError> {
        Ipv4Addr::from_str(self.host.trim()).map_err(|_| {
            config::ConfigError::Message(format!(
                "server.host must be a valid IPv4 address string, got {:?}",
                self.host
            ))
        })
    }

    /// Parses configured CORS origins into HTTP header values.
    ///
    /// # Returns
    ///
    /// Returns header values that can be passed to the CORS layer, or a
    /// configuration error when any configured origin is not a valid header
    /// value.
    pub fn cors_origin_values(&self) -> Result<Vec<HeaderValue>, config::ConfigError> {
        self.cors_allowed_origins
            .iter()
            .map(|origin| {
                HeaderValue::from_str(origin.trim()).map_err(|_| {
                    config::ConfigError::Message(format!(
                        "server.cors_allowed_origins contains an invalid origin: {:?}",
                        origin
                    ))
                })
            })
            .collect()
    }
}

impl SyncSettings {
    /// Validates whether this synchronization configuration can run safely.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` when disabled sync or a complete enabled sync config is
    /// provided.
    pub fn validate(&self) -> Result<(), config::ConfigError> {
        if self.interval_seconds < 5 {
            return Err(config::ConfigError::Message(
                "sync.interval_seconds must be at least 5".to_string(),
            ));
        }

        if self.enabled {
            if self.supabase_url.trim().is_empty() {
                return Err(config::ConfigError::Message(
                    "sync.supabase_url is required when sync.enabled is true".to_string(),
                ));
            }

            if self.service_role_key.trim().is_empty() {
                return Err(config::ConfigError::Message(
                    "sync.service_role_key is required when sync.enabled is true".to_string(),
                ));
            }
        }

        Ok(())
    }
}

/// Builds the expected user configuration path from the current home directory.
///
/// # Returns
///
/// Returns `Some(path)` when the `HOME` environment variable is available, or
/// `None` after logging a warning when the home directory cannot be resolved.
pub fn user_config_path() -> Option<PathBuf> {
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

/// Provides the default synchronization interval.
///
/// # Returns
///
/// Returns 60 seconds.
fn default_sync_interval_seconds() -> u64 {
    60
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::{DatabaseSettings, ServerSettings, Settings, SyncSettings, sqlite_url_from_path};
    use std::net::Ipv4Addr;

    #[test]
    fn user_config_env_name_is_parsed_as_toml() {
        let settings: Settings = config::Config::builder()
            .add_source(
                config::File::from_str(
                    r#"
                    [server]
                    host = "127.0.0.1"
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
        assert_eq!(settings.server.host, "127.0.0.1");
        assert_eq!(settings.database.path, "data/custom-zembra.db");
        assert_eq!(settings.logging.level, "INFO");
        assert_eq!(settings.logging.path, "logs");
        assert!(!settings.sync.enabled);
        assert_eq!(settings.sync.interval_seconds, 60);
        assert_eq!(settings.sync.supabase_url, "");
        assert_eq!(settings.sync.service_role_key, "");
    }

    #[test]
    fn logging_settings_are_loaded_from_toml() {
        let settings: Settings = config::Config::builder()
            .add_source(
                config::File::from_str(
                    r#"
                    [server]
                    host = "127.0.0.1"
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
    fn sync_settings_are_loaded_from_toml() {
        let settings: Settings = config::Config::builder()
            .add_source(
                config::File::from_str(
                    r#"
                    [server]
                    host = "127.0.0.1"
                    port = 3010

                    [database]
                    path = "data/custom-zembra.db"

                    [sync]
                    enabled = true
                    interval_seconds = 30
                    supabase_url = "https://example.supabase.co"
                    service_role_key = "test-service-role-key"
                    "#,
                    config::FileFormat::Toml,
                )
                .format(config::FileFormat::Toml),
            )
            .build()
            .expect("test TOML config should build")
            .try_deserialize::<Settings>()
            .expect("test TOML config should deserialize");

        settings
            .sync
            .validate()
            .expect("complete enabled sync config should validate");
        assert!(settings.sync.enabled);
        assert_eq!(settings.sync.interval_seconds, 30);
        assert_eq!(settings.sync.supabase_url, "https://example.supabase.co");
        assert_eq!(settings.sync.service_role_key, "test-service-role-key");
    }

    #[test]
    fn enabled_sync_requires_supabase_url() {
        let settings = SyncSettings {
            enabled: true,
            interval_seconds: 60,
            supabase_url: "   ".to_string(),
            service_role_key: "test-service-role-key".to_string(),
        };

        assert!(settings.validate().is_err());
    }

    #[test]
    fn enabled_sync_requires_service_role_key() {
        let settings = SyncSettings {
            enabled: true,
            interval_seconds: 60,
            supabase_url: "https://example.supabase.co".to_string(),
            service_role_key: "   ".to_string(),
        };

        assert!(settings.validate().is_err());
    }

    #[test]
    fn sync_interval_has_minimum_value() {
        let settings = SyncSettings {
            enabled: false,
            interval_seconds: 4,
            supabase_url: String::new(),
            service_role_key: String::new(),
        };

        assert!(settings.validate().is_err());
    }

    #[test]
    fn server_host_parses_loopback_ipv4_string() {
        let settings = ServerSettings {
            host: "127.0.0.1".to_string(),
            port: 3000,
            cors_allowed_origins: Vec::new(),
        };

        assert_eq!(settings.host_addr().unwrap(), Ipv4Addr::new(127, 0, 0, 1));
    }

    #[test]
    fn server_host_parses_unspecified_ipv4_string() {
        let settings = ServerSettings {
            host: "0.0.0.0".to_string(),
            port: 3000,
            cors_allowed_origins: Vec::new(),
        };

        assert_eq!(settings.host_addr().unwrap(), Ipv4Addr::new(0, 0, 0, 0));
    }

    #[test]
    fn server_host_rejects_non_ipv4_strings() {
        let settings = ServerSettings {
            host: "localhost".to_string(),
            port: 3000,
            cors_allowed_origins: Vec::new(),
        };

        assert!(settings.host_addr().is_err());
    }

    #[test]
    fn server_cors_origins_default_to_empty_list() {
        let settings: Settings = config::Config::builder()
            .add_source(
                config::File::from_str(
                    r#"
                    [server]
                    host = "127.0.0.1"
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

        assert!(settings.server.cors_allowed_origins.is_empty());
    }

    #[test]
    fn server_cors_origins_parse_header_values() {
        let settings = ServerSettings {
            host: "127.0.0.1".to_string(),
            port: 3000,
            cors_allowed_origins: vec!["http://192.168.1.20:5173".to_string()],
        };

        assert_eq!(
            settings.cors_origin_values().unwrap(),
            vec![HeaderValue::from_static("http://192.168.1.20:5173")]
        );
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
