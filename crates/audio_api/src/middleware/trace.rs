//! Middleware de correlação W3C Trace Context — eco do header `traceparent`
//! em todas as respostas (QA-0005).
//!
//! Contrato (`docs/03-CONTRATOS-API.md` §1):
//! > "cliente envia `traceparent` (W3C); servidor devolve no response"
//!
//! O extrator em `middleware/otel.rs` (`TraceParent`) já lê o header, mas
//! apenas como tipo de parâmetro em handlers — não há eco. Este middleware
//! garante que toda resposta (de qualquer rota, do `/healthz` ao fallback
//! 404) carregue o `traceparent`, seja o recebido (eco) ou um gerado
//! ad-hoc quando o cliente não enviou.
//!
//! A geração usa RNG do `rand` para `trace_id` (32 hex) e `span_id`
//! (16 hex), conforme W3C Trace Context Level 1 §3.3:
//! - `version-trace_id-parent_id-trace_flags`
//! - `trace_id` e `parent_id` não podem ser todos zeros
//! - `trace_flags`: `01` (sampled) por padrão

use axum::{extract::Request, http::HeaderValue, middleware::Next, response::Response};
use rand::Rng;

pub const TRACEPARENT_HEADER: &str = "traceparent";

/// Versão do formato W3C Trace Context atualmente em uso.
const TRACE_CONTEXT_VERSION: &str = "00";
/// Flags padrão: sampled bit setado (`01`).
const TRACE_FLAGS_SAMPLED: &str = "01";

/// Middleware axum (`from_fn`) que garante `traceparent` no response.
///
/// Fluxo:
/// 1. Lê `traceparent` da request.
/// 2. Se presente e válido (formato W3C), ecoa no response.
/// 3. Se ausente ou inválido, gera um novo no formato W3C e adiciona ao
///    response. O `trace_id` gerado também é inserido nas extensões da
///    request para que handlers e o fallback 404 possam ler e incluir
///    no `problem+json` (campo `trace_id`).
pub async fn echo_traceparent(mut req: Request, next: Next) -> Response {
    // Lê o `traceparent` da request, se presente.
    let incoming = req
        .headers()
        .get(TRACEPARENT_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string());

    let (echo_value, generated_trace_id) = match incoming.as_deref() {
        Some(raw) if is_valid_w3c_traceparent(raw) => (raw.to_string(), trace_id_of(raw)),
        _ => {
            // Cliente não enviou (ou enviou inválido): gera um novo.
            let generated = generate_w3c_traceparent();
            let trace_id = trace_id_of(&generated);
            (generated, trace_id)
        },
    };

    // Disponibiliza o trace_id para handlers via extensões da request.
    // O fallback 404 e os handlers de erro podem ler `req.extensions()`
    // para incluir o `trace_id` no `problem+json`. Para um traceparent
    // válido (ecoado ou gerado), `trace_id_of` sempre devolve `Some`;
    // `unwrap_or_default` cobre o caso patológico de None sem panic.
    req.extensions_mut().insert(TraceIdInResponse(
        generated_trace_id.clone().unwrap_or_default(),
    ));

    let mut response = next.run(req).await;

    // Adiciona o header ao response. Não sobrescreve caso o handler já
    // tenha setado (raro — só o faz quem quer trocar o trace_id por
    // motivo de span interno; respeitamos).
    if !response.headers().contains_key(TRACEPARENT_HEADER) {
        if let Ok(hv) = HeaderValue::from_str(&echo_value) {
            response.headers_mut().insert(TRACEPARENT_HEADER, hv);
        }
    }

    response
}

/// Extensão de request carregando o `trace_id` (32 hex) que será visível
/// no response. Handlers e fallbacks podem ler isto para construir
/// `problem+json` com campo `trace_id`.
#[derive(Debug, Clone)]
pub struct TraceIdInResponse(pub String);

/// Extrai o `trace_id` (segundo campo) de um `traceparent` válido.
/// Formato: `version-trace_id-parent_id-trace_flags` (4 campos separados
/// por `-`). Retorna `None` se o formato não casar.
fn trace_id_of(raw: &str) -> Option<String> {
    let parts: Vec<&str> = raw.split('-').collect();
    if parts.len() == 4 && parts[1].len() == 32 {
        Some(parts[1].to_lowercase())
    } else {
        None
    }
}

/// Validação léxica de um `traceparent` W3C (formato Level 1 §3.3).
///
/// Aceita: `00-<32 hex lowercase>-<16 hex lowercase>-<2 hex lowercase>`.
/// Rejeita: versão diferente de `00` (futura prova — só aceitamos a versão
/// atual), `trace_id` ou `parent_id` todos zeros, caracteres não-hex.
pub fn is_valid_w3c_traceparent(raw: &str) -> bool {
    let parts: Vec<&str> = raw.split('-').collect();
    if parts.len() != 4 {
        return false;
    }
    let [version, trace_id, parent_id, flags] = [parts[0], parts[1], parts[2], parts[3]];
    if version != TRACE_CONTEXT_VERSION || flags.len() != 2 {
        return false;
    }
    if trace_id.len() != 32 || parent_id.len() != 16 {
        return false;
    }
    if !is_hex_lowercase(trace_id) || !is_hex_lowercase(parent_id) || !is_hex_lowercase(flags) {
        return false;
    }
    // trace_id e parent_id não podem ser todos zeros.
    !trace_id.chars().all(|c| c == '0') && !parent_id.chars().all(|c| c == '0')
}

