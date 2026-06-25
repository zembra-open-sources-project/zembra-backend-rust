use super::{CreateNoteInput, NoteLinkInput, NotesRepository, UpdateNoteInput};
use crate::dto::notes::RecentNotesRoleFilter;
use crate::error::ApiError;
use crate::repositories::database::Database;
use crate::repositories::sync::list_sync_changes;
use crate::repositories::taxonomy::{DEFAULT_WORKSPACE_ID, TaxonomyRepository};

/// Creates an in-memory notes repository for tests.
///
/// # Returns
///
/// Returns a repository with migrated SQLite schema.
async fn notes_repository() -> NotesRepository {
    let database = Database::connect("sqlite://:memory:").await.unwrap();
    NotesRepository::new(database.pool)
}

/// Creates an in-memory taxonomy repository for tests.
///
/// # Returns
///
/// Returns a repository with migrated SQLite schema.
async fn taxonomy_repository() -> TaxonomyRepository {
    let database = Database::connect("sqlite://:memory:").await.unwrap();
    TaxonomyRepository::new(database.pool)
}

/// Creates an in-memory taxonomy repository with its pool for tests.
///
/// # Returns
///
/// Returns a taxonomy repository and the migrated SQLite pool.
async fn taxonomy_repository_with_pool() -> (TaxonomyRepository, sqlx::SqlitePool) {
    let database = Database::connect("sqlite://:memory:").await.unwrap();
    let pool = database.pool.clone();
    (TaxonomyRepository::new(database.pool), pool)
}

/// Builds a default note creation input.
///
/// # Arguments
///
/// * `content` - Note body content.
///
/// # Returns
///
/// Returns a note creation input.
fn input(content: &str) -> CreateNoteInput {
    CreateNoteInput {
        content: content.to_string(),
        field: Some("work".to_string()),
        tags: vec!["rust".to_string(), "sqlite".to_string()],
        role: "Human".to_string(),
        device_id: None,
        links: Vec::new(),
    }
}

/// Builds a note creation input with a specific role.
///
/// # Arguments
///
/// * `content` - Note body content.
/// * `role` - Stored note creator role.
///
/// # Returns
///
/// Returns a note creation input for the role.
fn input_with_role(content: &str, role: &str) -> CreateNoteInput {
    CreateNoteInput {
        role: role.to_string(),
        ..input(content)
    }
}

#[tokio::test]
async fn create_note_writes_revision_field_and_tags() {
    let repository = notes_repository().await;

    let created = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("hello"))
        .await
        .unwrap();
    let revisions = repository
        .list_note_revisions(DEFAULT_WORKSPACE_ID, &created.note.id)
        .await
        .unwrap();
    let tags = repository
        .list_note_tags(DEFAULT_WORKSPACE_ID, &created.note.id)
        .await
        .unwrap();

    assert_eq!(created.note.content, "hello");
    assert_eq!(created.note.role, "Human");
    assert_eq!(created.field.as_deref(), Some("work"));
    assert_eq!(created.tags, vec!["rust", "sqlite"]);
    assert_eq!(revisions.len(), 1);
    assert_eq!(revisions[0].content, "hello");
    assert_eq!(
        tags.iter().map(|tag| tag.name.as_str()).collect::<Vec<_>>(),
        vec!["rust", "sqlite"]
    );
}

#[tokio::test]
async fn create_note_records_sync_changes() {
    let repository = notes_repository().await;

    let created = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("hello sync"))
        .await
        .unwrap();
    let changes = list_sync_changes(&repository.pool).await.unwrap();

    assert_eq!(changes.len(), 7);
    assert!(changes.iter().any(|change| {
        change.entity_type == "field"
            && change.operation == "insert"
            && change.payload.contains("\"name\":\"work\"")
    }));
    assert_eq!(
        changes
            .iter()
            .filter(|change| change.entity_type == "tag" && change.operation == "insert")
            .count(),
        2
    );
    assert_eq!(
        changes
            .iter()
            .filter(|change| change.entity_type == "note_tag" && change.operation == "attach")
            .count(),
        2
    );
    assert!(changes.iter().any(|change| {
        change.entity_type == "note"
            && change.entity_id == created.note.id
            && change.operation == "insert"
    }));
    assert!(
        changes.iter().any(|change| {
            change.entity_type == "note_revision" && change.operation == "insert"
        })
    );
}

