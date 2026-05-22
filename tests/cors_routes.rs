mod support;

use axum::body::Body;
use axum::http::{
    HeaderValue, Method, Request, StatusCode,
    header::{ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_REQUEST_METHOD, ORIGIN},
};

#[tokio::test]
async fn cors_preflight_allows_configured_origin() {
    let origin = HeaderValue::from_static("http://192.168.1.20:5173");
    let response = support::app::send_with_cors(
        support::app::test_state().await,
        vec![zembra_backend_rust::config::CorsOriginRule::Exact(
            origin.clone(),
        )],
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
    let response = support::app::send_with_cors(
        support::app::test_state().await,
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
    let response = support::app::send_with_cors(
        support::app::test_state().await,
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
    let response = support::app::send_with_cors(
        support::app::test_state().await,
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
    let settings = zembra_backend_rust::config::ServerSettings {
        host: "127.0.0.1".to_string(),
        port: 3000,
        cors_allowed_origins: vec!["http://192.168.1.*:5173".to_string()],
    };
    let response = support::app::send_with_cors(
        support::app::test_state().await,
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
    let settings = zembra_backend_rust::config::ServerSettings {
        host: "127.0.0.1".to_string(),
        port: 3000,
        cors_allowed_origins: vec!["http://192.168.1.*:5173".to_string()],
    };
    let response = support::app::send_with_cors(
        support::app::test_state().await,
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
    let response = support::app::send_with_cors(
        support::app::test_state().await,
        vec![zembra_backend_rust::config::CorsOriginRule::Exact(
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
