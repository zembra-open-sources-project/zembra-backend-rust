mod support;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{Value, json};
use zembra_backend_rust::repositories::taxonomy::DEFAULT_WORKSPACE_ID;

const SECOND_WORKSPACE_ID: &str = "11111111-1111-4111-8111-111111111111";
const ARCHIVED_WORKSPACE_ID: &str = "22222222-2222-4222-8222-222222222222";
const DELETED_WORKSPACE_ID: &str = "33333333-3333-4333-8333-333333333333";

#[tokio::test]
async fn create_note_without_workspace_id_returns_not_found() {
    let response = support::app::send(
        Request::builder()
            .method("POST")
            .uri("/notes")
            .header("content-type", "application/json")
            .body(Body::from(json!({ "content": "api note" }).to_string()))
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body: Value = support::app::response_json(response).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "record_not_found");
}

#[tokio::test]
async fn create_note_route_returns_created_note() {
    let response = support::app::send(
        Request::builder()
            .method("POST")
            .uri(format!("/notes?workspace_id={DEFAULT_WORKSPACE_ID}"))
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "content": "api note",
                    "field": "work",
                    "tags": ["rust", "rust", "api"],
                    "role": "Human",
                    "device_id": null
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body: Value = support::app::response_json(response).await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["note"]["content"], "api note");
    assert_eq!(body["metadata"]["field"], "work");
    assert_eq!(body["metadata"]["tags"], json!(["rust", "api"]));
}

#[tokio::test]
async fn create_note_rejects_invalid_role() {
    let response = support::app::send(
        Request::builder()
            .method("POST")
            .uri(format!("/notes?workspace_id={DEFAULT_WORKSPACE_ID}"))
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "content": "api note",
                    "role": "Robot"
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body: Value = support::app::response_json(response).await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "validation_error");
}

#[tokio::test]
async fn notes_routes_return_not_found_for_invalid_or_inactive_workspace() {
    let state = support::app::test_state().await;
    seed_workspace(&state, ARCHIVED_WORKSPACE_ID, Some(100), None).await;
    seed_workspace(&state, DELETED_WORKSPACE_ID, None, Some(100)).await;

    for workspace_id in [
        "not-a-uuid",
        "44444444-4444-4444-8444-444444444444",
        ARCHIVED_WORKSPACE_ID,
        DELETED_WORKSPACE_ID,
    ] {
        let response = support::app::send_with_state(
            state.clone(),
            Request::builder()
                .method("GET")
                .uri(format!("/notes?workspace_id={workspace_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        let status = response.status();
        let body: Value = support::app::response_json(response).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "record_not_found");
    }
}

#[tokio::test]
async fn list_notes_route_is_scoped_to_query_workspace() {
    let state = support::app::test_state().await;
    seed_workspace(&state, SECOND_WORKSPACE_ID, None, None).await;
    seed_note(&state, DEFAULT_WORKSPACE_ID, "default workspace").await;
    seed_note(&state, SECOND_WORKSPACE_ID, "second workspace").await;

    let response = support::app::send_with_state(
        state,
        Request::builder()
            .method("GET")
            .uri(format!("/notes?workspace_id={SECOND_WORKSPACE_ID}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body: Value = support::app::response_json(response).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["notes"].as_array().unwrap().len(), 1);
    assert_eq!(body["notes"][0]["content"], "second workspace");
}

#[tokio::test]
async fn create_note_records_sync_change_for_query_workspace() {
    let state = support::app::test_state().await;
    seed_workspace(&state, SECOND_WORKSPACE_ID, None, None).await;
    support::notes::TestNoteBuilder::new("default first")
        .create_in_workspace(&state, DEFAULT_WORKSPACE_ID)
        .await;

    let response = support::app::send_with_state(
        state.clone(),
        Request::builder()
            .method("POST")
            .uri(format!("/notes?workspace_id={SECOND_WORKSPACE_ID}"))
            .header("content-type", "application/json")
            .body(Body::from(json!({ "content": "synced note" }).to_string()))
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body: Value = support::app::response_json(response).await;
    let note_id = body["note"]["id"].as_str().unwrap();
    let changes = zembra_backend_rust::repositories::sync::list_sync_changes(&state.database.pool)
        .await
        .unwrap();
    let note_change = changes
        .iter()
        .find(|change| change.entity_type == "note" && change.entity_id == note_id)
        .unwrap();
    let payload: Value = serde_json::from_str(&note_change.payload).unwrap();

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(note_change.workspace_id, SECOND_WORKSPACE_ID);
    assert_eq!(payload["workspace_id"], SECOND_WORKSPACE_ID);
}

/// Inserts a workspace row for CRUD route tests.
///
/// # Arguments
///
/// * `state` - Shared application state.
/// * `workspace_id` - Workspace identifier to insert.
/// * `archived_at` - Optional archived timestamp.
/// * `deleted_at` - Optional deleted timestamp.
async fn seed_workspace(
    state: &zembra_backend_rust::app::AppState,
    workspace_id: &str,
    archived_at: Option<i64>,
    deleted_at: Option<i64>,
) {
    sqlx::query(
        "INSERT INTO workspaces (id, workspace_name, created_at, updated_at, archived_at, deleted_at)
         VALUES (?, NULL, 1, 1, ?, ?)",
    )
    .bind(workspace_id)
    .bind(archived_at)
    .bind(deleted_at)
    .execute(&state.database.pool)
    .await
    .unwrap();
}

/// Inserts a note row for CRUD route tests without recording sync changes.
///
/// # Arguments
///
/// * `state` - Shared application state.
/// * `workspace_id` - Workspace identifier for the note.
/// * `content` - Note content to insert.
async fn seed_note(state: &zembra_backend_rust::app::AppState, workspace_id: &str, content: &str) {
    let note_id = uuid::Uuid::new_v4().simple().to_string();
    sqlx::query(
        "INSERT INTO notes
         (id, workspace_id, content, role, created_at, updated_at, archived_at, deleted_at)
         VALUES (?, ?, ?, 'Human', 1, 1, NULL, NULL)",
    )
    .bind(note_id)
    .bind(workspace_id)
    .bind(content)
    .execute(&state.database.pool)
    .await
    .unwrap();
}
