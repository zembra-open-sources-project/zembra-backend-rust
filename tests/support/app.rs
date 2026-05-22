//! Application test helpers for HTTP integration tests.

use axum::body::{Body, to_bytes};
use axum::http::Request;
use axum::response::Response;
use serde_json::Value;
use std::sync::atomic::{AtomicUsize, Ordering};
use tower::ServiceExt;
use zembra_backend_rust::app::AppState;

static TEST_CONFIG_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Creates application state backed by an in-memory database.
///
/// # Returns
///
/// Returns test application state with migrated SQLite schema.
pub async fn test_state() -> AppState {
    let database =
        zembra_backend_rust::repositories::database::Database::connect("sqlite://:memory:")
            .await
            .unwrap();
    let settings = zembra_backend_rust::config::SyncSettings::default();
    let sync =
        zembra_backend_rust::services::sync::SyncService::new(database.pool.clone(), &settings);

    let sync_config_id = TEST_CONFIG_COUNTER.fetch_add(1, Ordering::Relaxed);
    let sync_config_path =
        std::env::temp_dir().join(format!("zembra-test-sync-config-{sync_config_id}.toml"));
    let _ = std::fs::remove_file(&sync_config_path);
    let sync_config =
        zembra_backend_rust::services::sync_config::SyncConfigService::new(sync_config_path);

    AppState {
        database,
        sync,
        sync_config,
    }
}

/// Sends a request to the application router in tests.
///
/// # Arguments
///
/// * `request` - HTTP request to dispatch through the router.
///
/// # Returns
///
/// Returns the HTTP response produced by the router.
pub async fn send(request: Request<Body>) -> Response {
    send_with_state(test_state().await, request).await
}

/// Sends a request to the application router with explicit state.
///
/// # Arguments
///
/// * `state` - Shared application state for the router.
/// * `request` - HTTP request to dispatch through the router.
///
/// # Returns
///
/// Returns the HTTP response produced by the router.
pub async fn send_with_state(state: AppState, request: Request<Body>) -> Response {
    zembra_backend_rust::app::build_router(state)
        .oneshot(request)
        .await
        .unwrap()
}

/// Sends a request to the application router with explicit CORS origins.
///
/// # Arguments
///
/// * `state` - Shared application state for the router.
/// * `cors_allowed_origins` - Browser origin rules allowed by the router.
/// * `request` - HTTP request to dispatch through the router.
///
/// # Returns
///
/// Returns the HTTP response produced by the router.
pub async fn send_with_cors(
    state: AppState,
    cors_allowed_origins: Vec<zembra_backend_rust::config::CorsOriginRule>,
    request: Request<Body>,
) -> Response {
    zembra_backend_rust::app::build_router_with_cors(state, cors_allowed_origins)
        .oneshot(request)
        .await
        .unwrap()
}

/// Reads a response body as JSON.
///
/// # Arguments
///
/// * `response` - HTTP response to read.
///
/// # Returns
///
/// Returns parsed JSON.
pub async fn response_json(response: Response) -> Value {
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}
