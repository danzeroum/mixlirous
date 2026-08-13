//! Sprint 4 — `GET /metrics` endpoint (Prometheus text format).
//! Exposed without auth (docs/07-OBSERVABILIDADE.md §4).

use axum::http::HeaderValue;
use axum::response::IntoResponse;

/// Prometheus text format response.
pub async fn prometheus_metrics() -> impl IntoResponse {
    let body = crate::metrics::render_prometheus();
    (
        [(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; version=0.0.4; charset=utf-8"),
        )],
        body,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_metrics_endpoint_returns_200() {
        let app = axum::Router::new().route("/metrics", axum::routing::get(prometheus_metrics));
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_metrics_response_contains_mixlirous() {
        let app = axum::Router::new().route("/metrics", axum::routing::get(prometheus_metrics));
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(
            text.contains("mixlirous_"),
            "metrics should contain mixlirous_ prefix"
        );
        assert!(
            text.contains("mixlirous_build_info"),
            "should contain build info"
        );
    }

    #[tokio::test]
    async fn test_metrics_content_type() {
        let app = axum::Router::new().route("/metrics", axum::routing::get(prometheus_metrics));
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let ct = resp
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .unwrap();
        assert!(ct.to_str().unwrap().contains("text/plain"));
    }
}
