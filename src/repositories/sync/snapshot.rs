use super::SyncRepository;
use crate::sync::table_snapshot::{
    DeviceSnapshotRow, FieldSnapshotRow, NoteLinkSnapshotRow, NoteRevisionSnapshotRow,
    NoteSnapshotRow, NoteTagSnapshotRow, SyncChangeSnapshotRow, SyncTableSnapshot, TagSnapshotRow,
    WorkspaceSnapshotRow,
};

impl SyncRepository {
    /// Reads the current local rows for all synchronized tables.
    ///
    /// # Returns
    ///
    /// Returns a stable snapshot of the nine synchronized tables.
    pub async fn read_local_table_snapshot(&self) -> Result<SyncTableSnapshot, sqlx::Error> {
        Ok(SyncTableSnapshot {
            workspaces: self.read_workspace_snapshot().await?,
            devices: self.read_device_snapshot().await?,
            fields: self.read_field_snapshot().await?,
            tags: self.read_tag_snapshot().await?,
            notes: self.read_note_snapshot().await?,
            note_revisions: self.read_note_revision_snapshot().await?,
            note_tags: self.read_note_tag_snapshot().await?,
            note_links: self.read_note_link_snapshot().await?,
            sync_changes: self.read_sync_change_snapshot().await?,
        })
    }

    /// Reads workspace rows in primary-key order.
    ///
    /// # Returns
    ///
    /// Returns workspace snapshot rows.
    async fn read_workspace_snapshot(&self) -> Result<Vec<WorkspaceSnapshotRow>, sqlx::Error> {
        sqlx::query_as::<_, WorkspaceSnapshotRow>(
            "SELECT id, workspace_name, created_at, updated_at, archived_at, deleted_at \
             FROM workspaces ORDER BY id ASC",
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Reads device rows in primary-key order.
    ///
    /// # Returns
    ///
    /// Returns device snapshot rows.
    async fn read_device_snapshot(&self) -> Result<Vec<DeviceSnapshotRow>, sqlx::Error> {
        sqlx::query_as::<_, DeviceSnapshotRow>(
            "SELECT id, workspace_id, name, platform, created_at, last_seen_at, sync_enabled, last_synced_at \
             FROM devices ORDER BY id ASC",
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Reads field rows in primary-key order.
    ///
    /// # Returns
    ///
    /// Returns field snapshot rows.
    async fn read_field_snapshot(&self) -> Result<Vec<FieldSnapshotRow>, sqlx::Error> {
        sqlx::query_as::<_, FieldSnapshotRow>(
            "SELECT id, workspace_id, name, created_at FROM fields ORDER BY id ASC",
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Reads tag rows in primary-key order.
    ///
    /// # Returns
    ///
    /// Returns tag snapshot rows.
    async fn read_tag_snapshot(&self) -> Result<Vec<TagSnapshotRow>, sqlx::Error> {
        sqlx::query_as::<_, TagSnapshotRow>(
            "SELECT id, workspace_id, name, parent_tag_id, path, depth, created_at \
             FROM tags ORDER BY id ASC",
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Reads note rows in primary-key order.
    ///
    /// # Returns
    ///
    /// Returns note snapshot rows including hidden note state.
    async fn read_note_snapshot(&self) -> Result<Vec<NoteSnapshotRow>, sqlx::Error> {
        sqlx::query_as::<_, NoteSnapshotRow>(
            "SELECT id, workspace_id, content, role, field_id, created_at, updated_at, archived_at, deleted_at, current_revision_id, last_change_id, conflict_status \
             FROM notes ORDER BY id ASC",
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Reads note revision rows in primary-key order.
    ///
    /// # Returns
    ///
    /// Returns note revision snapshot rows.
    async fn read_note_revision_snapshot(
        &self,
    ) -> Result<Vec<NoteRevisionSnapshotRow>, sqlx::Error> {
        sqlx::query_as::<_, NoteRevisionSnapshotRow>(
            "SELECT id, workspace_id, note_id, content, title, device_id, created_at, base_revision_id, change_id \
             FROM note_revisions ORDER BY id ASC",
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Reads note tag relation rows in composite-key order.
    ///
    /// # Returns
    ///
    /// Returns note tag snapshot rows.
    async fn read_note_tag_snapshot(&self) -> Result<Vec<NoteTagSnapshotRow>, sqlx::Error> {
        sqlx::query_as::<_, NoteTagSnapshotRow>(
            "SELECT workspace_id, note_id, tag_id, created_at \
             FROM note_tags ORDER BY workspace_id ASC, note_id ASC, tag_id ASC",
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Reads note link rows in primary-key order.
    ///
    /// # Returns
    ///
    /// Returns note link snapshot rows.
    async fn read_note_link_snapshot(&self) -> Result<Vec<NoteLinkSnapshotRow>, sqlx::Error> {
        sqlx::query_as::<_, NoteLinkSnapshotRow>(
            "SELECT id, workspace_id, source_note_id, target_note_id, anchor_text, position, created_at \
             FROM note_links ORDER BY id ASC",
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Reads sync change rows in synchronization order.
    ///
    /// # Returns
    ///
    /// Returns sync change snapshot rows.
    async fn read_sync_change_snapshot(&self) -> Result<Vec<SyncChangeSnapshotRow>, sqlx::Error> {
        sqlx::query_as::<_, SyncChangeSnapshotRow>(
            "SELECT id, workspace_id, device_id, entity_type, entity_id, operation, base_revision_id, new_revision_id, payload, created_at, applied_at, supabase_committed_at \
             FROM sync_changes ORDER BY created_at ASC, id ASC",
        )
        .fetch_all(&self.pool)
        .await
    }
}
