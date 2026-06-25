use axum::http::{HeaderValue, Uri};
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

/// Runtime CORS origin matching rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorsOriginRule {
    /// Exact browser origin match.
    Exact(HeaderValue),
    /// IPv4 origin match that allows wildcard octets.
    Ipv4Wildcard(Ipv4CorsOriginRule),
}

/// Runtime CORS IPv4 wildcard matching rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ipv4CorsOriginRule {
    /// Original origin value echoed when a request matches this rule.
    pub raw: HeaderValue,
    /// Required URI scheme.
    pub scheme: String,
    /// IPv4 octets where `None` represents a wildcard.
    pub octets: [Option<u8>; 4],
    /// Required origin port.
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
    /// Supabase secret key used only by the local backend.
    #[serde(default)]
    pub secret_key: String,
    /// Supabase database password used only for remote schema migrations.
    #[serde(default)]
    pub remote_database_password: String,
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
            secret_key: String::new(),
            remote_database_password: String::new(),
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
        settings.validate()?;

        Ok(settings)
    }

    /// Validates settings that depend on runtime safety rules.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` when all settings are internally consistent.
    fn validate(&self) -> Result<(), config::ConfigError> {
        self.database.validate()?;
        self.sync.validate()
    }
}

impl DatabaseSettings {
    /// Validates whether this database configuration can run safely.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` when the SQLite database path is absolute.
    pub fn validate(&self) -> Result<(), config::ConfigError> {
        if Path::new(self.path.trim()).is_absolute() {
            Ok(())
        } else {
            Err(config::ConfigError::Message(
                "database.path must be an absolute SQLite database file path".to_string(),
            ))
        }
    }

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

    /// Parses configured CORS origins into runtime matching rules.
    ///
    /// # Returns
    ///
    /// Returns exact and IPv4 wildcard CORS rules, or a configuration error when
    /// a configured wildcard is unsafe or malformed.
    pub fn cors_origin_rules(&self) -> Result<Vec<CorsOriginRule>, config::ConfigError> {
        self.cors_allowed_origins
            .iter()
            .map(|origin| parse_cors_origin_rule(origin))
            .collect()
    }
}

/// Parses a configured CORS origin string into a runtime matching rule.
///
/// # Arguments
///
/// * `origin` - Configured browser origin.
///
/// # Returns
///
/// Returns an exact or IPv4 wildcard CORS origin rule.
fn parse_cors_origin_rule(origin: &str) -> Result<CorsOriginRule, config::ConfigError> {
    let trimmed = origin.trim();
    let raw = HeaderValue::from_str(trimmed).map_err(|_| {
        config::ConfigError::Message(format!(
            "server.cors_allowed_origins contains an invalid origin: {origin:?}"
        ))
    })?;

    if !trimmed.contains('*') {
        return Ok(CorsOriginRule::Exact(raw));
    }

    let uri = trimmed.parse::<Uri>().map_err(|_| {
        config::ConfigError::Message(format!(
            "server.cors_allowed_origins contains an invalid wildcard origin: {origin:?}"
        ))
    })?;
    let scheme = uri.scheme_str().ok_or_else(|| {
        config::ConfigError::Message(format!(
            "wildcard CORS origin must include http or https scheme: {origin:?}"
        ))
    })?;
    if scheme != "http" && scheme != "https" {
        return Err(config::ConfigError::Message(format!(
            "wildcard CORS origin must use http or https scheme: {origin:?}"
        )));
    }

    let host = uri.host().ok_or_else(|| {
        config::ConfigError::Message(format!(
            "wildcard CORS origin must include an IPv4 host: {origin:?}"
        ))
    })?;
    let port = uri.port_u16().ok_or_else(|| {
        config::ConfigError::Message(format!(
            "wildcard CORS origin must include an exact numeric port: {origin:?}"
        ))
    })?;
    let octets = parse_ipv4_wildcard_host(host, origin)?;

    Ok(CorsOriginRule::Ipv4Wildcard(Ipv4CorsOriginRule {
        raw,
        scheme: scheme.to_string(),
        octets,
        port,
    }))
}

