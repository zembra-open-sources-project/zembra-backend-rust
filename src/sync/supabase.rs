use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};

use crate::repositories::sync::SyncChangeRecord;

/// Supabase REST client for synchronization tables.
#[derive(Debug, Clone)]
pub struct SupabaseClient {
    /// Supabase project URL without a trailing slash.
    base_url: String,
    /// Supabase secret key used for backend-only requests.
    secret_key: String,
    /// Shared HTTP client.
    client: reqwest::Client,
}

impl SupabaseClient {
    /// Creates a Supabase REST client from sync settings.
    ///
    /// # Arguments
    ///
    /// * `settings` - Synchronization settings containing URL and key.
    ///
    /// # Returns
    ///
    /// Returns a configured client.
    pub fn from_settings(settings: &crate::config::SyncSettings) -> Self {
        Self::new(&settings.supabase_url, &settings.secret_key)
    }

    /// Creates a Supabase REST client from explicit values.
    ///
    /// # Arguments
    ///
    /// * `base_url` - Supabase project URL.
    /// * `secret_key` - Secret key for backend access.
    ///
    /// # Returns
    ///
    /// Returns a configured client.
    pub fn new(base_url: &str, secret_key: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            secret_key: secret_key.to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Upserts local sync changes into Supabase.
    ///
    /// # Arguments
    ///
    /// * `changes` - Local changes to push.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` when Supabase accepts the request.
    pub async fn upsert_sync_changes(
        &self,
        changes: &[SyncChangeRecord],
    ) -> Result<(), SupabaseError> {
        if changes.is_empty() {
            return Ok(());
        }

        let request = self.build_upsert_sync_changes_request(changes)?;
        let response = self.client.execute(request).await?;
        ensure_success(response).await
    }

    /// Fetches remote sync changes after the provided cursor.
    ///
    /// # Arguments
    ///
    /// * `created_after` - Last processed change timestamp.
    /// * `change_id_after` - Last processed change ID for timestamp ties.
    /// * `limit` - Maximum number of changes to fetch.
    ///
    /// # Returns
    ///
    /// Returns remote changes ordered by cursor.
    pub async fn fetch_remote_changes(
        &self,
        created_after: i64,
        change_id_after: &str,
        limit: i64,
    ) -> Result<Vec<SyncChangeRecord>, SupabaseError> {
        let request =
            self.build_fetch_remote_changes_request(created_after, change_id_after, limit)?;
        let response = self.client.execute(request).await?;
        if !response.status().is_success() {
            return Err(SupabaseError::Status {
                status: response.status().as_u16(),
                body: response.text().await.unwrap_or_default(),
            });
        }

        let changes = response.json::<Vec<SupabaseSyncChangeRecord>>().await?;
        Ok(changes.into_iter().map(Into::into).collect())
    }

    /// Tests whether Supabase REST accepts authenticated requests.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` when the sync changes endpoint responds with success.
    pub async fn test_connection(&self) -> Result<(), SupabaseError> {
        let request = self
            .client
            .get(format!("{}/rest/v1/sync_changes", self.base_url))
            .headers(self.headers()?)
            .query(&[("limit", "1")])
            .build()?;
        let response = self.client.execute(request).await?;
        ensure_success(response).await
    }

    /// Builds an upsert request for sync changes.
    ///
    /// # Arguments
    ///
    /// * `changes` - Changes to serialize.
    ///
    /// # Returns
    ///
    /// Returns a request ready to execute.
    pub fn build_upsert_sync_changes_request(
        &self,
        changes: &[SyncChangeRecord],
    ) -> Result<reqwest::Request, SupabaseError> {
        self.client
            .post(format!("{}/rest/v1/sync_changes", self.base_url))
            .headers(self.headers()?)
            .header("Prefer", "resolution=merge-duplicates")
            .json(&supabase_changes(changes))
            .build()
            .map_err(Into::into)
    }

    /// Builds a fetch request for remote sync changes.
    ///
    /// # Arguments
    ///
    /// * `created_after` - Last processed change timestamp.
    /// * `change_id_after` - Last processed change ID for timestamp ties.
    /// * `limit` - Maximum number of changes to fetch.
    ///
    /// # Returns
    ///
    /// Returns a request ready to execute.
    pub fn build_fetch_remote_changes_request(
        &self,
        created_after: i64,
        change_id_after: &str,
        limit: i64,
    ) -> Result<reqwest::Request, SupabaseError> {
        self.client
            .get(format!("{}/rest/v1/sync_changes", self.base_url))
            .headers(self.headers()?)
            .query(&[
                ("workspace_id", format!("eq.{}", crate::repositories::taxonomy::DEFAULT_WORKSPACE_ID)),
                ("device_id", format!("neq.{}", crate::repositories::sync::DEFAULT_DEVICE_ID)),
                ("or", format!("(created_at.gt.{created_after},and(created_at.eq.{created_after},id.gt.{change_id_after}))")),
                ("order", "created_at.asc,id.asc".to_string()),
                ("limit", limit.to_string()),
            ])
            .build()
            .map_err(Into::into)
    }

    /// Builds authenticated Supabase headers.
    ///
    /// # Returns
    ///
    /// Returns headers containing the secret key.
    fn headers(&self) -> Result<HeaderMap, SupabaseError> {
        let mut headers = HeaderMap::new();
        let key =
            HeaderValue::from_str(&self.secret_key).map_err(|_| SupabaseError::InvalidSecretKey)?;
        headers.insert("apikey", key.clone());
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.secret_key))
                .map_err(|_| SupabaseError::InvalidSecretKey)?,
        );

        Ok(headers)
    }
}

