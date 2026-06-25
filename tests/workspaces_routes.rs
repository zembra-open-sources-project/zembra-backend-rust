mod support;

use axum::body::Body;
use axum::http::{Request, StatusCode};

#[tokio::test]
async fn list_workspaces_returns_visible_note_summary_in_stable_order() {
    let state = support::app::test_state().await;
    seed_workspace(
        &state,
        "aaaaaaaa-0000-4000-8000-000000000001",
        Some("alpha"),
        10,
    )
    .await;
    seed_workspace(
        &state,
        "bbbbbbbb-0000-4000-8000-000000000002",
        Some("beta"),
        20,
    )
    .await;
    seed_workspace(&state, "cccccccc-0000-4000-8000-000000000003", None, 30).await;
    seed_note(
        &state,
        "note-a1",
        "aaaaaaaa-0000-4000-8000-000000000001",
        100,
        None,
        None,
    )
    .await;
    seed_note(
        &state,
        "note-a2",
        "aaaaaaaa-0000-4000-8000-000000000001",
        120,
        None,
        None,
    )
    .await;
    seed_note(
        &state,
        "note-b1",
        "bbbbbbbb-0000-4000-8000-000000000002",
        200,
        None,
        None,
    )
    .await;
    seed_note(
        &state,
        "note-b2",
        "bbbbbbbb-0000-4000-8000-000000000002",
        210,
        Some(220),
        None,
    )
    .await;
    seed_note(
        &state,
        "note-b3",
        "bbbbbbbb-0000-4000-8000-000000000002",
        220,
        None,
        Some(230),
    )
    .await;

    let response = support::app::send_with_state(
        state,
        Request::builder()
            .uri("/workspaces")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["workspaces"][0]["workspace_id"],
        "aaaaaaaa-0000-4000-8000-000000000001"
    );
    assert_eq!(body["workspaces"][0]["short_hash"], "aaaaaaaa");
    assert_eq!(body["workspaces"][0]["workspace_name"], "alpha");
    assert_eq!(body["workspaces"][0]["visible_note_count"], 2);
    assert_eq!(body["workspaces"][0]["latest_note_created_at"], 120);
    assert_eq!(
        body["workspaces"][1]["workspace_id"],
        "bbbbbbbb-0000-4000-8000-000000000002"
    );
    assert_eq!(body["workspaces"][1]["workspace_name"], "beta");
    assert_eq!(body["workspaces"][1]["visible_note_count"], 1);
    assert_eq!(body["workspaces"][1]["latest_note_created_at"], 200);
    assert_eq!(
        body["workspaces"][2]["workspace_id"],
        "00000000-0000-4000-8000-000000000300"
    );
    assert_eq!(body["workspaces"][2]["visible_note_count"], 0);
    assert!(body["workspaces"][2]["latest_note_created_at"].is_null());
    assert_eq!(
        body["workspaces"][3]["workspace_id"],
        "cccccccc-0000-4000-8000-000000000003"
    );
    assert!(body["workspaces"][3]["workspace_name"].is_null());
    assert_eq!(body["workspaces"][3]["visible_note_count"], 0);
    assert!(body["workspaces"][3]["latest_note_created_at"].is_null());
}

/// Inserts a workspace row for route tests.
///
/// # Arguments
///
/// * `state` - Application state containing the test database.
/// * `workspace_id` - Workspace identifier to insert.
/// * `workspace_name` - Optional workspace display name.
/// * `created_at` - Workspace creation timestamp.
async fn seed_workspace(
    state: &zembra_backend_rust::app::AppState,
    workspace_id: &str,
    workspace_name: Option<&str>,
    created_at: i64,
) {
    sqlx::query(
        "INSERT INTO workspaces (id, workspace_name, created_at, updated_at)
         VALUES (?, ?, ?, ?)",
    )
    .bind(workspace_id)
    .bind(workspace_name)
    .bind(created_at)
    .bind(created_at)
    .execute(&state.database.pool)
    .await
    .unwrap();
}

/// Inserts a note row for route tests.
///
/// # Arguments
///
/// * `state` - Application state containing the test database.
/// * `note_id` - Note identifier to insert.
/// * `workspace_id` - Owning workspace identifier.
/// * `created_at` - Note creation timestamp.
/// * `archived_at` - Optional archive timestamp.
/// * `deleted_at` - Optional deletion timestamp.
async fn seed_note(
    state: &zembra_backend_rust::app::AppState,
    note_id: &str,
    workspace_id: &str,
    created_at: i64,
    archived_at: Option<i64>,
    deleted_at: Option<i64>,
) {
    sqlx::query(
        "INSERT INTO notes
         (id, workspace_id, content, role, created_at, updated_at, archived_at, deleted_at)
         VALUES (?, ?, 'content', 'Human', ?, ?, ?, ?)",
    )
    .bind(note_id)
    .bind(workspace_id)
    .bind(created_at)
    .bind(created_at)
    .bind(archived_at)
    .bind(deleted_at)
    .execute(&state.database.pool)
    .await
    .unwrap();
}
