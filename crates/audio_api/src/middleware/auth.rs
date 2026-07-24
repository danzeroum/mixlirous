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

/// Extractor de autenticação: decodifica e valida o JWT do header
/// `Authorization: Bearer <token>`. `tenant_id` do token é a única fonte de
/// verdade — nenhum handler aceita `tenant_id` no corpo ou na query.
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
}
