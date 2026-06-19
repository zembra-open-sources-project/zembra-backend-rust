use std::collections::HashMap;

use crate::sync::table_snapshot::{NoteTagSnapshotRow, SyncChangeSnapshotRow, SyncTableSnapshot};

/// Direction for one table row synchronization operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncWriteTarget {
    /// Write the row to local SQLite.
    Local,
    /// Write the row to Supabase.
    Remote,
}

/// Synchronized table name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SyncTableName {
    /// `workspaces` table.
    Workspaces,
    /// `devices` table.
    Devices,
    /// `fields` table.
    Fields,
    /// `tags` table.
    Tags,
    /// `notes` table.
    Notes,
    /// `note_revisions` table.
    NoteRevisions,
    /// `note_tags` table.
    NoteTags,
    /// `note_links` table.
    NoteLinks,
    /// `sync_changes` table.
    SyncChanges,
}

impl SyncTableName {
    /// Returns the stable write order for synchronized tables.
    ///
    /// # Returns
    ///
    /// Returns the foreign-key-safe write order.
    fn write_order(self) -> usize {
        match self {
            Self::Workspaces => 0,
            Self::Devices => 1,
            Self::Fields => 2,
            Self::Tags => 3,
            Self::Notes => 4,
            Self::NoteRevisions => 5,
            Self::NoteTags => 6,
            Self::NoteLinks => 7,
            Self::SyncChanges => 8,
        }
    }
}

/// One row-level difference between local SQLite and Supabase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncTableDiff {
    /// Table that contains the differing row.
    pub table: SyncTableName,
    /// Stable primary or composite key string for the row.
    pub key: String,
    /// Target side that should receive the row.
    pub target: SyncWriteTarget,
}

/// Conflict that cannot be resolved by `sync_changes.created_at`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncDiffConflict {
    /// Table that contains the unresolved conflict.
    pub table: SyncTableName,
    /// Stable primary or composite key string for the row.
    pub key: String,
    /// Human-readable conflict reason safe for logs or local conflict rows.
    pub reason: String,
}

/// Complete comparison result for local and remote snapshots.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncSnapshotDiff {
    /// Differences that should be written to local SQLite.
    pub write_local: Vec<SyncTableDiff>,
    /// Differences that should be written to Supabase.
    pub write_remote: Vec<SyncTableDiff>,
    /// Conflicts that require stopping or explicit conflict recording.
    pub conflicts: Vec<SyncDiffConflict>,
}

/// Compares local and remote synchronized table snapshots.
///
/// # Arguments
///
/// * `local` - Local SQLite table snapshot.
/// * `remote` - Supabase table snapshot.
///
/// # Returns
///
/// Returns row-level differences ordered by safe write order.
pub fn diff_snapshots(local: &SyncTableSnapshot, remote: &SyncTableSnapshot) -> SyncSnapshotDiff {
    let mut diff = SyncSnapshotDiff::default();
    let local_changes = latest_changes(&local.sync_changes);
    let remote_changes = latest_changes(&remote.sync_changes);

    compare_rows(
        &mut diff,
        SyncTableName::Workspaces,
        &local.workspaces,
        &remote.workspaces,
        |row| row.id.clone(),
        "workspace",
        &local_changes,
        &remote_changes,
    );
    compare_rows(
        &mut diff,
        SyncTableName::Devices,
        &local.devices,
        &remote.devices,
        |row| row.id.clone(),
        "device",
        &local_changes,
        &remote_changes,
    );
    compare_rows(
        &mut diff,
        SyncTableName::Fields,
        &local.fields,
        &remote.fields,
        |row| row.id.clone(),
        "field",
        &local_changes,
        &remote_changes,
    );
    compare_rows(
        &mut diff,
        SyncTableName::Tags,
        &local.tags,
        &remote.tags,
        |row| row.id.clone(),
        "tag",
        &local_changes,
        &remote_changes,
    );
    compare_rows(
        &mut diff,
        SyncTableName::Notes,
        &local.notes,
        &remote.notes,
        |row| row.id.clone(),
        "note",
        &local_changes,
        &remote_changes,
    );
    compare_rows(
        &mut diff,
        SyncTableName::NoteRevisions,
        &local.note_revisions,
        &remote.note_revisions,
        |row| row.id.clone(),
        "note_revision",
        &local_changes,
        &remote_changes,
    );
    compare_rows(
        &mut diff,
        SyncTableName::NoteTags,
        &local.note_tags,
        &remote.note_tags,
        note_tag_key,
        "note_tag",
        &local_changes,
        &remote_changes,
    );
    compare_rows(
        &mut diff,
        SyncTableName::NoteLinks,
        &local.note_links,
        &remote.note_links,
        |row| row.id.clone(),
        "note_link",
        &local_changes,
        &remote_changes,
    );
    compare_rows(
        &mut diff,
        SyncTableName::SyncChanges,
        &local.sync_changes,
        &remote.sync_changes,
        |row| row.id.clone(),
        "sync_change",
        &local_changes,
        &remote_changes,
    );

    sort_diffs(&mut diff.write_local);
    sort_diffs(&mut diff.write_remote);
    diff
}

