use super::auth::AuthContext;
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use uuid::Uuid;

/// Escopo de tenant resolvido a partir do JWT. Handlers que s├│ precisam do
/// `tenant_id` (sem o resto das claims) usam este extractor ÔÇö mant├®m o
/// princ├¡pio de `docs/03-CONTRATOS-API.md` ┬º1: nenhum endpoint aceita
/// `tenant_id` vindo de corpo ou query.
#[derive(Debug, Clone, Copy)]
pub struct TenantScope(pub Uuid);

impl<S> FromRequestParts<S> for TenantScope
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let AuthContext(claims) = AuthContext::from_request_parts(parts, state).await?;
        Ok(TenantScope(claims.tenant_id))
    }
}
