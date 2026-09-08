//! Erros estruturados no formato `application/problem+json` (RFC 7807),
//! com as extensões de catálogo definidas em `docs/03-CONTRATOS-API.md` §4
//! (`code`, `trace_id`, `errors[]`).
//!
//! Este módulo é a fonte canônica de resposta de erro da API. Handlers que
//! precisam devolver um erro estruturado constroem um `Problem` e o retornam
//! — o `IntoResponse` cuida do `Content-Type` e do status.
//!
//! Para rotas não mapeadas (404 do axum), o fallback em `routes::mod::api_router`
//! e o fallback global em `main.rs` constroem um `Problem` a partir da URI
//! da requisição — corrigindo QA-0006 (corpo vazio em `/api/*` 404).

use axum::{
    http::{header, HeaderValue, StatusCode, Uri},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// `Content-Type` para RFC 7807 — `application/problem+json` (não é
/// `application/json` puro: o `+json` é sufixo estrutural da RFC 6839).
pub const CONTENT_TYPE: &str = "application/problem+json";

/// Documento de problema — RFC 7807 + extensões de catálogo (`code`,
/// `trace_id`, `errors[]`) definidas em `docs/03-CONTRATOS-API.md` §4.
///
/// `type` segue a convenção da RFC: uma URI que identifica o tipo de
/// problema. Para erros genéricos sem tipo próprio, usamos `about:blank`
/// (também pela RFC).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Problem {
    /// URI identificadora do tipo de problema (RFC 7807 §3.1.1).
    /// `about:blank` quando o próprio status HTTP já basta.
    #[serde(default = "default_type")]
    pub r#type: String,
    /// Resumo curto, legível por humano, estável por `code` (RFC §3.1.2).
    #[serde(default)]
    pub title: String,
    /// Status HTTP (espelha o do response; RFC §3.1.3).
    pub status: u16,
    /// Código de catálogo (docs/03 §4) — `not_found`, `unauthenticated`,
    /// `parameter_out_of_bounds` etc. É o que a UI faz match.
    #[serde(default)]
    pub code: String,
    /// Detalhe específico da ocorrência (não estável; pode incluir valores
    /// da requisição — RFC §3.1.5). Nunca inclui segredos.
    #[serde(default)]
    pub detail: String,
    /// Caminho da requisição (RFC §3.1.6 — `instance`).
    #[serde(default)]
    pub instance: String,
    /// `trace_id` W3C (32 hex) para correlação de logs/traces (docs/03 §4
    /// exige visível em 5xx; estendemos para todos os erros do catálogo).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    /// Erros de validação aninhados (extensão docs/03 §4 — campo/valor).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<Value>,
}

fn default_type() -> String {
    "about:blank".to_string()
}

impl Problem {
    /// Construtor mínimo: status + code. `title` é derivado do code quando
    /// vazio (catálogo comum em `docs/03-CONTRATOS-API.md` §4).
    pub fn new(status: StatusCode, code: &str) -> Self {
        let title = title_for_code(code).to_string();
        Self {
            r#type: default_type(),
            title,
            status: status.as_u16(),
            code: code.to_string(),
            detail: String::new(),
            instance: String::new(),
            trace_id: None,
            errors: Vec::new(),
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }

    pub fn with_instance(mut self, instance: impl Into<String>) -> Self {
        self.instance = instance.into();
        self
    }

    pub fn with_trace_id(mut self, trace_id: impl Into<String>) -> Self {
        self.trace_id = Some(trace_id.into());
        self
    }

    pub fn with_type(mut self, r#type: impl Into<String>) -> Self {
        self.r#type = r#type.into();
        self
    }

    /// Variação de `with_trace_id` que aceita `Option`. Usada pelos helpers
    /// de fallback que recebem `Option<&str>`.
    pub fn with_trace_id_opt(mut self, trace_id: Option<String>) -> Self {
        self.trace_id = trace_id;
        self
    }

    /// Empacota num `Response` axum com `Content-Type: application/problem+json`
    /// e o status correto. JSON é serializado pelo `serde_json` (ordem
    /// canônica das chaves conforme declaração do struct).
    pub fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let mut resp = Json(self).into_response();
        resp.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(CONTENT_TYPE),
        );
        *resp.status_mut() = status;
        resp
    }
}

/// Títulos curtos, estáveis por `code` (catálogo de `docs/03-CONTRATOS-API.md`
/// §4). Retornados em `title` para a UI apresentar de forma consistente —
/// a UI faz match por `code` (que é o contrato), mas o `title` é o fallback
/// legível por humano quando a UI não conhece o code.
fn title_for_code(code: &str) -> &'static str {
    match code {
        "malformed_request" => "Requisição malformada",
        "unauthenticated" => "Não autenticado",
        "forbidden" => "Acesso negado",
        "not_found" => "Recurso não encontrado",
        "job_not_editable" => "Job não é editável",
        "proposal_expired" => "Proposta expirada",
        "proposal_already_decided" => "Proposta já decidida",
        "provider_mismatch" => "Provedor inconsistente",
        "consent_not_accepted" => "Consentimento não aceito",
        "docker_unavailable" => "Docker indisponível",
        "file_too_large" => "Arquivo excede o limite",
        "unsupported_media_type" => "Tipo de mídia não suportado",
        "parameter_out_of_bounds" => "Parâmetro fora dos limites",
        "invalid_graph" => "Grafo inválido",
        "malicious_prompt" => "Prompt rejeitado",
        "limit_exceeded" => "Limite excedido",
        "rate_limited" => "Muitas requisições",
        "quota_exceeded" => "Quota do plano excedida",
        "internal_error" => "Erro interno",
        "llm_unavailable" => "Provedor LLM indisponível",
        "storage_unavailable" => "Armazenamento indisponível",
        "method_not_allowed" => "Método não permitido",
        "route_not_found" => "Rota não encontrada",
        _ => "Erro",
    }
}

