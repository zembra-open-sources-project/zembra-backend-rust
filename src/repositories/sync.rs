use serde::{Deserialize, Serialize};
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
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
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

/// Sync cursor row for one workspace, device, and direction.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct SyncStateRecord {
    /// Workspace that owns this cursor.
    pub workspace_id: String,
    /// Device that owns this cursor.
    pub device_id: String,
    /// Cursor direction.
    pub scope: String,
    /// Last processed change timestamp.
    pub last_change_created_at: i64,
    /// Last processed change identifier.
    pub last_change_id: String,
    /// Last successful sync timestamp.
    pub last_success_at: Option<i64>,
    /// Last failed sync timestamp.
    pub last_error_at: Option<i64>,
    /// Last failed sync message.
    pub last_error_message: Option<String>,
}

/// Repository for local synchronization state.
#[derive(Debug, Clone)]
pub struct SyncRepository {
    /// SQLx pool used by synchronization queries.
    pool: sqlx::SqlitePool,
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

    /// Lists local changes that have not yet been pushed to Supabase.
    ///
    /// # Arguments
    ///
    /// * `limit` - Maximum number of changes to return.
    ///
    /// # Returns
    ///
    /// Returns pending local changes ordered by sync cursor.
    pub async fn list_pending_push_changes(
        &self,
        limit: i64,
    ) -> Result<Vec<SyncChangeRecord>, sqlx::Error> {
        sqlx::query_as::<_, SyncChangeRecord>(
            "SELECT id, workspace_id, device_id, entity_type, entity_id, operation, base_revision_id, new_revision_id, payload, created_at, applied_at, supabase_committed_at \
             FROM sync_changes \
             WHERE workspace_id = ? AND supabase_committed_at IS NULL \
             ORDER BY created_at ASC, id ASC LIMIT ?",
        )
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    /// Reads a sync cursor, creating the default row when needed.
    ///
    /// # Arguments
    ///
    /// * `scope` - Cursor direction, either `push` or `pull`.
    ///
    /// # Returns
    ///
    /// Returns the persisted cursor row.
    pub async fn get_or_create_state(&self, scope: &str) -> Result<SyncStateRecord, sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        ensure_default_device_in_transaction(&mut transaction).await?;
        sqlx::query(
            "INSERT OR IGNORE INTO sync_state \
             (workspace_id, device_id, scope, last_change_created_at, last_change_id) \
             VALUES (?, ?, ?, 0, '0')",
        )
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(DEFAULT_DEVICE_ID)
        .bind(scope)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;

        sqlx::query_as::<_, SyncStateRecord>(
            "SELECT workspace_id, device_id, scope, last_change_created_at, last_change_id, last_success_at, last_error_at, last_error_message \
             FROM sync_state WHERE workspace_id = ? AND device_id = ? AND scope = ?",
        )
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(DEFAULT_DEVICE_ID)
        .bind(scope)
        .fetch_one(&self.pool)
        .await
    }

    /// Marks pushed changes as committed by Supabase.
    ///
    /// # Arguments
    ///
    /// * `changes` - Changes successfully pushed.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` after timestamps and cursor are updated.
    pub async fn mark_push_success(&self, changes: &[SyncChangeRecord]) -> Result<(), sqlx::Error> {
        if changes.is_empty() {
            self.record_success("push", 0, "0").await?;
            return Ok(());
        }

        let mut transaction = self.pool.begin().await?;
        for change in changes {
            sqlx::query("UPDATE sync_changes SET supabase_committed_at = unixepoch() WHERE id = ?")
                .bind(&change.id)
                .execute(&mut *transaction)
                .await?;
        }

        let last = changes
            .last()
            .expect("non-empty changes should have a last row");
        update_state_success(&mut transaction, "push", last.created_at, &last.id).await?;
        transaction.commit().await?;

        Ok(())
    }

    /// Records a successful sync state update.
    ///
    /// # Arguments
    ///
    /// * `scope` - Cursor direction.
    /// * `created_at` - Last processed change timestamp.
    /// * `change_id` - Last processed change identifier.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` after the cursor is updated.
    pub async fn record_success(
        &self,
        scope: &str,
        created_at: i64,
        change_id: &str,
    ) -> Result<(), sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        ensure_default_device_in_transaction(&mut transaction).await?;
        update_state_success(&mut transaction, scope, created_at, change_id).await?;
        transaction.commit().await?;

