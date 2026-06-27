use std::collections::HashMap;

use crate::sync::table_snapshot::{NoteTagSnapshotRow, SyncChangeSnapshotRow, SyncTableSnapshot};

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

/// Lifecycle synchronization action kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncDiffActionKind {
    /// Upsert the remote row into local SQLite.
    UpsertLocal,
    /// Upsert the local row into Supabase.
    UpsertRemote,
    /// Delete the local row or relation.
    DeleteLocal,
    /// Delete the remote row or relation.
    DeleteRemote,
    /// Copy a missing sync change to local SQLite.
    SyncChangeLocal,
    /// Copy a missing sync change to Supabase.
    SyncChangeRemote,
}

/// One lifecycle synchronization action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncDiffAction {
    /// Action kind.
    pub kind: SyncDiffActionKind,
    /// Table affected by the action.
    pub table: SyncTableName,
    /// Stable primary or composite key string for the row.
    pub key: String,
    /// Human-readable action reason safe for tests and diagnostics.
    pub reason: String,
}

/// Conflict that cannot be resolved from lifecycle facts.
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
    /// Lifecycle actions that should be executed.
    pub actions: Vec<SyncDiffAction>,
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

    compare_workspaces(&mut diff, local, remote, &local_changes, &remote_changes);
    compare_rows(
        &mut diff,
        SyncTableName::Devices,
        &local.devices,
        &remote.devices,
        |row| row.id.clone(),
        |row| row.id.clone(),
        "device",
        EntityLifecycle::Preserved,
        &local_changes,
        &remote_changes,
    );
    compare_rows(
        &mut diff,
        SyncTableName::Fields,
        &local.fields,
        &remote.fields,
        |row| row.id.clone(),
        |row| row.id.clone(),
        "field",
        EntityLifecycle::PhysicalDelete,
        &local_changes,
        &remote_changes,
    );
    compare_rows(
        &mut diff,
        SyncTableName::Tags,
        &local.tags,
        &remote.tags,
        |row| row.id.clone(),
        |row| row.id.clone(),
        "tag",
        EntityLifecycle::Preserved,
        &local_changes,
        &remote_changes,
    );
    compare_notes(
        &mut diff,
        &local.notes,
        &remote.notes,
        &local_changes,
        &remote_changes,
    );
    compare_rows(
        &mut diff,
        SyncTableName::NoteRevisions,
        &local.note_revisions,
        &remote.note_revisions,
        |row| row.id.clone(),
        |row| row.id.clone(),
        "note_revision",
        EntityLifecycle::AppendOnlyProjection,
        &local_changes,
        &remote_changes,
    );
    compare_rows(
        &mut diff,
        SyncTableName::NoteTags,
        &local.note_tags,
        &remote.note_tags,
        note_tag_key,
        note_tag_entity_id,
        "note_tag",
        EntityLifecycle::Relation,
        &local_changes,
        &remote_changes,
    );
    compare_rows(
        &mut diff,
        SyncTableName::NoteLinks,
        &local.note_links,
        &remote.note_links,
        |row| row.id.clone(),
        |row| row.id.clone(),
        "note_link",
        EntityLifecycle::Relation,
        &local_changes,
        &remote_changes,
    );
    compare_sync_changes(&mut diff, &local.sync_changes, &remote.sync_changes);

    sort_actions(&mut diff.actions);
    diff
}

