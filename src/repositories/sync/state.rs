use super::DEFAULT_DEVICE_ID;
use super::{
    SyncRepository, SyncStateRecord, ensure_default_device_in_transaction, update_state_success,
};
use crate::repositories::taxonomy::DEFAULT_WORKSPACE_ID;

impl SyncRepository {
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
}
