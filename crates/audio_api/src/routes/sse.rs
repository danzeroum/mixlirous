use axum::{
    extract::Path,
    response::sse::{Event, KeepAlive, Sse},
};
use futures::stream::{self, Stream, StreamExt};
use std::convert::Infallible;
use std::time::Duration;
use uuid::Uuid;

/// Streaming SSE do progresso/raciocínio de um job (ver
/// `docs/03-CONTRATOS-API.md` §5). Sprint 0 só emite `stream.ready` e mantém
/// a conexão viva com heartbeat — a publicação de eventos reais (agent.*,
/// job.*) chega junto do motor de fila (Sprint 1+).
pub async fn job_stream(
    Path(job_id): Path<Uuid>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let ready = Event::default()
        .event("stream.ready")
        .json_data(serde_json::json!({ "job_id": job_id, "resumed_from": null }))
        .unwrap_or_else(|_| Event::default().event("stream.ready"));

    let events = stream::once(async { Ok(ready) }).chain(stream::pending());

    Sse::new(events).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    )
}
