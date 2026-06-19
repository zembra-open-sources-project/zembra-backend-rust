use std::path::PathBuf;

use crate::config::SyncSettings;
use crate::dto::sync::{
    SyncConfigResponse, SyncConfigTestResponse, TestSyncConfigRequest, UpdateSyncConfigRequest,
};
use crate::error::ApiError;

/// Service for reading, writing, and testing Supabase synchronization settings.
#[derive(Debug, Clone)]
pub struct SyncConfigService {
    /// User configuration file path.
    path: PathBuf,
}

impl SyncConfigService {
    /// Creates a sync configuration service using the default user config path.
    ///
    /// # Returns
    ///
    /// Returns a service targeting `~/.zembra.env`, or an I/O error when HOME is
    /// not available.
    pub fn from_user_config() -> Result<Self, std::io::Error> {
        crate::config::user_config_path()
            .map(Self::new)
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "HOME is not set"))
    }

    /// Creates a sync configuration service with an explicit path.
    ///
    /// # Arguments
    ///
    /// * `path` - TOML configuration file path.
    ///
    /// # Returns
    ///
    /// Returns a configuration service bound to the path.
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Reads the current sync configuration.
    ///
    /// # Returns
    ///
    /// Returns sync settings from the configuration file, or default sync
    /// settings when the file or sync section is absent.
    pub fn read_settings(&self) -> Result<SyncSettings, ApiError> {
        let document = self.read_document()?;
        sync_settings_from_document(&document)
    }

    /// Reads the current sync configuration as a public response.
    ///
    /// # Returns
    ///
    /// Returns a sanitized sync configuration response.
    pub fn read_response(&self) -> Result<SyncConfigResponse, ApiError> {
        self.read_settings().map(sync_config_response)
    }

    /// Saves updated sync configuration and returns the merged settings.
    ///
    /// # Arguments
    ///
    /// * `request` - Sync configuration update request.
    ///
    /// # Returns
    ///
    /// Returns validated settings after merging the request with the previous
    /// secret key.
    pub fn save(&self, request: UpdateSyncConfigRequest) -> Result<SyncSettings, ApiError> {
        let mut document = self.read_document()?;
        let previous = sync_settings_from_document(&document)?;
        let settings = SyncSettings {
            enabled: request.enabled,
            interval_seconds: request.interval_seconds,
            supabase_url: request.supabase_url,
            secret_key: request.secret_key.unwrap_or(previous.secret_key),
            remote_database_password: previous.remote_database_password,
        };
        validate_sync_settings(&settings)?;

        let root = document
            .as_table_mut()
            .ok_or_else(|| ApiError::InvalidConfig("configuration root must be a table".into()))?;
        root.insert(
            "sync".to_string(),
            toml::Value::try_from(&settings)
                .map_err(|error| ApiError::InvalidConfig(error.to_string()))?,
        );
        self.write_document(&document)?;

        Ok(settings)
    }

    /// Tests Supabase connectivity without saving candidate settings.
    ///
    /// # Arguments
    ///
    /// * `request` - Optional candidate URL and secret key.
    ///
    /// # Returns
    ///
    /// Returns a sanitized connectivity test result.
    pub async fn test_connection(
        &self,
        request: TestSyncConfigRequest,
    ) -> Result<SyncConfigTestResponse, ApiError> {
        let current = self.read_settings()?;
        let settings = SyncSettings {
            enabled: true,
            interval_seconds: current.interval_seconds,
            supabase_url: request.supabase_url.unwrap_or(current.supabase_url),
            secret_key: request.secret_key.unwrap_or(current.secret_key),
            remote_database_password: current.remote_database_password,
        };
        validate_sync_settings(&settings)?;

        let client = crate::sync::supabase::SupabaseClient::new(
            &settings.supabase_url,
            &settings.secret_key,
        );
        Ok(match client.test_connection().await {
            Ok(()) => SyncConfigTestResponse {
                ok: true,
                message: "Supabase connection test succeeded.".to_string(),
            },
            Err(error) => SyncConfigTestResponse {
                ok: false,
                message: format!("Supabase connection test failed: {error}"),
            },
        })
    }

    /// Reads a TOML configuration document.
    ///
    /// # Returns
    ///
    /// Returns an empty document when the file does not exist.
    fn read_document(&self) -> Result<toml::Value, ApiError> {
        if !self.path.exists() {
            return Ok(toml::Value::Table(toml::map::Map::new()));
        }

        let content = std::fs::read_to_string(&self.path)?;
        if content.trim().is_empty() {
            return Ok(toml::Value::Table(toml::map::Map::new()));
        }

        content
            .parse::<toml::Value>()
            .map_err(|error| ApiError::InvalidConfig(error.to_string()))
    }

    /// Writes a TOML configuration document.
    ///
    /// # Arguments
    ///
    /// * `document` - Parsed TOML document to persist.
    fn write_document(&self, document: &toml::Value) -> Result<(), ApiError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(document)
            .map_err(|error| ApiError::InvalidConfig(error.to_string()))?;
        std::fs::write(&self.path, content)?;
        Ok(())
    }
}

