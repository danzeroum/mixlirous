use crate::domain::{AudioFingerprint, BeatBlock, PipelineConfig};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[async_trait::async_trait]
pub trait AudioRepo: Send + Sync {
    async fn save_job(&self, job_id: Uuid, tenant_id: Uuid, user_id: Uuid, config: &PipelineConfig, blocks: &[BeatBlock]) -> Result<(), RepoError>;
    async fn get_job(&self, job_id: Uuid, tenant_id: Uuid) -> Result<JobRecord, RepoError>;
    async fn list_jobs(&self, tenant_id: Uuid) -> Result<Vec<JobRecord>, RepoError>;
    async fn save_fingerprint(&self, job_id: Uuid, fingerprint: &AudioFingerprint) -> Result<(), RepoError>;
    async fn transition_job(&self, job_id: Uuid, new_status: JobStatus, audit_action: &str) -> Result<(), RepoError>;
    async fn list_audit_records(&self, job_id: Uuid) -> Result<Vec<AuditRecord>, RepoError>;
    async fn get_consent(&self, tenant_id: Uuid) -> Result<Option<ConsentRecord>, RepoError>;
    async fn save_consent(&self, tenant_id: Uuid, provider: String) -> Result<ConsentRecord, RepoError>;
    async fn claim_next_job(&self, worker_id: Uuid) -> Result<Option<JobRecord>, RepoError>;
    async fn heartbeat(&self, job_id: Uuid, worker_id: Uuid) -> Result<(), RepoError>;
    async fn fail_and_retry(&self, job_id: Uuid, max_attempts: u8) -> Result<(), RepoError>;
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
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum JobStatus {
    Queued,
    Processing,
    Completed,
    Failed,
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
}