        Ok(())
    }

    /// Records a sync failure without exposing secrets.
    ///
    /// # Arguments
    ///
    /// * `scope` - Cursor direction.
    /// * `message` - Sanitized error message.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` after the error is stored.
    pub async fn record_error(&self, scope: &str, message: &str) -> Result<(), sqlx::Error> {
        self.get_or_create_state(scope).await?;
        sqlx::query(
            "UPDATE sync_state SET last_error_at = unixepoch(), last_error_message = ? \
             WHERE workspace_id = ? AND device_id = ? AND scope = ?",
        )
        .bind(message)
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(DEFAULT_DEVICE_ID)
        .bind(scope)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Lists sync state rows for status responses.
    ///
    /// # Returns
    ///
    /// Returns all local sync state rows.
    pub async fn list_states(&self) -> Result<Vec<SyncStateRecord>, sqlx::Error> {
        sqlx::query_as::<_, SyncStateRecord>(
            "SELECT workspace_id, device_id, scope, last_change_created_at, last_change_id, last_success_at, last_error_at, last_error_message \
             FROM sync_state WHERE workspace_id = ? AND device_id = ? ORDER BY scope ASC",
        )
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(DEFAULT_DEVICE_ID)
        .fetch_all(&self.pool)
        .await
    }

    /// Applies remote changes to local business tables.
    ///
    /// # Arguments
    ///
    /// * `changes` - Remote changes fetched from Supabase.
    ///
    /// # Returns
    ///
    /// Returns the number of newly applied changes.
    pub async fn apply_remote_changes(
        &self,
        changes: &[SyncChangeRecord],
    ) -> Result<usize, sqlx::Error> {
        let mut applied = 0;

        for change in changes {
            let mut transaction = self.pool.begin().await?;
            ensure_remote_device_in_transaction(&mut transaction, &change.device_id).await?;

            if sync_change_exists(&mut transaction, &change.id).await? {
                transaction.commit().await?;
                continue;
            }

            let payload = match serde_json::from_str::<Value>(&change.payload) {
                Ok(payload) => payload,
                Err(error) => {
                    record_schema_conflict_in_transaction(
                        &mut transaction,
                        change,
                        &format!("invalid payload JSON: {error}"),
                    )
                    .await?;
                    insert_remote_sync_change_in_transaction(&mut transaction, change).await?;
                    transaction.commit().await?;
                    applied += 1;
                    continue;
                }
            };

            if let Err(message) =
                apply_remote_change_in_transaction(&mut transaction, change, &payload).await
            {
                record_schema_conflict_in_transaction(&mut transaction, change, &message).await?;
            }

            insert_remote_sync_change_in_transaction(&mut transaction, change).await?;
            transaction.commit().await?;
            applied += 1;
        }

        Ok(applied)
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

/// Ensures a remote device exists before recording its changes.
///
/// # Arguments
///
/// * `transaction` - Open SQLite transaction.
/// * `device_id` - Remote device identifier.
///
/// # Returns
///
/// Returns `Ok(())` after the device row is present.
async fn ensure_remote_device_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    device_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT OR IGNORE INTO devices (id, workspace_id, name, platform, created_at, sync_enabled) \
         VALUES (?, ?, ?, 'remote', unixepoch(), 1)",
    )
    .bind(device_id)
    .bind(DEFAULT_WORKSPACE_ID)
    .bind(device_id)
    .execute(&mut **transaction)
    .await?;

    Ok(())
}

/// Checks whether a sync change has already been recorded locally.
///
/// # Arguments
///
/// * `transaction` - Open SQLite transaction.
/// * `change_id` - Change ID to look up.
///
/// # Returns
///
/// Returns `true` when the change already exists.
async fn sync_change_exists(
    transaction: &mut Transaction<'_, Sqlite>,
    change_id: &str,
) -> Result<bool, sqlx::Error> {
    let exists =
        sqlx::query_scalar::<_, i64>("SELECT EXISTS(SELECT 1 FROM sync_changes WHERE id = ?)")
            .bind(change_id)
            .fetch_one(&mut **transaction)
            .await?;

    Ok(exists == 1)
}

