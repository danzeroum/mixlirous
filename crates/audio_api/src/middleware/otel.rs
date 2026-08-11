use axum::{extract::FromRequestParts, http::request::Parts};

/// Extrai o header `traceparent` (W3C Trace Context) da requisi├º├úo, para
/// correlacionar m├®tricas/traces/logs (ver `docs/07-OBSERVABILIDADE.md` ┬º1).
///
/// A integra├º├úo completa com OpenTelemetry/OTLP fica para quando a Sprint de
/// observabilidade entrar em pauta; por ora isto s├│ extrai e propaga o
/// `trace_id` para os spans de `tracing`.
#[derive(Debug, Clone, Default)]
pub struct TraceParent {
    pub trace_id: Option<String>,
}

impl<S> FromRequestParts<S> for TraceParent
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let trace_id = parts
            .headers
            .get("traceparent")
            .and_then(|v| v.to_str().ok())
            .and_then(|raw| raw.split('-').nth(1))
            .map(|s| s.to_string());

        Ok(TraceParent { trace_id })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;

    #[tokio::test]
    async fn test_extracts_trace_id_from_traceparent_header() {
        let req = Request::builder()
            .header(
                "traceparent",
                "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
            )
            .body(())
            .unwrap();
        let (mut parts, _) = req.into_parts();

        let extracted = TraceParent::from_request_parts(&mut parts, &())
            .await
            .unwrap();
        assert_eq!(
            extracted.trace_id.as_deref(),
            Some("4bf92f3577b34da6a3ce929d0e0e4736")
        );
    }

    #[tokio::test]
    async fn test_missing_header_yields_none() {
        let req = Request::builder().body(()).unwrap();
        let (mut parts, _) = req.into_parts();

        let extracted = TraceParent::from_request_parts(&mut parts, &())
            .await
            .unwrap();
        assert!(extracted.trace_id.is_none());
    }
}
