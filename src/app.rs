use axum::Router;
use axum::http::{HeaderValue, Method, header::CONTENT_TYPE};
use tower_http::cors::CorsLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

/// Shared application state available to all handlers.
#[derive(Debug, Clone)]
pub struct AppState {
    /// SQLite database handle used by repositories and health checks.
    pub database: crate::repositories::database::Database,
    /// Supabase synchronization service used by sync routes and worker.
    pub sync: crate::services::sync::SyncService,
    /// Supabase synchronization configuration service.
    pub sync_config: crate::services::sync_config::SyncConfigService,
}

/// Builds the root HTTP router for the backend service.
///
/// # Arguments
///
/// * `state` - Shared application state injected into route handlers.
///
/// # Returns
///
/// Returns an Axum router containing infrastructure routes only.
#[cfg(test)]
pub fn build_router(state: AppState) -> Router {
    build_router_with_cors(state, Vec::new())
}

/// Builds the root HTTP router with explicit CORS origins.
///
/// # Arguments
///
/// * `state` - Shared application state injected into route handlers.
/// * `cors_allowed_origins` - Browser origins allowed to call the API.
///
/// # Returns
///
/// Returns an Axum router with API routes, Swagger UI, and CORS handling.
pub fn build_router_with_cors(state: AppState, cors_allowed_origins: Vec<HeaderValue>) -> Router {
    Router::new()
        .merge(crate::routes::health::router())
        .merge(crate::routes::notes::router())
        .merge(crate::routes::taxonomy::router())
        .merge(crate::routes::sync::router())
        .merge(
            SwaggerUi::new("/swagger-ui")
                .url("/api-docs/openapi.json", crate::api_doc::ApiDoc::openapi()),
        )
        .layer(cors_layer(cors_allowed_origins))
        .with_state(state)
}

/// Builds the CORS layer used for browser clients.
///
/// # Arguments
///
/// * `cors_allowed_origins` - Browser origins allowed to call the API.
///
/// # Returns
///
/// Returns a `CorsLayer` that allows configured origins and common API methods.
fn cors_layer(cors_allowed_origins: Vec<HeaderValue>) -> CorsLayer {
    CorsLayer::new()
        .allow_origin(cors_allowed_origins)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([CONTENT_TYPE])
}

#[cfg(test)]
mod tests {
    use axum::body::{Body, to_bytes};
    use axum::http::{
        HeaderValue, Method, Request, StatusCode,
        header::{ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_REQUEST_METHOD, ORIGIN},
    };
    use axum::response::Response;
    use serde_json::{Value, json};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tower::ServiceExt;

    static TEST_CONFIG_COUNTER: AtomicUsize = AtomicUsize::new(0);

