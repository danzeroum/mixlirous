use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const AUTH_SCHEME: &str = "Bearer ";
/// Cookie de sessão same-origin (issue #33 — Lote 2 do plano Pareto). O
/// `EventSource` não consegue mandar header `Authorization`; o handshake
/// SSE valida este cookie como fallback. `HttpOnly` + `SameSite=Lax` +
/// `Path=/api/v1` + exp curta — emitido por `routes::auth::post_sse_session`
/// APENAS para quem já apresentou um Bearer válido.
pub const SSE_SESSION_COOKIE: &str = "mixlirous_session";

pub(crate) fn jwt_secret() -> String {
    std::env::var("JWT_SECRET").unwrap_or_else(|_| "local-dev-secret-change-me".to_string())
}

/// Chamado uma vez no boot (`main.rs`, logo após `AppConfig::load()`). O
/// fallback de `jwt_secret()` existe para rodar local sem configurar nada —
/// mas se isso chegar em qualquer ambiente que não seja `local` sem
/// `JWT_SECRET` definido, todo token é assinado com uma string hardcoded
/// neste repositório público. `.env.example` documenta três valores para
/// `CONFIG_ENV` (`local | default | production`)— fail-closed exige o
/// segredo nos dois que não são `local`, em vez de listar por nome só
/// `production` (falharia aberto em `default`, que já é modo VPS real com
/// Postgres/MinIO — ver `docker-compose.yml`, não é o laptop do
/// desenvolvedor). Falha o boot em vez de subir servindo com um segredo
/// forjável (ver `docs/08-SEGURANCA-MULTITENANCY.md` §9).
pub fn assert_secret_configured_for_production(config_env: &str, jwt_secret_is_set: bool) {
    assert!(
        config_env == "local" || jwt_secret_is_set,
        "JWT_SECRET não definido com CONFIG_ENV={config_env} — recusando subir com o \
         segredo de desenvolvimento hardcoded fora do modo local"
    );
}

/// Claims esperadas do JWT (ver `docs/03-CONTRATOS-API.md` §1). Só
/// `tenant_id` é consumido hoje (via `TenantScope`); os demais campos ficam
/// disponíveis para checagens de autorização por papel/plano na Sprint 1+.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantClaims {
    pub sub: Uuid,
    pub tenant_id: Uuid,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub plan: String,
    pub iat: usize,
    pub exp: usize,
}

/// Extrai o token de `parts`: header `Authorization: Bearer` tem
/// precedência; sem header, cai para o cookie `mixlirous_session` (issue
/// #33). O cookie é same-origin e HttpOnly — o browser só o envia para o
/// próprio host que o emitiu, via `EventSource`/`fetch` com
/// `credentials: 'same-origin'`.
fn extract_token(parts: &Parts) -> Option<String> {
    if let Some(header) = parts
        .headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        if let Some(token) = header.strip_prefix(AUTH_SCHEME) {
            return Some(token.to_string());
        }
    }

    let cookie_header = parts
        .headers
        .get(axum::http::header::COOKIE)?
        .to_str()
        .ok()?;
    for pair in cookie_header.split(';') {
        let pair = pair.trim();
        if let Some(value) = pair.strip_prefix(SSE_SESSION_COOKIE) {
            let value = value.strip_prefix('=')?;
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Extractor de autenticação: decodifica e valida o JWT do header
/// `Authorization: Bearer <token>` ou, no handshake SSE, do cookie de
/// sessão same-origin. `tenant_id` do token é a única fonte de
/// verdade — nenhum handler aceita `tenant_id` no corpo ou na query.
#[derive(Debug, Clone)]
pub struct AuthContext(pub TenantClaims);

impl<S> FromRequestParts<S> for AuthContext
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let token = extract_token(parts)
            .ok_or((StatusCode::UNAUTHORIZED, "unauthenticated".to_string()))?;

        let claims = decode_claims(&token, &jwt_secret())
            .map_err(|_| (StatusCode::UNAUTHORIZED, "unauthenticated".to_string()))?;

        Ok(AuthContext(claims))
    }
}

pub(crate) fn decode_claims(
    token: &str,
    secret: &str,
) -> Result<TenantClaims, jsonwebtoken::errors::Error> {
    use jsonwebtoken::{decode, DecodingKey, Validation};

    let key = DecodingKey::from_secret(secret.as_bytes());
    let data = decode::<TenantClaims>(token, &key, &Validation::default())?;
    Ok(data.claims)
}