#[tokio::test]
async fn batch_create_rolls_back_when_one_item_fails() {
    let repository = notes_repository().await;
    let items = vec![
        input("first"),
        CreateNoteInput {
            content: "second".to_string(),
            field: None,
            tags: Vec::new(),
            role: "Robot".to_string(),
            device_id: None,
            links: Vec::new(),
        },
    ];

    let result = repository
        .create_notes_batch(DEFAULT_WORKSPACE_ID, items)
        .await;
    let notes = repository
        .list_notes(DEFAULT_WORKSPACE_ID, None)
        .await
        .unwrap();

    assert!(matches!(result, Err(ApiError::Database(_))));
    assert!(notes.is_empty());
}

#[tokio::test]
async fn update_note_writes_new_revision() {
    let repository = notes_repository().await;
    let created = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("old"))
        .await
        .unwrap();

    let updated = repository
        .update_note(
            DEFAULT_WORKSPACE_ID,
            &created.note.id,
            UpdateNoteInput {
                content: "new".to_string(),
                device_id: None,
                field: None,
                tags: None,
                links: None,
            },
        )
        .await
        .unwrap();
    let revisions = repository
        .list_note_revisions(DEFAULT_WORKSPACE_ID, &created.note.id)
        .await
        .unwrap();

    assert_eq!(updated.content, "new");
    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[1].content, "new");
    assert_eq!(updated.current_revision_id, Some(revisions[1].id.clone()));
}

#[tokio::test]
async fn update_note_sets_field_and_replaces_tags() {
    let repository = notes_repository().await;
    let created = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("old"))
        .await
        .unwrap();

    let updated = repository
        .update_note(
            DEFAULT_WORKSPACE_ID,
            &created.note.id,
            UpdateNoteInput {
                content: "new".to_string(),
                device_id: None,
                field: Some("personal".to_string()),
                tags: Some(vec!["api".to_string(), "sqlite".to_string()]),
                links: None,
            },
        )
        .await
        .unwrap();
    let tags = repository
        .list_note_tags(DEFAULT_WORKSPACE_ID, &created.note.id)
        .await
        .unwrap();
    let field_name = sqlx::query_scalar::<_, String>(
        "SELECT name FROM fields WHERE workspace_id = ? AND id = ?",
    )
    .bind(crate::repositories::taxonomy::DEFAULT_WORKSPACE_ID)
    .bind(updated.field_id.as_deref().unwrap())
    .fetch_one(&repository.pool)
    .await
    .unwrap();

    assert_eq!(updated.content, "new");
    assert_eq!(field_name, "personal");
    assert_eq!(
        tags.iter().map(|tag| tag.name.as_str()).collect::<Vec<_>>(),
        vec!["api", "sqlite"]
    );
}

#[tokio::test]
async fn update_note_keeps_field_and_tags_when_absent() {
    let repository = notes_repository().await;
    let created = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("old"))
        .await
        .unwrap();
    let original_tags = repository
        .list_note_tags(DEFAULT_WORKSPACE_ID, &created.note.id)
        .await
        .unwrap();

    let updated = repository
        .update_note(
            DEFAULT_WORKSPACE_ID,
            &created.note.id,
            UpdateNoteInput {
                content: "new".to_string(),
                device_id: None,
                field: None,
                tags: None,
                links: None,
            },
        )
        .await
        .unwrap();
    let tags = repository
        .list_note_tags(DEFAULT_WORKSPACE_ID, &created.note.id)
        .await
        .unwrap();

    assert_eq!(updated.field_id, created.note.field_id);
    assert_eq!(
        tags.iter().map(|tag| tag.name.as_str()).collect::<Vec<_>>(),
        original_tags
            .iter()
            .map(|tag| tag.name.as_str())
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn update_note_can_clear_all_tag_associations() {
    let repository = notes_repository().await;
    let created = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("old"))
        .await
        .unwrap();

    repository
        .update_note(
            DEFAULT_WORKSPACE_ID,
            &created.note.id,
            UpdateNoteInput {
                content: "new".to_string(),
                device_id: None,
                field: None,
                tags: Some(Vec::new()),
                links: None,
            },
        )
        .await
        .unwrap();
    let tags = repository
        .list_note_tags(DEFAULT_WORKSPACE_ID, &created.note.id)
        .await
        .unwrap();

    assert!(tags.is_empty());
}

