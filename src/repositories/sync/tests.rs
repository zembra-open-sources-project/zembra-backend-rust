use super::{SyncChangeRecord, SyncRepository};
use crate::repositories::database::Database;
use crate::repositories::taxonomy::DEFAULT_WORKSPACE_ID;
use crate::sync::diff::{SyncDiffAction, SyncDiffActionKind, SyncTableName};
use crate::sync::table_snapshot::{
    DeviceSnapshotRow, FieldSnapshotRow, NoteRevisionSnapshotRow, NoteSnapshotRow,
    SyncChangeSnapshotRow, SyncTableSnapshot, WorkspaceSnapshotRow,
};

/// Builds a remote sync change for tests.
///
/// # Arguments
///
/// * `id` - Change identifier.
/// * `entity_type` - Entity type.
/// * `entity_id` - Entity identifier.
/// * `operation` - Change operation.
/// * `payload` - JSON payload.
///
/// # Returns
///
/// Returns a sync change record.
fn remote_change(
    id: &str,
    entity_type: &str,
    entity_id: &str,
    operation: &str,
    payload: serde_json::Value,
) -> SyncChangeRecord {
    SyncChangeRecord {
        id: id.to_string(),
        workspace_id: DEFAULT_WORKSPACE_ID.to_string(),
        device_id: "remote-device".to_string(),
        entity_type: entity_type.to_string(),
        entity_id: entity_id.to_string(),
        operation: operation.to_string(),
        base_revision_id: None,
        new_revision_id: None,
        payload: payload.to_string(),
        created_at: 100,
        applied_at: None,
        supabase_committed_at: Some(101),
    }
}

#[tokio::test]
async fn apply_remote_changes_is_idempotent() {
    let database = Database::connect("sqlite://:memory:").await.unwrap();
    let repository = SyncRepository::new(database.pool.clone());
    let changes = vec![remote_change(
        "remote-field-change",
        "field",
        "field-1",
        "insert",
        serde_json::json!({
            "id": "field-1",
            "name": "remote",
            "created_at": 100
        }),
    )];

    let first = repository.apply_remote_changes(&changes).await.unwrap();
    let second = repository.apply_remote_changes(&changes).await.unwrap();
    let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM fields WHERE id = 'field-1'")
        .fetch_one(&database.pool)
        .await
        .unwrap();

    assert_eq!(first, 1);
    assert_eq!(second, 0);
    assert_eq!(count, 1);
}

#[tokio::test]
async fn apply_remote_tag_insert_writes_hierarchical_fields() {
    let database = Database::connect("sqlite://:memory:").await.unwrap();
    let repository = SyncRepository::new(database.pool.clone());
    let parent = remote_change(
        "remote-tag-parent-change",
        "tag",
        "tag-books",
        "insert",
        serde_json::json!({
            "id": "tag-books",
            "name": "books",
            "parent_tag_id": null,
            "path": "books",
            "depth": 0,
            "created_at": 100
        }),
    );
    let child = remote_change(
        "remote-tag-child-change",
        "tag",
        "tag-python",
        "insert",
        serde_json::json!({
            "id": "tag-python",
            "name": "python",
            "parent_tag_id": "tag-books",
            "path": "books/python",
            "depth": 1,
            "created_at": 101
        }),
    );

    repository
        .apply_remote_changes(&[parent, child])
        .await
        .unwrap();
    let tag = sqlx::query_as::<_, (String, Option<String>, String, i64)>(
        "SELECT name, parent_tag_id, path, depth FROM tags WHERE id = 'tag-python'",
    )
    .fetch_one(&database.pool)
    .await
    .unwrap();

    assert_eq!(
        tag,
        (
            "python".to_string(),
            Some("tag-books".to_string()),
            "books/python".to_string(),
            1
        )
    );
}

#[tokio::test]
async fn apply_remote_tag_insert_records_conflict_for_missing_path() {
    let database = Database::connect("sqlite://:memory:").await.unwrap();
    let repository = SyncRepository::new(database.pool.clone());
    let change = remote_change(
        "remote-tag-invalid-change",
        "tag",
        "tag-invalid",
        "insert",
        serde_json::json!({
            "id": "tag-invalid",
            "name": "invalid",
            "parent_tag_id": null,
            "depth": 0,
            "created_at": 100
        }),
    );

    repository.apply_remote_changes(&[change]).await.unwrap();
    let conflict = sqlx::query_scalar::<_, String>(
        "SELECT resolution_note FROM sync_conflicts WHERE remote_change_id = 'remote-tag-invalid-change'",
    )
    .fetch_one(&database.pool)
    .await
    .unwrap();

    assert!(conflict.contains("missing text field path"));
}

