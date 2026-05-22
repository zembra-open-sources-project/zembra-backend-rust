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