#[tokio::test]
async fn create_note_writes_outgoing_links() {
    let repository = notes_repository().await;
    let target = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("target"))
        .await
        .unwrap();
    let mut source_input = input("source");
    source_input.links = vec![NoteLinkInput {
        target_note_ref: target.note.id[..8].to_string(),
        anchor_text: Some("target note".to_string()),
        position: Some(7),
    }];

    let created = repository
        .create_note(DEFAULT_WORKSPACE_ID, source_input)
        .await
        .unwrap();
    let outgoing = repository
        .list_visible_outgoing_links(DEFAULT_WORKSPACE_ID, &created.note.id)
        .await
        .unwrap();
    let backlinks = repository
        .list_visible_backlinks(DEFAULT_WORKSPACE_ID, &target.note.id)
        .await
        .unwrap();

    assert_eq!(created.links.len(), 1);
    assert_eq!(outgoing.len(), 1);
    assert_eq!(backlinks.len(), 1);
    assert_eq!(outgoing[0].source_note_id, created.note.id);
    assert_eq!(outgoing[0].target_note_id, target.note.id);
    assert_eq!(outgoing[0].anchor_text.as_deref(), Some("target note"));
    assert_eq!(outgoing[0].position, Some(7));
}

#[tokio::test]
async fn update_note_replaces_and_clears_outgoing_links() {
    let repository = notes_repository().await;
    let first = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("first target"))
        .await
        .unwrap();
    let second = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("second target"))
        .await
        .unwrap();
    let mut source_input = input("source");
    source_input.links = vec![NoteLinkInput {
        target_note_ref: first.note.id.clone(),
        anchor_text: Some("first".to_string()),
        position: Some(1),
    }];
    let source = repository
        .create_note(DEFAULT_WORKSPACE_ID, source_input)
        .await
        .unwrap();

    repository
        .update_note(
            DEFAULT_WORKSPACE_ID,
            &source.note.id,
            UpdateNoteInput {
                content: "new".to_string(),
                device_id: None,
                field: None,
                tags: None,
                links: Some(vec![NoteLinkInput {
                    target_note_ref: second.note.id.clone(),
                    anchor_text: Some("second".to_string()),
                    position: Some(2),
                }]),
            },
        )
        .await
        .unwrap();
    let outgoing = repository
        .list_visible_outgoing_links(DEFAULT_WORKSPACE_ID, &source.note.id)
        .await
        .unwrap();

    assert_eq!(outgoing.len(), 1);
    assert_eq!(outgoing[0].target_note_id, second.note.id);

    repository
        .update_note(
            DEFAULT_WORKSPACE_ID,
            &source.note.id,
            UpdateNoteInput {
                content: "clear".to_string(),
                device_id: None,
                field: None,
                tags: None,
                links: Some(Vec::new()),
            },
        )
        .await
        .unwrap();
    let outgoing = repository
        .list_visible_outgoing_links(DEFAULT_WORKSPACE_ID, &source.note.id)
        .await
        .unwrap();

    assert!(outgoing.is_empty());
}

#[tokio::test]
async fn update_note_keeps_links_when_absent() {
    let repository = notes_repository().await;
    let target = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("target"))
        .await
        .unwrap();
    let mut source_input = input("source");
    source_input.links = vec![NoteLinkInput {
        target_note_ref: target.note.id.clone(),
        anchor_text: None,
        position: None,
    }];
    let source = repository
        .create_note(DEFAULT_WORKSPACE_ID, source_input)
        .await
        .unwrap();

    repository
        .update_note(
            DEFAULT_WORKSPACE_ID,
            &source.note.id,
            UpdateNoteInput {
                content: "new".to_string(),
                device_id: None,
                field: None,
                tags: None,
                links: None,
            },
        )
        .await
        .unwrap();
    let outgoing = repository
        .list_visible_outgoing_links(DEFAULT_WORKSPACE_ID, &source.note.id)
        .await
        .unwrap();

    assert_eq!(outgoing.len(), 1);
    assert_eq!(outgoing[0].target_note_id, target.note.id);
}