/// Converts settings into a public sync configuration response.
///
/// # Arguments
///
/// * `settings` - Internal sync settings.
///
/// # Returns
///
/// Returns a response without the secret key.
pub fn sync_config_response(settings: SyncSettings) -> SyncConfigResponse {
    SyncConfigResponse {
        enabled: settings.enabled,
        interval_seconds: settings.interval_seconds,
        supabase_url: settings.supabase_url,
        secret_key_configured: !settings.secret_key.trim().is_empty(),
    }
}

/// Reads sync settings from a parsed TOML document.
///
/// # Arguments
///
/// * `document` - Parsed TOML document.
///
/// # Returns
///
/// Returns parsed sync settings or defaults when no sync section exists.
fn sync_settings_from_document(document: &toml::Value) -> Result<SyncSettings, ApiError> {
    match document.get("sync") {
        Some(value) => value
            .clone()
            .try_into::<SyncSettings>()
            .map_err(|error| ApiError::InvalidConfig(error.to_string())),
        None => Ok(SyncSettings::default()),
    }
}

/// Validates sync settings and maps config errors to API errors.
///
/// # Arguments
///
/// * `settings` - Settings to validate.
fn validate_sync_settings(settings: &SyncSettings) -> Result<(), ApiError> {
    settings
        .validate()
        .map_err(|error| ApiError::InvalidConfig(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::SyncConfigService;
    use crate::dto::sync::UpdateSyncConfigRequest;

    /// Builds a unique temporary config path for tests.
    ///
    /// # Returns
    ///
    /// Returns a path under the system temporary directory.
    fn temp_config_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "zembra-sync-config-{name}-{}.toml",
            std::process::id()
        ))
    }

    #[test]
    fn save_creates_sync_section_without_exposing_key() {
        let path = temp_config_path("create");
        let _ = std::fs::remove_file(&path);
        let service = SyncConfigService::new(path.clone());

        let settings = service
            .save(UpdateSyncConfigRequest {
                enabled: true,
                interval_seconds: 30,
                supabase_url: "https://example.supabase.co".to_string(),
                secret_key: Some("sb_secret_test-key".to_string()),
            })
            .unwrap();
        let response = super::sync_config_response(settings);
        let content = std::fs::read_to_string(&path).unwrap();

        assert!(content.contains("secret_key = \"sb_secret_test-key\""));
        assert!(response.secret_key_configured);
        assert_eq!(response.supabase_url, "https://example.supabase.co");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn save_preserves_existing_key_when_request_omits_key() {
        let path = temp_config_path("preserve");
        let _ = std::fs::remove_file(&path);
        std::fs::write(
            &path,
            r#"
                [sync]
                enabled = false
                interval_seconds = 60
                supabase_url = "https://old.supabase.co"
                secret_key = "sb_secret_old-key"
            "#,
        )
        .unwrap();
        let service = SyncConfigService::new(path.clone());

        service
            .save(UpdateSyncConfigRequest {
                enabled: true,
                interval_seconds: 15,
                supabase_url: "https://new.supabase.co".to_string(),
                secret_key: None,
            })
            .unwrap();
        let content = std::fs::read_to_string(&path).unwrap();

        assert!(content.contains("secret_key = \"sb_secret_old-key\""));
        assert!(content.contains("supabase_url = \"https://new.supabase.co\""));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn save_rejects_enabled_config_without_key() {
        let path = temp_config_path("invalid");
        let _ = std::fs::remove_file(&path);
        let service = SyncConfigService::new(path.clone());

        let error = service
            .save(UpdateSyncConfigRequest {
                enabled: true,
                interval_seconds: 15,
                supabase_url: "https://new.supabase.co".to_string(),
                secret_key: None,
            })
            .unwrap_err();

        assert_eq!(error.code(), "invalid_config");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn save_rejects_legacy_service_role_key() {
        let path = temp_config_path("legacy-key");
        let _ = std::fs::remove_file(&path);
        let service = SyncConfigService::new(path.clone());

        let error = service
            .save(UpdateSyncConfigRequest {
                enabled: true,
                interval_seconds: 15,
                supabase_url: "https://new.supabase.co".to_string(),
                secret_key: Some("eyJlegacy.jwt.service-role".to_string()),
            })
            .unwrap_err();

        assert_eq!(error.code(), "invalid_config");
        let _ = std::fs::remove_file(path);
    }
}
