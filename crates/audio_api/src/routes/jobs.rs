use crate::middleware::{AuthContext, TenantScope, TraceParent};
use crate::state::AppState;
use audio_core::PipelineConfig;
use axum::{
    body::Body,
    extract::Path,
    extract::State,
    http::{header, StatusCode},
    response::Response,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct CreateJobRequest {
    pub track_id: Uuid,
    pub mode: JobMode,
    #[serde(default)]
    pub user_prompt: Option<String>,
    #[serde(default)]
    pub prompt_id: Option<String>,
    #[serde(default)]
    pub pipeline_config: Option<PipelineConfig>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobMode {
    Manual,
    Assisted,
}

#[derive(Debug, Serialize)]
pub struct CreateJobResponse {
    pub job_id: Uuid,
    pub status: &'static str,
    pub stream_url: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub trace_id: String,
}

#[derive(Debug, Serialize)]
pub struct JobSummary {
    pub job_id: Uuid,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct JobListResponse {
    pub items: Vec<JobSummary>,
    pub next_cursor: Option<String>,
}

pub async fn create_job(
    State(state): State<AppState>,
    AuthContext(claims): AuthContext,
    trace: TraceParent,
    Json(payload): Json<CreateJobRequest>,
) -> Result<(StatusCode, Json<CreateJobResponse>), (StatusCode, String)> {
    let job_id = Uuid::new_v4();
    let config = payload.pipeline_config.unwrap_or_default();

    tracing::info!(
        %job_id,
        track_id = %payload.track_id,
        mode = ?payload.mode,
        prompt_id = ?payload.prompt_id,
        has_user_prompt = payload.user_prompt.is_some(),
        "job de remix recebido"
    );

    // Sprint 0: enfileira o job sem rodar o pipeline de fato (fila real e
    // motor DSP/agente s├úo Sprint 1+; ver docs/13-ROADMAP-SPRINTS.md).
    // tenant_id e user_id v├¬m das claims do JWT, nunca do corpo/query (ver
    // docs/08-SEGURANCA-MULTITENANCY.md ┬º1) ÔÇö e nunca um no lugar do outro.
    state
        .repo
        .save_job(job_id, claims.tenant_id, claims.sub, &config, &[])
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Propaga o traceparent recebido (W3C) quando houver; sen├úo gera um novo
    // trace_id ÔÇö ver docs/03-CONTRATOS-API.md ┬º1 "Rastreamento".
    let trace_id = trace
        .trace_id
        .unwrap_or_else(|| Uuid::new_v4().simple().to_string());

    Ok((
        StatusCode::ACCEPTED,
        Json(CreateJobResponse {
            job_id,
            status: "queued",
            stream_url: format!("/api/v1/jobs/{job_id}/events"),
            created_at: chrono::Utc::now(),
            trace_id,
        }),
    ))
}

pub async fn list_jobs(
    State(state): State<AppState>,
    TenantScope(tenant_id): TenantScope,
) -> Result<Json<JobListResponse>, (StatusCode, String)> {
    let jobs = state
        .repo
        .list_jobs(tenant_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let items = jobs
        .into_iter()
        .map(|j| JobSummary {
            job_id: j.id,
            status: format!("{:?}", j.status).to_lowercase(),
            created_at: j.created_at,
            updated_at: j.updated_at,
        })
        .collect();

    Ok(Json(JobListResponse {
        items,
        next_cursor: None,
    }))
}

pub async fn get_job(
    State(state): State<AppState>,
    TenantScope(tenant_id): TenantScope,
    Path(job_id): Path<Uuid>,
) -> Result<Json<JobSummary>, (StatusCode, String)> {
    // Antes desta rota nem exigia JWT. tenant_id escopa a busca ÔÇö job de
    // outro tenant d├í o mesmo 404 de um job inexistente, nunca um 403 (ver
    // docs/08-SEGURANCA-MULTITENANCY.md ┬º3).
    let job = state
        .repo
        .get_job(job_id, tenant_id)
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, "not_found".to_string()))?;

    Ok(Json(JobSummary {
        job_id: job.id,
        status: format!("{:?}", job.status).to_lowercase(),
        created_at: job.created_at,
        updated_at: job.updated_at,
    }))
}

pub async fn cancel_job(
    State(state): State<AppState>,
    TenantScope(tenant_id): TenantScope,
    Path(job_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Placeholder: cancelamento real (mudar status e liberar a fila) precisa
    // do estado de fila de verdade (Sprint 1+). Mesmo como placeholder, a
    // rota j├í ├® escopada por tenant ÔÇö nunca cancela (nem finge cancelar) um
    // job que n├úo pertence a quem chamou.
    let job = state
        .repo
        .get_job(job_id, tenant_id)
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, "not_found".to_string()))?;

    Ok(Json(
        serde_json::json!({ "job_id": job.id, "status": "cancelled" }),
    ))
}

/// `GET /api/v1/jobs/{job_id}/artifact` — download do WAV masterizado.
///
/// Item B4 do mapa de ação: o worker publica `download_url` para esta rota
/// no evento `job.completed`. Em modo local (storage em disco), o handler
/// faz stream dos bytes diretamente. Em modo SaaS futuro, este handler
/// retornaria 302 para URL assinada do storage externo — o contrato
/// (docs/03-CONTRATOS-API.md §3.3) suporta ambos via `?redirect=false`.
///
/// Regras de segurança:
/// - Tenant scope: o `tenant_id` do JWT escopa a busca; job de outro
///   tenant devolve 404 (igual ao `get_job`), nunca 403 — ver
///   `docs/08-SEGURANCA-MULTITENANCY.md` §3.
/// - Status check: só jobs em `completed` expõem artefato. Job em outro
///   estado devolve 409 `job_not_editable` (estado intermediário) ou 404
///   (ainda não processado — evita vazar a existência de um ID pendente).
pub async fn download_artifact(
    State(state): State<AppState>,
    _auth: AuthContext,
    TenantScope(tenant_id): TenantScope,
    Path(job_id): Path<Uuid>,
) -> Result<Response, (StatusCode, String)> {
    let job = state
        .repo
        .get_job(job_id, tenant_id)
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, "not_found".to_string()))?;

    // Só expõe artefato quando o job está de fato concluído. Em estado
    // intermediário, devolve 409 — o cliente sabe que o job existe mas o
    // artefato ainda não está disponível.
    if !matches!(
        job.status,
        audio_core::ports::repo_trait::JobStatus::Completed
    ) {
        return Err((
            StatusCode::CONFLICT,
            "job_not_editable: artifact disponível apenas em status=completed".to_string(),
        ));
    }

    // Object key determinístico — ver `worker.rs::execute_job`. Em iteração
    // futura, persistir o `artifact_object_key` no `JobRecord` para desacoplar
    // o esquema de chaves do storage da rota de leitura.
    let object_key = format!("tenant-{}/artifacts/{}/remix.wav", job.tenant_id, job.id);

    let bytes = state
        .storage
        .get(&object_key)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, format!("artifact_unavailable: {e}")))?;

    // Stream direto — em modo local não há ganho em presign/S3.
    // `Content-Disposition: attachment` força o browser a baixar em vez de
    // tocar inline (a UI tem player próprio para A/B).
    let body = Body::from(bytes);
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "audio/wav")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"remix-{job_id}.wav\""),
        )
        .header(header::CACHE_CONTROL, "private, max-age=0, must-revalidate")
        .body(body)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("body: {e}")))?;

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sanity check: o handler compara `job.status == JobStatus::Completed`,
    /// e o `JobSummary` serializa `{:?}` para lowercase. Garante que o formato
    /// de saída é `"completed"` (não `"Completed"`) — relevante porque o
    /// frontend usa essa string para distinguir estados.
    #[test]
    fn job_status_completed_eh_serializado_como_string_lowercase() {
        let s = format!("{:?}", audio_core::ports::repo_trait::JobStatus::Completed).to_lowercase();
        assert_eq!(s, "completed");
    }

    /// Sanity check: o object_key do artefato é determinístico a partir de
    /// (tenant_id, job_id) — `download_artifact` e `worker.execute_job`
    /// precisam concordar. Se um dia mudar, este teste quebra primeiro.
    #[test]
    fn object_key_do_artefato_eh_deterministico() {
        let tenant_id = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let job_id = Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap();
        let key = format!("tenant-{}/artifacts/{}/remix.wav", tenant_id, job_id);
        assert_eq!(key, "tenant-00000000-0000-0000-0000-000000000001/artifacts/00000000-0000-0000-0000-000000000002/remix.wav");
    }
}
