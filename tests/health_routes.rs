mod support;

use axum::body::Body;
use axum::http::{Request, StatusCode};

#[tokio::test]
async fn health_route_returns_ok() {
    let response = support::app::send(
        Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn health_route_returns_version_tracking_fields() {
    let response = support::app::send(
        Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(body["version_policy"], "semver");
    assert_eq!(body["release_channel"], "dev");
}