/// Assina um JWT com as claims dadas. Compartilhado pelo boot (token de
/// sessão local) e pelas rotas de auth (`routes/auth.rs`).
pub fn encode_claims(
    claims: &TenantClaims,
    secret: &str,
) -> Result<String, jsonwebtoken::errors::Error> {
    use jsonwebtoken::{encode, EncodingKey, Header};

    encode(
        &Header::default(),
        claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};

    fn parts_with(headers: Vec<(axum::http::HeaderName, &'static str)>) -> Parts {
        let mut builder = axum::http::Request::builder();
        for (name, value) in headers {
            builder = builder.header(name, value);
        }
        let (parts, _body) = builder.body(()).expect("request").into_parts();
        parts
    }

    fn sample_claims() -> TenantClaims {
        TenantClaims {
            sub: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            roles: vec!["owner".to_string()],
            plan: "free".to_string(),
            iat: 1_753_380_000,
            exp: 4_753_380_000,
        }
    }

    #[test]
    fn test_decode_claims_roundtrip() {
        let claims = sample_claims();
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(b"test-secret"),
        )
        .unwrap();

        let decoded = decode_claims(&token, "test-secret").unwrap();
        assert_eq!(decoded.tenant_id, claims.tenant_id);
        assert_eq!(decoded.plan, "free");
    }

    #[test]
    fn test_decode_claims_rejects_wrong_secret() {
        let claims = sample_claims();
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(b"other-secret"),
        )
        .unwrap();

        assert!(decode_claims(&token, "test-secret").is_err());
    }

    #[test]
    #[should_panic(expected = "JWT_SECRET não definido")]
    fn test_assert_secret_configured_panics_in_production_without_secret() {
        assert_secret_configured_for_production("production", false);
    }

    #[test]
    #[should_panic(expected = "JWT_SECRET não definido")]
    fn test_assert_secret_configured_panics_in_default_without_secret() {
        // Fail-closed: "default" (modo VPS, ver docker-compose.yml) não é
        // "local" — não pode passar sem segredo só por não se chamar
        // "production". Regressão direta contra a versão anterior deste
        // guard, que só checava `config_env != "production"`.
        assert_secret_configured_for_production("default", false);
    }

    #[test]
    #[should_panic(expected = "JWT_SECRET não definido")]
    fn test_assert_secret_configured_panics_on_empty_env_without_secret() {
        // Fail-closed também cobre CONFIG_ENV vazia/não setada — o valor
        // default de `AppConfig::load()` é "local" (string literal), mas
        // esta função não deve confiar nisso: qualquer string que não seja
        // exatamente "local" exige segredo.
        assert_secret_configured_for_production("", false);
    }

    #[test]
    fn test_assert_secret_configured_allows_production_with_secret() {
        assert_secret_configured_for_production("production", true);
    }

    #[test]
    fn test_assert_secret_configured_allows_default_with_secret() {
        assert_secret_configured_for_production("default", true);
    }

    #[test]
    fn test_assert_secret_configured_allows_local_without_secret() {
        assert_secret_configured_for_production("local", false);
    }

    /// #33 — extração de token: Bearer no header tem precedência.
    #[test]
    fn extract_token_prefere_header_bearer() {
        let parts = parts_with(vec![
            (axum::http::header::AUTHORIZATION, "Bearer header-token"),
            (axum::http::header::COOKIE, "mixlirous_session=cookie-token"),
        ]);
        assert_eq!(extract_token(&parts).as_deref(), Some("header-token"));
    }

    /// #33 — sem header, o cookie `mixlirous_session` fornece o token
    /// (é o caminho do handshake do EventSource).
    #[test]
    fn extract_token_cai_para_cookie_de_sessao() {
        let parts = parts_with(vec![(
            axum::http::header::COOKIE,
            "outro=v1; mixlirous_session=cookie-token; mais=v2",
        )]);
        assert_eq!(extract_token(&parts).as_deref(), Some("cookie-token"));
    }

    #[test]
    fn extract_token_sem_nada_e_none() {
        let parts = parts_with(vec![]);
        assert_eq!(extract_token(&parts), None);
    }

    #[test]
    fn extract_token_cookie_vazio_e_none() {
        let parts = parts_with(vec![(axum::http::header::COOKIE, "mixlirous_session=")]);
        assert_eq!(extract_token(&parts), None);
    }

    /// roundtrip completo via encode/decode com o token vindo do cookie —
    /// prova que o fallback é trust-equivalente ao header.
    #[test]
    fn cookie_token_faz_roundtrip_de_claims() {
        let claims = sample_claims();
        let token = encode_claims(&claims, "test-secret").unwrap();

        let cookie_value = Box::leak(format!("{SSE_SESSION_COOKIE}={token}").into_boxed_str());
        let parts = parts_with(vec![(axum::http::header::COOKIE, cookie_value)]);

        let extracted = extract_token(&parts).unwrap();
        let decoded = decode_claims(&extracted, "test-secret").unwrap();
        assert_eq!(decoded.tenant_id, claims.tenant_id);
        assert_eq!(decoded.sub, claims.sub);
    }
}
