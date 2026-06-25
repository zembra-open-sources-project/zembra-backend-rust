use sqlx::{Sqlite, Transaction};

use super::SyncRepository;
use crate::sync::table_snapshot::*;

impl SyncRepository {
    /// Writes a partial synchronized table snapshot into local SQLite.
    ///
    /// # Arguments
    ///
    /// * `snapshot` - Rows to upsert into local synchronized tables.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` after all rows are written in one transaction.
    pub async fn write_local_table_snapshot(
        &self,
        snapshot: &SyncTableSnapshot,
    ) -> Result<(), sqlx::Error> {
        let mut transaction = self.pool.begin().await?;

        for row in &snapshot.workspaces {
            upsert_workspace(&mut transaction, row).await?;
        }
        for row in &snapshot.devices {
            upsert_device(&mut transaction, row).await?;
        }
        for row in &snapshot.fields {
            upsert_field(&mut transaction, row).await?;
        }
        let mut tags = snapshot.tags.iter().collect::<Vec<_>>();
        tags.sort_by(|left, right| {
            left.depth
                .cmp(&right.depth)
                .then_with(|| left.id.cmp(&right.id))
        });
        for row in tags {
            upsert_tag(&mut transaction, row).await?;
        }
        for row in &snapshot.notes {
            upsert_note(&mut transaction, row).await?;
        }
        for row in &snapshot.note_revisions {
            upsert_note_revision(&mut transaction, row).await?;
        }
        for row in &snapshot.note_tags {
            upsert_note_tag(&mut transaction, row).await?;
        }
        for row in &snapshot.note_links {
            upsert_note_link(&mut transaction, row).await?;
        }
        for row in &snapshot.sync_changes {
            upsert_sync_change(&mut transaction, row).await?;
        }

        transaction.commit().await
    }
}

/// Upserts a workspace row.
async fn upsert_workspace(
    transaction: &mut Transaction<'_, Sqlite>,
    row: &WorkspaceSnapshotRow,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO workspaces (id, workspace_name, created_at, updated_at, archived_at, deleted_at) \
         VALUES (?, ?, ?, ?, ?, ?) \
         ON CONFLICT(id) DO UPDATE SET workspace_name = excluded.workspace_name, created_at = excluded.created_at, updated_at = excluded.updated_at, archived_at = excluded.archived_at, deleted_at = excluded.deleted_at",
    )
    .bind(&row.id)
    .bind(&row.workspace_name)
    .bind(row.created_at)
    .bind(row.updated_at)
    .bind(row.archived_at)
    .bind(row.deleted_at)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

