use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const AUTH_SCHEME: &str = "Bearer ";

fn jwt_secret() -> String {
    std::env::var("JWT_SECRET").unwrap_or_else(|_| "local-dev-secret-change-me".to_string())
}

/// Chamado uma vez no boot (`main.rs`, logo ap├│s `AppConfig::load()`). O
/// fallback de `jwt_secret()` existe para rodar local sem configurar nada ÔÇö
/// mas se isso chegar em qualquer ambiente que n├úo seja `local` sem
/// `JWT_SECRET` definido, todo token ├® assinado com uma string hardcoded
/// neste reposit├│rio p├║blico. `.env.example` documenta tr├¬s valores para
/// `CONFIG_ENV` (`local | default | production`) ÔÇö fail-closed exige o
/// segredo nos dois que n├úo s├úo `local`, em vez de listar por nome s├│
/// `production` (falharia aberto em `default`, que j├í ├® modo VPS real com
/// Postgres/MinIO ÔÇö ver `docker-compose.yml`, n├úo ├® o laptop do
/// desenvolvedor). Falha o boot em vez de subir servindo com um segredo
/// forj├ível (ver `docs/08-SEGURANCA-MULTITENANCY.md` ┬º9).
pub fn assert_secret_configured_for_production(config_env: &str, jwt_secret_is_set: bool) {
    assert!(
        config_env == "local" || jwt_secret_is_set,
        "JWT_SECRET n├úo definido com CONFIG_ENV={config_env} ÔÇö recusando subir com o \
         segredo de desenvolvimento hardcoded fora do modo local"
    );
}

/// Claims esperadas do JWT (ver `docs/03-CONTRATOS-API.md` ┬º1). S├│
/// `tenant_id` ├® consumido hoje (via `TenantScope`); os demais campos ficam
/// dispon├¡veis para checagens de autoriza├º├úo por papel/plano na Sprint 1+.
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

/// Extractor de autentica├º├úo: decodifica e valida o JWT do header
/// `Authorization: Bearer <token>`. `tenant_id` do token ├® a ├║nica fonte de
/// verdade ÔÇö nenhum handler aceita `tenant_id` no corpo ou na query.
#[derive(Debug, Clone)]
pub struct AuthContext(pub TenantClaims);

impl<S> FromRequestParts<S> for AuthContext
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or((StatusCode::UNAUTHORIZED, "unauthenticated".to_string()))?;

        let token = header
            .strip_prefix(AUTH_SCHEME)
            .ok_or((StatusCode::UNAUTHORIZED, "unauthenticated".to_string()))?;

        let claims = decode_claims(token, &jwt_secret())
            .map_err(|_| (StatusCode::UNAUTHORIZED, "unauthenticated".to_string()))?;

        Ok(AuthContext(claims))
    }
}

fn decode_claims(token: &str, secret: &str) -> Result<TenantClaims, jsonwebtoken::errors::Error> {
    use jsonwebtoken::{decode, DecodingKey, Validation};

    let key = DecodingKey::from_secret(secret.as_bytes());
    let data = decode::<TenantClaims>(token, &key, &Validation::default())?;
    Ok(data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};

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
    #[should_panic(expected = "JWT_SECRET n├úo definido")]
    fn test_assert_secret_configured_panics_in_production_without_secret() {
        assert_secret_configured_for_production("production", false);
    }

    #[test]
    #[should_panic(expected = "JWT_SECRET n├úo definido")]
    fn test_assert_secret_configured_panics_in_default_without_secret() {
        // Fail-closed: "default" (modo VPS, ver docker-compose.yml) n├úo ├®
        // "local" ÔÇö n├úo pode passar sem segredo s├│ por n├úo se chamar
        // "production". Regress├úo direta contra a vers├úo anterior deste
        // guard, que s├│ checava `config_env != "production"`.
        assert_secret_configured_for_production("default", false);
    }

    #[test]
    #[should_panic(expected = "JWT_SECRET n├úo definido")]
    fn test_assert_secret_configured_panics_on_empty_env_without_secret() {
        // Fail-closed tamb├®m cobre CONFIG_ENV vazia/n├úo setada ÔÇö o valor
        // default de `AppConfig::load()` ├® "local" (string literal), mas
        // esta fun├º├úo n├úo deve confiar nisso: qualquer string que n├úo seja
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
}