    /// Creates application state backed by an in-memory database.
    ///
    /// # Returns
    ///
    /// Returns test application state with migrated SQLite schema.
    async fn test_state() -> super::AppState {
        let database = crate::repositories::database::Database::connect("sqlite://:memory:")
            .await
            .unwrap();
        let settings = crate::config::SyncSettings::default();
        let sync = crate::services::sync::SyncService::new(database.pool.clone(), &settings);

        let sync_config_id = TEST_CONFIG_COUNTER.fetch_add(1, Ordering::Relaxed);
        let sync_config_path =
            std::env::temp_dir().join(format!("zembra-test-sync-config-{sync_config_id}.toml"));
        let _ = std::fs::remove_file(&sync_config_path);
        let sync_config = crate::services::sync_config::SyncConfigService::new(sync_config_path);

        super::AppState {
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
    async fn send(request: Request<Body>) -> Response {
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
    async fn send_with_state(state: super::AppState, request: Request<Body>) -> Response {
        super::build_router(state).oneshot(request).await.unwrap()
    }

    /// Sends a request to the application router with explicit CORS origins.
    ///
    /// # Arguments
    ///
    /// * `state` - Shared application state for the router.
    /// * `cors_allowed_origins` - Browser origins allowed by the router.
    /// * `request` - HTTP request to dispatch through the router.
    ///
    /// # Returns
    ///
    /// Returns the HTTP response produced by the router.
    async fn send_with_cors(
        state: super::AppState,
        cors_allowed_origins: Vec<HeaderValue>,
        request: Request<Body>,
    ) -> Response {
        super::build_router_with_cors(state, cors_allowed_origins)
            .oneshot(request)
            .await
            .unwrap()
    }

    /// Creates a note through the notes service.
    ///
    /// # Arguments
    ///
    /// * `state` - Shared application state.
    /// * `content` - Note body content.
    ///
    /// # Returns
    ///
    /// Returns the created note ID.
    async fn create_note(state: &super::AppState, content: &str) -> String {
        let service = crate::services::notes::NotesService::new(state.database.pool.clone());
        service
            .create_note(crate::dto::notes::CreateNoteRequest {
                content: content.to_string(),
                field: None,
                tags: Vec::new(),
                role: "Human".to_string(),
                device_id: None,
            })
            .await
            .unwrap()
            .note
            .id
    }

    /// Updates a note timestamp directly for deterministic ordering tests.
    ///
    /// # Arguments
    ///
    /// * `state` - Shared application state.
    /// * `note_id` - Note ID to update.
    /// * `updated_at` - Timestamp value to write.
    async fn set_updated_at(state: &super::AppState, note_id: &str, updated_at: i64) {
        sqlx::query("UPDATE notes SET updated_at = ? WHERE id = ?")
            .bind(updated_at)
            .bind(note_id)
            .execute(&state.database.pool)
            .await
            .unwrap();
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
    async fn response_json(response: Response) -> Value {
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    #[tokio::test]
    async fn health_route_returns_ok() {
        let response = send(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn cors_preflight_allows_configured_origin() {
        let origin = HeaderValue::from_static("http://192.168.1.20:5173");
        let response = send_with_cors(
            test_state().await,
            vec![origin.clone()],
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/notes")
                .header(ORIGIN, origin.clone())
                .header(ACCESS_CONTROL_REQUEST_METHOD, "POST")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(ACCESS_CONTROL_ALLOW_ORIGIN),
            Some(&origin)
        );
    }

    #[tokio::test]
    async fn cors_preflight_rejects_unconfigured_origin() {
        let response = send_with_cors(
            test_state().await,
            vec![HeaderValue::from_static("http://192.168.1.20:5173")],
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/notes")
                .header(ORIGIN, "http://192.168.1.30:5173")
                .header(ACCESS_CONTROL_REQUEST_METHOD, "POST")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            response
                .headers()
                .get(ACCESS_CONTROL_ALLOW_ORIGIN)
                .is_none()
        );
    }

    #[tokio::test]
    async fn create_note_route_returns_created_note() {
        let response = send(
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
        let body = response_json(response).await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["note"]["content"], "api note");
        assert_eq!(body["metadata"]["field"], "work");
        assert_eq!(body["metadata"]["tags"], json!(["rust", "api"]));
    }

    #[tokio::test]
    async fn create_note_rejects_invalid_role() {
        let response = send(
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
        let body = response_json(response).await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["error"]["code"], "validation_error");
    }

    #[tokio::test]
    async fn openapi_json_lists_runtime_api_paths() {
        let response = send(
            Request::builder()
                .uri("/api-docs/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        let status = response.status();
        let body = response_json(response).await;

        assert_eq!(status, StatusCode::OK);
        assert!(body["paths"].get("/health").is_some());
        assert!(body["paths"].get("/notes").is_some());
        assert!(body["paths"].get("/notes/recent").is_some());
        assert!(body["paths"].get("/notes/batch").is_some());
        assert!(body["paths"].get("/fields").is_some());
        assert!(body["paths"].get("/tags").is_some());
        assert!(body["paths"].get("/sync/status").is_some());
        assert!(body["paths"].get("/sync/config").is_some());
        assert!(body["paths"].get("/sync/config/test").is_some());
        assert!(body["paths"].get("/sync/run").is_some());
        assert!(body["paths"].get("/sync/push").is_some());
        assert!(body["paths"].get("/sync/pull").is_some());
    }

    #[tokio::test]
    async fn sync_config_route_returns_sanitized_defaults() {
        let response = send(
            Request::builder()
                .uri("/sync/config")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        let status = response.status();
        let body = response_json(response).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["enabled"], false);
        assert_eq!(body["interval_seconds"], 60);
        assert_eq!(body["service_role_key_configured"], false);
        assert!(body.get("service_role_key").is_none());
    }

    #[tokio::test]
    async fn update_sync_config_route_persists_sanitized_config() {
        let state = test_state().await;
        let response = send_with_state(
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
                        "service_role_key": "secret-key"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await;
        let status = response.status();
        let body = response_json(response).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["enabled"], false);
        assert_eq!(body["interval_seconds"], 15);
        assert_eq!(body["service_role_key_configured"], true);
        assert!(body.get("service_role_key").is_none());
    }

    #[tokio::test]
    async fn update_sync_config_route_rejects_enabled_config_without_key() {
        let response = send(
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
        let body = response_json(response).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_config");
    }

    #[tokio::test]
    async fn sync_status_route_returns_disabled_status() {
        let response = send(
            Request::builder()
                .uri("/sync/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        let status = response.status();
        let body = response_json(response).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["enabled"], false);
        assert_eq!(body["states"], json!([]));
    }

    #[tokio::test]
    async fn manual_sync_route_returns_disabled_error_when_sync_is_off() {
        let response = send(
            Request::builder()
                .method("POST")
                .uri("/sync/run")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        let status = response.status();
        let body = response_json(response).await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["error"]["code"], "sync_disabled");
    }

    #[tokio::test]
    async fn recent_notes_route_returns_ordered_visible_notes() {
        let state = test_state().await;
        let old = create_note(&state, "old").await;
        let archived = create_note(&state, "archived").await;
        let deleted = create_note(&state, "deleted").await;
        let new = create_note(&state, "new").await;
        set_updated_at(&state, &old, 2_000_000_010).await;
        set_updated_at(&state, &archived, 2_000_000_040).await;
        set_updated_at(&state, &deleted, 2_000_000_030).await;
        set_updated_at(&state, &new, 2_000_000_020).await;

        let service = crate::services::notes::NotesService::new(state.database.pool.clone());
        service.archive_note(&archived).await.unwrap();
        service.delete_note(&deleted).await.unwrap();

        let response = send_with_state(
            state,
            Request::builder()
                .method("POST")
                .uri("/notes/recent")
                .header("content-type", "application/json")
                .body(Body::from(json!({"limit": 10}).to_string()))
                .unwrap(),
        )
        .await;
        let status = response.status();
        let body = response_json(response).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["notes"][0]["content"], "new");
        assert_eq!(body["notes"][1]["content"], "old");
        assert_eq!(body["notes"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn recent_notes_route_uses_default_and_custom_limit() {
        let state = test_state().await;
        create_note(&state, "first").await;
        create_note(&state, "second").await;

        let response = send_with_state(
            state,
            Request::builder()
                .method("POST")
                .uri("/notes/recent")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "limit": 1 }).to_string()))
                .unwrap(),
        )
        .await;
        let status = response.status();
        let body = response_json(response).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["notes"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn recent_notes_route_uses_note_uuid_cursor() {
        let state = test_state().await;
        let old = create_note(&state, "old").await;
        let cursor = create_note(&state, "cursor").await;
        let new = create_note(&state, "new").await;
        set_updated_at(&state, &old, 2_000_000_010).await;
        set_updated_at(&state, &cursor, 2_000_000_020).await;
        set_updated_at(&state, &new, 2_000_000_030).await;

        let response = send_with_state(
            state,
            Request::builder()
                .method("POST")
                .uri("/notes/recent")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "limit": 10,
                        "note_uuid": cursor
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await;
        let status = response.status();
        let body = response_json(response).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["notes"].as_array().unwrap().len(), 1);
        assert_eq!(body["notes"][0]["content"], "old");
    }

    #[tokio::test]
    async fn recent_notes_route_rejects_invalid_limit() {
        let response = send(
            Request::builder()
                .method("POST")
                .uri("/notes/recent")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "limit": 101 }).to_string()))
                .unwrap(),
        )
        .await;
        let status = response.status();
        let body = response_json(response).await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["error"]["code"], "validation_error");
    }

    #[tokio::test]
    async fn recent_notes_route_rejects_invalid_note_uuid() {
        let response = send(
            Request::builder()
                .method("POST")
                .uri("/notes/recent")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "note_uuid": "abcd" }).to_string()))
                .unwrap(),
        )
        .await;
        let status = response.status();
        let body = response_json(response).await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["error"]["code"], "validation_error");
    }

    #[tokio::test]
    async fn recent_notes_route_returns_not_found_for_hidden_note_uuid() {
        let state = test_state().await;
        let archived = create_note(&state, "archived").await;
        let service = crate::services::notes::NotesService::new(state.database.pool.clone());
        service.archive_note(&archived).await.unwrap();

        let response = send_with_state(
            state,
            Request::builder()
                .method("POST")
                .uri("/notes/recent")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "note_uuid": archived }).to_string()))
                .unwrap(),
        )
        .await;
        let status = response.status();
        let body = response_json(response).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "record_not_found");
    }

    #[tokio::test]
    async fn swagger_ui_is_available() {
        let response = send(
            Request::builder()
                .uri("/swagger-ui/")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
    }
}