/// Compares one table's local and remote rows.
///
/// # Arguments
///
/// * `diff` - Accumulated difference result.
/// * `table` - Table being compared.
/// * `local_rows` - Local rows.
/// * `remote_rows` - Remote rows.
/// * `key_fn` - Function that returns a stable row key.
/// * `entity_type` - Sync change entity type for freshness lookup.
/// * `local_changes` - Latest local changes by entity key.
/// * `remote_changes` - Latest remote changes by entity key.
fn compare_rows<T, F>(
    diff: &mut SyncSnapshotDiff,
    table: SyncTableName,
    local_rows: &[T],
    remote_rows: &[T],
    key_fn: F,
    entity_type: &str,
    local_changes: &HashMap<(String, String), i64>,
    remote_changes: &HashMap<(String, String), i64>,
) where
    T: Eq,
    F: Fn(&T) -> String,
{
    let local_by_key = local_rows
        .iter()
        .map(|row| (key_fn(row), row))
        .collect::<HashMap<_, _>>();
    let remote_by_key = remote_rows
        .iter()
        .map(|row| (key_fn(row), row))
        .collect::<HashMap<_, _>>();

    for key in local_by_key.keys() {
        if !remote_by_key.contains_key(key) {
            push_diff(diff, table, key, SyncWriteTarget::Remote);
        }
    }

    for key in remote_by_key.keys() {
        let Some(local_row) = local_by_key.get(key) else {
            push_diff(diff, table, key, SyncWriteTarget::Local);
            continue;
        };
        let remote_row = remote_by_key
            .get(key)
            .expect("remote key should be present while iterating remote rows");

        if local_row == remote_row {
            continue;
        }

        match newer_target(entity_type, key, local_changes, remote_changes) {
            Some(SyncWriteTarget::Remote) => push_diff(diff, table, key, SyncWriteTarget::Remote),
            Some(SyncWriteTarget::Local) => push_diff(diff, table, key, SyncWriteTarget::Local),
            None => diff.conflicts.push(SyncDiffConflict {
                table,
                key: key.clone(),
                reason: "cannot determine newer row from sync_changes.created_at".to_string(),
            }),
        }
    }
}

/// Returns latest change timestamps by entity type and entity ID.
///
/// # Arguments
///
/// * `changes` - Sync change rows.
///
/// # Returns
///
/// Returns latest timestamp per entity key.
fn latest_changes(changes: &[SyncChangeSnapshotRow]) -> HashMap<(String, String), i64> {
    let mut latest: HashMap<(String, String), i64> = HashMap::new();
    for change in changes {
        let key = (change.entity_type.clone(), change.entity_id.clone());
        latest
            .entry(key)
            .and_modify(|created_at| *created_at = (*created_at).max(change.created_at))
            .or_insert(change.created_at);
    }
    latest
}