/// Inserts a remote sync change into the local change log.
///
/// # Arguments
///
/// * `transaction` - Open SQLite transaction.
/// * `change` - Remote change to store.
///
/// # Returns
///
/// Returns `Ok(())` after the change is stored.
async fn insert_remote_sync_change_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    change: &SyncChangeRecord,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT OR IGNORE INTO sync_changes \
         (id, workspace_id, device_id, entity_type, entity_id, operation, base_revision_id, new_revision_id, payload, created_at, applied_at, supabase_committed_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, unixepoch(), ?)",
    )
    .bind(&change.id)
    .bind(DEFAULT_WORKSPACE_ID)
    .bind(&change.device_id)
    .bind(&change.entity_type)
    .bind(&change.entity_id)
    .bind(&change.operation)
    .bind(change.base_revision_id.as_deref())
    .bind(change.new_revision_id.as_deref())
    .bind(&change.payload)
    .bind(change.created_at)
    .bind(change.supabase_committed_at)
    .execute(&mut **transaction)
    .await?;

    Ok(())
}

/// Applies a remote change payload to business tables.
///
/// # Arguments
///
/// * `transaction` - Open SQLite transaction.
/// * `change` - Remote change metadata.
/// * `payload` - Parsed remote payload.
///
/// # Returns
///
/// Returns `Ok(())` after applying supported changes, or an explanatory message
/// when the payload cannot be applied.
async fn apply_remote_change_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    change: &SyncChangeRecord,
    payload: &Value,
) -> Result<(), String> {
    match (change.entity_type.as_str(), change.operation.as_str()) {
        ("field", "insert") => {
            sqlx::query(
                "INSERT OR IGNORE INTO fields (id, workspace_id, name, created_at) VALUES (?, ?, ?, ?)",
            )
            .bind(required_text(payload, "id")?)
            .bind(DEFAULT_WORKSPACE_ID)
            .bind(required_text(payload, "name")?)
            .bind(required_i64(payload, "created_at")?)
            .execute(&mut **transaction)
            .await
            .map_err(|error| error.to_string())?;
        }
        ("tag", "insert") => {
            sqlx::query(
                "INSERT OR IGNORE INTO tags (id, workspace_id, name, created_at) VALUES (?, ?, ?, ?)",
            )
            .bind(required_text(payload, "id")?)
            .bind(DEFAULT_WORKSPACE_ID)
            .bind(required_text(payload, "name")?)
            .bind(required_i64(payload, "created_at")?)
            .execute(&mut **transaction)
            .await
            .map_err(|error| error.to_string())?;
        }
        ("note", "insert" | "update" | "delete" | "restore") => {
            sqlx::query(
                "INSERT INTO notes \
                 (id, workspace_id, content, role, field_id, created_at, updated_at, archived_at, deleted_at, current_revision_id, conflict_status) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'none') \
                 ON CONFLICT(id) DO UPDATE SET \
                 content = excluded.content, role = excluded.role, field_id = excluded.field_id, updated_at = excluded.updated_at, \
                 archived_at = excluded.archived_at, deleted_at = excluded.deleted_at, current_revision_id = excluded.current_revision_id",
            )
            .bind(required_text(payload, "id")?)
            .bind(DEFAULT_WORKSPACE_ID)
            .bind(required_text(payload, "content")?)
            .bind(required_text(payload, "role")?)
            .bind(optional_text(payload, "field_id"))
            .bind(required_i64(payload, "created_at")?)
            .bind(required_i64(payload, "updated_at")?)
            .bind(optional_i64(payload, "archived_at"))
            .bind(optional_i64(payload, "deleted_at"))
            .bind(optional_text(payload, "current_revision_id"))
            .execute(&mut **transaction)
            .await
            .map_err(|error| error.to_string())?;
        }
        ("note_revision", "insert") => {
            sqlx::query(
                "INSERT OR IGNORE INTO note_revisions \
                 (id, workspace_id, note_id, content, title, device_id, created_at, base_revision_id, change_id) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(required_text(payload, "id")?)
            .bind(DEFAULT_WORKSPACE_ID)
            .bind(required_text(payload, "note_id")?)
            .bind(required_text(payload, "content")?)
            .bind(optional_text(payload, "title"))
            .bind(optional_text(payload, "device_id"))
            .bind(required_i64(payload, "created_at")?)
            .bind(optional_text(payload, "base_revision_id"))
            .bind(&change.id)
            .execute(&mut **transaction)
            .await
            .map_err(|error| error.to_string())?;
            refresh_note_winner_revision(transaction, required_text(payload, "note_id")?).await?;
        }
        ("note_tag", "attach") => {
            sqlx::query(
                "INSERT OR IGNORE INTO note_tags (workspace_id, note_id, tag_id, created_at) VALUES (?, ?, ?, COALESCE(?, unixepoch()))",
            )
            .bind(DEFAULT_WORKSPACE_ID)
            .bind(required_text(payload, "note_id")?)
            .bind(required_text(payload, "tag_id")?)
            .bind(optional_i64(payload, "created_at"))
            .execute(&mut **transaction)
            .await
            .map_err(|error| error.to_string())?;
        }
        ("note_tag", "detach") => {
            sqlx::query(
                "DELETE FROM note_tags WHERE workspace_id = ? AND note_id = ? AND tag_id = ?",
            )
            .bind(DEFAULT_WORKSPACE_ID)
            .bind(required_text(payload, "note_id")?)
            .bind(required_text(payload, "tag_id")?)
            .execute(&mut **transaction)
            .await
            .map_err(|error| error.to_string())?;
        }
        _ => {
            return Err(format!(
                "unsupported remote change {} {}",
                change.entity_type, change.operation
            ));
        }
    }

    Ok(())
}

/// Refreshes a note's current revision using the deterministic winner rule.
///
/// # Arguments
///
/// * `transaction` - Open SQLite transaction.
/// * `note_id` - Note whose winner revision should be refreshed.
///
/// # Returns
///
/// Returns `Ok(())` after updating the note.
async fn refresh_note_winner_revision(
    transaction: &mut Transaction<'_, Sqlite>,
    note_id: &str,
) -> Result<(), String> {
    let winner = sqlx::query_scalar::<_, String>(
        "SELECT id FROM note_revisions \
         WHERE workspace_id = ? AND note_id = ? \
         ORDER BY created_at DESC, COALESCE(device_id, '') DESC, id DESC LIMIT 1",
    )
    .bind(DEFAULT_WORKSPACE_ID)
    .bind(note_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(|error| error.to_string())?;

    sqlx::query(
        "UPDATE notes SET current_revision_id = ?, updated_at = MAX(updated_at, unixepoch()) \
         WHERE workspace_id = ? AND id = ?",
    )
    .bind(winner)
    .bind(DEFAULT_WORKSPACE_ID)
    .bind(note_id)
    .execute(&mut **transaction)
    .await
    .map_err(|error| error.to_string())?;

    Ok(())
}

/// Records a schema incompatibility conflict.
///
/// # Arguments
///
/// * `transaction` - Open SQLite transaction.
/// * `change` - Change that could not be applied.
/// * `message` - Conflict explanation.
///
/// # Returns
///
/// Returns `Ok(())` after inserting the conflict row.
async fn record_schema_conflict_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    change: &SyncChangeRecord,
    message: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT OR IGNORE INTO sync_conflicts \
         (id, workspace_id, entity_type, entity_id, conflict_type, remote_change_id, status, created_at, resolution_note) \
         VALUES (?, ?, ?, ?, 'schema_incompatible', ?, 'open', unixepoch(), ?)",
    )
    .bind(new_id())
    .bind(DEFAULT_WORKSPACE_ID)
    .bind(&change.entity_type)
    .bind(&change.entity_id)
    .bind(&change.id)
    .bind(message)
    .execute(&mut **transaction)
    .await?;

    Ok(())
}