#[tokio::test]
async fn note_links_reject_hidden_and_self_targets() {
    let repository = notes_repository().await;
    let archived = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("archived"))
        .await
        .unwrap();
    repository
        .archive_note(DEFAULT_WORKSPACE_ID, &archived.note.id)
        .await
        .unwrap();
    let deleted = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("deleted"))
        .await
        .unwrap();
    repository
        .delete_note(DEFAULT_WORKSPACE_ID, &deleted.note.id)
        .await
        .unwrap();
    let source = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("source"))
        .await
        .unwrap();

    let archived_result = repository
        .update_note(
            DEFAULT_WORKSPACE_ID,
            &source.note.id,
            UpdateNoteInput {
                content: "archived".to_string(),
                device_id: None,
                field: None,
                tags: None,
                links: Some(vec![NoteLinkInput {
                    target_note_ref: archived.note.id,
                    anchor_text: None,
                    position: None,
                }]),
            },
        )
        .await;
    let deleted_result = repository
        .update_note(
            DEFAULT_WORKSPACE_ID,
            &source.note.id,
            UpdateNoteInput {
                content: "deleted".to_string(),
                device_id: None,
                field: None,
                tags: None,
                links: Some(vec![NoteLinkInput {
                    target_note_ref: deleted.note.id,
                    anchor_text: None,
                    position: None,
                }]),
            },
        )
        .await;
    let self_result = repository
        .update_note(
            DEFAULT_WORKSPACE_ID,
            &source.note.id,
            UpdateNoteInput {
                content: "self".to_string(),
                device_id: None,
                field: None,
                tags: None,
                links: Some(vec![NoteLinkInput {
                    target_note_ref: source.note.id.clone(),
                    anchor_text: None,
                    position: None,
                }]),
            },
        )
        .await;

    assert!(matches!(archived_result, Err(ApiError::RecordNotFound(_))));
    assert!(matches!(deleted_result, Err(ApiError::RecordNotFound(_))));
    assert!(matches!(self_result, Err(ApiError::Validation)));
}

#[tokio::test]
async fn update_archive_and_delete_record_note_sync_changes() {
    let repository = notes_repository().await;
    let created = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("old"))
        .await
        .unwrap();
    let deleted_note = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("delete me"))
        .await
        .unwrap();

    repository
        .update_note(
            DEFAULT_WORKSPACE_ID,
            &created.note.id,
            UpdateNoteInput {
                content: "new".to_string(),
                device_id: None,
                field: None,
                tags: None,
                links: None,
            },
        )
        .await
        .unwrap();
    repository
        .archive_note(DEFAULT_WORKSPACE_ID, &created.note.id)
        .await
        .unwrap();
    repository
        .delete_note(DEFAULT_WORKSPACE_ID, &deleted_note.note.id)
        .await
        .unwrap();
    let changes = list_sync_changes(&repository.pool).await.unwrap();

    assert!(
        changes.iter().any(|change| {
            change.entity_type == "note_revision" && change.operation == "insert"
        })
    );
    assert!(
        changes
            .iter()
            .any(|change| { change.entity_type == "note" && change.operation == "update" })
    );
    assert!(
        changes
            .iter()
            .any(|change| { change.entity_type == "note" && change.operation == "delete" })
    );
}

#[tokio::test]
async fn delete_note_hides_record_from_default_reads() {
    let repository = notes_repository().await;
    let created = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("hidden"))
        .await
        .unwrap();

    repository
        .delete_note(DEFAULT_WORKSPACE_ID, &created.note.id)
        .await
        .unwrap();
    let list = repository
        .list_notes(DEFAULT_WORKSPACE_ID, None)
        .await
        .unwrap();
    let get = repository
        .get_note_by_ref(DEFAULT_WORKSPACE_ID, &created.note.id)
        .await;

    assert!(list.is_empty());
    assert!(matches!(get, Err(ApiError::RecordNotFound(_))));
}

#[tokio::test]
async fn taxonomy_lists_records_by_name() {
    let repository = taxonomy_repository().await;

    repository.get_or_create_field("zeta").await.unwrap();
    repository.get_or_create_field("alpha").await.unwrap();
    repository.get_or_create_tag("rust").await.unwrap();
    repository.get_or_create_tag("api").await.unwrap();

    let fields = repository.list_fields(None).await.unwrap();
    let tags = repository.list_tags(None).await.unwrap();

    assert_eq!(
        fields
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        vec!["alpha", "zeta"]
    );
    assert_eq!(
        tags.iter().map(|tag| tag.name.as_str()).collect::<Vec<_>>(),
        vec!["api", "rust"]
    );
}

