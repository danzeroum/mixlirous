//! Testes de integração dos fixes QA-0005 e QA-0006 — eco de `traceparent`
//! (W3C Trace Context) e fallback problem+json (RFC 7807).
//!
//! Estes testes exercitam o stack completo (middleware + fallback +
//! nesting em /api/v1) usando `tower::ServiceExt::oneshot` sobre o
//! `api_router()` real, com o mesmo nesting do binário (`/api/v1`).

use audio_agent::llm::mock::MockLlm;
use audio_agent::validator::ValidationLayer;
use audio_agent::ReActOrchestrator;
use audio_api::adapters::InMemoryRepo;
use audio_api::config::AppConfig;
use audio_api::routes::api_router;
use audio_api::state::AppState;
use audio_api::storage::LocalFsStorage;
use audio_core::ports::{AudioRepo, Storage};
use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
    Router,
};
use std::sync::Arc;
use tower::ServiceExt;

const TRACEPARENT_HEADER: &str = "traceparent";
const VALID_TRACEPARENT: &str = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
const VALID_TRACE_ID: &str = "4bf92f3577b34da6a3ce929d0e0e4736";

fn state() -> (AppState, tempfile::TempDir) {
    std::env::set_var("JWT_SECRET", "test-secret-qa-contracts");
    let repo: Arc<dyn AudioRepo> = InMemoryRepo::new();
    let tmp = tempfile::tempdir().expect("tempdir");
    let storage: Arc<dyn Storage> =
        Arc::new(LocalFsStorage::new(tmp.path().to_path_buf()).expect("local fs"));
    let validator = Arc::new(ValidationLayer::new());
    let mock = Arc::new(MockLlm::new());
    let orchestrator = Arc::new(ReActOrchestrator::<MockLlm>::new(validator, mock, 5));
    let hub = Arc::new(audio_api::sse::EventHub::new());
    let app = AppState::new(
        repo,
        orchestrator,
        Arc::new(AppConfig::default()),
        hub,
        storage,
    );
    (app, tmp)
}

/// Router de teste que reproduz a topologia do binário (`main.rs`):
/// api_router sob /api/v1, fallback global no app, e middleware de eco
/// de traceparent como camada mais externa. Sem isso, os testes de
/// fallback /api/* (fora de /api/v1) e a propagação de `trace_id` para
/// o problem+json não exercitam o stack real.
fn router() -> Router {
    let (state, _tmp) = state();
    let app = axum::Router::new()
        .nest("/api/v1", api_router())
        .fallback(audio_api::routes::app_fallback_problem)
        .with_state(state);
    app.layer(axum::middleware::from_fn(
        audio_api::middleware::trace::echo_traceparent,
    ))
}

async fn body_str(resp: axum::response::Response) -> String {
    let bytes = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

// ===========================================================================
// QA-0005 — eco de `traceparent` (W3C Trace Context)
// ===========================================================================

#[tokio::test]
async fn qa0005_rota_existente_ecoa_traceparent_do_cliente() {
    // Contrato docs/03 §1: "cliente envia traceparent (W3C); servidor
    // devolve no response". Rota válida (/api/v1/tools) com traceparent.
    let app = router();
    let req = Request::builder()
        .uri("/api/v1/tools")
        .header(TRACEPARENT_HEADER, VALID_TRACEPARENT)
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let echoed = resp
        .headers()
        .get(TRACEPARENT_HEADER)
        .and_then(|v| v.to_str().ok())
        .expect("traceparent deve ser ecoado");
    assert_eq!(echoed, VALID_TRACEPARENT);
}

#[tokio::test]
async fn qa0005_rota_existente_gera_traceparent_se_ausente() {
    // Sem traceparent na request, servidor ainda devolve um no response.
    let app = router();
    let req = Request::builder()
        .uri("/api/v1/tools")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let echoed = resp
        .headers()
        .get(TRACEPARENT_HEADER)
        .and_then(|v| v.to_str().ok())
        .expect("traceparent deve estar presente mesmo sem cliente enviar");
    // Deve ser um W3C traceparent válido (versão 00, 4 campos).
    let parts: Vec<&str> = echoed.split('-').collect();
    assert_eq!(parts.len(), 4);
    assert_eq!(parts[0], "00");
    assert_eq!(parts[1].len(), 32);
    assert_eq!(parts[2].len(), 16);
    assert_eq!(parts[3].len(), 2);
}

#[tokio::test]
async fn qa0005_traceparent_invalido_e_descartado_e_gerado_novo() {
    let app = router();
    let req = Request::builder()
        .uri("/api/v1/tools")
        .header(TRACEPARENT_HEADER, "lixo-não-é-w3c-válido")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let echoed = resp
        .headers()
        .get(TRACEPARENT_HEADER)
        .and_then(|v| v.to_str().ok())
        .unwrap();
    assert_ne!(echoed, "lixo-não-é-w3c-válido");
    let parts: Vec<&str> = echoed.split('-').collect();
    assert_eq!(parts.len(), 4, "traceparent gerado deve ser W3C válido");
}

#[tokio::test]
async fn qa0005_fallback_404_tambem_carrega_traceparent() {
    // Mesmo o 404 do fallback leva o header — prova que o middleware é
    // camada externa e cobre TODAS as respostas.
    let app = router();
    let req = Request::builder()
        .uri("/api/v1/webqa-nao-existe")
        .header(TRACEPARENT_HEADER, VALID_TRACEPARENT)
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let echoed = resp
        .headers()
        .get(TRACEPARENT_HEADER)
        .and_then(|v| v.to_str().ok())
        .expect("mesmo 404 deve carregar traceparent");
    assert_eq!(echoed, VALID_TRACEPARENT);
}

// ===========================================================================
// QA-0006 — fallback problem+json (RFC 7807)
// ===========================================================================

#[tokio::test]
async fn qa0006_rota_inexistente_em_api_v1_devolve_problem_json() {
    // /api/v1/<inexistente> cai no fallback do api_router.
    let app = router();
    let req = Request::builder()
        .uri("/api/v1/webqa-nao-existe")
        .header(header::ACCEPT, "application/json")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let ct = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap();
    assert!(
        ct.starts_with("application/problem+json"),
        "Content-Type deve ser problem+json, foi {ct}"
    );
    let body = body_str(resp).await;
    assert!(!body.is_empty(), "body do 404 não pode ser vazio");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["status"], 404);
    assert_eq!(v["code"], "not_found");
    assert_eq!(v["instance"], "/api/v1/webqa-nao-existe");
}