/// Compares note rows with dependency-aware field tombstone handling.
fn compare_notes(
    diff: &mut SyncSnapshotDiff,
    local_rows: &[crate::sync::table_snapshot::NoteSnapshotRow],
    remote_rows: &[crate::sync::table_snapshot::NoteSnapshotRow],
    local_changes: &HashMap<(String, String), SyncChangeSnapshotRow>,
    remote_changes: &HashMap<(String, String), SyncChangeSnapshotRow>,
) {
    let local_by_key = local_rows
        .iter()
        .map(|row| (row.id.clone(), row))
        .collect::<HashMap<_, _>>();
    let remote_by_key = remote_rows
        .iter()
        .map(|row| (row.id.clone(), row))
        .collect::<HashMap<_, _>>();

    for key in local_by_key.keys() {
        if !remote_by_key.contains_key(key) {
            handle_missing_remote(
                diff,
                SyncTableName::Notes,
                key,
                "note",
                key,
                EntityLifecycle::SoftDelete,
                local_changes,
                remote_changes,
            );
        }
    }

    for (key, remote_row) in &remote_by_key {
        let Some(local_row) = local_by_key.get(key) else {
            handle_missing_local(
                diff,
                SyncTableName::Notes,
                key,
                "note",
                key,
                EntityLifecycle::SoftDelete,
                local_changes,
                remote_changes,
            );
            continue;
        };

        if local_row == remote_row {
            sync_missing_entity_changes(diff, "note", key, local_changes, remote_changes);
            continue;
        }

        if let Some(kind) =
            note_field_tombstone_action(local_row, remote_row, local_changes, remote_changes)
        {
            push_action(
                diff,
                kind,
                SyncTableName::Notes,
                key,
                "note field reference follows field tombstone",
            );
            continue;
        }

        match newer_target("note", key, local_changes, remote_changes) {
            Some(SyncDiffActionKind::UpsertRemote) => push_action(
                diff,
                SyncDiffActionKind::UpsertRemote,
                SyncTableName::Notes,
                key,
                "local note fact is newer",
            ),
            Some(SyncDiffActionKind::UpsertLocal) => push_action(
                diff,
                SyncDiffActionKind::UpsertLocal,
                SyncTableName::Notes,
                key,
                "remote note fact is newer",
            ),
            Some(_) => diff.conflicts.push(SyncDiffConflict {
                table: SyncTableName::Notes,
                key: key.clone(),
                reason: "unexpected non-upsert freshness action".to_string(),
            }),
            None => diff.conflicts.push(SyncDiffConflict {
                table: SyncTableName::Notes,
                key: key.clone(),
                reason: "cannot determine newer row from sync_changes.created_at".to_string(),
            }),
        }
    }
}

/// Resolves note field reference differences caused by field tombstones.
fn note_field_tombstone_action(
    local: &crate::sync::table_snapshot::NoteSnapshotRow,
    remote: &crate::sync::table_snapshot::NoteSnapshotRow,
    local_changes: &HashMap<(String, String), SyncChangeSnapshotRow>,
    remote_changes: &HashMap<(String, String), SyncChangeSnapshotRow>,
) -> Option<SyncDiffActionKind> {
    if local.field_id.is_none()
        && remote.field_id.is_some()
        && note_equal_ignoring_field(local, remote)
        && tombstone_change(
            local_changes,
            "field",
            remote.field_id.as_ref().expect("remote field should exist"),
        )
        .is_some()
    {
        return Some(SyncDiffActionKind::UpsertRemote);
    }

    if remote.field_id.is_none()
        && local.field_id.is_some()
        && note_equal_ignoring_field(local, remote)
        && tombstone_change(
            remote_changes,
            "field",
            local.field_id.as_ref().expect("local field should exist"),
        )
        .is_some()
    {
        return Some(SyncDiffActionKind::UpsertLocal);
    }

    None
}

/// Compares note rows while ignoring field references.
fn note_equal_ignoring_field(
    left: &crate::sync::table_snapshot::NoteSnapshotRow,
    right: &crate::sync::table_snapshot::NoteSnapshotRow,
) -> bool {
    left.id == right.id
        && left.workspace_id == right.workspace_id
        && left.content == right.content
        && left.role == right.role
        && left.created_at == right.created_at
        && left.updated_at == right.updated_at
        && left.archived_at == right.archived_at
        && left.deleted_at == right.deleted_at
        && left.current_revision_id == right.current_revision_id
        && left.last_change_id == right.last_change_id
        && left.conflict_status == right.conflict_status
}

/// Entity lifecycle strategy used by snapshot diffing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntityLifecycle {
    /// Row contains its deletion state and is not physically deleted by sync.
    SoftDelete,
    /// Row absence plus a delete tombstone means the entity was deleted.
    PhysicalDelete,
    /// Relation row absence plus a detach/delete tombstone means the relation was removed.
    Relation,
    /// Entity does not currently support delete tombstones.
    Preserved,
    /// Projection is retained by append-only semantics and is not physically deleted.
    AppendOnlyProjection,
}

