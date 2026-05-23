use serde_json::Value;
use sqlx::{Sqlite, Transaction};

use super::{SyncChangeRecord, SyncRepository};
use crate::repositories::taxonomy::{DEFAULT_WORKSPACE_ID, new_id};

impl SyncRepository {
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
        ("note_link", "attach") => {
            sqlx::query(
                "INSERT OR IGNORE INTO note_links \
                 (id, workspace_id, source_note_id, target_note_id, anchor_text, position, created_at) \
                 VALUES (?, ?, ?, ?, ?, ?, COALESCE(?, unixepoch()))",
            )
            .bind(required_text(payload, "id")?)
            .bind(DEFAULT_WORKSPACE_ID)
            .bind(required_text(payload, "source_note_id")?)
            .bind(required_text(payload, "target_note_id")?)
            .bind(optional_text(payload, "anchor_text"))
            .bind(optional_i64(payload, "position"))
            .bind(optional_i64(payload, "created_at"))
            .execute(&mut **transaction)
            .await
            .map_err(|error| error.to_string())?;
        }
        ("note_link", "detach") => {
            sqlx::query("DELETE FROM note_links WHERE workspace_id = ? AND id = ?")
                .bind(DEFAULT_WORKSPACE_ID)
                .bind(required_text(payload, "id")?)
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