/// Reads a required string field from payload.
///
/// # Arguments
///
/// * `payload` - JSON payload to inspect.
/// * `field` - Field name to read.
///
/// # Returns
///
/// Returns the field value or an error message.
fn required_text<'a>(payload: &'a Value, field: &str) -> Result<&'a str, String> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("missing text field {field}"))
}

/// Reads an optional string field from payload.
///
/// # Arguments
///
/// * `payload` - JSON payload to inspect.
/// * `field` - Field name to read.
///
/// # Returns
///
/// Returns the optional field value.
fn optional_text<'a>(payload: &'a Value, field: &str) -> Option<&'a str> {
    payload.get(field).and_then(Value::as_str)
}

/// Reads a required integer field from payload.
///
/// # Arguments
///
/// * `payload` - JSON payload to inspect.
/// * `field` - Field name to read.
///
/// # Returns
///
/// Returns the field value or an error message.
fn required_i64(payload: &Value, field: &str) -> Result<i64, String> {
    payload
        .get(field)
        .and_then(Value::as_i64)
        .ok_or_else(|| format!("missing integer field {field}"))
}

/// Reads an optional integer field from payload.
///
/// # Arguments
///
/// * `payload` - JSON payload to inspect.
/// * `field` - Field name to read.
///
/// # Returns
///
/// Returns the optional field value.
fn optional_i64(payload: &Value, field: &str) -> Option<i64> {
    payload.get(field).and_then(Value::as_i64)
}

