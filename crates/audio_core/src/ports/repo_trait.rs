use crate::domain::{AudioFingerprint, BeatBlock, PipelineConfig};
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

/// Metadados de um job que o `save_job` precisa persistir atomicamente
/// junto do registro (era o gap de integração fechado no Lote 2 do plano
/// Pareto: os adapters ignoravam mode/user_prompt/track_id, e o worker
/// nunca recebia o `track_id` — o pipeline falhava com
/// "no track_id/object_key associated with job" em todo job criado via
/// `POST /jobs`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct JobMeta {
    /// "manual" | "assisted"
    pub mode: Option<String>,
    pub user_prompt: Option<String>,
    pub track_id: Option<Uuid>,
}

#[async_trait::async_trait]
pub trait AudioRepo: Send + Sync {
    async fn save_job(
        &self,
        job_id: Uuid,
        tenant_id: Uuid,
        user_id: Uuid,
        config: &PipelineConfig,
        blocks: &[BeatBlock],
        meta: &JobMeta,
    ) -> Result<(), RepoError>;

    /// Cancelamento real (Lote 2, C6): transição validada + registro de
    /// auditoria, atômicos no adapter. Só cancela job em `Queued` ou
    /// `Processing` — estado terminal devolve `RepoError::InvalidState`
    /// (o handler traduz para 409 `job_not_editable`). Retorna o registro
    /// atualizado para o handler publicar o evento SSE.
    async fn cancel_job(&self, job_id: Uuid, tenant_id: Uuid) -> Result<JobRecord, RepoError>;
    async fn get_job(&self, job_id: Uuid, tenant_id: Uuid) -> Result<JobRecord, RepoError>;
    async fn list_jobs(&self, tenant_id: Uuid) -> Result<Vec<JobRecord>, RepoError>;
    async fn save_fingerprint(
        &self,
        job_id: Uuid,
        fingerprint: &AudioFingerprint,
    ) -> Result<(), RepoError>;
    async fn transition_job(
        &self,
        job_id: Uuid,
        new_status: JobStatus,
        audit_action: &str,
    ) -> Result<(), RepoError>;
    async fn list_audit_records(&self, job_id: Uuid) -> Result<Vec<AuditRecord>, RepoError>;
    async fn get_consent(&self, tenant_id: Uuid) -> Result<Option<ConsentRecord>, RepoError>;
    async fn save_consent(
        &self,
        tenant_id: Uuid,
        provider: String,
    ) -> Result<ConsentRecord, RepoError>;
    async fn claim_next_job(&self, worker_id: Uuid) -> Result<Option<JobRecord>, RepoError>;
    async fn heartbeat(&self, job_id: Uuid, worker_id: Uuid) -> Result<(), RepoError>;
    async fn fail_and_retry(&self, job_id: Uuid, max_attempts: u8) -> Result<(), RepoError>;

    // --- Tracks ---
    async fn save_track(&self, track: &TrackRecord) -> Result<(), RepoError>;
    async fn get_track(&self, track_id: Uuid, tenant_id: Uuid) -> Result<TrackRecord, RepoError>;
    async fn list_tracks(&self, tenant_id: Uuid) -> Result<Vec<TrackRecord>, RepoError>;

    // --- System (no tenant) ---
    async fn list_processing_jobs(&self) -> Result<Vec<JobRecord>, RepoError>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConsentRecord {
    pub tenant_id: Uuid,
    pub assisted_mode_accepted_at: DateTime<Utc>,
    pub provider_at_accept: String,
}

#[derive(Debug, Clone)]
pub struct AuditRecord {
    pub job_id: Uuid,
    pub action: String,
    pub new_status: JobStatus,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct JobRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub config: serde_json::Value,
    pub blocks: serde_json::Value,
    pub status: JobStatus,
    pub worker_id: Option<Uuid>,
    pub attempts: u8,
    pub last_heartbeat: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// "manual" | "assisted"
    pub mode: Option<String>,
    pub user_prompt: Option<String>,
    pub track_id: Option<Uuid>,
}

/// Status de uma faixa de áudio.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, Serialize)]
pub enum TrackStatus {
    Uploaded,
    Analyzing,
    Ready,
    Failed,
}

#[derive(Debug, Clone)]
pub struct TrackRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub project_id: Option<Uuid>,
    pub object_key: String,
    pub display_name: String,
    pub status: TrackStatus,
    pub duration_sec: Option<f64>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    pub sha256: Option<String>,
    pub analysis: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum JobMode {
    Manual,
    Assisted,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum JobStatus {
    Queued,
    Processing,
    Completed,
    Failed,
    Cancelled,
    RolledBack,
}

#[derive(Debug, thiserror::Error)]
pub enum RepoError {
    #[error("job not found: {0}")]
    NotFound(Uuid),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("storage backend error: {0}")]
    Backend(String),
    #[error("job already claimed by another worker: {0}")]
    AlreadyClaimed(Uuid),
    #[error("job {0} não aceita esta transição a partir do estado atual")]
    InvalidState(Uuid),
}