/// Helper para o fallback 404 — constrói um `Problem` a partir da URI.
/// Usado tanto no `api_router` (fallback) quanto no `main.rs` (fallback
/// global, capturando `/api/*` não mapeado — ex.: `/api/webqa-nao-existe`
/// que não casa com nenhum `/api/v1/*` registrado).
pub fn not_found(uri: &Uri, trace_id: Option<&str>) -> Response {
    let path = uri.path().to_string();
    Problem::new(StatusCode::NOT_FOUND, "not_found")
        .with_detail(format!("Nenhuma rota corresponde a {path}"))
        .with_instance(path.clone())
        .with_type(format!("https://mixlirous.dev/errors/not_found"))
        .with_trace_id_opt(trace_id.map(|s| s.to_string()))
        .into_response()
}

/// Helper para 405 Method Not Allowed — quando a rota existe mas o método
/// não bate (ex.: `POST /healthz`). Reaproveita `Problem` com code
/// `method_not_allowed`.
pub fn method_not_allowed(uri: &Uri, trace_id: Option<&str>) -> Response {
    let path = uri.path().to_string();
    Problem::new(StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed")
        .with_detail(format!("Método não permitido para {path}"))
        .with_instance(path)
        .with_type(format!("https://mixlirous.dev/errors/method_not_allowed"))
        .with_trace_id_opt(trace_id.map(|s| s.to_string()))
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    fn body_str(resp: Response) -> String {
        let bytes = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async { to_bytes(resp.into_body(), 8 * 1024).await.unwrap() });
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    #[test]
    fn problem_serializa_campos_do_catalogo() {
        let p = Problem::new(StatusCode::NOT_FOUND, "not_found")
            .with_detail("Nenhuma rota corresponde a /api/x")
            .with_instance("/api/x")
            .with_trace_id("4bf92f3577b34da6a3ce929d0e0e4736");
        let v: Value = serde_json::to_value(&p).unwrap();
        assert_eq!(v["status"], 404);
        assert_eq!(v["code"], "not_found");
        assert_eq!(v["title"], "Recurso não encontrado");
        assert_eq!(v["trace_id"], "4bf92f3577b34da6a3ce929d0e0e4736");
        assert_eq!(v["instance"], "/api/x");
        assert!(v["detail"].as_str().unwrap().contains("/api/x"));
    }

    #[test]
    fn into_response_content_type_e_problem_json() {
        let resp = Problem::new(StatusCode::NOT_FOUND, "not_found").into_response();
        let ct = resp
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap();
        assert_eq!(ct, CONTENT_TYPE);
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn into_response_corpo_nao_vazio() {
        // QA-0006: o body do 404 não pode ser vazio. RFC 7807 exige o
        // documento problem+json no body.
        let resp = Problem::new(StatusCode::NOT_FOUND, "not_found")
            .with_detail("rota inexistente")
            .into_response();
        let body = body_str(resp);
        assert!(!body.is_empty());
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["code"], "not_found");
    }

    #[test]
    fn not_found_helper_inclui_path_e_trace_id() {
        let uri: Uri = "/api/webqa-nao-existe".parse().unwrap();
        let resp = not_found(&uri, Some("deadbeefdeadbeefdeadbeefdeadbeef"));
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let body = body_str(resp);
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["instance"], "/api/webqa-nao-existe");
        assert_eq!(v["trace_id"], "deadbeefdeadbeefdeadbeefdeadbeef");
        assert_eq!(v["code"], "not_found");
    }

    #[test]
    fn not_found_helper_sem_trace_id_omite_campo() {
        let uri: Uri = "/api/x".parse().unwrap();
        let resp = not_found(&uri, None);
        let body = body_str(resp);
        let v: Value = serde_json::from_str(&body).unwrap();
        // trace_id é Option::None → omitido pelo serde (skip_serializing_if)
        assert!(v.get("trace_id").is_none() || v["trace_id"].is_null());
    }

    #[test]
    fn titulo_estavel_para_code_conhecido() {
        assert_eq!(title_for_code("not_found"), "Recurso não encontrado");
        assert_eq!(title_for_code("rate_limited"), "Muitas requisições");
        assert_eq!(title_for_code("internal_error"), "Erro interno");
        // code desconhecido cai no fallback genérico
        assert_eq!(title_for_code("codigo_nao_catalogado"), "Erro");
    }
}