#[cfg(test)]
mod tests {
    use super::{SyncChangeRecord, SyncRepository};
    use crate::repositories::database::Database;

    /// Builds a remote sync change for tests.
    ///
    /// # Arguments
    ///
    /// * `id` - Change identifier.
    /// * `entity_type` - Entity type.
    /// * `entity_id` - Entity identifier.
    /// * `operation` - Change operation.
    /// * `payload` - JSON payload.
    ///
    /// # Returns
    ///
    /// Returns a sync change record.
    fn remote_change(
        id: &str,
        entity_type: &str,
        entity_id: &str,
        operation: &str,
        payload: serde_json::Value,
    ) -> SyncChangeRecord {
        SyncChangeRecord {
            id: id.to_string(),
            workspace_id: crate::repositories::taxonomy::DEFAULT_WORKSPACE_ID.to_string(),
            device_id: "remote-device".to_string(),
            entity_type: entity_type.to_string(),
            entity_id: entity_id.to_string(),
            operation: operation.to_string(),
            base_revision_id: None,
            new_revision_id: None,
            payload: payload.to_string(),
            created_at: 100,
            applied_at: None,
            supabase_committed_at: Some(101),
        }
    }

    #[tokio::test]
    async fn apply_remote_changes_is_idempotent() {
        let database = Database::connect("sqlite://:memory:").await.unwrap();
        let repository = SyncRepository::new(database.pool.clone());
        let changes = vec![remote_change(
            "remote-field-change",
            "field",
            "field-1",
            "insert",
            serde_json::json!({
                "id": "field-1",
                "name": "remote",
                "created_at": 100
            }),
        )];

        let first = repository.apply_remote_changes(&changes).await.unwrap();
        let second = repository.apply_remote_changes(&changes).await.unwrap();
        let count =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM fields WHERE id = 'field-1'")
                .fetch_one(&database.pool)
                .await
                .unwrap();

        assert_eq!(first, 1);
        assert_eq!(second, 0);
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn apply_remote_revision_selects_deterministic_winner() {
        let database = Database::connect("sqlite://:memory:").await.unwrap();
        let repository = SyncRepository::new(database.pool.clone());
        let note = remote_change(
            "remote-note-change",
            "note",
            "note-1",
            "insert",
            serde_json::json!({
                "id": "note-1",
                "content": "base",
                "role": "Human",
                "field_id": null,
                "created_at": 100,
                "updated_at": 100,
                "archived_at": null,
                "deleted_at": null,
                "current_revision_id": null
            }),
        );
        let older_revision = remote_change(
            "remote-revision-1-change",
            "note_revision",
            "revision-1",
            "insert",
            serde_json::json!({
                "id": "revision-1",
                "note_id": "note-1",
                "content": "older",
                "title": null,
                "device_id": "remote-device",
                "created_at": 100,
                "base_revision_id": null
            }),
        );
        let newer_revision = remote_change(
            "remote-revision-2-change",
            "note_revision",
            "revision-2",
            "insert",
            serde_json::json!({
                "id": "revision-2",
                "note_id": "note-1",
                "content": "newer",
                "title": null,
                "device_id": "remote-device",
                "created_at": 101,
                "base_revision_id": "revision-1"
            }),
        );

        repository
            .apply_remote_changes(&[note, older_revision, newer_revision])
            .await
            .unwrap();
        let current_revision_id = sqlx::query_scalar::<_, String>(
            "SELECT current_revision_id FROM notes WHERE id = 'note-1'",
        )
        .fetch_one(&database.pool)
        .await
        .unwrap();

        assert_eq!(current_revision_id, "revision-2");
    }
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
async fn update_state_success(
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
