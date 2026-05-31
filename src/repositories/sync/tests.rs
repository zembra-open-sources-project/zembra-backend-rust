use super::{SyncChangeRecord, SyncRepository};
use crate::repositories::database::Database;

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
        workspace_id: crate::repositories::taxonomy::DEFAULT_WORKSPACE_ID.to_string(),
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
