mod support;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;

#[tokio::test]
async fn sync_status_route_returns_disabled_status() {
    let response = support::app::send(
        Request::builder()
            .uri("/sync/status")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["enabled"], false);
    assert_eq!(body["states"], json!([]));
}

#[tokio::test]
async fn manual_sync_route_returns_disabled_error_when_sync_is_off() {
    let response = support::app::send(
        Request::builder()
            .method("POST")
            .uri("/sync/run")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "sync_disabled");
}
