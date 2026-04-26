use axum::Router;

/// Builds the root HTTP router for the backend service.
///
/// # Returns
///
/// Returns an Axum router containing infrastructure routes only.
pub fn build_router() -> Router {
    Router::new().merge(crate::routes::health::router())
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::response::Response;
    use tower::ServiceExt;

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
        super::build_router().oneshot(request).await.unwrap()
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
}