#[tokio::test]
async fn taxonomy_creates_hierarchical_tag_nodes() {
    let repository = notes_repository().await;
    let mut input = input("tagged");
    input.tags = Vec::new();
    let created = repository
        .create_note(DEFAULT_WORKSPACE_ID, input)
        .await
        .unwrap();

    repository
        .add_tag_to_note(DEFAULT_WORKSPACE_ID, &created.note.id, "rust/web")
        .await
        .unwrap();

    let tags = repository
        .list_note_tags(DEFAULT_WORKSPACE_ID, &created.note.id)
        .await
        .unwrap();
    let rows = sqlx::query_as::<_, (String, Option<String>, String, i64)>(
        "SELECT name, parent_tag_id, path, depth FROM tags WHERE workspace_id = ? ORDER BY depth ASC, path ASC",
    )
    .bind(DEFAULT_WORKSPACE_ID)
    .fetch_all(&repository.pool)
    .await
    .unwrap();

    assert_eq!(
        tags.iter().map(|tag| tag.name.as_str()).collect::<Vec<_>>(),
        vec!["web"]
    );
    assert_eq!(
        tags.iter().map(|tag| tag.path.as_str()).collect::<Vec<_>>(),
        vec!["rust/web"]
    );
    assert_eq!(rows[0], ("rust".to_string(), None, "rust".to_string(), 0));
    assert_eq!(rows[1].0, "web");
    assert_eq!(rows[1].2, "rust/web");
    assert_eq!(rows[1].3, 1);
}

#[tokio::test]
async fn taxonomy_creates_sync_changes_only_for_new_records() {
    let (repository, pool) = taxonomy_repository_with_pool().await;

    repository.get_or_create_field("work").await.unwrap();
    repository.get_or_create_field("work").await.unwrap();
    repository.get_or_create_tag("rust").await.unwrap();
    repository.get_or_create_tag("rust").await.unwrap();
    let changes = list_sync_changes(&pool).await.unwrap();

    assert_eq!(
        changes
            .iter()
            .filter(|change| change.entity_type == "field" && change.operation == "insert")
            .count(),
        1
    );
    assert_eq!(
        changes
            .iter()
            .filter(|change| change.entity_type == "tag" && change.operation == "insert")
            .count(),
        1
    );
}

#[tokio::test]
async fn tag_association_is_idempotent_and_removable() {
    let repository = notes_repository().await;
    let created = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("tagged"))
        .await
        .unwrap();

    repository
        .add_tag_to_note(DEFAULT_WORKSPACE_ID, &created.note.id, "rust")
        .await
        .unwrap();
    repository
        .remove_tag_from_note(DEFAULT_WORKSPACE_ID, &created.note.id, "rust")
        .await
        .unwrap();
    let tags = repository
        .list_note_tags(DEFAULT_WORKSPACE_ID, &created.note.id)
        .await
        .unwrap();

    assert_eq!(
        tags.iter().map(|tag| tag.name.as_str()).collect::<Vec<_>>(),
        vec!["sqlite"]
    );

    let changes = list_sync_changes(&repository.pool).await.unwrap();
    assert_eq!(
        changes
            .iter()
            .filter(|change| change.entity_type == "note_tag" && change.operation == "detach")
            .count(),
        1
    );
}