/// Determines which side should receive an updated row.
///
/// # Arguments
///
/// * `entity_type` - Sync change entity type.
/// * `entity_id` - Sync change entity ID.
/// * `local_changes` - Latest local changes by entity key.
/// * `remote_changes` - Latest remote changes by entity key.
///
/// # Returns
///
/// Returns the side that should be written, or `None` when direction is unclear.
fn newer_target(
    entity_type: &str,
    entity_id: &str,
    local_changes: &HashMap<(String, String), i64>,
    remote_changes: &HashMap<(String, String), i64>,
) -> Option<SyncWriteTarget> {
    let key = (entity_type.to_string(), entity_id.to_string());
    let local_created_at = local_changes.get(&key)?;
    let remote_created_at = remote_changes.get(&key)?;

    match local_created_at.cmp(remote_created_at) {
        std::cmp::Ordering::Greater => Some(SyncWriteTarget::Remote),
        std::cmp::Ordering::Less => Some(SyncWriteTarget::Local),
        std::cmp::Ordering::Equal => None,
    }
}

/// Adds a row difference to the requested side.
///
/// # Arguments
///
/// * `diff` - Accumulated difference result.
/// * `table` - Table that contains the row.
/// * `key` - Row key.
/// * `target` - Side that should receive the row.
fn push_diff(
    diff: &mut SyncSnapshotDiff,
    table: SyncTableName,
    key: &str,
    target: SyncWriteTarget,
) {
    let row_diff = SyncTableDiff {
        table,
        key: key.to_string(),
        target,
    };

    match target {
        SyncWriteTarget::Local => diff.write_local.push(row_diff),
        SyncWriteTarget::Remote => diff.write_remote.push(row_diff),
    }
}

/// Sorts differences by safe table write order and stable row key.
///
/// # Arguments
///
/// * `diffs` - Differences to sort.
fn sort_diffs(diffs: &mut [SyncTableDiff]) {
    diffs.sort_by(|left, right| {
        left.table
            .write_order()
            .cmp(&right.table.write_order())
            .then_with(|| left.key.cmp(&right.key))
    });
}

/// Returns the composite note tag key.
///
/// # Arguments
///
/// * `row` - Note tag row.
///
/// # Returns
///
/// Returns `workspace_id:note_id:tag_id`.
fn note_tag_key(row: &NoteTagSnapshotRow) -> String {
    format!("{}:{}:{}", row.workspace_id, row.note_id, row.tag_id)
}

#[cfg(test)]
mod tests {
    use super::{SyncTableName, SyncWriteTarget, diff_snapshots};
    use crate::repositories::taxonomy::DEFAULT_WORKSPACE_ID;
    use crate::sync::table_snapshot::{
        FieldSnapshotRow, NoteSnapshotRow, NoteTagSnapshotRow, SyncChangeSnapshotRow,
        SyncTableSnapshot,
    };

    #[test]
    fn diff_snapshots_detects_missing_rows_on_both_sides() {
        let mut local = SyncTableSnapshot::default();
        local.fields.push(field("local-only", "Local"));
        let mut remote = SyncTableSnapshot::default();
        remote.fields.push(field("remote-only", "Remote"));

        let diff = diff_snapshots(&local, &remote);

        assert_eq!(diff.write_remote[0].table, SyncTableName::Fields);
        assert_eq!(diff.write_remote[0].key, "local-only");
        assert_eq!(diff.write_local[0].table, SyncTableName::Fields);
        assert_eq!(diff.write_local[0].key, "remote-only");
        assert!(diff.conflicts.is_empty());
    }

    #[test]
    fn diff_snapshots_uses_sync_change_time_for_field_differences() {
        let mut local = SyncTableSnapshot::default();
        local.notes.push(note("note-1", "local"));
        local
            .sync_changes
            .push(change("local-change", "note", "note-1", 20));
        let mut remote = SyncTableSnapshot::default();
        remote.notes.push(note("note-1", "remote"));
        remote
            .sync_changes
            .push(change("remote-change", "note", "note-1", 10));

        let diff = diff_snapshots(&local, &remote);

        assert!(diff.write_remote.iter().any(|row| {
            row.table == SyncTableName::Notes
                && row.key == "note-1"
                && row.target == SyncWriteTarget::Remote
        }));
        assert!(diff.conflicts.is_empty());
    }

