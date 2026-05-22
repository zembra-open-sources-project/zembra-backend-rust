mod support;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;

#[tokio::test]
async fn patch_note_updates_content_field_and_tags() {
    let state = support::app::test_state().await;
    let note_id = support::notes::TestNoteBuilder::new("old")
        .field("work")
        .tags(["rust", "sqlite"])
        .create(&state)
        .await;

    let response = support::app::send_with_state(
        state.clone(),
        Request::builder()
            .method("PATCH")
            .uri(format!("/notes/{note_id}"))
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "content": "new",
                    "field": "personal",
                    "tags": ["api", "api", " sqlite ", ""]
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;
    let tags = zembra_backend_rust::services::notes::NotesService::new(state.database.pool.clone())
        .list_note_tags(&note_id)
        .await
        .unwrap();
    let field_name = sqlx::query_scalar::<_, String>(
        "SELECT name FROM fields WHERE workspace_id = ? AND id = ?",
    )
    .bind(zembra_backend_rust::repositories::taxonomy::DEFAULT_WORKSPACE_ID)
    .bind(body["note"]["field_id"].as_str().unwrap())
    .fetch_one(&state.database.pool)
    .await
    .unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["note"]["content"], "new");
    assert_eq!(field_name, "personal");
    assert_eq!(
        tags.iter().map(|tag| tag.name.as_str()).collect::<Vec<_>>(),
        vec!["api", "sqlite"]
    );
}

#[tokio::test]
async fn patch_note_keeps_missing_field_and_tags_unchanged() {
    let state = support::app::test_state().await;
    let note_id = support::notes::TestNoteBuilder::new("old")
        .field("work")
        .tags(["rust"])
        .create(&state)
        .await;

    let response = support::app::send_with_state(
        state.clone(),
        Request::builder()
            .method("PATCH")
            .uri(format!("/notes/{note_id}"))
            .header("content-type", "application/json")
            .body(Body::from(json!({ "content": "new" }).to_string()))
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;
    let tags = zembra_backend_rust::services::notes::NotesService::new(state.database.pool.clone())
        .list_note_tags(&note_id)
        .await
        .unwrap();
    let field_name = sqlx::query_scalar::<_, String>(
        "SELECT name FROM fields WHERE workspace_id = ? AND id = ?",
    )
    .bind(zembra_backend_rust::repositories::taxonomy::DEFAULT_WORKSPACE_ID)
    .bind(body["note"]["field_id"].as_str().unwrap())
    .fetch_one(&state.database.pool)
    .await
    .unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["note"]["content"], "new");
    assert_eq!(field_name, "work");
    assert_eq!(
        tags.iter().map(|tag| tag.name.as_str()).collect::<Vec<_>>(),
        vec!["rust"]
    );
}

#[tokio::test]
async fn patch_note_null_field_uses_inbox() {
    let state = support::app::test_state().await;
    let note_id = support::notes::create_field_note(&state, "old", "work").await;

    let response = support::app::send_with_state(
        state.clone(),
        Request::builder()
            .method("PATCH")
            .uri(format!("/notes/{note_id}"))
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "content": "new",
                    "field": null
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;
    let field_name = sqlx::query_scalar::<_, String>(
        "SELECT name FROM fields WHERE workspace_id = ? AND id = ?",
    )
    .bind(zembra_backend_rust::repositories::taxonomy::DEFAULT_WORKSPACE_ID)
    .bind(body["note"]["field_id"].as_str().unwrap())
    .fetch_one(&state.database.pool)
    .await
    .unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(field_name, "inbox");
}

#[tokio::test]
async fn patch_note_rejects_blank_field() {
    let state = support::app::test_state().await;
    let note_id = support::notes::create_note(&state, "old").await;

    let response = support::app::send_with_state(
        state,
        Request::builder()
            .method("PATCH")
            .uri(format!("/notes/{note_id}"))
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "content": "new",
                    "field": "   "
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "validation_error");
}