fn is_hex_lowercase(s: &str) -> bool {
    s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Gera um `traceparent` W3C válido aleatório (RNG thread-local).
/// `trace_id` = 32 hex (16 bytes), `parent_id` = 16 hex (8 bytes).
pub fn generate_w3c_traceparent() -> String {
    let mut rng = rand::rng();
    let trace_id_bytes: [u8; 16] = rng.random();
    let parent_id_bytes: [u8; 8] = rng.random();

    // W3C exige que nem todos os bytes sejam zero. Probabilidade
    // desprezível, mas garantimos.
    let trace_id_bytes = if trace_id_bytes.iter().all(|&b| b == 0) {
        [1u8; 16]
    } else {
        trace_id_bytes
    };
    let parent_id_bytes = if parent_id_bytes.iter().all(|&b| b == 0) {
        [1u8; 8]
    } else {
        parent_id_bytes
    };

    format!(
        "{}-{}-{}-{}",
        TRACE_CONTEXT_VERSION,
        hex_encode(&trace_id_bytes),
        hex_encode(&parent_id_bytes),
        TRACE_FLAGS_SAMPLED
    )
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::get,
        Router,
    };
    use tower::ServiceExt;

    async fn echo_handler() -> StatusCode {
        StatusCode::OK
    }

    fn app() -> Router {
        Router::new()
            .route("/probe", get(echo_handler))
            .layer(axum::middleware::from_fn(echo_traceparent))
    }

    #[tokio::test]
    async fn eco_traceparent_quando_cliente_envia() {
        let app = app();
        let req = Request::builder()
            .uri("/probe")
            .header(
                TRACEPARENT_HEADER,
                "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
            )
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let echoed = resp
            .headers()
            .get(TRACEPARENT_HEADER)
            .and_then(|v| v.to_str().ok())
            .unwrap();
        assert_eq!(
            echoed,
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
        );
    }

    #[tokio::test]
    async fn gera_traceparent_quando_cliente_nao_envia() {
        // QA-0005: sem traceparent na request, o servidor ainda devolve
        // um no response (gerado, W3C válido).
        let app = app();
        let req = Request::builder()
            .uri("/probe")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let echoed = resp
            .headers()
            .get(TRACEPARENT_HEADER)
            .and_then(|v| v.to_str().ok())
            .expect("traceparent deve estar presente mesmo sem cliente enviar");
        assert!(is_valid_w3c_traceparent(echoed));
    }

    #[tokio::test]
    async fn rejeita_traceparent_invalido_e_gera_novo() {
        // Cliente manda lixo no traceparent — servidor descarta e gera um novo.
        let app = app();
        let req = Request::builder()
            .uri("/probe")
            .header(TRACEPARENT_HEADER, "não-é-um-traceparent-válido")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let echoed = resp
            .headers()
            .get(TRACEPARENT_HEADER)
            .and_then(|v| v.to_str().ok())
            .unwrap();
        assert!(is_valid_w3c_traceparent(echoed));
        assert_ne!(echoed, "não-é-um-traceparent-válido");
    }

    #[tokio::test]
    async fn extensao_trace_id_disponivel_para_handler() {
        // Handler que lê a extensão inserida pelo middleware e a devolve
        // no body — prova que handlers e fallbacks podem acessar o
        // trace_id para incluir em problem+json.
        use axum::Extension;
        async fn ler_trace(Extension(tid): Extension<TraceIdInResponse>) -> String {
            tid.0
        }
        let app = Router::new()
            .route("/probe", get(ler_trace))
            .layer(axum::middleware::from_fn(echo_traceparent));
        let req = Request::builder()
            .uri("/probe")
            .header(
                TRACEPARENT_HEADER,
                "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
            )
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let tid = String::from_utf8(bytes.to_vec()).unwrap();
        assert_eq!(tid, "4bf92f3577b34da6a3ce929d0e0e4736");
    }

    #[test]
    fn validacao_aceita_formato_canonico() {
        assert!(is_valid_w3c_traceparent(
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
        ));
        // flags diferentes são válidas (00, 01, etc.)
        assert!(is_valid_w3c_traceparent(
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-00"
        ));
    }

    #[test]
    fn validacao_rejeita_tracced_id_zero() {
        assert!(!is_valid_w3c_traceparent(
            "00-00000000000000000000000000000000-00f067aa0ba902b7-01"
        ));
    }

    #[test]
    fn validacao_rejeita_parent_id_zero() {
        assert!(!is_valid_w3c_traceparent(
            "00-4bf92f3577b34da6a3ce929d0e0e4736-0000000000000000-01"
        ));
    }

    #[test]
    fn validacao_rejeita_maiusculas() {
        // W3C exige lowercase.
        assert!(!is_valid_w3c_traceparent(
            "00-4BF92F3577B34DA6A3CE929D0E0E4736-00F067AA0BA902B7-01"
        ));
    }

    #[test]
    fn validacao_rejeita_versao_nao_zero() {
        // Apenas versão "00" é suportada atualmente.
        assert!(!is_valid_w3c_traceparent(
            "ff-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
        ));
    }

    #[test]
    fn validacao_rejeita_campos_errados() {
        // Poucos campos.
        assert!(!is_valid_w3c_traceparent(
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7"
        ));
        // trace_id curto.
        assert!(!is_valid_w3c_traceparent("00-deadbeef-00f067aa0ba902b7-01"));
    }

    #[test]
    fn geracao_produz_w3c_valido() {
        let g = generate_w3c_traceparent();
        assert!(is_valid_w3c_traceparent(&g));
        // Probabilidade de colisão entre dois gerados é desprezível;
        // checamos só por sanidade que o gerador não retorna sempre o
        // mesmo valor.
        let g2 = generate_w3c_traceparent();
        assert_ne!(g, g2);
    }
}
