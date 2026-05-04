use serde_json::Value;
use sqlx::{Executor, FromRow, Sqlite, Transaction};

use crate::repositories::taxonomy::{DEFAULT_WORKSPACE_ID, new_id};

/// Default device used by the backend until explicit device registration exists.
pub const DEFAULT_DEVICE_ID: &str = "local-backend";

/// Input used to record a local synchronization change.
#[derive(Debug, Clone)]
pub struct SyncChangeInput {
    /// Entity type affected by this change.
    pub entity_type: &'static str,
    /// Entity identifier affected by this change.
    pub entity_id: String,
    /// Operation applied to the entity.
    pub operation: &'static str,
    /// Optional base revision identifier.
    pub base_revision_id: Option<String>,
    /// Optional new revision identifier.
    pub new_revision_id: Option<String>,
    /// JSON payload containing the entity snapshot or relation change.
    pub payload: Value,
}

/// Synchronization change row used by tests and sync services.
#[derive(Debug, Clone, FromRow)]
#[allow(dead_code)]
pub struct SyncChangeRecord {
    /// Unique change identifier.
    pub id: String,
    /// Workspace that owns this change.
    pub workspace_id: String,
    /// Device that produced this change.
    pub device_id: String,
    /// Entity type affected by this change.
    pub entity_type: String,
    /// Entity identifier affected by this change.
    pub entity_id: String,
    /// Operation applied to the entity.
    pub operation: String,
    /// Optional base revision identifier.
    pub base_revision_id: Option<String>,
    /// Optional new revision identifier.
    pub new_revision_id: Option<String>,
    /// JSON payload stored as text in SQLite.
    pub payload: String,
    /// Unix timestamp for change creation.
    pub created_at: i64,
    /// Unix timestamp for local application.
    pub applied_at: Option<i64>,
    /// Unix timestamp for Supabase commit.
    pub supabase_committed_at: Option<i64>,
}

/// Ensures the default backend device exists for sync change foreign keys.
///
/// # Arguments
///
/// * `transaction` - Open SQLite transaction.
///
/// # Returns
///
/// Returns `Ok(())` when the device row exists.
pub async fn ensure_default_device_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
) -> Result<(), sqlx::Error> {
    ensure_default_device_with_executor(&mut **transaction).await
}

/// Records a synchronization change inside an existing transaction.
///
/// # Arguments
///
/// * `transaction` - Open SQLite transaction.
/// * `input` - Change data to persist.
///
/// # Returns
///
/// Returns the generated change ID.
pub async fn record_sync_change_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    input: SyncChangeInput,
) -> Result<String, sqlx::Error> {
    ensure_default_device_in_transaction(transaction).await?;

    let change_id = new_id();
    sqlx::query(
        "INSERT INTO sync_changes \
         (id, workspace_id, device_id, entity_type, entity_id, operation, base_revision_id, new_revision_id, payload, created_at, applied_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, unixepoch(), unixepoch())",
    )
    .bind(&change_id)
    .bind(DEFAULT_WORKSPACE_ID)
    .bind(DEFAULT_DEVICE_ID)
    .bind(input.entity_type)
    .bind(&input.entity_id)
    .bind(input.operation)
    .bind(input.base_revision_id.as_deref())
    .bind(input.new_revision_id.as_deref())
    .bind(input.payload.to_string())
    .execute(&mut **transaction)
    .await?;

    Ok(change_id)
}

/// Lists synchronization changes for tests and status APIs.
///
/// # Arguments
///
/// * `executor` - SQLx executor used to read local changes.
///
/// # Returns
///
/// Returns sync changes ordered by creation sequence.
#[allow(dead_code)]
pub async fn list_sync_changes<'e, E>(executor: E) -> Result<Vec<SyncChangeRecord>, sqlx::Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as::<_, SyncChangeRecord>(
        "SELECT id, workspace_id, device_id, entity_type, entity_id, operation, base_revision_id, new_revision_id, payload, created_at, applied_at, supabase_committed_at \
         FROM sync_changes ORDER BY created_at ASC, id ASC",
    )
    .fetch_all(executor)
    .await
}

/// Ensures the default backend device exists using a generic executor.
///
/// # Arguments
///
/// * `executor` - SQLx executor used to upsert the device row.
///
/// # Returns
///
/// Returns `Ok(())` when the default device is present.
async fn ensure_default_device_with_executor<'e, E>(executor: E) -> Result<(), sqlx::Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "INSERT OR IGNORE INTO devices (id, workspace_id, name, platform, created_at, sync_enabled) \
         VALUES (?, ?, 'Local Backend', 'backend', unixepoch(), 1)",
    )
    .bind(DEFAULT_DEVICE_ID)
    .bind(DEFAULT_WORKSPACE_ID)
    .execute(executor)
    .await?;

    Ok(())
}
