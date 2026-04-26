use serde::Deserialize;

/// Runtime settings loaded from files and environment variables.
#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    /// HTTP server settings.
    pub server: ServerSettings,
    /// SQLite database settings.
    pub database: DatabaseSettings,
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
    /// SQLx-compatible SQLite database URL.
    pub url: String,
}

impl Settings {
    /// Loads service settings from `config/default.toml` and environment variables.
    ///
    /// # Returns
    ///
    /// Returns parsed settings on success, or a configuration error when required
    /// fields are missing or invalid.
    pub fn load() -> Result<Self, config::ConfigError> {
        config::Config::builder()
            .add_source(config::File::with_name("config/default"))
            .add_source(config::Environment::with_prefix("ZEMBRA").separator("__"))
            .build()?
            .try_deserialize()
    }
}