#[tokio::test]
async fn list_recent_notes_orders_and_filters_hidden_records() {
    let repository = notes_repository().await;
    let oldest = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("oldest"))
        .await
        .unwrap();
    let archived = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("archived"))
        .await
        .unwrap();
    let deleted = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("deleted"))
        .await
        .unwrap();
    let newest = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("newest"))
        .await
        .unwrap();

    sqlx::query("UPDATE notes SET updated_at = ? WHERE id = ?")
        .bind(2_000_000_010_i64)
        .bind(&oldest.note.id)
        .execute(&repository.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE notes SET updated_at = ? WHERE id = ?")
        .bind(2_000_000_040_i64)
        .bind(&archived.note.id)
        .execute(&repository.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE notes SET updated_at = ? WHERE id = ?")
        .bind(2_000_000_030_i64)
        .bind(&deleted.note.id)
        .execute(&repository.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE notes SET updated_at = ? WHERE id = ?")
        .bind(2_000_000_020_i64)
        .bind(&newest.note.id)
        .execute(&repository.pool)
        .await
        .unwrap();
    repository
        .archive_note(DEFAULT_WORKSPACE_ID, &archived.note.id)
        .await
        .unwrap();
    repository
        .delete_note(DEFAULT_WORKSPACE_ID, &deleted.note.id)
        .await
        .unwrap();

    let recent = repository
        .list_recent_notes(DEFAULT_WORKSPACE_ID, 10, None, RecentNotesRoleFilter::Both)
        .await
        .unwrap();

    assert_eq!(
        recent
            .iter()
            .map(|note| note.content.as_str())
            .collect::<Vec<_>>(),
        vec!["newest", "oldest"]
    );
}

#[tokio::test]
async fn list_recent_notes_applies_limit() {
    let repository = notes_repository().await;
    repository
        .create_note(DEFAULT_WORKSPACE_ID, input("first"))
        .await
        .unwrap();
    repository
        .create_note(DEFAULT_WORKSPACE_ID, input("second"))
        .await
        .unwrap();

    let recent = repository
        .list_recent_notes(DEFAULT_WORKSPACE_ID, 1, None, RecentNotesRoleFilter::Both)
        .await
        .unwrap();

    assert_eq!(recent.len(), 1);
}

#[tokio::test]
async fn list_recent_notes_filters_by_role() {
    let repository = notes_repository().await;
    let human_old = repository
        .create_note(DEFAULT_WORKSPACE_ID, input_with_role("human old", "Human"))
        .await
        .unwrap();
    let agent = repository
        .create_note(DEFAULT_WORKSPACE_ID, input_with_role("agent", "Agent"))
        .await
        .unwrap();
    let human_new = repository
        .create_note(DEFAULT_WORKSPACE_ID, input_with_role("human new", "Human"))
        .await
        .unwrap();

    sqlx::query("UPDATE notes SET updated_at = ? WHERE id = ?")
        .bind(2_000_000_010_i64)
        .bind(&human_old.note.id)
        .execute(&repository.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE notes SET updated_at = ? WHERE id = ?")
        .bind(2_000_000_020_i64)
        .bind(&agent.note.id)
        .execute(&repository.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE notes SET updated_at = ? WHERE id = ?")
        .bind(2_000_000_030_i64)
        .bind(&human_new.note.id)
        .execute(&repository.pool)
        .await
        .unwrap();

    let humans = repository
        .list_recent_notes(DEFAULT_WORKSPACE_ID, 10, None, RecentNotesRoleFilter::Human)
        .await
        .unwrap();
    let agents = repository
        .list_recent_notes(DEFAULT_WORKSPACE_ID, 10, None, RecentNotesRoleFilter::Agent)
        .await
        .unwrap();
    let both = repository
        .list_recent_notes(DEFAULT_WORKSPACE_ID, 10, None, RecentNotesRoleFilter::Both)
        .await
        .unwrap();

    assert_eq!(
        humans
            .iter()
            .map(|note| note.content.as_str())
            .collect::<Vec<_>>(),
        vec!["human new", "human old"]
    );
    assert_eq!(
        agents
            .iter()
            .map(|note| note.content.as_str())
            .collect::<Vec<_>>(),
        vec!["agent"]
    );
    assert_eq!(
        both.iter()
            .map(|note| note.content.as_str())
            .collect::<Vec<_>>(),
        vec!["human new", "agent", "human old"]
    );
}

#[tokio::test]
async fn list_recent_notes_applies_role_filter_with_limit_and_cursor() {
    let repository = notes_repository().await;
    let human_old = repository
        .create_note(DEFAULT_WORKSPACE_ID, input_with_role("human old", "Human"))
        .await
        .unwrap();
    let agent_between = repository
        .create_note(
            DEFAULT_WORKSPACE_ID,
            input_with_role("agent between", "Agent"),
        )
        .await
        .unwrap();
    let human_cursor = repository
        .create_note(
            DEFAULT_WORKSPACE_ID,
            input_with_role("human cursor", "Human"),
        )
        .await
        .unwrap();
    let human_new = repository
        .create_note(DEFAULT_WORKSPACE_ID, input_with_role("human new", "Human"))
        .await
        .unwrap();

    for (note_id, updated_at) in [
        (&human_old.note.id, 2_000_000_010_i64),
        (&agent_between.note.id, 2_000_000_020_i64),
        (&human_cursor.note.id, 2_000_000_030_i64),
        (&human_new.note.id, 2_000_000_040_i64),
    ] {
        sqlx::query("UPDATE notes SET updated_at = ? WHERE id = ?")
            .bind(updated_at)
            .bind(note_id)
            .execute(&repository.pool)
            .await
            .unwrap();
    }

    let humans = repository
        .list_recent_notes(
            DEFAULT_WORKSPACE_ID,
            1,
            Some(&human_cursor.note.id),
            RecentNotesRoleFilter::Human,
        )
        .await
        .unwrap();

    assert_eq!(humans.len(), 1);
    assert_eq!(humans[0].content, "human old");
}

#[tokio::test]
async fn list_recent_notes_uses_full_note_uuid_cursor() {
    let repository = notes_repository().await;
    let oldest = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("oldest"))
        .await
        .unwrap();
    let cursor = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("cursor"))
        .await
        .unwrap();
    let newest = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("newest"))
        .await
        .unwrap();

    sqlx::query("UPDATE notes SET updated_at = ? WHERE id = ?")
        .bind(2_000_000_010_i64)
        .bind(&oldest.note.id)
        .execute(&repository.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE notes SET updated_at = ? WHERE id = ?")
        .bind(2_000_000_020_i64)
        .bind(&cursor.note.id)
        .execute(&repository.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE notes SET updated_at = ? WHERE id = ?")
        .bind(2_000_000_030_i64)
        .bind(&newest.note.id)
        .execute(&repository.pool)
        .await
        .unwrap();

    let recent = repository
        .list_recent_notes(
            DEFAULT_WORKSPACE_ID,
            10,
            Some(&cursor.note.id),
            RecentNotesRoleFilter::Both,
        )
        .await
        .unwrap();

    assert_eq!(
        recent
            .iter()
            .map(|note| note.content.as_str())
            .collect::<Vec<_>>(),
        vec!["oldest"]
    );
}

#[tokio::test]
async fn list_recent_notes_uses_id_tiebreaker_for_cursor() {
    let repository = notes_repository().await;
    let low_id = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("low"))
        .await
        .unwrap();
    let cursor_id = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("cursor"))
        .await
        .unwrap();
    let high_id = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("high"))
        .await
        .unwrap();

    sqlx::query("UPDATE notes SET id = ?, updated_at = ? WHERE id = ?")
        .bind("10000000000000000000000000000000")
        .bind(2_000_000_010_i64)
        .bind(&low_id.note.id)
        .execute(&repository.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE notes SET id = ?, updated_at = ? WHERE id = ?")
        .bind("20000000000000000000000000000000")
        .bind(2_000_000_010_i64)
        .bind(&cursor_id.note.id)
        .execute(&repository.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE notes SET id = ?, updated_at = ? WHERE id = ?")
        .bind("30000000000000000000000000000000")
        .bind(2_000_000_010_i64)
        .bind(&high_id.note.id)
        .execute(&repository.pool)
        .await
        .unwrap();

    let recent = repository
        .list_recent_notes(
            DEFAULT_WORKSPACE_ID,
            10,
            Some("20000000000000000000000000000000"),
            RecentNotesRoleFilter::Both,
        )
        .await
        .unwrap();

    assert_eq!(
        recent
            .iter()
            .map(|note| note.content.as_str())
            .collect::<Vec<_>>(),
        vec!["low"]
    );
}

#[tokio::test]
async fn list_recent_notes_rejects_invalid_or_hidden_cursor() {
    let repository = notes_repository().await;
    let archived = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("archived"))
        .await
        .unwrap();
    repository
        .archive_note(DEFAULT_WORKSPACE_ID, &archived.note.id)
        .await
        .unwrap();

    let invalid = repository
        .list_recent_notes(
            DEFAULT_WORKSPACE_ID,
            10,
            Some("abcd"),
            RecentNotesRoleFilter::Both,
        )
        .await;
    let hidden = repository
        .list_recent_notes(
            DEFAULT_WORKSPACE_ID,
            10,
            Some(&archived.note.id),
            RecentNotesRoleFilter::Both,
        )
        .await;
    let missing = repository
        .list_recent_notes(
            DEFAULT_WORKSPACE_ID,
            10,
            Some("ffffffffffffffffffffffffffffffff"),
            RecentNotesRoleFilter::Both,
        )
        .await;

    assert!(matches!(invalid, Err(ApiError::Validation)));
    assert!(matches!(hidden, Err(ApiError::RecordNotFound(_))));
    assert!(matches!(missing, Err(ApiError::RecordNotFound(_))));
}

#[tokio::test]
async fn list_random_notes_filters_hidden_records_and_applies_limit() {
    let repository = notes_repository().await;
    repository
        .create_note(DEFAULT_WORKSPACE_ID, input("first"))
        .await
        .unwrap();
    repository
        .create_note(DEFAULT_WORKSPACE_ID, input("second"))
        .await
        .unwrap();
    let archived = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("archived"))
        .await
        .unwrap();
    let deleted = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("deleted"))
        .await
        .unwrap();
    repository
        .archive_note(DEFAULT_WORKSPACE_ID, &archived.note.id)
        .await
        .unwrap();
    repository
        .delete_note(DEFAULT_WORKSPACE_ID, &deleted.note.id)
        .await
        .unwrap();

    let notes = repository
        .list_random_notes(DEFAULT_WORKSPACE_ID, 1)
        .await
        .unwrap();

    assert_eq!(notes.len(), 1);
    assert!(notes[0].content == "first" || notes[0].content == "second");
}

