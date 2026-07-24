use crate::middleware::{AuthContext, TenantScope, TraceParent};
use crate::state::AppState;
use audio_core::PipelineConfig;
use axum::{extract::Path, extract::State, http::StatusCode, Json};
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
    // motor DSP/agente são Sprint 1+; ver docs/13-ROADMAP-SPRINTS.md).
    // tenant_id e user_id vêm das claims do JWT, nunca do corpo/query (ver
    // docs/08-SEGURANCA-MULTITENANCY.md §1) — e nunca um no lugar do outro.
    state
        .repo
        .save_job(job_id, claims.tenant_id, claims.sub, &config, &[])
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Propaga o traceparent recebido (W3C) quando houver; senão gera um novo
    // trace_id — ver docs/03-CONTRATOS-API.md §1 "Rastreamento".
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
    Path(job_id): Path<Uuid>,
) -> Result<Json<JobSummary>, (StatusCode, String)> {
    let job = state
        .repo
        .get_job(job_id)
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, "not_found".to_string()))?;

    Ok(Json(JobSummary {
        job_id: job.id,
        status: format!("{:?}", job.status).to_lowercase(),
        created_at: job.created_at,
        updated_at: job.updated_at,
    }))
}

pub async fn cancel_job(Path(job_id): Path<Uuid>) -> Json<serde_json::Value> {
    // Placeholder: cancelamento real precisa do estado de fila (Sprint 1+).
    Json(serde_json::json!({ "job_id": job_id, "status": "cancelled" }))
}
