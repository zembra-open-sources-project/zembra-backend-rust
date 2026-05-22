mod support;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{Value, json};

#[tokio::test]
async fn create_note_route_returns_created_note() {
    let response = support::app::send(
        Request::builder()
            .method("POST")
            .uri("/notes")
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
            .uri("/notes")
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