/// Compares workspace rows while ignoring local-only empty initialization workspaces.
///
/// # Arguments
///
/// * `diff` - Accumulated difference result.
/// * `local` - Local snapshot.
/// * `remote` - Remote snapshot.
/// * `local_changes` - Latest local changes by entity key.
/// * `remote_changes` - Latest remote changes by entity key.
fn compare_workspaces(
    diff: &mut SyncSnapshotDiff,
    local: &SyncTableSnapshot,
    remote: &SyncTableSnapshot,
    local_changes: &HashMap<(String, String), SyncChangeSnapshotRow>,
    remote_changes: &HashMap<(String, String), SyncChangeSnapshotRow>,
) {
    let local_by_key = local
        .workspaces
        .iter()
        .map(|row| (row.id.clone(), row))
        .collect::<HashMap<_, _>>();
    let remote_by_key = remote
        .workspaces
        .iter()
        .map(|row| (row.id.clone(), row))
        .collect::<HashMap<_, _>>();

    for key in local_by_key.keys() {
        if !remote_by_key.contains_key(key) && !local_workspace_is_empty(local, key) {
            push_action(
                diff,
                SyncDiffActionKind::UpsertRemote,
                SyncTableName::Workspaces,
                key,
                "local workspace is present and remote is missing",
            );
        }
    }

    for key in remote_by_key.keys() {
        let Some(local_row) = local_by_key.get(key) else {
            push_action(
                diff,
                SyncDiffActionKind::UpsertLocal,
                SyncTableName::Workspaces,
                key,
                "remote workspace is present and local is missing",
            );
            continue;
        };
        let remote_row = remote_by_key
            .get(key)
            .expect("remote key should be present while iterating remote workspaces");

        if local_row == remote_row {
            continue;
        }

        match newer_target("workspace", key, local_changes, remote_changes) {
            Some(SyncDiffActionKind::UpsertRemote) => push_action(
                diff,
                SyncDiffActionKind::UpsertRemote,
                SyncTableName::Workspaces,
                key,
                "local workspace fact is newer",
            ),
            Some(SyncDiffActionKind::UpsertLocal) => push_action(
                diff,
                SyncDiffActionKind::UpsertLocal,
                SyncTableName::Workspaces,
                key,
                "remote workspace fact is newer",
            ),
            Some(_) => diff.conflicts.push(SyncDiffConflict {
                table: SyncTableName::Workspaces,
                key: key.clone(),
                reason: "unexpected non-upsert freshness action".to_string(),
            }),
            None => diff.conflicts.push(SyncDiffConflict {
                table: SyncTableName::Workspaces,
                key: key.clone(),
                reason: "cannot determine newer row from sync_changes.created_at".to_string(),
            }),
        }
    }
}

