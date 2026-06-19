use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::repositories::sync::SyncChangeRecord;
use crate::sync::table_snapshot::{
    NoteLinkSnapshotRow, NoteTagSnapshotRow, SyncChangeSnapshotRow, SyncTableSnapshot,
};

const SUPABASE_PAGE_SIZE: usize = 1000;

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

    /// Ensures the default workspace and backend device exist in Supabase.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` when Supabase accepts both identity upserts.
    pub async fn ensure_default_sync_identity(&self) -> Result<(), SupabaseError> {
        let timestamp = unix_timestamp();

        let workspace_request = self.build_upsert_default_workspace_request(timestamp)?;
        let workspace_response = self.client.execute(workspace_request).await?;
        ensure_success(workspace_response).await?;

        let device_request = self.build_upsert_default_device_request(timestamp)?;
        let device_response = self.client.execute(device_request).await?;
        ensure_success(device_response).await
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

    /// Fetches all synchronized table rows from Supabase.
    ///
    /// # Returns
    ///
    /// Returns a remote snapshot for the nine synchronized tables.
    pub async fn fetch_table_snapshot(&self) -> Result<SyncTableSnapshot, SupabaseError> {
        Ok(SyncTableSnapshot {
            workspaces: self.fetch_table_rows("workspaces", "id.asc", false).await?,
            devices: self.fetch_table_rows("devices", "id.asc", true).await?,
            fields: self.fetch_table_rows("fields", "id.asc", true).await?,
            tags: self.fetch_table_rows("tags", "id.asc", true).await?,
            notes: self.fetch_table_rows("notes", "id.asc", true).await?,
            note_revisions: self
                .fetch_table_rows("note_revisions", "id.asc", true)
                .await?,
            note_tags: self
                .fetch_table_rows("note_tags", "workspace_id.asc,note_id.asc,tag_id.asc", true)
                .await?,
            note_links: self.fetch_table_rows("note_links", "id.asc", true).await?,
            sync_changes: self
                .fetch_table_rows("sync_changes", "created_at.asc,id.asc", true)
                .await?,
        })
    }

    /// Upserts synchronized table rows to Supabase in foreign-key order.
    ///
    /// # Arguments
    ///
    /// * `snapshot` - Rows to upsert.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` after all non-empty table batches are accepted.
    pub async fn upsert_table_snapshot(
        &self,
        snapshot: &SyncTableSnapshot,
    ) -> Result<(), SupabaseError> {
        self.upsert_table_rows("workspaces", &snapshot.workspaces)
            .await?;
        self.upsert_table_rows("devices", &snapshot.devices).await?;
        self.upsert_table_rows("fields", &snapshot.fields).await?;
        self.upsert_table_rows("tags", &snapshot.tags).await?;
        self.upsert_table_rows("notes", &snapshot.notes).await?;
        self.upsert_table_rows("note_revisions", &snapshot.note_revisions)
            .await?;
        self.upsert_table_rows("note_tags", &snapshot.note_tags)
            .await?;
        self.upsert_table_rows("note_links", &snapshot.note_links)
            .await?;
        let sync_changes = supabase_snapshot_changes(&snapshot.sync_changes);
        self.upsert_table_rows("sync_changes", &sync_changes)
            .await?;

        Ok(())
    }

    /// Deletes a note tag relation from Supabase.
    ///
    /// # Arguments
    ///
    /// * `row` - Relation row key to delete.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` after Supabase accepts the delete.
    pub async fn delete_note_tag(&self, row: &NoteTagSnapshotRow) -> Result<(), SupabaseError> {
        let request = self.build_delete_note_tag_request(row)?;
        let response = self.client.execute(request).await?;
        ensure_success(response).await
    }

    /// Deletes a note link from Supabase.
    ///
    /// # Arguments
    ///
    /// * `row` - Link row key to delete.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` after Supabase accepts the delete.
    pub async fn delete_note_link(&self, row: &NoteLinkSnapshotRow) -> Result<(), SupabaseError> {
        let request = self.build_delete_note_link_request(row)?;
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

    /// Builds an upsert request for the default workspace.
    ///
    /// # Arguments
    ///
    /// * `timestamp` - Unix timestamp used for required schema fields.
    ///
    /// # Returns
    ///
    /// Returns a request ready to execute.
    pub fn build_upsert_default_workspace_request(
        &self,
        timestamp: i64,
    ) -> Result<reqwest::Request, SupabaseError> {
        self.client
            .post(format!("{}/rest/v1/workspaces", self.base_url))
            .headers(self.headers()?)
            .header("Prefer", "resolution=merge-duplicates")
            .json(&[SupabaseWorkspaceRecord {
                id: crate::repositories::taxonomy::DEFAULT_WORKSPACE_ID.to_string(),
                workspace_name: None,
                created_at: timestamp,
                updated_at: timestamp,
                archived_at: None,
                deleted_at: None,
            }])
            .build()
            .map_err(Into::into)
    }

    /// Builds an upsert request for the backend default device.
    ///
    /// # Arguments
    ///
    /// * `timestamp` - Unix timestamp used for required schema fields.
    ///
    /// # Returns
    ///
    /// Returns a request ready to execute.
    pub fn build_upsert_default_device_request(
        &self,
        timestamp: i64,
    ) -> Result<reqwest::Request, SupabaseError> {
        self.client
            .post(format!("{}/rest/v1/devices", self.base_url))
            .headers(self.headers()?)
            .header("Prefer", "resolution=merge-duplicates")
            .json(&[SupabaseDeviceRecord {
                id: crate::repositories::sync::DEFAULT_DEVICE_ID.to_string(),
                workspace_id: crate::repositories::taxonomy::DEFAULT_WORKSPACE_ID.to_string(),
                name: "Local Backend".to_string(),
                platform: "backend".to_string(),
                created_at: timestamp,
                last_seen_at: None,
                sync_enabled: true,
                last_synced_at: None,
            }])
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

    /// Builds a synchronized table fetch request.
    ///
    /// # Arguments
    ///
    /// * `table` - Supabase table name.
    /// * `order` - PostgREST ordering expression.
    /// * `workspace_scoped` - Whether to add the default workspace filter.
    /// * `offset` - Zero-based page start.
    /// * `limit` - Maximum rows for the page.
    ///
    /// # Returns
    ///
    /// Returns a request ready to execute.
    pub fn build_fetch_table_request(
        &self,
        table: &str,
        order: &str,
        workspace_scoped: bool,
        offset: usize,
        limit: usize,
    ) -> Result<reqwest::Request, SupabaseError> {
        let mut request = self
            .client
            .get(format!("{}/rest/v1/{table}", self.base_url))
            .headers(self.headers()?)
            .query(&[("select", "*"), ("order", order)])
            .header("Range-Unit", "items")
            .header(
                "Range",
                format!(
                    "{}-{}",
                    offset,
                    offset.saturating_add(limit).saturating_sub(1)
                ),
            );

        if workspace_scoped {
            request = request.query(&[(
                "workspace_id",
                format!("eq.{}", crate::repositories::taxonomy::DEFAULT_WORKSPACE_ID),
            )]);
        }

        request.build().map_err(Into::into)
    }

    /// Builds a synchronized table upsert request.
    ///
    /// # Arguments
    ///
    /// * `table` - Supabase table name.
    /// * `rows` - Rows to serialize as JSON.
    ///
    /// # Returns
    ///
    /// Returns a request ready to execute.
    pub fn build_upsert_table_request<T>(
        &self,
        table: &str,
        rows: &[T],
    ) -> Result<reqwest::Request, SupabaseError>
    where
        T: Serialize,
    {
        self.client
            .post(format!("{}/rest/v1/{table}", self.base_url))
            .headers(self.headers()?)
            .header("Prefer", "resolution=merge-duplicates")
            .json(rows)
            .build()
            .map_err(Into::into)
    }

    /// Builds a note tag relation delete request.
    ///
    /// # Arguments
    ///
    /// * `row` - Relation row key to delete.
    ///
    /// # Returns
    ///
    /// Returns a request ready to execute.
    pub fn build_delete_note_tag_request(
        &self,
        row: &NoteTagSnapshotRow,
    ) -> Result<reqwest::Request, SupabaseError> {
        self.client
            .delete(format!("{}/rest/v1/note_tags", self.base_url))
            .headers(self.headers()?)
            .query(&[
                ("workspace_id", format!("eq.{}", row.workspace_id)),
                ("note_id", format!("eq.{}", row.note_id)),
                ("tag_id", format!("eq.{}", row.tag_id)),
            ])
            .build()
            .map_err(Into::into)
    }

    /// Builds a note link delete request.
    ///
    /// # Arguments
    ///
    /// * `row` - Link row key to delete.
    ///
    /// # Returns
    ///
    /// Returns a request ready to execute.
    pub fn build_delete_note_link_request(
        &self,
        row: &NoteLinkSnapshotRow,
    ) -> Result<reqwest::Request, SupabaseError> {
        self.client
            .delete(format!("{}/rest/v1/note_links", self.base_url))
            .headers(self.headers()?)
            .query(&[
                ("workspace_id", format!("eq.{}", row.workspace_id)),
                ("id", format!("eq.{}", row.id)),
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

    /// Fetches all rows for one synchronized table using paginated requests.
    ///
    /// # Arguments
    ///
    /// * `table` - Supabase table name.
    /// * `order` - PostgREST ordering expression.
    /// * `workspace_scoped` - Whether to filter by default workspace.
    ///
    /// # Returns
    ///
    /// Returns all fetched rows.
    async fn fetch_table_rows<T>(
        &self,
        table: &str,
        order: &str,
        workspace_scoped: bool,
    ) -> Result<Vec<T>, SupabaseError>
    where
        T: DeserializeOwned,
    {
        let mut offset = 0;
        let mut rows = Vec::new();

        loop {
            let request = self.build_fetch_table_request(
                table,
                order,
                workspace_scoped,
                offset,
                SUPABASE_PAGE_SIZE,
            )?;
            let response = self.client.execute(request).await?;
            if !response.status().is_success() {
                return Err(SupabaseError::Status {
                    status: response.status().as_u16(),
                    body: response.text().await.unwrap_or_default(),
                });
            }

            let mut page = response.json::<Vec<T>>().await?;
            let page_len = page.len();
            rows.append(&mut page);

            if page_len < SUPABASE_PAGE_SIZE {
                break;
            }
            offset += SUPABASE_PAGE_SIZE;
        }

        Ok(rows)
    }

    /// Upserts rows for one synchronized table.
    ///
    /// # Arguments
    ///
    /// * `table` - Supabase table name.
    /// * `rows` - Rows to upsert.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` after Supabase accepts the request.
    async fn upsert_table_rows<T>(&self, table: &str, rows: &[T]) -> Result<(), SupabaseError>
    where
        T: Serialize,
    {
        if rows.is_empty() {
            return Ok(());
        }

        let request = self.build_upsert_table_request(table, rows)?;
        let response = self.client.execute(request).await?;
        ensure_success(response).await
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

/// Workspace row used to register the default sync workspace in Supabase.
#[derive(Debug, Clone, Serialize)]
struct SupabaseWorkspaceRecord {
    /// Workspace identifier.
    id: String,
    /// Optional display name.
    workspace_name: Option<String>,
    /// Unix timestamp for creation.
    created_at: i64,
    /// Unix timestamp for last update.
    updated_at: i64,
    /// Optional archive timestamp.
    archived_at: Option<i64>,
    /// Optional deletion timestamp.
    deleted_at: Option<i64>,
}

/// Device row used to register the backend sync device in Supabase.
#[derive(Debug, Clone, Serialize)]
struct SupabaseDeviceRecord {
    /// Device identifier.
    id: String,
    /// Workspace identifier.
    workspace_id: String,
    /// Human-readable device name.
    name: String,
    /// Device platform.
    platform: String,
    /// Unix timestamp for creation.
    created_at: i64,
    /// Optional last seen timestamp.
    last_seen_at: Option<i64>,
    /// Whether the device participates in sync.
    sync_enabled: bool,
    /// Optional last synced timestamp.
    last_synced_at: Option<i64>,
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

/// Converts sync change snapshot rows into Supabase JSON records.
///
/// # Arguments
///
/// * `changes` - Snapshot change rows.
///
/// # Returns
///
/// Returns records ready to serialize to Supabase.
fn supabase_snapshot_changes(changes: &[SyncChangeSnapshotRow]) -> Vec<SupabaseSyncChangeRecord> {
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

/// Returns the current Unix timestamp in seconds.
///
/// # Returns
///
/// Returns a non-negative Unix timestamp.
fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::SupabaseClient;
    use crate::sync::table_snapshot::{FieldSnapshotRow, NoteLinkSnapshotRow, NoteTagSnapshotRow};

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

    #[test]
    fn workspace_upsert_request_targets_workspaces() {
        let client = SupabaseClient::new("https://example.supabase.co", "sb_secret_test-key");
        let request = client.build_upsert_default_workspace_request(123).unwrap();

        assert_eq!(
            request.url().as_str(),
            "https://example.supabase.co/rest/v1/workspaces"
        );
        assert_eq!(request.headers()["prefer"], "resolution=merge-duplicates");
    }

    #[test]
    fn device_upsert_request_targets_devices() {
        let client = SupabaseClient::new("https://example.supabase.co", "sb_secret_test-key");
        let request = client.build_upsert_default_device_request(123).unwrap();

        assert_eq!(
            request.url().as_str(),
            "https://example.supabase.co/rest/v1/devices"
        );
        assert_eq!(request.headers()["prefer"], "resolution=merge-duplicates");
    }

    #[test]
    fn table_fetch_request_uses_workspace_filter_order_and_range() {
        let client = SupabaseClient::new("https://example.supabase.co", "sb_secret_test-key");
        let request = client
            .build_fetch_table_request(
                "note_tags",
                "workspace_id.asc,note_id.asc,tag_id.asc",
                true,
                1000,
                1000,
            )
            .unwrap();
        let url = request.url().as_str();

        assert!(url.starts_with("https://example.supabase.co/rest/v1/note_tags?"));
        assert!(url.contains("select="));
        assert!(url.contains("workspace_id=eq."));
        assert!(url.contains("order=workspace_id.asc"));
        assert_eq!(request.headers()["range-unit"], "items");
        assert_eq!(request.headers()["range"], "1000-1999");
    }

    #[test]
    fn table_upsert_request_targets_business_table() {
        let client = SupabaseClient::new("https://example.supabase.co", "sb_secret_test-key");
        let rows = vec![FieldSnapshotRow {
            id: "field-1".to_string(),
            workspace_id: crate::repositories::taxonomy::DEFAULT_WORKSPACE_ID.to_string(),
            name: "field".to_string(),
            created_at: 123,
        }];
        let request = client.build_upsert_table_request("fields", &rows).unwrap();

        assert_eq!(
            request.url().as_str(),
            "https://example.supabase.co/rest/v1/fields"
        );
        assert_eq!(request.headers()["prefer"], "resolution=merge-duplicates");
    }

    #[test]
    fn note_tag_delete_request_filters_composite_key() {
        let client = SupabaseClient::new("https://example.supabase.co", "sb_secret_test-key");
        let row = NoteTagSnapshotRow {
            workspace_id: crate::repositories::taxonomy::DEFAULT_WORKSPACE_ID.to_string(),
            note_id: "note-1".to_string(),
            tag_id: "tag-1".to_string(),
            created_at: 123,
        };
        let request = client.build_delete_note_tag_request(&row).unwrap();
        let url = request.url().as_str();

        assert_eq!(request.method(), reqwest::Method::DELETE);
        assert!(url.starts_with("https://example.supabase.co/rest/v1/note_tags?"));
        assert!(url.contains("workspace_id=eq."));
        assert!(url.contains("note_id=eq.note-1"));
        assert!(url.contains("tag_id=eq.tag-1"));
    }

    #[test]
    fn note_link_delete_request_filters_id() {
        let client = SupabaseClient::new("https://example.supabase.co", "sb_secret_test-key");
        let row = NoteLinkSnapshotRow {
            id: "link-1".to_string(),
            workspace_id: crate::repositories::taxonomy::DEFAULT_WORKSPACE_ID.to_string(),
            source_note_id: "note-1".to_string(),
            target_note_id: "note-2".to_string(),
            anchor_text: None,
            position: None,
            created_at: 123,
        };
        let request = client.build_delete_note_link_request(&row).unwrap();
        let url = request.url().as_str();

        assert_eq!(request.method(), reqwest::Method::DELETE);
        assert!(url.starts_with("https://example.supabase.co/rest/v1/note_links?"));
        assert!(url.contains("workspace_id=eq."));
        assert!(url.contains("id=eq.link-1"));
    }
}
