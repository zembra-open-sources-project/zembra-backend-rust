use sqlx::{Sqlite, Transaction};

use super::core::NotesRepository;
use crate::error::ApiError;
use crate::models::revision::NoteRevisionRecord;

impl NotesRepository {
    /// Lists revisions for a note ordered by creation time.
    ///
    /// # Arguments
    ///
    /// * `note_ref` - Full note ID or unique prefix.
    ///
    /// # Returns
    ///
    /// Returns revision records for the note.
    pub async fn list_note_revisions(
        &self,
        workspace_id: &str,
        note_ref: &str,
    ) -> Result<Vec<NoteRevisionRecord>, ApiError> {
        let note = self.get_note_by_ref(workspace_id, note_ref).await?;

        sqlx::query_as::<_, NoteRevisionRecord>(
            "SELECT id, note_id, content, title, device_id, created_at \
             FROM note_revisions WHERE workspace_id = ? AND note_id = ? ORDER BY created_at ASC, rowid ASC",
        )
        .bind(workspace_id)
        .bind(note.id)
        .fetch_all(&self.pool)
        .await
        .map_err(ApiError::from)
    }
}

/// Inserts a note revision inside an existing transaction.
///
/// # Arguments
///
/// * `transaction` - Open SQLite transaction.
/// * `revision_id` - Exact revision identifier.
/// * `note_id` - Exact note identifier.
/// * `content` - Revision content.
/// * `device_id` - Optional device identifier.
///
/// # Returns
///
/// Returns `Ok(())` after the revision is inserted.
pub(super) async fn insert_note_revision_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    workspace_id: &str,
    revision_id: &str,
    note_id: &str,
    content: &str,
    device_id: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO note_revisions (id, workspace_id, note_id, content, title, device_id, created_at) \
         VALUES (?, ?, ?, ?, NULL, ?, unixepoch())",
    )
    .bind(revision_id)
    .bind(workspace_id)
    .bind(note_id)
    .bind(content)
    .bind(device_id)
    .execute(&mut **transaction)
    .await?;

    Ok(())
}
