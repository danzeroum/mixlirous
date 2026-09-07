//! Rotas de autenticação de sessão — issue #33 (Lote 2 do plano Pareto).
//!
//! `POST /api/v1/auth/sse-session`: emitido para quem JÁ apresentou um
//! Bearer válido; grava um cookie `HttpOnly`/`SameSite=Lax` de curta
//! duração que o `EventSource` (que não manda headers) apresenta
//! automaticamente no handshake, same-origin.
//!
//! `GET /api/v1/auth/local-session`: modo local single-user (contrato
//! docs/03 §1 "Modo local"): o servidor emite um JWT de sessão e o
//! frontend o usa como Bearer nos comandos REST e como semente do cookie
//! de SSE. **Fail-closed**: fora de `CONFIG_ENV=local` a rota nem existe
//! para o cliente (404) — nunca é um login público disfarçado.

use crate::middleware::auth::{
    decode_claims, encode_claims, AuthContext, TenantClaims, SSE_SESSION_COOKIE,
};
use crate::state::AppState;
use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use uuid::Uuid;

/// TTL do cookie de SSE (segundos). Curto de propósito: o cookie é um
/// porta de entrada de LEITURA de eventos; 1 hora cobre uma sessão de
/// escuta com reconexões e limita a janela se o disco/memória do browser
/// for espiado.
const SSE_COOKIE_TTL_SEC: u64 = 3600;
/// TTL do token de sessão local (contrato docs/03 §1 — sessão local
/// persistida no cliente; 30 dias, revogada por rotação do `JWT_SECRET`).
const LOCAL_SESSION_TTL_SEC: i64 = 30 * 24 * 3600;

#[derive(Debug, Serialize)]
pub struct LocalSessionResponse {
    pub token: String,
    pub expires_at: String,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
}

/// Emite um JWT com claims novas a partir das claims autenticadas,
/// com `exp` relativa a agora.
fn mint_claims(base: &TenantClaims, ttl_sec: i64) -> TenantClaims {
    let now = chrono::Utc::now();
    TenantClaims {
        sub: base.sub,
        tenant_id: base.tenant_id,
        roles: base.roles.clone(),
        plan: base.plan.clone(),
        iat: now.timestamp() as usize,
        exp: (now + chrono::Duration::seconds(ttl_sec)).timestamp() as usize,
    }
}

fn set_session_cookie(mut resp: Response, token: &str, max_age: u64) -> Response {
    let cookie = format!(
        "{SSE_SESSION_COOKIE}={token}; Max-Age={max_age}; Path=/api/v1; HttpOnly; SameSite=Lax"
    );
    resp.headers_mut().insert(
        header::SET_COOKIE,
        header::HeaderValue::from_str(&cookie)
            .unwrap_or_else(|_| header::HeaderValue::from_static(SSE_SESSION_COOKIE)),
    );
    resp
}

/// POST /api/v1/auth/sse-session — exige Bearer OU cookie ainda válido
/// (o `AuthContext` cobre os dois caminhos). Renova o cookie de sessão
/// de SSE com claims recém-assinadas. 204 No Content — a resposta útil
/// está no `Set-Cookie`.
pub async fn post_sse_session(
    _state: State<AppState>,
    AuthContext(claims): AuthContext,
) -> Result<Response, (StatusCode, String)> {
    let token = encode_claims(
        &mint_claims(&claims, SSE_COOKIE_TTL_SEC as i64),
        &crate::middleware::auth::jwt_secret(),
    )
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("mint sse token: {e}"),
        )
    })?;

    let resp = Response::builder()
        .status(StatusCode::NO_CONTENT)
        .body(axum::body::Body::empty())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("body: {e}")))?;

    Ok(set_session_cookie(resp, &token, SSE_COOKIE_TTL_SEC))
}

/// GET /api/v1/auth/local-session — modo local apenas (fail-closed fora
/// de `CONFIG_ENV=local`: 404, como rota inexistente). Emite o JWT de
/// sessão do single-user local no corpo E no cookie (mesmo token), para
/// que a UI consiga: (a) mandar Bearer nos comandos REST e (b) abrir o
/// `EventSource` com o cookie de sessão sem segunda chamada.
pub async fn get_local_session(
    State(state): State<AppState>,
) -> Result<Response, (StatusCode, String)> {
    // fix CI (PR #59): o modo é lido do AppConfig capturado no boot — não
    // mais de `std::env::var` por request, que era mutável por qualquer
    // thread do processo e fazia os testes de integração de local-session
    // correrem (set_var paralelo de "local" x "production" no mesmo binário).
    if state.config.config_env != "local" {
        // 404 de propósito: fora do modo local a rota não existe. Nunca
        // é um endpoint de login anônimo em produção (docs/08 §1).
        return Err((StatusCode::NOT_FOUND, "not_found".to_string()));
    }

    let now = chrono::Utc::now();
    let claims = TenantClaims {
        sub: Uuid::new_v4(),
        tenant_id: Uuid::new_v4(),
        roles: vec!["owner".to_string()],
        plan: "free".to_string(),
        iat: now.timestamp() as usize,
        exp: (now + chrono::Duration::seconds(LOCAL_SESSION_TTL_SEC)).timestamp() as usize,
    };

    let token = encode_claims(&claims, &crate::middleware::auth::jwt_secret()).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("mint local token: {e}"),
        )
    })?;

    // Sanidade: o token emitido precisa validar com o MESMO segredo que o
    // extractor vai usar — pega o bug de dois segredos divergentes na hora.
    decode_claims(&token, &crate::middleware::auth::jwt_secret()).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("self-check: {e}"),
        )
    })?;

    let body = LocalSessionResponse {
        token: token.clone(),
        expires_at: (now + chrono::Duration::seconds(LOCAL_SESSION_TTL_SEC)).to_rfc3339(),
        tenant_id: claims.tenant_id,
        user_id: claims.sub,
    };

    let resp = Json(body).into_response();

    Ok(set_session_cookie(
        resp,
        &token,
        LOCAL_SESSION_TTL_SEC as u64,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O cookie emitido carrega os atributos mínimos de segurança (#33).
    #[test]
    fn cookie_tem_atributos_de_seguranca() {
        let resp = Response::builder()
            .status(StatusCode::NO_CONTENT)
            .body(axum::body::Body::empty())
            .unwrap();
        let resp = set_session_cookie(resp, "tok123", 60);
        let cookie = resp
            .headers()
            .get(header::SET_COOKIE)
            .and_then(|v| v.to_str().ok())
            .unwrap();
        assert!(cookie.starts_with("mixlirous_session=tok123"));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Lax"));
        assert!(cookie.contains("Path=/api/v1"));
        assert!(cookie.contains("Max-Age=60"));
    }

    /// mint_claims renova iat/exp preservando identidade (sub/tenant).
    #[test]
    fn mint_claims_preserva_identidade_e_renova_exp() {
        let base = TenantClaims {
            sub: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            roles: vec!["owner".to_string()],
            plan: "free".to_string(),
            iat: 0,
            exp: 0,
        };
        let minted = mint_claims(&base, 3600);
        assert_eq!(minted.sub, base.sub);
        assert_eq!(minted.tenant_id, base.tenant_id);
        assert!(minted.exp > base.exp);
        assert!(minted.iat > 0);
    }
}
