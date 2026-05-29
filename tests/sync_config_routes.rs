mod support;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;

#[tokio::test]
async fn sync_config_route_returns_sanitized_defaults() {
    let response = support::app::send(
        Request::builder()
            .uri("/sync/config")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["enabled"], false);
    assert_eq!(body["interval_seconds"], 60);
    assert_eq!(body["secret_key_configured"], false);
    assert!(body.get("secret_key").is_none());
}

#[tokio::test]
async fn update_sync_config_route_persists_sanitized_config() {
    let state = support::app::test_state().await;
    let response = support::app::send_with_state(
        state,
        Request::builder()
            .method("PUT")
            .uri("/sync/config")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "enabled": false,
                    "interval_seconds": 15,
                    "supabase_url": "https://example.supabase.co",
                    "secret_key": "sb_secret_test-key"
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["enabled"], false);
    assert_eq!(body["interval_seconds"], 15);
    assert_eq!(body["secret_key_configured"], true);
    assert!(body.get("secret_key").is_none());
}

#[tokio::test]
async fn update_sync_config_route_rejects_enabled_config_without_key() {
    let response = support::app::send(
        Request::builder()
            .method("PUT")
            .uri("/sync/config")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "enabled": true,
                    "interval_seconds": 15,
                    "supabase_url": "https://example.supabase.co"
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_config");
}

#[tokio::test]
async fn update_sync_config_route_rejects_legacy_service_role_key() {
    let response = support::app::send(
        Request::builder()
            .method("PUT")
            .uri("/sync/config")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "enabled": true,
                    "interval_seconds": 15,
                    "supabase_url": "https://example.supabase.co",
                    "secret_key": "eyJlegacy.jwt.service-role"
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    let status = response.status();
    let body = support::app::response_json(response).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_config");
}
