use std::collections::HashSet;

use sqlx::{Sqlite, Transaction};

use super::core::NotesRepository;
use super::payloads::{note_tag_entity_id, note_tag_payload};
use crate::error::ApiError;
use crate::models::note::NoteRecord;
use crate::models::tag::TagRecord;
use crate::repositories::sync::{SyncChangeInput, record_sync_change_in_transaction};
use crate::repositories::taxonomy::{DEFAULT_WORKSPACE_ID, get_or_create_tag_in_transaction};

impl NotesRepository {
    /// Lists tags associated with a note.
    ///
    /// # Arguments
    ///
    /// * `note_ref` - Full note ID or unique prefix.
    ///
    /// # Returns
    ///
    /// Returns tag records ordered by name.
    pub async fn list_note_tags(&self, note_ref: &str) -> Result<Vec<TagRecord>, ApiError> {
        let note = self.get_note_by_ref(note_ref).await?;

        self.list_note_tags_by_id(&note.id).await
    }

    /// Lists tags associated with a note ID.
    ///
    /// # Arguments
    ///
    /// * `note_id` - Exact note identifier.
    ///
    /// # Returns
    ///
    /// Returns tag records ordered by name.
    pub async fn list_note_tags_by_id(&self, note_id: &str) -> Result<Vec<TagRecord>, ApiError> {
        sqlx::query_as::<_, TagRecord>(
            "SELECT tags.id, tags.name, tags.parent_tag_id, tags.path, tags.depth, tags.created_at \
             FROM tags INNER JOIN note_tags ON tags.workspace_id = note_tags.workspace_id AND tags.id = note_tags.tag_id \
             WHERE note_tags.workspace_id = ? AND note_tags.note_id = ? ORDER BY tags.path ASC",
        )
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(note_id)
        .fetch_all(&self.pool)
        .await
        .map_err(ApiError::from)
    }

    /// Adds a tag association to a note.
    ///
    /// # Arguments
    ///
    /// * `note_ref` - Full note ID or unique prefix.
    /// * `tag_name` - Normalized tag name.
    ///
    /// # Returns
    ///
    /// Returns the tag associated with the note.
    pub async fn add_tag_to_note(
        &self,
        note_ref: &str,
        tag_name: &str,
    ) -> Result<TagRecord, ApiError> {
        let note = self.get_note_by_ref(note_ref).await?;
        let mut transaction = self.pool.begin().await?;
        let tag = attach_tag_to_note_in_transaction(&mut transaction, &note, tag_name).await?;

        transaction.commit().await?;
        Ok(tag)
    }

    /// Removes a tag association from a note.
    ///
    /// # Arguments
    ///
    /// * `note_ref` - Full note ID or unique prefix.
    /// * `tag_name` - Tag name to remove.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` after the association is removed.
    pub async fn remove_tag_from_note(
        &self,
        note_ref: &str,
        tag_name: &str,
    ) -> Result<(), ApiError> {
        let note = self.get_note_by_ref(note_ref).await?;
        let mut transaction = self.pool.begin().await?;
        detach_tag_from_note_in_transaction(&mut transaction, &note, tag_name).await?;
        transaction.commit().await?;

        Ok(())
    }
}

/// Adds a tag association to a note inside an existing transaction.
///
/// # Arguments
///
/// * `transaction` - Open SQLite transaction.
/// * `note` - Note receiving the tag.
/// * `tag_name` - Normalized tag name.
///
/// # Returns
///
/// Returns the resolved tag record.
pub(super) async fn attach_tag_to_note_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    note: &NoteRecord,
    tag_name: &str,
) -> Result<TagRecord, ApiError> {
    let tag = get_or_create_tag_in_transaction(transaction, tag_name).await?;

    let result = sqlx::query(
        "INSERT OR IGNORE INTO note_tags (workspace_id, note_id, tag_id, created_at) VALUES (?, ?, ?, unixepoch())",
    )
    .bind(DEFAULT_WORKSPACE_ID)
    .bind(&note.id)
    .bind(&tag.id)
    .execute(&mut **transaction)
    .await?;

    if result.rows_affected() > 0 {
        record_sync_change_in_transaction(
            transaction,
            SyncChangeInput {
                entity_type: "note_tag",
                entity_id: note_tag_entity_id(&note.id, &tag.id),
                operation: "attach",
                base_revision_id: None,
                new_revision_id: None,
                payload: note_tag_payload(&note.id, &tag.id),
            },
        )
        .await?;
    }

    Ok(tag)
}