#[tokio::test]
async fn apply_remote_revision_selects_deterministic_winner() {
    let database = Database::connect("sqlite://:memory:").await.unwrap();
    let repository = SyncRepository::new(database.pool.clone());
    let note = remote_change(
        "remote-note-change",
        "note",
        "note-1",
        "insert",
        serde_json::json!({
            "id": "note-1",
            "content": "base",
            "role": "Human",
            "field_id": null,
            "created_at": 100,
            "updated_at": 100,
            "archived_at": null,
            "deleted_at": null,
            "current_revision_id": null
        }),
    );
    let older_revision = remote_change(
        "remote-revision-1-change",
        "note_revision",
        "revision-1",
        "insert",
        serde_json::json!({
            "id": "revision-1",
            "note_id": "note-1",
            "content": "older",
            "title": null,
            "device_id": "remote-device",
            "created_at": 100,
            "base_revision_id": null
        }),
    );
    let newer_revision = remote_change(
        "remote-revision-2-change",
        "note_revision",
        "revision-2",
        "insert",
        serde_json::json!({
            "id": "revision-2",
            "note_id": "note-1",
            "content": "newer",
            "title": null,
            "device_id": "remote-device",
            "created_at": 101,
            "base_revision_id": "revision-1"
        }),
    );

    repository
        .apply_remote_changes(&[note, older_revision, newer_revision])
        .await
        .unwrap();
    let current_revision_id = sqlx::query_scalar::<_, String>(
        "SELECT current_revision_id FROM notes WHERE id = 'note-1'",
    )
    .fetch_one(&database.pool)
    .await
    .unwrap();

    assert_eq!(current_revision_id, "revision-2");
}

#[tokio::test]
async fn apply_remote_note_link_attach_and_detach() {
    let database = Database::connect("sqlite://:memory:").await.unwrap();
    let repository = SyncRepository::new(database.pool.clone());
    let source = remote_change(
        "remote-source-note-change",
        "note",
        "source-note",
        "insert",
        serde_json::json!({
            "id": "source-note",
            "content": "source",
            "role": "Human",
            "field_id": null,
            "created_at": 100,
            "updated_at": 100,
            "archived_at": null,
            "deleted_at": null,
            "current_revision_id": null
        }),
    );
    let target = remote_change(
        "remote-target-note-change",
        "note",
        "target-note",
        "insert",
        serde_json::json!({
            "id": "target-note",
            "content": "target",
            "role": "Human",
            "field_id": null,
            "created_at": 100,
            "updated_at": 100,
            "archived_at": null,
            "deleted_at": null,
            "current_revision_id": null
        }),
    );
    let attach = remote_change(
        "remote-link-attach",
        "note_link",
        "link-1",
        "attach",
        serde_json::json!({
            "id": "link-1",
            "source_note_id": "source-note",
            "target_note_id": "target-note",
            "anchor_text": "target",
            "position": 3,
            "created_at": 101
        }),
    );
    let detach = remote_change(
        "remote-link-detach",
        "note_link",
        "link-1",
        "detach",
        serde_json::json!({
            "id": "link-1"
        }),
    );

    repository
        .apply_remote_changes(&[source, target, attach])
        .await
        .unwrap();
    let attached =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM note_links WHERE id = 'link-1'")
            .fetch_one(&database.pool)
            .await
            .unwrap();

    repository.apply_remote_changes(&[detach]).await.unwrap();
    let detached =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM note_links WHERE id = 'link-1'")
            .fetch_one(&database.pool)
            .await
            .unwrap();

    assert_eq!(attached, 1);
    assert_eq!(detached, 0);
}