#[tokio::test]
async fn qa0006_rota_em_api_fora_de_v1_devolve_problem_json() {
    // /api/<inexistente> (sem /v1) cai no fallback global do app.
    let app = router();
    let req = Request::builder()
        .uri("/api/webqa-nao-existe")
        .header(header::ACCEPT, "application/json")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let ct = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap();
    assert!(
        ct.starts_with("application/problem+json"),
        "Content-Type deve ser problem+json, foi {ct}"
    );
    let body = body_str(resp).await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["code"], "not_found");
    assert_eq!(v["instance"], "/api/webqa-nao-existe");
}

#[tokio::test]
async fn qa0006_problem_json_inclui_trace_id_quando_cliente_envia_traceparent() {
    // Correlação: o `trace_id` do traceparent do cliente aparece no body
    // do problem+json — é o que transforma um ticket de suporte de horas
    // em minutos (docs/03 §4).
    let app = router();
    let req = Request::builder()
        .uri("/api/webqa-nao-existe")
        .header(TRACEPARENT_HEADER, VALID_TRACEPARENT)
        .header(header::ACCEPT, "application/json")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let body = body_str(resp).await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["trace_id"], VALID_TRACE_ID);
}

#[tokio::test]
async fn qa0006_problem_json_inclui_trace_id_gerado_se_cliente_nao_enviou() {
    // Mesmo sem traceparent do cliente, o servidor gera um e o inclui
    // no problem+json — todo erro tem correlação.
    let app = router();
    let req = Request::builder()
        .uri("/api/webqa-nao-existe")
        .header(header::ACCEPT, "application/json")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let body = body_str(resp).await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    let trace_id = v["trace_id"].as_str().expect("trace_id deve estar no body");
    assert_eq!(trace_id.len(), 32, "trace_id gerado deve ter 32 hex chars");
    assert!(
        trace_id.chars().all(|c| c.is_ascii_hexdigit()),
        "trace_id deve ser hex"
    );
}

#[tokio::test]
async fn qa0006_problem_json_tem_type_uri_estavel() {
    // RFC 7807 §3.1.1: `type` é uma URI que identifica o tipo de problema.
    // Para `not_found`, expomos uma URI canônica do namespace do projeto.
    let app = router();
    let req = Request::builder()
        .uri("/api/v1/webqa-nao-existe")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let body = body_str(resp).await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    let type_uri = v["type"].as_str().unwrap();
    assert!(
        type_uri.starts_with("https://mixlirous.dev/errors/"),
        "type deve ser URI do namespace do projeto, foi {type_uri}"
    );
    assert!(type_uri.ends_with("not_found"));
}

#[tokio::test]
async fn qa0006_problem_json_tem_title_estavel_por_code() {
    // `title` é estável por `code` (RFC §3.1.2) — a UI pode apresentá-lo
    // sem conhecer o `code` específico.
    let app = router();
    let req = Request::builder()
        .uri("/api/webqa-nao-existe")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let body = body_str(resp).await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["title"], "Recurso não encontrado");
}
