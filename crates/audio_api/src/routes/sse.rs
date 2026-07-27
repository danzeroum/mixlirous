use crate::middleware::AuthContext;
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
///
/// **Sobre o `AuthContext`.** Esta era a única rota de `/api/v1` sem
/// extractor de autenticação, enquanto `GET /jobs/{job_id}` ao lado exige
/// `TenantScope`. Hoje isso não vaza nada — o handler ignora o `job_id` além
/// de ecoá-lo e devolve `stream::pending()` —, mas vira vazamento entre
/// tenants no instante em que a Sprint 1 publicar eventos reais.
///
/// O extractor entra agora, fechado por padrão, mesmo sem consumidor. Não
/// resolve o problema de fundo: `EventSource` (usado por
/// `ui/src/hooks/useParamStream.ts`) **não consegue mandar header
/// `Authorization`**, então autenticar de verdade exige decisão de design —
/// token na query, cookie, ou trocar por um cliente SSE via `fetch`. A
/// diferença é que agora quem for ligar a UI bate no extractor e resolve
/// naquele momento, em vez de descobrir uma rota aberta depois que ela já
/// publica evento real. `tenant_id` ainda não é usado para filtrar porque não
/// há o que filtrar; quando houver, o escopo já está em mãos.
pub async fn job_stream(
    _auth: AuthContext,
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
