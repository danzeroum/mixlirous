use crate::middleware::AuthContext;
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::HeaderMap,
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
///
/// **QA-0024 — Replay via Last-Event-ID.** O docs/03 §5 exige: "O servidor
/// mantém um buffer dos últimos 200 eventos por job para replay.
/// Reconexão usa `Last-Event-ID`." Antes deste fix, o `subscribe` criava
/// o canal broadcast na primeira chamada — se o `publish` veio antes
/// (worker completou em 1s, antes da UI abrir o SSE), o evento era
/// descartado. Agora, todo publish também armazena no buffer; este
/// handler lê o header `Last-Event-ID` (se presente) e pede replay ao
/// `subscribe_with_replay`, devolvendo os eventos passados antes de
/// começar a stream ao vivo.
pub async fn job_stream(
    _auth: AuthContext,
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
    headers: HeaderMap,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // QA-0024: lê Last-Event-ID do header (W3C EventSource envia
    // automaticamente em reconexão). Se ausente, replay completo.
    let last_event_id = headers
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());

    let sub = state.hub.subscribe_with_replay(job_id, last_event_id).await;
    let mut rx = sub.rx;
    let replay = sub.replay;

    // stream.ready avisa o cliente que a conexão está ativa. Inclui
    // `resumed_from` = last_event_id se houve replay, ou null se começa
    // do zero. Em reconexão com Last-Event-ID, o cliente pode usar isso
    // para validar que o replay começou do ponto certo.
    let ready = Event::default()
        .event("stream.ready")
        .json_data(serde_json::json!({
            "job_id": job_id,
            "resumed_from": last_event_id,
            "replay_count": replay.len(),
        }))
        .unwrap_or_else(|_| Event::default().event("stream.ready"));

    let event_stream = async_stream::stream! {
        yield Ok(ready);

        // QA-0024: primeiro envia o replay (eventos publicados antes
        // do subscribe). Cada evento mantém seu `seq` original via
        // `id()` no SSE — o cliente usa isso para reconhecer
        // duplicatas e para enviar `Last-Event-ID` em reconexão.
        for past_event in replay {
            let sse_event = Event::default()
                .event(&past_event.event_type)
                .json_data(&past_event.data)
                .unwrap_or_else(|_| {
                    Event::default()
                        .event(&past_event.event_type)
                        .data("{}")
                })
                .id(past_event.seq.to_string());
            yield Ok(sse_event);
        }

        // Depois do replay, ao vivo: eventos novos do broadcast.
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
                        })
                        .id(job_event.seq.to_string());
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