#[tokio::test]
async fn list_random_notes_returns_empty_when_none_exist() {
    let repository = notes_repository().await;

    let notes = repository
        .list_random_notes(DEFAULT_WORKSPACE_ID, 50)
        .await
        .unwrap();

    assert!(notes.is_empty());
}

#[tokio::test]
async fn random_tags_returns_existing_tags_when_limit_is_larger() {
    let repository = notes_repository().await;
    repository
        .create_note(DEFAULT_WORKSPACE_ID, input("first"))
        .await
        .unwrap();

    let tags = repository
        .random_tags(DEFAULT_WORKSPACE_ID, 20)
        .await
        .unwrap();

    assert_eq!(tags.len(), 2);
    assert!(
        tags.iter()
            .all(|tag| tag.name == "rust" || tag.name == "sqlite")
    );
}

#[tokio::test]
async fn list_visible_notes_by_tag_filters_hidden_records() {
    let repository = notes_repository().await;
    let visible = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("visible"))
        .await
        .unwrap();
    let archived = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("archived"))
        .await
        .unwrap();
    let deleted = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("deleted"))
        .await
        .unwrap();
    repository
        .archive_note(DEFAULT_WORKSPACE_ID, &archived.note.id)
        .await
        .unwrap();
    repository
        .delete_note(DEFAULT_WORKSPACE_ID, &deleted.note.id)
        .await
        .unwrap();
    let tag = repository
        .list_note_tags(DEFAULT_WORKSPACE_ID, &visible.note.id)
        .await
        .unwrap()
        .into_iter()
        .find(|tag| tag.name == "rust")
        .unwrap();

    let notes = repository
        .list_visible_notes_by_tag(DEFAULT_WORKSPACE_ID, &tag.id, 10)
        .await
        .unwrap();

    assert_eq!(
        notes
            .iter()
            .map(|note| note.content.as_str())
            .collect::<Vec<_>>(),
        vec!["visible"]
    );
}

