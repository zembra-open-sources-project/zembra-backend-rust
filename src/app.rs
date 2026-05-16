use axum::Router;
use axum::http::{HeaderValue, Method, header::CONTENT_TYPE};
use std::net::{IpAddr, Ipv4Addr};
use tower_http::cors::{AllowOrigin, CorsLayer};
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
pub fn build_router_with_cors(
    state: AppState,
    cors_allowed_origins: Vec<crate::config::CorsOriginRule>,
) -> Router {
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
fn cors_layer(cors_allowed_origins: Vec<crate::config::CorsOriginRule>) -> CorsLayer {
    let configured_origin_rules = cors_allowed_origins;

    CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(move |origin, _request_head| {
            configured_origin_rules
                .iter()
                .any(|rule| cors_origin_rule_matches(rule, origin))
                || is_local_browser_origin(origin)
        }))
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

/// Checks whether a configured CORS rule matches a browser origin.
///
/// # Arguments
///
/// * `rule` - Configured exact or wildcard CORS rule.
/// * `origin` - Browser `Origin` header value.
///
/// # Returns
///
/// Returns `true` when the request origin is allowed by the rule.
fn cors_origin_rule_matches(rule: &crate::config::CorsOriginRule, origin: &HeaderValue) -> bool {
    match rule {
        crate::config::CorsOriginRule::Exact(allowed) => allowed == origin,
        crate::config::CorsOriginRule::Ipv4Wildcard(rule) => {
            wildcard_cors_origin_matches(rule, origin)
        }
    }
}

/// Checks whether an IPv4 wildcard CORS rule matches a browser origin.
///
/// # Arguments
///
/// * `rule` - Configured IPv4 wildcard CORS rule.
/// * `origin` - Browser `Origin` header value.
///
/// # Returns
///
/// Returns `true` when scheme, exact port, and IPv4 octets match.
fn wildcard_cors_origin_matches(
    rule: &crate::config::Ipv4CorsOriginRule,
    origin: &HeaderValue,
) -> bool {
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    let Some(parts) = origin_parts(origin) else {
        return false;
    };
    let Ok(ip) = parts.host.parse::<Ipv4Addr>() else {
        return false;
    };

    parts.scheme == rule.scheme
        && parts.port == Some(rule.port)
        && rule
            .octets
            .iter()
            .zip(ip.octets())
            .all(|(expected, actual)| expected.is_none_or(|octet| octet == actual))
}

/// Checks whether an origin belongs to local development.
///
/// # Arguments
///
/// * `origin` - Browser `Origin` header value.
///
/// # Returns
///
/// Returns `true` for localhost and loopback origins.
fn is_local_browser_origin(origin: &HeaderValue) -> bool {
    let Ok(origin) = origin.to_str() else {
        return false;
    };

    let Some(parts) = origin_parts(origin) else {
        return false;
    };

    if parts.host.eq_ignore_ascii_case("localhost") {
        return true;
    }

    match parts.host.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => ip.is_loopback(),
        Ok(IpAddr::V6(ip)) => ip.is_loopback(),
        Err(_) => false,
    }
}

/// Browser origin parts used by CORS matching.
struct OriginParts<'a> {
    /// URI scheme.
    scheme: &'a str,
    /// Host without brackets or port.
    host: &'a str,
    /// Numeric port when the origin includes one.
    port: Option<u16>,
}

