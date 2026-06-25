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
        .merge(crate::routes::workspaces::router())
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