    #[test]
    fn diff_snapshots_records_conflict_when_change_time_is_unclear() {
        let mut local = SyncTableSnapshot::default();
        local.notes.push(note("note-1", "local"));
        local
            .sync_changes
            .push(change("local-change", "note", "note-1", 20));
        let mut remote = SyncTableSnapshot::default();
        remote.notes.push(note("note-1", "remote"));
        remote
            .sync_changes
            .push(change("remote-change", "note", "note-1", 20));

        let diff = diff_snapshots(&local, &remote);

        assert!(diff.conflicts.iter().any(|conflict| {
            conflict.table == SyncTableName::Notes && conflict.key == "note-1"
        }));
    }

    #[test]
    fn diff_snapshots_orders_relation_after_note() {
        let mut local = SyncTableSnapshot::default();
        local.notes.push(note("note-1", "local"));
        local.note_tags.push(note_tag("note-1", "tag-1"));
        let remote = SyncTableSnapshot::default();

        let diff = diff_snapshots(&local, &remote);
        let tables = diff
            .write_remote
            .iter()
            .map(|row| row.table)
            .collect::<Vec<_>>();

        assert_eq!(tables, vec![SyncTableName::Notes, SyncTableName::NoteTags]);
    }

    /// Builds a field row for diff tests.
    ///
    /// # Arguments
    ///
    /// * `id` - Field identifier.
    /// * `name` - Field name.
    ///
    /// # Returns
    ///
    /// Returns a field snapshot row.
    fn field(id: &str, name: &str) -> FieldSnapshotRow {
        FieldSnapshotRow {
            id: id.to_string(),
            workspace_id: DEFAULT_WORKSPACE_ID.to_string(),
            name: name.to_string(),
            created_at: 1,
        }
    }

    /// Builds a note row for diff tests.
    ///
    /// # Arguments
    ///
    /// * `id` - Note identifier.
    /// * `content` - Note content.
    ///
    /// # Returns
    ///
    /// Returns a note snapshot row.
    fn note(id: &str, content: &str) -> NoteSnapshotRow {
        NoteSnapshotRow {
            id: id.to_string(),
            workspace_id: DEFAULT_WORKSPACE_ID.to_string(),
            content: content.to_string(),
            role: "Human".to_string(),
            field_id: None,
            created_at: 1,
            updated_at: 1,
            archived_at: None,
            deleted_at: None,
            current_revision_id: None,
            last_change_id: None,
            conflict_status: "none".to_string(),
        }
    }

    /// Builds a note tag relation row for diff tests.
    ///
    /// # Arguments
    ///
    /// * `note_id` - Note identifier.
    /// * `tag_id` - Tag identifier.
    ///
    /// # Returns
    ///
    /// Returns a note tag snapshot row.
    fn note_tag(note_id: &str, tag_id: &str) -> NoteTagSnapshotRow {
        NoteTagSnapshotRow {
            workspace_id: DEFAULT_WORKSPACE_ID.to_string(),
            note_id: note_id.to_string(),
            tag_id: tag_id.to_string(),
            created_at: 1,
        }
    }

    /// Builds a sync change row for diff tests.
    ///
    /// # Arguments
    ///
    /// * `id` - Change identifier.
    /// * `entity_type` - Entity type.
    /// * `entity_id` - Entity identifier.
    /// * `created_at` - Change timestamp.
    ///
    /// # Returns
    ///
    /// Returns a sync change snapshot row.
    fn change(
        id: &str,
        entity_type: &str,
        entity_id: &str,
        created_at: i64,
    ) -> SyncChangeSnapshotRow {
        SyncChangeSnapshotRow {
            id: id.to_string(),
            workspace_id: DEFAULT_WORKSPACE_ID.to_string(),
            device_id: "device-1".to_string(),
            entity_type: entity_type.to_string(),
            entity_id: entity_id.to_string(),
            operation: "update".to_string(),
            base_revision_id: None,
            new_revision_id: None,
            payload: "{}".to_string(),
            created_at,
            applied_at: None,
            supabase_committed_at: None,
        }
    }
}