/// Removes a tag association from a note inside an existing transaction.
///
/// # Arguments
///
/// * `transaction` - Open SQLite transaction.
/// * `note` - Note losing the tag association.
/// * `tag_name` - Tag name to remove.
///
/// # Returns
///
/// Returns `Ok(())` after the association is removed.
async fn detach_tag_from_note_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    note: &NoteRecord,
    tag_name: &str,
) -> Result<(), ApiError> {
    let tag_id =
        sqlx::query_scalar::<_, String>("SELECT id FROM tags WHERE workspace_id = ? AND path = ?")
            .bind(DEFAULT_WORKSPACE_ID)
            .bind(tag_name)
            .fetch_optional(&mut **transaction)
            .await?;

    if let Some(tag_id) = tag_id {
        let result = sqlx::query(
            "DELETE FROM note_tags WHERE workspace_id = ? AND note_id = ? AND tag_id = ?",
        )
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(&note.id)
        .bind(&tag_id)
        .execute(&mut **transaction)
        .await?;

        if result.rows_affected() > 0 {
            record_sync_change_in_transaction(
                transaction,
                SyncChangeInput {
                    entity_type: "note_tag",
                    entity_id: note_tag_entity_id(&note.id, &tag_id),
                    operation: "detach",
                    base_revision_id: None,
                    new_revision_id: None,
                    payload: note_tag_payload(&note.id, &tag_id),
                },
            )
            .await?;
        }
    }

    Ok(())
}

/// Replaces all tag associations for a note inside an existing transaction.
///
/// # Arguments
///
/// * `transaction` - Open SQLite transaction.
/// * `note_id` - Exact note identifier.
/// * `tag_names` - Normalized final tag names for the note.
///
/// # Returns
///
/// Returns `Ok(())` after tag associations and sync changes are updated.
pub(super) async fn replace_note_tags_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    note_id: &str,
    tag_names: &[String],
) -> Result<(), ApiError> {
    let current_tags = sqlx::query_as::<_, TagRecord>(
        "SELECT tags.id, tags.name, tags.parent_tag_id, tags.path, tags.depth, tags.created_at \
         FROM tags INNER JOIN note_tags ON tags.workspace_id = note_tags.workspace_id AND tags.id = note_tags.tag_id \
         WHERE note_tags.workspace_id = ? AND note_tags.note_id = ?",
    )
    .bind(DEFAULT_WORKSPACE_ID)
    .bind(note_id)
    .fetch_all(&mut **transaction)
    .await?;
    let current_ids = current_tags
        .iter()
        .map(|tag| tag.id.clone())
        .collect::<HashSet<_>>();

    let mut target_tags = Vec::with_capacity(tag_names.len());
    for tag_name in tag_names {
        target_tags.push(get_or_create_tag_in_transaction(transaction, tag_name).await?);
    }
    let target_ids = target_tags
        .iter()
        .map(|tag| tag.id.clone())
        .collect::<HashSet<_>>();

    for tag in &target_tags {
        if !current_ids.contains(&tag.id) {
            sqlx::query(
                "INSERT OR IGNORE INTO note_tags (workspace_id, note_id, tag_id, created_at) VALUES (?, ?, ?, unixepoch())",
            )
            .bind(DEFAULT_WORKSPACE_ID)
            .bind(note_id)
            .bind(&tag.id)
            .execute(&mut **transaction)
            .await?;
            record_sync_change_in_transaction(
                transaction,
                SyncChangeInput {
                    entity_type: "note_tag",
                    entity_id: note_tag_entity_id(note_id, &tag.id),
                    operation: "attach",
                    base_revision_id: None,
                    new_revision_id: None,
                    payload: note_tag_payload(note_id, &tag.id),
                },
            )
            .await?;
        }
    }

    for tag in &current_tags {
        if !target_ids.contains(&tag.id) {
            sqlx::query(
                "DELETE FROM note_tags WHERE workspace_id = ? AND note_id = ? AND tag_id = ?",
            )
            .bind(DEFAULT_WORKSPACE_ID)
            .bind(note_id)
            .bind(&tag.id)
            .execute(&mut **transaction)
            .await?;
            record_sync_change_in_transaction(
                transaction,
                SyncChangeInput {
                    entity_type: "note_tag",
                    entity_id: note_tag_entity_id(note_id, &tag.id),
                    operation: "detach",
                    base_revision_id: None,
                    new_revision_id: None,
                    payload: note_tag_payload(note_id, &tag.id),
                },
            )
            .await?;
        }
    }

    Ok(())
}