/// Sync change representation used for Supabase JSON payloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SupabaseSyncChangeRecord {
    /// Unique change identifier.
    id: String,
    /// Workspace that owns this change.
    workspace_id: String,
    /// Device that produced this change.
    device_id: String,
    /// Entity type affected by this change.
    entity_type: String,
    /// Entity identifier affected by this change.
    entity_id: String,
    /// Operation applied to the entity.
    operation: String,
    /// Optional base revision identifier.
    base_revision_id: Option<String>,
    /// Optional new revision identifier.
    new_revision_id: Option<String>,
    /// JSON payload stored as jsonb in Supabase.
    payload: serde_json::Value,
    /// Unix timestamp for change creation.
    created_at: i64,
    /// Unix timestamp for local application.
    applied_at: Option<i64>,
    /// Unix timestamp for Supabase commit.
    supabase_committed_at: Option<i64>,
}

impl From<SupabaseSyncChangeRecord> for SyncChangeRecord {
    /// Converts a Supabase change into the local SQLite representation.
    ///
    /// # Returns
    ///
    /// Returns a local sync change record.
    fn from(change: SupabaseSyncChangeRecord) -> Self {
        Self {
            id: change.id,
            workspace_id: change.workspace_id,
            device_id: change.device_id,
            entity_type: change.entity_type,
            entity_id: change.entity_id,
            operation: change.operation,
            base_revision_id: change.base_revision_id,
            new_revision_id: change.new_revision_id,
            payload: change.payload.to_string(),
            created_at: change.created_at,
            applied_at: change.applied_at,
            supabase_committed_at: change.supabase_committed_at,
        }
    }
}

/// Converts local sync changes into Supabase JSON records.
///
/// # Arguments
///
/// * `changes` - Local sync changes.
///
/// # Returns
///
/// Returns records ready to serialize to Supabase.
fn supabase_changes(changes: &[SyncChangeRecord]) -> Vec<SupabaseSyncChangeRecord> {
    changes
        .iter()
        .map(|change| SupabaseSyncChangeRecord {
            id: change.id.clone(),
            workspace_id: change.workspace_id.clone(),
            device_id: change.device_id.clone(),
            entity_type: change.entity_type.clone(),
            entity_id: change.entity_id.clone(),
            operation: change.operation.clone(),
            base_revision_id: change.base_revision_id.clone(),
            new_revision_id: change.new_revision_id.clone(),
            payload: serde_json::from_str(&change.payload).unwrap_or(serde_json::Value::Null),
            created_at: change.created_at,
            applied_at: change.applied_at,
            supabase_committed_at: change.supabase_committed_at,
        })
        .collect()
}

/// Error returned by Supabase synchronization requests.
#[derive(Debug, thiserror::Error)]
pub enum SupabaseError {
    /// Service role key could not be encoded as an HTTP header.
    #[error("invalid Supabase secret key")]
    InvalidSecretKey,
    /// HTTP request construction or transport failed.
    #[error("Supabase request failed: {0}")]
    Request(#[from] reqwest::Error),
    /// Supabase returned a non-success status code.
    #[error("Supabase returned status {status}: {body}")]
    Status {
        /// HTTP status code returned by Supabase.
        status: u16,
        /// Response body returned by Supabase.
        body: String,
    },
}

/// Converts a Supabase response status into a result.
///
/// # Arguments
///
/// * `response` - HTTP response returned by Supabase.
///
/// # Returns
///
/// Returns `Ok(())` for success status codes.
async fn ensure_success(response: reqwest::Response) -> Result<(), SupabaseError> {
    if response.status().is_success() {
        return Ok(());
    }

    Err(SupabaseError::Status {
        status: response.status().as_u16(),
        body: response.text().await.unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::SupabaseClient;

    #[test]
    fn upsert_request_contains_supabase_auth_headers() {
        let client = SupabaseClient::new("https://example.supabase.co/", "sb_secret_test-key");
        let request = client.build_upsert_sync_changes_request(&[]).unwrap();

        assert_eq!(
            request.url().as_str(),
            "https://example.supabase.co/rest/v1/sync_changes"
        );
        assert_eq!(request.headers()["apikey"], "sb_secret_test-key");
        assert_eq!(
            request.headers()["authorization"],
            "Bearer sb_secret_test-key"
        );
        assert_eq!(request.headers()["prefer"], "resolution=merge-duplicates");
    }

    #[test]
    fn fetch_request_uses_cursor_query() {
        let client = SupabaseClient::new("https://example.supabase.co", "sb_secret_test-key");
        let request = client
            .build_fetch_remote_changes_request(10, "abc", 25)
            .unwrap();
        let url = request.url().as_str();

        assert!(url.starts_with("https://example.supabase.co/rest/v1/sync_changes?"));
        assert!(url.contains("workspace_id=eq."));
        assert!(url.contains("device_id=neq.local-backend"));
        assert!(url.contains("limit=25"));
    }
}
