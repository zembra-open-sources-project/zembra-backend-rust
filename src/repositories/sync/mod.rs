use sqlx::{Executor, Sqlite, Transaction};

mod apply;
mod ids;
mod outbox;
mod payload;
mod snapshot;
mod state;
mod types;
mod write_snapshot;

pub use types::{DEFAULT_DEVICE_ID, SyncChangeInput, SyncChangeRecord, SyncStateRecord};

#[cfg(test)]
mod tests;

use crate::repositories::taxonomy::{DEFAULT_WORKSPACE_ID, new_id};

/// Repository for local synchronization state.
#[derive(Debug, Clone)]
pub struct SyncRepository {
    /// SQLx pool used by synchronization queries.
    pub(super) pool: sqlx::SqlitePool,
}

impl SyncRepository {
    /// Creates a synchronization repository backed by a SQLite pool.
    ///
    /// # Arguments
    ///
    /// * `pool` - Shared SQLite connection pool.
    ///
    /// # Returns
    ///
    /// Returns a sync repository.
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self { pool }
    }
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

/// Updates a sync state row as a success inside an existing transaction.
///
/// # Arguments
///
/// * `transaction` - Open SQLite transaction.
/// * `scope` - Cursor direction.
/// * `created_at` - Last processed change timestamp.
/// * `change_id` - Last processed change identifier.
///
/// # Returns
///
/// Returns `Ok(())` after the state row is upserted.
pub(super) async fn update_state_success(
    transaction: &mut Transaction<'_, Sqlite>,
    scope: &str,
    created_at: i64,
    change_id: &str,
) -> Result<(), sqlx::Error> {
    ensure_default_device_in_transaction(transaction).await?;
    sqlx::query(
        "INSERT INTO sync_state \
         (workspace_id, device_id, scope, last_change_created_at, last_change_id, last_success_at, last_error_at, last_error_message) \
         VALUES (?, ?, ?, ?, ?, unixepoch(), NULL, NULL) \
         ON CONFLICT(workspace_id, device_id, scope) DO UPDATE SET \
         last_change_created_at = excluded.last_change_created_at, \
         last_change_id = excluded.last_change_id, \
         last_success_at = excluded.last_success_at, \
         last_error_at = NULL, \
         last_error_message = NULL",
    )
    .bind(DEFAULT_WORKSPACE_ID)
    .bind(DEFAULT_DEVICE_ID)
    .bind(scope)
    .bind(created_at)
    .bind(change_id)
    .execute(&mut **transaction)
    .await?;

    Ok(())
}