/// Extracts scheme, host, and port from a browser origin string.
///
/// # Arguments
///
/// * `origin` - Browser origin string with scheme, host, and optional port.
///
/// # Returns
///
/// Returns parsed parts when the origin uses HTTP or HTTPS.
fn origin_parts(origin: &str) -> Option<OriginParts<'_>> {
    let (scheme, authority) = origin
        .strip_prefix("http://")
        .map(|authority| ("http", authority))
        .or_else(|| {
            origin
                .strip_prefix("https://")
                .map(|authority| ("https", authority))
        })?;

    let authority = authority.split('/').next().unwrap_or(authority);

    if authority.starts_with('[') {
        let (host, rest) = authority.strip_prefix('[')?.split_once(']')?;
        return Some(OriginParts {
            scheme,
            host,
            port: rest.strip_prefix(':').and_then(|port| port.parse().ok()),
        });
    }

    let (host, port) = authority
        .split_once(':')
        .map_or((authority, None), |(host, port)| {
            (host, port.parse::<u16>().ok())
        });

    Some(OriginParts { scheme, host, port })
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
    /// * `cors_allowed_origins` - Browser origin rules allowed by the router.
    /// * `request` - HTTP request to dispatch through the router.
    ///
    /// # Returns
    ///
    /// Returns the HTTP response produced by the router.
    async fn send_with_cors(
        state: super::AppState,
        cors_allowed_origins: Vec<crate::config::CorsOriginRule>,
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
        create_tagged_note(state, content, Vec::new()).await
    }

    /// Creates a note with tags through the notes service.
    ///
    /// # Arguments
    ///
    /// * `state` - Shared application state.
    /// * `content` - Note body content.
    /// * `tags` - Tag names to associate with the note.
    ///
    /// # Returns
    ///
    /// Returns the created note ID.
    async fn create_tagged_note(
        state: &super::AppState,
        content: &str,
        tags: Vec<String>,
    ) -> String {
        create_note_with_metadata(state, content, None, tags).await
    }

    /// Creates a note with a field through the notes service.
    ///
    /// # Arguments
    ///
    /// * `state` - Shared application state.
    /// * `content` - Note body content.
    /// * `field` - Field name to associate with the note.
    ///
    /// # Returns
    ///
    /// Returns the created note ID.
    async fn create_field_note(state: &super::AppState, content: &str, field: &str) -> String {
        create_note_with_metadata(state, content, Some(field.to_string()), Vec::new()).await
    }

    /// Creates a note with optional metadata through the notes service.
    ///
    /// # Arguments
    ///
    /// * `state` - Shared application state.
    /// * `content` - Note body content.
    /// * `field` - Optional field name.
    /// * `tags` - Tag names to associate with the note.
    ///
    /// # Returns
    ///
    /// Returns the created note ID.
    async fn create_note_with_metadata(
        state: &super::AppState,
        content: &str,
        field: Option<String>,
        tags: Vec<String>,
    ) -> String {
        let service = crate::services::notes::NotesService::new(state.database.pool.clone());
        service
            .create_note(crate::dto::notes::CreateNoteRequest {
                content: content.to_string(),
                field,
                tags,
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
            vec![crate::config::CorsOriginRule::Exact(origin.clone())],
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
    async fn cors_preflight_allows_default_localhost_origin() {
        let origin = HeaderValue::from_static("http://localhost:5173");
        let response = send_with_cors(
            test_state().await,
            Vec::new(),
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
    async fn cors_preflight_allows_default_loopback_origin() {
        let origin = HeaderValue::from_static("http://127.0.0.1:5173");
        let response = send_with_cors(
            test_state().await,
            Vec::new(),
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
    async fn cors_preflight_requires_config_for_private_lan_origin() {
        let response = send_with_cors(
            test_state().await,
            Vec::new(),
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/notes")
                .header(ORIGIN, "http://192.168.1.20:5173")
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
    async fn cors_preflight_allows_configured_ipv4_wildcard_origin() {
        let origin = HeaderValue::from_static("http://192.168.1.20:5173");
        let settings = crate::config::ServerSettings {
            host: "127.0.0.1".to_string(),
            port: 3000,
            cors_allowed_origins: vec!["http://192.168.1.*:5173".to_string()],
        };
        let response = send_with_cors(
            test_state().await,
            settings.cors_origin_rules().unwrap(),
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
    async fn cors_preflight_rejects_wildcard_origin_on_different_port() {
        let settings = crate::config::ServerSettings {
            host: "127.0.0.1".to_string(),
            port: 3000,
            cors_allowed_origins: vec!["http://192.168.1.*:5173".to_string()],
        };
        let response = send_with_cors(
            test_state().await,
            settings.cors_origin_rules().unwrap(),
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/notes")
                .header(ORIGIN, "http://192.168.1.20:5174")
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
    async fn cors_preflight_rejects_public_unconfigured_origin() {
        let response = send_with_cors(
            test_state().await,
            vec![crate::config::CorsOriginRule::Exact(
                HeaderValue::from_static("http://192.168.1.20:5173"),
            )],
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/notes")
                .header(ORIGIN, "https://example.com")
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
        assert!(body["paths"].get("/random/notes").is_some());
        assert!(body["paths"].get("/random/tags").is_some());
        assert!(body["paths"].get("/random/fields").is_some());
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
    async fn random_notes_route_returns_random_visible_notes() {
        let state = test_state().await;
        create_note(&state, "first").await;
        create_note(&state, "second").await;
        let archived = create_note(&state, "archived").await;
        let deleted = create_note(&state, "deleted").await;
        let service = crate::services::notes::NotesService::new(state.database.pool.clone());
        service.archive_note(&archived).await.unwrap();
        service.delete_note(&deleted).await.unwrap();

        let response = send_with_state(
            state,
            Request::builder()
                .uri("/random/notes?n=50")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        let status = response.status();
        let body = response_json(response).await;
        let notes = body["notes"].as_array().unwrap();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(notes.len(), 2);
        assert!(
            notes
                .iter()
                .all(|note| note["content"] == "first" || note["content"] == "second")
        );
    }

    #[tokio::test]
    async fn random_notes_route_applies_limit() {
        let state = test_state().await;
        create_note(&state, "first").await;
        create_note(&state, "second").await;
        create_note(&state, "third").await;

        let response = send_with_state(
            state,
            Request::builder()
                .uri("/random/notes?n=2")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        let status = response.status();
        let body = response_json(response).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["notes"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn random_notes_route_returns_empty_when_no_visible_notes_exist() {
        let response = send(
            Request::builder()
                .uri("/random/notes?n=5")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        let status = response.status();
        let body = response_json(response).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["notes"], json!([]));
    }

    #[tokio::test]
    async fn random_notes_route_rejects_invalid_n() {
        for uri in ["/random/notes?n=0", "/random/notes?n=51"] {
            let response = send(Request::builder().uri(uri).body(Body::empty()).unwrap()).await;
            let status = response.status();
            let body = response_json(response).await;

            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(body["error"]["code"], "validation_error");
        }
    }

    #[tokio::test]
    async fn random_tags_route_groups_visible_notes_by_tag() {
        let state = test_state().await;
        let shared = create_tagged_note(
            &state,
            "shared",
            vec!["rust".to_string(), "sqlite".to_string()],
        )
        .await;
        let rust_only = create_tagged_note(&state, "rust only", vec!["rust".to_string()]).await;
        let archived = create_tagged_note(&state, "archived", vec!["rust".to_string()]).await;
        let deleted = create_tagged_note(&state, "deleted", vec!["sqlite".to_string()]).await;
        let service = crate::services::notes::NotesService::new(state.database.pool.clone());
        service.archive_note(&archived).await.unwrap();
        service.delete_note(&deleted).await.unwrap();

        let response = send_with_state(
            state,
            Request::builder()
                .uri("/random/tags?n=2&count=10")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        let status = response.status();
        let body = response_json(response).await;
        let groups = body["tagged_notes"].as_array().unwrap();
        let shared_count = groups
            .iter()
            .filter(|group| {
                group["notes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|note| note["id"] == shared)
            })
            .count();
        let rust_count = groups
            .iter()
            .flat_map(|group| group["notes"].as_array().unwrap())
            .filter(|note| note["id"] == rust_only)
            .count();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(groups.len(), 2);
        assert_eq!(shared_count, 2);
        assert_eq!(rust_count, 1);
        assert!(
            groups
                .iter()
                .flat_map(|group| group["notes"].as_array().unwrap())
                .all(|note| note["content"] != "archived" && note["content"] != "deleted")
        );
    }

    #[tokio::test]
    async fn random_tags_route_limits_cumulative_notes() {
        let state = test_state().await;
        for content in ["first", "second", "third"] {
            create_tagged_note(&state, content, vec!["rust".to_string()]).await;
        }

        let response = send_with_state(
            state,
            Request::builder()
                .uri("/random/tags?n=1&count=2")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        let status = response.status();
        let body = response_json(response).await;
        let groups = body["tagged_notes"].as_array().unwrap();
        let notes_count = groups
            .iter()
            .map(|group| group["notes"].as_array().unwrap().len())
            .sum::<usize>();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(groups.len(), 1);
        assert_eq!(notes_count, 2);
    }

    #[tokio::test]
    async fn random_tags_route_returns_existing_tags_when_n_is_larger() {
        let state = test_state().await;
        create_tagged_note(&state, "rust", vec!["rust".to_string()]).await;
        create_tagged_note(&state, "sqlite", vec!["sqlite".to_string()]).await;

        let response = send_with_state(
            state,
            Request::builder()
                .uri("/random/tags?n=20")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        let status = response.status();
        let body = response_json(response).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["tagged_notes"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn random_tags_route_uses_default_limit() {
        let state = test_state().await;
        for name in ["alpha", "beta", "gamma", "delta"] {
            create_tagged_note(&state, name, vec![name.to_string()]).await;
        }
        for index in 0..25 {
            create_tagged_note(&state, &format!("extra {index}"), vec!["alpha".to_string()]).await;
        }

        let response = send_with_state(
            state,
            Request::builder()
                .uri("/random/tags")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        let status = response.status();
        let body = response_json(response).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["tagged_notes"].as_array().unwrap().len(), 3);
        assert!(
            body["tagged_notes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|group| group["notes"].as_array().unwrap().len())
                .sum::<usize>()
                <= 20
        );
    }

    #[tokio::test]
    async fn random_tags_route_rejects_invalid_query_values() {
        for uri in [
            "/random/tags?n=0",
            "/random/tags?n=21",
            "/random/tags?count=0",
            "/random/tags?count=101",
        ] {
            let response = send(Request::builder().uri(uri).body(Body::empty()).unwrap()).await;
            let status = response.status();
            let body = response_json(response).await;

            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(body["error"]["code"], "validation_error");
        }
    }

    #[tokio::test]
    async fn random_fields_route_limits_cumulative_notes() {
        let state = test_state().await;
        for content in ["first", "second", "third"] {
            create_field_note(&state, content, "work").await;
        }

        let response = send_with_state(
            state,
            Request::builder()
                .uri("/random/fields?n=1&count=2")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        let status = response.status();
        let body = response_json(response).await;
        let groups = body["field_notes"].as_array().unwrap();
        let notes_count = groups
            .iter()
            .map(|group| group["notes"].as_array().unwrap().len())
            .sum::<usize>();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(groups.len(), 1);
        assert_eq!(notes_count, 2);
    }

    #[tokio::test]
    async fn random_fields_route_filters_hidden_notes_and_keeps_empty_fields() {
        let state = test_state().await;
        let visible = create_field_note(&state, "visible", "work").await;
        let archived = create_field_note(&state, "archived", "work").await;
        let deleted = create_field_note(&state, "deleted", "empty").await;
        let service = crate::services::notes::NotesService::new(state.database.pool.clone());
        service.archive_note(&archived).await.unwrap();
        service.delete_note(&deleted).await.unwrap();

        let response = send_with_state(
            state,
            Request::builder()
                .uri("/random/fields?n=2&count=10")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        let status = response.status();
        let body = response_json(response).await;
        let groups = body["field_notes"].as_array().unwrap();
        let all_notes = groups
            .iter()
            .flat_map(|group| group["notes"].as_array().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(groups.len(), 2);
        assert!(all_notes.iter().any(|note| note["id"] == visible));
        assert!(
            all_notes
                .iter()
                .all(|note| { note["content"] != "archived" && note["content"] != "deleted" })
        );
        assert!(
            groups
                .iter()
                .any(|group| group["notes"].as_array().unwrap().is_empty())
        );
    }

    #[tokio::test]
    async fn random_fields_route_returns_existing_fields_when_n_is_larger() {
        let state = test_state().await;
        create_field_note(&state, "work", "work").await;
        create_field_note(&state, "life", "life").await;

        let response = send_with_state(
            state,
            Request::builder()
                .uri("/random/fields?n=20&count=20")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        let status = response.status();
        let body = response_json(response).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["field_notes"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn random_fields_route_uses_default_query_values() {
        let state = test_state().await;
        for name in ["alpha", "beta", "gamma", "delta"] {
            create_field_note(&state, name, name).await;
        }

        let response = send_with_state(
            state,
            Request::builder()
                .uri("/random/fields")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        let status = response.status();
        let body = response_json(response).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["field_notes"].as_array().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn random_fields_route_rejects_invalid_query_values() {
        for uri in [
            "/random/fields?n=0",
            "/random/fields?n=21",
            "/random/fields?count=0",
            "/random/fields?count=101",
        ] {
            let response = send(Request::builder().uri(uri).body(Body::empty()).unwrap()).await;
            let status = response.status();
            let body = response_json(response).await;

            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(body["error"]["code"], "validation_error");
        }
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