/// Parses an IPv4 wildcard host.
///
/// # Arguments
///
/// * `host` - Host portion of a configured CORS origin.
/// * `origin` - Original configured origin used in error messages.
///
/// # Returns
///
/// Returns four IPv4 octets where `None` represents `*`.
fn parse_ipv4_wildcard_host(
    host: &str,
    origin: &str,
) -> Result<[Option<u8>; 4], config::ConfigError> {
    let parts = host.split('.').collect::<Vec<_>>();
    if parts.len() != 4 {
        return Err(config::ConfigError::Message(format!(
            "wildcard CORS origin only supports IPv4 octet wildcards: {origin:?}"
        )));
    }

    let mut has_wildcard = false;
    let mut octets = [None; 4];
    for (index, part) in parts.iter().enumerate() {
        if *part == "*" {
            has_wildcard = true;
            octets[index] = None;
            continue;
        }

        if part.contains('*') {
            return Err(config::ConfigError::Message(format!(
                "wildcard CORS origin only supports full IPv4 octet wildcards: {origin:?}"
            )));
        }

        octets[index] = Some(part.parse::<u8>().map_err(|_| {
            config::ConfigError::Message(format!(
                "wildcard CORS origin contains an invalid IPv4 octet: {origin:?}"
            ))
        })?);
    }

    if !has_wildcard {
        return Err(config::ConfigError::Message(format!(
            "wildcard CORS origin must include at least one IPv4 octet wildcard: {origin:?}"
        )));
    }

    Ok(octets)
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

            if self.secret_key.trim().is_empty() {
                return Err(config::ConfigError::Message(
                    "sync.secret_key is required when sync.enabled is true".to_string(),
                ));
            }

            if !self.secret_key.trim().starts_with("sb_secret_") {
                return Err(config::ConfigError::Message(
                    "sync.secret_key must use a Supabase sb_secret_ key".to_string(),
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
/// Returns `sqlite://{path}` for an absolute database path.
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
    use super::{
        CorsOriginRule, DatabaseSettings, ServerSettings, Settings, SyncSettings,
        sqlite_url_from_path,
    };
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
                    path = "/tmp/custom-zembra.db"
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
        assert_eq!(settings.database.path, "/tmp/custom-zembra.db");
        assert_eq!(settings.logging.level, "INFO");
        assert_eq!(settings.logging.path, "logs");
        assert!(!settings.sync.enabled);
        assert_eq!(settings.sync.interval_seconds, 60);
        assert_eq!(settings.sync.supabase_url, "");
        assert_eq!(settings.sync.secret_key, "");
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
                    path = "/tmp/custom-zembra.db"

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
                    path = "/tmp/custom-zembra.db"

                    [sync]
                    enabled = true
                    interval_seconds = 30
                    supabase_url = "https://example.supabase.co"
                    secret_key = "sb_secret_test-key"
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
        assert_eq!(settings.sync.secret_key, "sb_secret_test-key");
    }

    #[test]
    fn enabled_sync_requires_supabase_url() {
        let settings = SyncSettings {
            enabled: true,
            interval_seconds: 60,
            supabase_url: "   ".to_string(),
            secret_key: "sb_secret_test-key".to_string(),
            remote_database_password: String::new(),
        };

        assert!(settings.validate().is_err());
    }

    #[test]
    fn enabled_sync_requires_secret_key() {
        let settings = SyncSettings {
            enabled: true,
            interval_seconds: 60,
            supabase_url: "https://example.supabase.co".to_string(),
            secret_key: "   ".to_string(),
            remote_database_password: String::new(),
        };

        assert!(settings.validate().is_err());
    }

    #[test]
    fn enabled_sync_rejects_legacy_service_role_key() {
        let settings = SyncSettings {
            enabled: true,
            interval_seconds: 60,
            supabase_url: "https://example.supabase.co".to_string(),
            secret_key: "eyJlegacy.jwt.service-role".to_string(),
            remote_database_password: String::new(),
        };

        assert!(settings.validate().is_err());
    }

    #[test]
    fn sync_interval_has_minimum_value() {
        let settings = SyncSettings {
            enabled: false,
            interval_seconds: 4,
            supabase_url: String::new(),
            secret_key: String::new(),
            remote_database_password: String::new(),
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
                    path = "/tmp/custom-zembra.db"
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
    fn server_cors_origins_parse_ipv4_wildcard_rules() {
        let settings = ServerSettings {
            host: "127.0.0.1".to_string(),
            port: 3000,
            cors_allowed_origins: vec!["http://192.168.1.*:5173".to_string()],
        };

        let rules = settings.cors_origin_rules().unwrap();

        assert!(matches!(
            &rules[0],
            CorsOriginRule::Ipv4Wildcard(rule)
                if rule.scheme == "http"
                    && rule.octets == [Some(192), Some(168), Some(1), None]
                    && rule.port == 5173
        ));
    }

    #[test]
    fn server_cors_origins_reject_domain_wildcards() {
        let settings = ServerSettings {
            host: "127.0.0.1".to_string(),
            port: 3000,
            cors_allowed_origins: vec!["http://*.example.local:5173".to_string()],
        };

        assert!(settings.cors_origin_rules().is_err());
    }

    #[test]
    fn server_cors_origins_reject_port_wildcards() {
        let settings = ServerSettings {
            host: "127.0.0.1".to_string(),
            port: 3000,
            cors_allowed_origins: vec!["http://192.168.1.*:*".to_string()],
        };

        assert!(settings.cors_origin_rules().is_err());
    }

    #[test]
    fn database_path_rejects_relative_paths() {
        let settings = DatabaseSettings {
            path: "data/zembra.db".to_string(),
        };

        assert!(settings.validate().is_err());
    }

    #[test]
    fn sqlite_url_preserves_absolute_database_paths() {
        assert_eq!(
            sqlite_url_from_path("/path/to/zembra.sqlite3"),
            "sqlite:///path/to/zembra.sqlite3"
        );
    }
}
