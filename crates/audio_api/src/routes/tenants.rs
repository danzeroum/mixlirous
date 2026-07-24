use crate::middleware::TenantScope;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct TenantQuota {
    pub jobs: JobsQuota,
    pub storage: StorageQuota,
}

#[derive(Debug, Serialize)]
pub struct JobsQuota {
    pub used: u32,
    pub limit: u32,
    pub period: &'static str,
}

#[derive(Debug, Serialize)]
pub struct StorageQuota {
    pub used_gb: f32,
    pub limit_gb: f32,
}

/// Placeholder: números fixos até a fila real existir (Sprint 1+). O
/// endpoint já respeita o formato de `docs/03-CONTRATOS-API.md` §3.8.
pub async fn get_quota(TenantScope(_tenant_id): TenantScope) -> axum::Json<TenantQuota> {
    axum::Json(TenantQuota {
        jobs: JobsQuota {
            used: 0,
            limit: 1000,
            period: "month",
        },
        storage: StorageQuota {
            used_gb: 0.0,
            limit_gb: 10.0,
        },
    })
}
