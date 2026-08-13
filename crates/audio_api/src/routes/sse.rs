use crate::middleware::AuthContext;
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    response::sse::{Event, KeepAlive, Sse},
};
use futures::Stream;
use std::convert::Infallible;
use std::time::Duration;
use uuid::Uuid;

/// Streaming SSE do progresso/raciocínio de um job (ver
/// `docs/03-CONTRATOS-API.md` §5).
///
/// Task 3.9 do roadmap: conectado ao `EventHub` real — eventos publicados pelo
/// worker (`job.*`) e pelo agente (`agent.*`) chegam aqui via broadcast.
///
/// **Sobre o `AuthContext`.** O `EventSource` do browser (usado por
/// `ui/src/hooks/useParamStream.ts`) não consegue mandar header
/// `Authorization`, então a rota usa o extractor mas ainda não filtra por
/// tenant (a exposição fica para a decisão de design documentada em
/// `docs/18-DEPLOY-PUBLICO-NGINX.md`).
pub async fn job_stream(
    _auth: AuthContext,
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut rx = state.hub.subscribe(job_id).await;

    let ready = Event::default()
        .event("stream.ready")
        .json_data(serde_json::json!({ "job_id": job_id, "resumed_from": null }))
        .unwrap_or_else(|_| Event::default().event("stream.ready"));

    let event_stream = async_stream::stream! {
        yield Ok(ready);

        loop {
            match rx.recv().await {
                Ok(job_event) => {
                    let sse_event = Event::default()
                        .event(&job_event.event_type)
                        .json_data(&job_event.data)
                        .unwrap_or_else(|_| {
                            Event::default()
                                .event(&job_event.event_type)
                                .data("{}")
                        });
                    yield Ok(sse_event);
                },
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(missed = n, %job_id, "SSE lagged");
                    continue;
                },
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    break;
                },
            }
        }
    };

    Sse::new(event_stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    )
}