#[tokio::test]
async fn list_visible_notes_by_tag_applies_limit() {
    let repository = notes_repository().await;
    let first = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("first"))
        .await
        .unwrap();
    repository
        .create_note(DEFAULT_WORKSPACE_ID, input("second"))
        .await
        .unwrap();
    let tag = repository
        .list_note_tags(DEFAULT_WORKSPACE_ID, &first.note.id)
        .await
        .unwrap()
        .into_iter()
        .find(|tag| tag.name == "rust")
        .unwrap();

    let notes = repository
        .list_visible_notes_by_tag(DEFAULT_WORKSPACE_ID, &tag.id, 1)
        .await
        .unwrap();

    assert_eq!(notes.len(), 1);
}

#[tokio::test]
async fn random_fields_returns_existing_fields_when_limit_is_larger() {
    let repository = notes_repository().await;
    repository
        .create_note(DEFAULT_WORKSPACE_ID, input("first"))
        .await
        .unwrap();

    let fields = repository
        .random_fields(DEFAULT_WORKSPACE_ID, 20)
        .await
        .unwrap();

    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].name, "work");
}

#[tokio::test]
async fn list_visible_notes_by_field_filters_hidden_records_and_applies_limit() {
    let repository = notes_repository().await;
    let visible = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("visible"))
        .await
        .unwrap();
    repository
        .create_note(DEFAULT_WORKSPACE_ID, input("second visible"))
        .await
        .unwrap();
    let archived = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("archived"))
        .await
        .unwrap();
    let deleted = repository
        .create_note(DEFAULT_WORKSPACE_ID, input("deleted"))
        .await
        .unwrap();
    repository
        .archive_note(DEFAULT_WORKSPACE_ID, &archived.note.id)
        .await
        .unwrap();
    repository
        .delete_note(DEFAULT_WORKSPACE_ID, &deleted.note.id)
        .await
        .unwrap();
    let field_id = visible.note.field_id.as_deref().unwrap();

    let notes = repository
        .list_visible_notes_by_field(DEFAULT_WORKSPACE_ID, field_id, 1)
        .await
        .unwrap();

    assert_eq!(notes.len(), 1);
    assert!(notes[0].content == "visible" || notes[0].content == "second visible");
}
