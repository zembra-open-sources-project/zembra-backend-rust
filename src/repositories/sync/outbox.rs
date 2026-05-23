use super::{SyncChangeRecord, SyncRepository, update_state_success};
use crate::repositories::taxonomy::DEFAULT_WORKSPACE_ID;

impl SyncRepository {
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
}
