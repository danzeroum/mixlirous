use crate::middleware::AuthContext;
use axum::{
    extract::Path,
    response::sse::{Event, KeepAlive, Sse},
};
use futures::stream::{self, Stream, StreamExt};
use std::convert::Infallible;
use std::time::Duration;
use uuid::Uuid;

/// Streaming SSE do progresso/racioc├¡nio de um job (ver
/// `docs/03-CONTRATOS-API.md` ┬º5). Sprint 0 s├│ emite `stream.ready` e mant├®m
/// a conex├úo viva com heartbeat ÔÇö a publica├º├úo de eventos reais (agent.*,
/// job.*) chega junto do motor de fila (Sprint 1+).
///
/// **Sobre o `AuthContext`.** Esta era a ├║nica rota de `/api/v1` sem
/// extractor de autentica├º├úo, enquanto `GET /jobs/{job_id}` ao lado exige
/// `TenantScope`. Hoje isso n├úo vaza nada ÔÇö o handler ignora o `job_id` al├®m
/// de eco├í-lo e devolve `stream::pending()` ÔÇö, mas vira vazamento entre
/// tenants no instante em que a Sprint 1 publicar eventos reais.
///
/// O extractor entra agora, fechado por padr├úo, mesmo sem consumidor. N├úo
/// resolve o problema de fundo: `EventSource` (usado por
/// `ui/src/hooks/useParamStream.ts`) **n├úo consegue mandar header
/// `Authorization`**, ent├úo autenticar de verdade exige decis├úo de design ÔÇö
/// token na query, cookie, ou trocar por um cliente SSE via `fetch`. A
/// diferen├ºa ├® que agora quem for ligar a UI bate no extractor e resolve
/// naquele momento, em vez de descobrir uma rota aberta depois que ela j├í
/// publica evento real. `tenant_id` ainda n├úo ├® usado para filtrar porque n├úo
/// h├í o que filtrar; quando houver, o escopo j├í est├í em m├úos.
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
