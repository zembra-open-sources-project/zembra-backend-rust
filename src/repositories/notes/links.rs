use sqlx::{Sqlite, Transaction};

use super::core::{
    NotesRepository, resolve_visible_note_by_ref_in_transaction, select_note_link_by_id,
};
use super::payloads::note_link_payload;
use super::types::NoteLinkInput;
use crate::error::ApiError;
use crate::models::note_link::NoteLinkRecord;
use crate::repositories::sync::{SyncChangeInput, record_sync_change_in_transaction};
use crate::repositories::taxonomy::new_id;

impl NotesRepository {
    /// Lists visible outgoing links for a note.
    ///
    /// # Arguments
    ///
    /// * `note_id` - Exact note identifier.
    ///
    /// # Returns
    ///
    /// Returns links whose target notes are non-deleted and non-archived.
    pub async fn list_visible_outgoing_links(
        &self,
        workspace_id: &str,
        note_id: &str,
    ) -> Result<Vec<NoteLinkRecord>, ApiError> {
        sqlx::query_as::<_, NoteLinkRecord>(
            "SELECT note_links.id, note_links.source_note_id, note_links.target_note_id, note_links.anchor_text, note_links.position, note_links.created_at \
             FROM note_links \
             INNER JOIN notes target ON note_links.workspace_id = target.workspace_id AND note_links.target_note_id = target.id \
             WHERE note_links.workspace_id = ? \
             AND note_links.source_note_id = ? \
             AND target.deleted_at IS NULL \
             AND target.archived_at IS NULL \
             ORDER BY note_links.position ASC, note_links.created_at ASC, note_links.id ASC",
        )
        .bind(workspace_id)
        .bind(note_id)
        .fetch_all(&self.pool)
        .await
        .map_err(ApiError::from)
    }

    /// Lists visible backlinks for a note.
    ///
    /// # Arguments
    ///
    /// * `note_id` - Exact note identifier.
    ///
    /// # Returns
    ///
    /// Returns links whose source notes are non-deleted and non-archived.
    pub async fn list_visible_backlinks(
        &self,
        workspace_id: &str,
        note_id: &str,
    ) -> Result<Vec<NoteLinkRecord>, ApiError> {
        sqlx::query_as::<_, NoteLinkRecord>(
            "SELECT note_links.id, note_links.source_note_id, note_links.target_note_id, note_links.anchor_text, note_links.position, note_links.created_at \
             FROM note_links \
             INNER JOIN notes source ON note_links.workspace_id = source.workspace_id AND note_links.source_note_id = source.id \
             WHERE note_links.workspace_id = ? \
             AND note_links.target_note_id = ? \
             AND source.deleted_at IS NULL \
             AND source.archived_at IS NULL \
             ORDER BY note_links.created_at ASC, note_links.id ASC",
        )
        .bind(workspace_id)
        .bind(note_id)
        .fetch_all(&self.pool)
        .await
        .map_err(ApiError::from)
    }
}

/// Inserts outgoing note links inside an existing transaction.
///
/// # Arguments
///
/// * `transaction` - Open SQLite transaction.
/// * `source_note_id` - Exact source note identifier.
/// * `links` - Normalized link inputs.
///
/// # Returns
///
/// Returns persisted link records.
pub(super) async fn insert_note_links_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    workspace_id: &str,
    source_note_id: &str,
    links: &[NoteLinkInput],
) -> Result<Vec<NoteLinkRecord>, ApiError> {
    let mut created = Vec::with_capacity(links.len());

    for link in links {
        let target = resolve_visible_note_by_ref_in_transaction(
            transaction,
            workspace_id,
            &link.target_note_ref,
        )
        .await?;
        if target.id == source_note_id {
            return Err(ApiError::Validation);
        }

        let link_id = new_id();
        sqlx::query(
            "INSERT INTO note_links \
             (id, workspace_id, source_note_id, target_note_id, anchor_text, position, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, unixepoch())",
        )
        .bind(&link_id)
        .bind(workspace_id)
        .bind(source_note_id)
        .bind(&target.id)
        .bind(link.anchor_text.as_deref())
        .bind(link.position)
        .execute(&mut **transaction)
        .await?;

        let record = select_note_link_by_id(transaction, workspace_id, &link_id).await?;
        record_sync_change_in_transaction(
            transaction,
            SyncChangeInput {
                workspace_id: workspace_id.to_string(),
                entity_type: "note_link",
                entity_id: record.id.clone(),
                operation: "attach",
                base_revision_id: None,
                new_revision_id: None,
                payload: note_link_payload(workspace_id, &record),
            },
        )
        .await?;
        created.push(record);
    }

    Ok(created)
}

/// Replaces outgoing note links inside an existing transaction.
///
/// # Arguments
///
/// * `transaction` - Open SQLite transaction.
/// * `source_note_id` - Exact source note identifier.
/// * `links` - Normalized final links for the source note.
///
/// # Returns
///
/// Returns `Ok(())` after link associations and sync changes are updated.
pub(super) async fn replace_note_links_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    workspace_id: &str,
    source_note_id: &str,
    links: &[NoteLinkInput],
) -> Result<(), ApiError> {
    let current_links = sqlx::query_as::<_, NoteLinkRecord>(
        "SELECT id, source_note_id, target_note_id, anchor_text, position, created_at \
         FROM note_links WHERE workspace_id = ? AND source_note_id = ?",
    )
    .bind(workspace_id)
    .bind(source_note_id)
    .fetch_all(&mut **transaction)
    .await?;

    for link in current_links {
        sqlx::query("DELETE FROM note_links WHERE workspace_id = ? AND id = ?")
            .bind(workspace_id)
            .bind(&link.id)
            .execute(&mut **transaction)
            .await?;
        record_sync_change_in_transaction(
            transaction,
            SyncChangeInput {
                workspace_id: workspace_id.to_string(),
                entity_type: "note_link",
                entity_id: link.id.clone(),
                operation: "detach",
                base_revision_id: None,
                new_revision_id: None,
                payload: note_link_payload(workspace_id, &link),
            },
        )
        .await?;
    }

    insert_note_links_in_transaction(transaction, workspace_id, source_note_id, links).await?;

    Ok(())
}