/// Checks whether a local workspace has no dependent synchronized rows.
///
/// # Arguments
///
/// * `snapshot` - Local synchronized table snapshot.
/// * `workspace_id` - Workspace identifier to inspect.
///
/// # Returns
///
/// Returns `true` when the workspace has no synchronized data except its workspace row.
fn local_workspace_is_empty(snapshot: &SyncTableSnapshot, workspace_id: &str) -> bool {
    !snapshot
        .devices
        .iter()
        .any(|row| row.workspace_id == workspace_id)
        && !snapshot
            .fields
            .iter()
            .any(|row| row.workspace_id == workspace_id)
        && !snapshot
            .tags
            .iter()
            .any(|row| row.workspace_id == workspace_id)
        && !snapshot
            .notes
            .iter()
            .any(|row| row.workspace_id == workspace_id)
        && !snapshot
            .note_revisions
            .iter()
            .any(|row| row.workspace_id == workspace_id)
        && !snapshot
            .note_tags
            .iter()
            .any(|row| row.workspace_id == workspace_id)
        && !snapshot
            .note_links
            .iter()
            .any(|row| row.workspace_id == workspace_id)
        && !snapshot
            .sync_changes
            .iter()
            .any(|row| row.workspace_id == workspace_id)
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
#[allow(clippy::too_many_arguments)]
fn compare_rows<T, F, E>(
    diff: &mut SyncSnapshotDiff,
    table: SyncTableName,
    local_rows: &[T],
    remote_rows: &[T],
    key_fn: F,
    entity_id_fn: E,
    entity_type: &str,
    lifecycle: EntityLifecycle,
    local_changes: &HashMap<(String, String), SyncChangeSnapshotRow>,
    remote_changes: &HashMap<(String, String), SyncChangeSnapshotRow>,
) where
    T: Eq,
    F: Fn(&T) -> String,
    E: Fn(&T) -> String,
{
    let local_by_key = local_rows
        .iter()
        .map(|row| (key_fn(row), (entity_id_fn(row), row)))
        .collect::<HashMap<_, _>>();
    let remote_by_key = remote_rows
        .iter()
        .map(|row| (key_fn(row), (entity_id_fn(row), row)))
        .collect::<HashMap<_, _>>();

    for (key, (entity_id, _row)) in &local_by_key {
        if !remote_by_key.contains_key(key) {
            handle_missing_remote(
                diff,
                table,
                key,
                entity_type,
                entity_id,
                lifecycle,
                local_changes,
                remote_changes,
            );
        }
    }

    for (key, (entity_id, remote_row)) in &remote_by_key {
        let Some((_, local_row)) = local_by_key.get(key) else {
            handle_missing_local(
                diff,
                table,
                key,
                entity_type,
                entity_id,
                lifecycle,
                local_changes,
                remote_changes,
            );
            continue;
        };

        if local_row == remote_row {
            sync_missing_entity_changes(
                diff,
                entity_type,
                entity_id,
                local_changes,
                remote_changes,
            );
            continue;
        }

        match newer_target(entity_type, entity_id, local_changes, remote_changes) {
            Some(SyncDiffActionKind::UpsertRemote) => push_action(
                diff,
                SyncDiffActionKind::UpsertRemote,
                table,
                key,
                "local entity fact is newer",
            ),
            Some(SyncDiffActionKind::UpsertLocal) => push_action(
                diff,
                SyncDiffActionKind::UpsertLocal,
                table,
                key,
                "remote entity fact is newer",
            ),
            Some(_) => diff.conflicts.push(SyncDiffConflict {
                table,
                key: key.clone(),
                reason: "unexpected non-upsert freshness action".to_string(),
            }),
            None => diff.conflicts.push(SyncDiffConflict {
                table,
                key: key.clone(),
                reason: "cannot determine newer row from sync_changes.created_at".to_string(),
            }),
        }
    }
}

/// Handles a row that is present locally but absent remotely.
#[allow(clippy::too_many_arguments)]
fn handle_missing_remote(
    diff: &mut SyncSnapshotDiff,
    table: SyncTableName,
    row_key: &str,
    entity_type: &str,
    entity_id: &str,
    lifecycle: EntityLifecycle,
    local_changes: &HashMap<(String, String), SyncChangeSnapshotRow>,
    remote_changes: &HashMap<(String, String), SyncChangeSnapshotRow>,
) {
    if let Some(remote_tombstone) = tombstone_change(remote_changes, entity_type, entity_id) {
        match lifecycle {
            EntityLifecycle::PhysicalDelete | EntityLifecycle::Relation => {
                push_action(
                    diff,
                    SyncDiffActionKind::DeleteLocal,
                    table,
                    row_key,
                    "remote tombstone deletes local row",
                );
                push_missing_change_action(
                    diff,
                    SyncDiffActionKind::SyncChangeLocal,
                    &remote_tombstone.id,
                );
            }
            EntityLifecycle::Preserved | EntityLifecycle::AppendOnlyProjection => push_conflict(
                diff,
                table,
                row_key,
                "delete tombstone is not supported for entity",
            ),
            EntityLifecycle::SoftDelete => push_action(
                diff,
                SyncDiffActionKind::UpsertRemote,
                table,
                row_key,
                "soft-delete entity row remains authoritative",
            ),
        }
        return;
    }

    sync_missing_entity_changes(diff, entity_type, entity_id, local_changes, remote_changes);
    push_action(
        diff,
        SyncDiffActionKind::UpsertRemote,
        table,
        row_key,
        "local row is present and remote is missing",
    );
}

/// Handles a row that is present remotely but absent locally.
#[allow(clippy::too_many_arguments)]
fn handle_missing_local(
    diff: &mut SyncSnapshotDiff,
    table: SyncTableName,
    row_key: &str,
    entity_type: &str,
    entity_id: &str,
    lifecycle: EntityLifecycle,
    local_changes: &HashMap<(String, String), SyncChangeSnapshotRow>,
    remote_changes: &HashMap<(String, String), SyncChangeSnapshotRow>,
) {
    if let Some(local_tombstone) = tombstone_change(local_changes, entity_type, entity_id) {
        match lifecycle {
            EntityLifecycle::PhysicalDelete | EntityLifecycle::Relation => {
                push_action(
                    diff,
                    SyncDiffActionKind::DeleteRemote,
                    table,
                    row_key,
                    "local tombstone deletes remote row",
                );
                push_missing_change_action(
                    diff,
                    SyncDiffActionKind::SyncChangeRemote,
                    &local_tombstone.id,
                );
            }
            EntityLifecycle::Preserved | EntityLifecycle::AppendOnlyProjection => push_conflict(
                diff,
                table,
                row_key,
                "delete tombstone is not supported for entity",
            ),
            EntityLifecycle::SoftDelete => push_action(
                diff,
                SyncDiffActionKind::UpsertLocal,
                table,
                row_key,
                "soft-delete entity row remains authoritative",
            ),
        }
        return;
    }

    sync_missing_entity_changes(diff, entity_type, entity_id, local_changes, remote_changes);
    push_action(
        diff,
        SyncDiffActionKind::UpsertLocal,
        table,
        row_key,
        "remote row is present and local is missing",
    );
}

/// Adds missing sync-change fact actions for an entity.
fn sync_missing_entity_changes(
    diff: &mut SyncSnapshotDiff,
    entity_type: &str,
    entity_id: &str,
    local_changes: &HashMap<(String, String), SyncChangeSnapshotRow>,
    remote_changes: &HashMap<(String, String), SyncChangeSnapshotRow>,
) {
    let key = (entity_type.to_string(), entity_id.to_string());
    match (local_changes.get(&key), remote_changes.get(&key)) {
        (Some(local_change), None) => {
            push_missing_change_action(diff, SyncDiffActionKind::SyncChangeRemote, &local_change.id)
        }
        (None, Some(remote_change)) => {
            push_missing_change_action(diff, SyncDiffActionKind::SyncChangeLocal, &remote_change.id)
        }
        _ => {}
    }
}

/// Compares sync changes as append-only facts keyed by change id.
fn compare_sync_changes(
    diff: &mut SyncSnapshotDiff,
    local_changes: &[SyncChangeSnapshotRow],
    remote_changes: &[SyncChangeSnapshotRow],
) {
    let local_by_id = local_changes
        .iter()
        .map(|row| (row.id.clone(), row))
        .collect::<HashMap<_, _>>();
    let remote_by_id = remote_changes
        .iter()
        .map(|row| (row.id.clone(), row))
        .collect::<HashMap<_, _>>();

    for key in local_by_id.keys() {
        if !remote_by_id.contains_key(key) {
            push_missing_change_action(diff, SyncDiffActionKind::SyncChangeRemote, key);
        }
    }

    for key in remote_by_id.keys() {
        let Some(local_change) = local_by_id.get(key) else {
            push_missing_change_action(diff, SyncDiffActionKind::SyncChangeLocal, key);
            continue;
        };
        let remote_change = remote_by_id
            .get(key)
            .expect("remote key should be present while iterating remote changes");
        if local_change != remote_change {
            push_conflict(
                diff,
                SyncTableName::SyncChanges,
                key,
                "sync change fact differs for the same change id",
            );
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
fn latest_changes(
    changes: &[SyncChangeSnapshotRow],
) -> HashMap<(String, String), SyncChangeSnapshotRow> {
    let mut latest: HashMap<(String, String), SyncChangeSnapshotRow> = HashMap::new();
    for change in changes {
        let key = (change.entity_type.clone(), change.entity_id.clone());
        latest
            .entry(key)
            .and_modify(|current| {
                if change.created_at > current.created_at
                    || (change.created_at == current.created_at && change.id > current.id)
                {
                    *current = change.clone();
                }
            })
            .or_insert_with(|| change.clone());
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
/// Returns the upsert action kind, or `None` when direction is unclear.
fn newer_target(
    entity_type: &str,
    entity_id: &str,
    local_changes: &HashMap<(String, String), SyncChangeSnapshotRow>,
    remote_changes: &HashMap<(String, String), SyncChangeSnapshotRow>,
) -> Option<SyncDiffActionKind> {
    let key = (entity_type.to_string(), entity_id.to_string());
    match (local_changes.get(&key), remote_changes.get(&key)) {
        (Some(local_change), Some(remote_change)) => {
            match local_change.created_at.cmp(&remote_change.created_at) {
                std::cmp::Ordering::Greater => Some(SyncDiffActionKind::UpsertRemote),
                std::cmp::Ordering::Less => Some(SyncDiffActionKind::UpsertLocal),
                std::cmp::Ordering::Equal => None,
            }
        }
        (Some(_), None) => Some(SyncDiffActionKind::UpsertRemote),
        (None, Some(_)) => Some(SyncDiffActionKind::UpsertLocal),
        (None, None) => None,
    }
}

/// Returns the latest tombstone change for an entity.
fn tombstone_change<'a>(
    changes: &'a HashMap<(String, String), SyncChangeSnapshotRow>,
    entity_type: &str,
    entity_id: &str,
) -> Option<&'a SyncChangeSnapshotRow> {
    let key = (entity_type.to_string(), entity_id.to_string());
    changes.get(&key).filter(|change| {
        matches!(
            change.operation.as_str(),
            "delete" | "detach" | "remove" | "unlink"
        )
    })
}

/// Adds a sync change fact action.
fn push_missing_change_action(
    diff: &mut SyncSnapshotDiff,
    kind: SyncDiffActionKind,
    change_id: &str,
) {
    push_action(
        diff,
        kind,
        SyncTableName::SyncChanges,
        change_id,
        "sync change fact is missing on target side",
    );
}

/// Adds an action to the diff.
///
/// # Arguments
///
/// * `diff` - Accumulated difference result.
/// * `kind` - Action kind.
/// * `table` - Table that contains the row.
/// * `key` - Row key.
/// * `reason` - Diagnostic action reason.
fn push_action(
    diff: &mut SyncSnapshotDiff,
    kind: SyncDiffActionKind,
    table: SyncTableName,
    key: &str,
    reason: &str,
) {
    diff.actions.push(SyncDiffAction {
        kind,
        table,
        key: key.to_string(),
        reason: reason.to_string(),
    });
}

/// Adds a conflict to the diff.
fn push_conflict(diff: &mut SyncSnapshotDiff, table: SyncTableName, key: &str, reason: &str) {
    diff.conflicts.push(SyncDiffConflict {
        table,
        key: key.to_string(),
        reason: reason.to_string(),
    });
}

/// Sorts actions by kind, safe table write order, and stable row key.
///
/// # Arguments
///
/// * `actions` - Actions to sort.
fn sort_actions(actions: &mut [SyncDiffAction]) {
    actions.sort_by(|left, right| {
        action_order(&left.kind)
            .cmp(&action_order(&right.kind))
            .then_with(|| left.table.write_order().cmp(&right.table.write_order()))
            .then_with(|| left.key.cmp(&right.key))
    });
}

/// Returns the stable action execution order.
fn action_order(kind: &SyncDiffActionKind) -> usize {
    match kind {
        SyncDiffActionKind::UpsertLocal | SyncDiffActionKind::UpsertRemote => 0,
        SyncDiffActionKind::DeleteLocal | SyncDiffActionKind::DeleteRemote => 1,
        SyncDiffActionKind::SyncChangeLocal | SyncDiffActionKind::SyncChangeRemote => 2,
    }
}

/// Returns the composite note tag entity id used in sync changes.
///
/// # Arguments
///
/// * `row` - Note tag row.
///
/// # Returns
///
/// Returns `note_id:tag_id`.
fn note_tag_entity_id(row: &NoteTagSnapshotRow) -> String {
    format!("{}:{}", row.note_id, row.tag_id)
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
    use super::{SyncDiffActionKind, SyncTableName, diff_snapshots};
    use crate::repositories::taxonomy::DEFAULT_WORKSPACE_ID;
    use crate::sync::table_snapshot::{
        FieldSnapshotRow, NoteLinkSnapshotRow, NoteSnapshotRow, NoteTagSnapshotRow,
        SyncChangeSnapshotRow, SyncTableSnapshot, WorkspaceSnapshotRow,
    };

    #[test]
    fn diff_snapshots_detects_missing_rows_on_both_sides() {
        let mut local = SyncTableSnapshot::default();
        local.fields.push(field("local-only", "Local"));
        let mut remote = SyncTableSnapshot::default();
        remote.fields.push(field("remote-only", "Remote"));

        let diff = diff_snapshots(&local, &remote);

        assert!(diff.actions.iter().any(|action| {
            action.kind == SyncDiffActionKind::UpsertRemote
                && action.table == SyncTableName::Fields
                && action.key == "local-only"
        }));
        assert!(diff.actions.iter().any(|action| {
            action.kind == SyncDiffActionKind::UpsertLocal
                && action.table == SyncTableName::Fields
                && action.key == "remote-only"
        }));
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

        assert!(diff.actions.iter().any(|action| {
            action.kind == SyncDiffActionKind::UpsertRemote
                && action.table == SyncTableName::Notes
                && action.key == "note-1"
        }));
        assert!(diff.conflicts.is_empty());
    }

    #[test]
    fn diff_snapshots_pushes_local_row_when_remote_change_is_missing() {
        let mut local = SyncTableSnapshot::default();
        let mut deleted = note("note-1", "local");
        deleted.updated_at = 20;
        deleted.deleted_at = Some(20);
        local.notes.push(deleted);
        local
            .sync_changes
            .push(change("local-delete", "note", "note-1", 20));
        let mut remote = SyncTableSnapshot::default();
        remote.notes.push(note("note-1", "remote"));

        let diff = diff_snapshots(&local, &remote);

        assert!(diff.actions.iter().any(|action| {
            action.kind == SyncDiffActionKind::UpsertRemote
                && action.table == SyncTableName::Notes
                && action.key == "note-1"
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
            .actions
            .iter()
            .filter(|action| action.kind == SyncDiffActionKind::UpsertRemote)
            .map(|row| row.table)
            .collect::<Vec<_>>();

        assert_eq!(tables, vec![SyncTableName::Notes, SyncTableName::NoteTags]);
    }

    #[test]
    fn diff_snapshots_does_not_push_local_only_empty_workspace() {
        let mut local = SyncTableSnapshot::default();
        local.workspaces.push(workspace("local-empty"));
        let mut remote = SyncTableSnapshot::default();
        remote.workspaces.push(workspace(DEFAULT_WORKSPACE_ID));

        let diff = diff_snapshots(&local, &remote);

        assert!(!diff.actions.iter().any(|row| {
            row.kind == SyncDiffActionKind::UpsertRemote
                && row.table == SyncTableName::Workspaces
                && row.key == "local-empty"
        }));
        assert!(diff.actions.iter().any(|row| {
            row.kind == SyncDiffActionKind::UpsertLocal
                && row.table == SyncTableName::Workspaces
                && row.key == DEFAULT_WORKSPACE_ID
        }));
        assert!(diff.conflicts.is_empty());
    }

    #[test]
    fn diff_snapshots_deletes_remote_field_when_local_has_tombstone() {
        let mut local = SyncTableSnapshot::default();
        local.sync_changes.push(change_with_operation(
            "local-delete",
            "field",
            "field-1",
            "delete",
            20,
        ));
        let mut remote = SyncTableSnapshot::default();
        remote.fields.push(field("field-1", "Deleted"));

        let diff = diff_snapshots(&local, &remote);

        assert!(diff.actions.iter().any(|action| {
            action.kind == SyncDiffActionKind::DeleteRemote
                && action.table == SyncTableName::Fields
                && action.key == "field-1"
        }));
        assert!(!diff.actions.iter().any(|row| {
            row.kind == SyncDiffActionKind::UpsertLocal
                && row.table == SyncTableName::Fields
                && row.key == "field-1"
        }));
        assert!(diff.conflicts.is_empty());
    }

    #[test]
    fn diff_snapshots_deletes_local_field_when_remote_has_tombstone() {
        let mut local = SyncTableSnapshot::default();
        local.fields.push(field("field-1", "Deleted"));
        let mut remote = SyncTableSnapshot::default();
        remote.sync_changes.push(change_with_operation(
            "remote-delete",
            "field",
            "field-1",
            "delete",
            20,
        ));

        let diff = diff_snapshots(&local, &remote);

        assert!(diff.actions.iter().any(|action| {
            action.kind == SyncDiffActionKind::DeleteLocal
                && action.table == SyncTableName::Fields
                && action.key == "field-1"
        }));
        assert!(!diff.actions.iter().any(|row| {
            row.kind == SyncDiffActionKind::UpsertRemote
                && row.table == SyncTableName::Fields
                && row.key == "field-1"
        }));
        assert!(diff.conflicts.is_empty());
    }

    #[test]
    fn diff_snapshots_syncs_change_facts_without_sync_change_freshness() {
        let mut local = SyncTableSnapshot::default();
        local
            .sync_changes
            .push(change("local-change", "note", "note-1", 20));
        let remote = SyncTableSnapshot::default();

        let diff = diff_snapshots(&local, &remote);

        assert!(diff.actions.iter().any(|action| {
            action.kind == SyncDiffActionKind::SyncChangeRemote
                && action.table == SyncTableName::SyncChanges
                && action.key == "local-change"
        }));
        assert!(diff.conflicts.is_empty());
    }

    #[test]
    fn diff_snapshots_detaches_remote_note_tag_when_local_has_tombstone() {
        let mut local = SyncTableSnapshot::default();
        local.sync_changes.push(change_with_operation(
            "local-detach",
            "note_tag",
            "note-1:tag-1",
            "detach",
            20,
        ));
        let mut remote = SyncTableSnapshot::default();
        remote.note_tags.push(note_tag("note-1", "tag-1"));

        let diff = diff_snapshots(&local, &remote);

        assert!(diff.actions.iter().any(|action| {
            action.kind == SyncDiffActionKind::DeleteRemote
                && action.table == SyncTableName::NoteTags
                && action.key == format!("{DEFAULT_WORKSPACE_ID}:note-1:tag-1")
        }));
        assert!(diff.conflicts.is_empty());
    }

    #[test]
    fn diff_snapshots_detaches_remote_note_link_when_local_has_tombstone() {
        let mut local = SyncTableSnapshot::default();
        local.sync_changes.push(change_with_operation(
            "local-detach",
            "note_link",
            "link-1",
            "detach",
            20,
        ));
        let mut remote = SyncTableSnapshot::default();
        remote.note_links.push(note_link("link-1"));

        let diff = diff_snapshots(&local, &remote);

        assert!(diff.actions.iter().any(|action| {
            action.kind == SyncDiffActionKind::DeleteRemote
                && action.table == SyncTableName::NoteLinks
                && action.key == "link-1"
        }));
        assert!(diff.conflicts.is_empty());
    }

    #[test]
    fn diff_snapshots_pushes_note_field_cleanup_when_field_is_tombstoned() {
        let mut local = SyncTableSnapshot::default();
        let mut local_note = note("note-1", "hidden");
        local_note.deleted_at = Some(20);
        local_note.updated_at = 20;
        local.notes.push(local_note);
        local.sync_changes.push(change_with_operation(
            "field-delete",
            "field",
            "field-1",
            "delete",
            30,
        ));
        local.sync_changes.push(change_with_operation(
            "note-delete",
            "note",
            "note-1",
            "delete",
            20,
        ));

        let mut remote = SyncTableSnapshot::default();
        let mut remote_note = note("note-1", "hidden");
        remote_note.deleted_at = Some(20);
        remote_note.updated_at = 20;
        remote_note.field_id = Some("field-1".to_string());
        remote.notes.push(remote_note);
        remote.sync_changes.push(change_with_operation(
            "note-delete",
            "note",
            "note-1",
            "delete",
            20,
        ));

        let diff = diff_snapshots(&local, &remote);

        assert!(diff.actions.iter().any(|action| {
            action.kind == SyncDiffActionKind::UpsertRemote
                && action.table == SyncTableName::Notes
                && action.key == "note-1"
        }));
        assert!(diff.conflicts.is_empty());
    }

    /// Builds a workspace row for diff tests.
    ///
    /// # Arguments
    ///
    /// * `id` - Workspace identifier.
    ///
    /// # Returns
    ///
    /// Returns a workspace snapshot row.
    fn workspace(id: &str) -> WorkspaceSnapshotRow {
        WorkspaceSnapshotRow {
            id: id.to_string(),
            workspace_name: None,
            created_at: 1,
            updated_at: 1,
            archived_at: None,
            deleted_at: None,
        }
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

    /// Builds a note link row for diff tests.
    ///
    /// # Arguments
    ///
    /// * `id` - Link identifier.
    ///
    /// # Returns
    ///
    /// Returns a note link snapshot row.
    fn note_link(id: &str) -> NoteLinkSnapshotRow {
        NoteLinkSnapshotRow {
            id: id.to_string(),
            workspace_id: DEFAULT_WORKSPACE_ID.to_string(),
            source_note_id: "source-note".to_string(),
            target_note_id: "target-note".to_string(),
            anchor_text: None,
            position: None,
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
        change_with_operation(id, entity_type, entity_id, "update", created_at)
    }

    /// Builds a sync change row with a custom operation for diff tests.
    ///
    /// # Arguments
    ///
    /// * `id` - Change identifier.
    /// * `entity_type` - Entity type.
    /// * `entity_id` - Entity identifier.
    /// * `operation` - Change operation.
    /// * `created_at` - Change timestamp.
    ///
    /// # Returns
    ///
    /// Returns a sync change snapshot row.
    fn change_with_operation(
        id: &str,
        entity_type: &str,
        entity_id: &str,
        operation: &str,
        created_at: i64,
    ) -> SyncChangeSnapshotRow {
        SyncChangeSnapshotRow {
            id: id.to_string(),
            workspace_id: DEFAULT_WORKSPACE_ID.to_string(),
            device_id: "device-1".to_string(),
            entity_type: entity_type.to_string(),
            entity_id: entity_id.to_string(),
            operation: operation.to_string(),
            base_revision_id: None,
            new_revision_id: None,
            payload: "{}".to_string(),
            created_at,
            applied_at: None,
            supabase_committed_at: None,
        }
    }
}