/// Upserts a device row.
async fn upsert_device(
    transaction: &mut Transaction<'_, Sqlite>,
    row: &DeviceSnapshotRow,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO devices (id, workspace_id, name, platform, created_at, last_seen_at, sync_enabled, last_synced_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT(id) DO UPDATE SET workspace_id = excluded.workspace_id, name = excluded.name, platform = excluded.platform, created_at = excluded.created_at, last_seen_at = excluded.last_seen_at, sync_enabled = excluded.sync_enabled, last_synced_at = excluded.last_synced_at",
    )
    .bind(&row.id)
    .bind(&row.workspace_id)
    .bind(&row.name)
    .bind(&row.platform)
    .bind(row.created_at)
    .bind(row.last_seen_at)
    .bind(row.sync_enabled)
    .bind(row.last_synced_at)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

/// Upserts a field row.
async fn upsert_field(
    transaction: &mut Transaction<'_, Sqlite>,
    row: &FieldSnapshotRow,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO fields (id, workspace_id, name, created_at) VALUES (?, ?, ?, ?) \
         ON CONFLICT(id) DO UPDATE SET workspace_id = excluded.workspace_id, name = excluded.name, created_at = excluded.created_at",
    )
    .bind(&row.id)
    .bind(&row.workspace_id)
    .bind(&row.name)
    .bind(row.created_at)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

/// Upserts a tag row.
async fn upsert_tag(
    transaction: &mut Transaction<'_, Sqlite>,
    row: &TagSnapshotRow,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO tags (id, workspace_id, name, parent_tag_id, path, depth, created_at) VALUES (?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT(id) DO UPDATE SET workspace_id = excluded.workspace_id, name = excluded.name, parent_tag_id = excluded.parent_tag_id, path = excluded.path, depth = excluded.depth, created_at = excluded.created_at",
    )
    .bind(&row.id)
    .bind(&row.workspace_id)
    .bind(&row.name)
    .bind(&row.parent_tag_id)
    .bind(&row.path)
    .bind(row.depth)
    .bind(row.created_at)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

/// Upserts a note row.
async fn upsert_note(
    transaction: &mut Transaction<'_, Sqlite>,
    row: &NoteSnapshotRow,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO notes (id, workspace_id, content, role, field_id, created_at, updated_at, archived_at, deleted_at, current_revision_id, last_change_id, conflict_status) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT(id) DO UPDATE SET workspace_id = excluded.workspace_id, content = excluded.content, role = excluded.role, field_id = excluded.field_id, created_at = excluded.created_at, updated_at = excluded.updated_at, archived_at = excluded.archived_at, deleted_at = excluded.deleted_at, current_revision_id = excluded.current_revision_id, last_change_id = excluded.last_change_id, conflict_status = excluded.conflict_status",
    )
    .bind(&row.id)
    .bind(&row.workspace_id)
    .bind(&row.content)
    .bind(&row.role)
    .bind(&row.field_id)
    .bind(row.created_at)
    .bind(row.updated_at)
    .bind(row.archived_at)
    .bind(row.deleted_at)
    .bind(&row.current_revision_id)
    .bind(&row.last_change_id)
    .bind(&row.conflict_status)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

/// Upserts a note revision row.
async fn upsert_note_revision(
    transaction: &mut Transaction<'_, Sqlite>,
    row: &NoteRevisionSnapshotRow,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO note_revisions (id, workspace_id, note_id, content, title, device_id, created_at, base_revision_id, change_id) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT(id) DO UPDATE SET workspace_id = excluded.workspace_id, note_id = excluded.note_id, content = excluded.content, title = excluded.title, device_id = excluded.device_id, created_at = excluded.created_at, base_revision_id = excluded.base_revision_id, change_id = excluded.change_id",
    )
    .bind(&row.id)
    .bind(&row.workspace_id)
    .bind(&row.note_id)
    .bind(&row.content)
    .bind(&row.title)
    .bind(&row.device_id)
    .bind(row.created_at)
    .bind(&row.base_revision_id)
    .bind(&row.change_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

/// Upserts a note tag relation row.
async fn upsert_note_tag(
    transaction: &mut Transaction<'_, Sqlite>,
    row: &NoteTagSnapshotRow,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO note_tags (workspace_id, note_id, tag_id, created_at) VALUES (?, ?, ?, ?) \
         ON CONFLICT(workspace_id, note_id, tag_id) DO UPDATE SET created_at = excluded.created_at",
    )
    .bind(&row.workspace_id)
    .bind(&row.note_id)
    .bind(&row.tag_id)
    .bind(row.created_at)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

/// Upserts a note link row.
async fn upsert_note_link(
    transaction: &mut Transaction<'_, Sqlite>,
    row: &NoteLinkSnapshotRow,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO note_links (id, workspace_id, source_note_id, target_note_id, anchor_text, position, created_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT(id) DO UPDATE SET workspace_id = excluded.workspace_id, source_note_id = excluded.source_note_id, target_note_id = excluded.target_note_id, anchor_text = excluded.anchor_text, position = excluded.position, created_at = excluded.created_at",
    )
    .bind(&row.id)
    .bind(&row.workspace_id)
    .bind(&row.source_note_id)
    .bind(&row.target_note_id)
    .bind(&row.anchor_text)
    .bind(row.position)
    .bind(row.created_at)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

/// Upserts a sync change row.
async fn upsert_sync_change(
    transaction: &mut Transaction<'_, Sqlite>,
    row: &SyncChangeSnapshotRow,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO sync_changes (id, workspace_id, device_id, entity_type, entity_id, operation, base_revision_id, new_revision_id, payload, created_at, applied_at, supabase_committed_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT(id) DO UPDATE SET workspace_id = excluded.workspace_id, device_id = excluded.device_id, entity_type = excluded.entity_type, entity_id = excluded.entity_id, operation = excluded.operation, base_revision_id = excluded.base_revision_id, new_revision_id = excluded.new_revision_id, payload = excluded.payload, created_at = excluded.created_at, applied_at = excluded.applied_at, supabase_committed_at = excluded.supabase_committed_at",
    )
    .bind(&row.id)
    .bind(&row.workspace_id)
    .bind(&row.device_id)
    .bind(&row.entity_type)
    .bind(&row.entity_id)
    .bind(&row.operation)
    .bind(&row.base_revision_id)
    .bind(&row.new_revision_id)
    .bind(&row.payload)
    .bind(row.created_at)
    .bind(row.applied_at)
    .bind(row.supabase_committed_at)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}
