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
    use axum::http::{Request, StatusCode};
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
}