#[tokio::test]
async fn read_local_table_snapshot_returns_all_sync_tables_in_stable_order() {
    let database = Database::connect("sqlite://:memory:").await.unwrap();
    let repository = SyncRepository::new(database.pool.clone());

    sqlx::query("INSERT INTO devices (id, workspace_id, name, platform, created_at, sync_enabled) VALUES ('local-backend', ?, 'Local Backend', 'backend', 1, 1)")
        .bind(DEFAULT_WORKSPACE_ID)
        .execute(&database.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO fields (id, workspace_id, name, created_at) VALUES ('field-b', ?, 'field b', 20), ('field-a', ?, 'field a', 10)")
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(DEFAULT_WORKSPACE_ID)
    .execute(&database.pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO tags (id, workspace_id, name, parent_tag_id, path, depth, created_at) VALUES ('tag-b', ?, 'tag b', NULL, 'tag-b', 0, 20), ('tag-a', ?, 'tag a', NULL, 'tag-a', 0, 10)")
        .bind(DEFAULT_WORKSPACE_ID)
        .bind(DEFAULT_WORKSPACE_ID)
    .execute(&database.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO notes \
         (id, workspace_id, content, role, field_id, created_at, updated_at, archived_at, deleted_at, current_revision_id, last_change_id, conflict_status) \
         VALUES \
         ('note-b', ?, 'note b', 'Human', 'field-b', 20, 21, 22, NULL, NULL, NULL, 'none'), \
         ('note-a', ?, 'note a', 'Agent', 'field-a', 10, 11, NULL, 12, NULL, NULL, 'none')",
    )
    .bind(DEFAULT_WORKSPACE_ID)
    .bind(DEFAULT_WORKSPACE_ID)
    .execute(&database.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO note_revisions \
         (id, workspace_id, note_id, content, title, device_id, created_at, base_revision_id, change_id) \
         VALUES ('revision-a', ?, 'note-a', 'note a', NULL, NULL, 10, NULL, NULL)",
    )
    .bind(DEFAULT_WORKSPACE_ID)
    .execute(&database.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO note_tags (workspace_id, note_id, tag_id, created_at) VALUES \
         (?, 'note-b', 'tag-b', 20), \
         (?, 'note-a', 'tag-a', 10)",
    )
    .bind(DEFAULT_WORKSPACE_ID)
    .bind(DEFAULT_WORKSPACE_ID)
    .execute(&database.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO note_links \
         (id, workspace_id, source_note_id, target_note_id, anchor_text, position, created_at) \
         VALUES ('link-a', ?, 'note-a', 'note-b', 'note b', 3, 30)",
    )
    .bind(DEFAULT_WORKSPACE_ID)
    .execute(&database.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO sync_changes \
         (id, workspace_id, device_id, entity_type, entity_id, operation, payload, created_at, applied_at, supabase_committed_at) \
         VALUES \
         ('change-b', ?, 'local-backend', 'note', 'note-b', 'insert', '{}', 20, 20, NULL), \
         ('change-a', ?, 'local-backend', 'note', 'note-a', 'insert', '{}', 10, 10, NULL)",
    )
    .bind(DEFAULT_WORKSPACE_ID)
    .bind(DEFAULT_WORKSPACE_ID)
    .execute(&database.pool)
    .await
    .unwrap();

    let snapshot = repository.read_local_table_snapshot().await.unwrap();

    assert_eq!(snapshot.workspaces.len(), 1);
    assert_eq!(snapshot.devices[0].id, "local-backend");
    assert_eq!(
        snapshot
            .fields
            .iter()
            .map(|field| field.id.as_str())
            .collect::<Vec<_>>(),
        vec!["field-a", "field-b"]
    );
    assert_eq!(
        snapshot
            .tags
            .iter()
            .map(|tag| tag.id.as_str())
            .collect::<Vec<_>>(),
        vec!["tag-a", "tag-b"]
    );
    assert_eq!(snapshot.notes[0].id, "note-a");
    assert_eq!(snapshot.notes[0].deleted_at, Some(12));
    assert_eq!(snapshot.notes[1].archived_at, Some(22));
    assert_eq!(snapshot.note_revisions[0].id, "revision-a");
    assert_eq!(snapshot.note_tags[0].note_id, "note-a");
    assert_eq!(snapshot.note_links[0].id, "link-a");
    assert_eq!(
        snapshot
            .sync_changes
            .iter()
            .map(|change| change.id.as_str())
            .collect::<Vec<_>>(),
        vec!["change-a", "change-b"]
    );
}

#[tokio::test]
async fn write_local_table_snapshot_upserts_remote_rows() {
    let database = Database::connect("sqlite://:memory:").await.unwrap();
    let repository = SyncRepository::new(database.pool.clone());
    let snapshot = SyncTableSnapshot {
        workspaces: vec![WorkspaceSnapshotRow {
            id: DEFAULT_WORKSPACE_ID.to_string(),
            workspace_name: Some("Remote Workspace".to_string()),
            created_at: 1,
            updated_at: 2,
            archived_at: None,
            deleted_at: None,
        }],
        devices: vec![DeviceSnapshotRow {
            id: "remote-device".to_string(),
            workspace_id: DEFAULT_WORKSPACE_ID.to_string(),
            name: "Remote Device".to_string(),
            platform: "remote".to_string(),
            created_at: 1,
            last_seen_at: Some(2),
            sync_enabled: true,
            last_synced_at: Some(3),
        }],
        fields: vec![FieldSnapshotRow {
            id: "field-remote".to_string(),
            workspace_id: DEFAULT_WORKSPACE_ID.to_string(),
            name: "Remote Field".to_string(),
            created_at: 3,
        }],
        notes: vec![NoteSnapshotRow {
            id: "note-remote".to_string(),
            workspace_id: DEFAULT_WORKSPACE_ID.to_string(),
            content: "remote note".to_string(),
            role: "Human".to_string(),
            field_id: Some("field-remote".to_string()),
            created_at: 4,
            updated_at: 5,
            archived_at: None,
            deleted_at: None,
            current_revision_id: Some("revision-remote".to_string()),
            last_change_id: Some("change-remote".to_string()),
            conflict_status: "none".to_string(),
        }],
        note_revisions: vec![NoteRevisionSnapshotRow {
            id: "revision-remote".to_string(),
            workspace_id: DEFAULT_WORKSPACE_ID.to_string(),
            note_id: "note-remote".to_string(),
            content: "remote note".to_string(),
            title: None,
            device_id: Some("remote-device".to_string()),
            created_at: 5,
            base_revision_id: None,
            change_id: Some("change-remote".to_string()),
        }],
        sync_changes: vec![SyncChangeSnapshotRow {
            id: "change-remote".to_string(),
            workspace_id: DEFAULT_WORKSPACE_ID.to_string(),
            device_id: "remote-device".to_string(),
            entity_type: "note".to_string(),
            entity_id: "note-remote".to_string(),
            operation: "insert".to_string(),
            base_revision_id: None,
            new_revision_id: Some("revision-remote".to_string()),
            payload: "{}".to_string(),
            created_at: 6,
            applied_at: Some(6),
            supabase_committed_at: Some(7),
        }],
        ..SyncTableSnapshot::default()
    };

    repository
        .write_local_table_snapshot(&snapshot)
        .await
        .unwrap();
    let written = repository.read_local_table_snapshot().await.unwrap();

    assert!(written.fields.iter().any(|row| row.id == "field-remote"));
    assert!(written.notes.iter().any(|row| row.id == "note-remote"));
    assert!(
        written
            .note_revisions
            .iter()
            .any(|row| row.id == "revision-remote")
    );
    assert!(
        written
            .sync_changes
            .iter()
            .any(|row| row.id == "change-remote")
    );
}

#[tokio::test]
async fn delete_local_actions_deletes_field_and_clears_hidden_note_reference() {
    let database = Database::connect("sqlite://:memory:").await.unwrap();
    let repository = SyncRepository::new(database.pool.clone());
    sqlx::query(
        "INSERT INTO fields (id, workspace_id, name, created_at) VALUES ('field-delete', ?, 'Delete', 1)",
    )
    .bind(DEFAULT_WORKSPACE_ID)
    .execute(&database.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO notes \
         (id, workspace_id, content, role, field_id, created_at, updated_at, archived_at, deleted_at, conflict_status) \
         VALUES ('hidden-note', ?, 'hidden', 'Human', 'field-delete', 1, 1, 2, NULL, 'none')",
    )
    .bind(DEFAULT_WORKSPACE_ID)
    .execute(&database.pool)
    .await
    .unwrap();
    let snapshot = repository.read_local_table_snapshot().await.unwrap();
    let actions = vec![SyncDiffAction {
        kind: SyncDiffActionKind::DeleteLocal,
        table: SyncTableName::Fields,
        key: "field-delete".to_string(),
        reason: "test".to_string(),
    }];

    repository
        .delete_local_actions(&actions, &snapshot)
        .await
        .unwrap();

    let field_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM fields WHERE id = 'field-delete'")
            .fetch_one(&database.pool)
            .await
            .unwrap();
    let note_field_id = sqlx::query_scalar::<_, Option<String>>(
        "SELECT field_id FROM notes WHERE id = 'hidden-note'",
    )
    .fetch_one(&database.pool)
    .await
    .unwrap();

    assert_eq!(field_count, 0);
    assert_eq!(note_field_id, None);
}

#[tokio::test]
async fn local_schema_contract_version_reads_current_version() {
    let database = Database::connect("sqlite://:memory:").await.unwrap();
    let repository = SyncRepository::new(database.pool.clone());

    let version = repository
        .local_schema_contract_version()
        .await
        .unwrap()
        .unwrap();

    assert_eq!(version, "0.5.0");
}
