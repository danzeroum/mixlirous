use crate::domain::{AudioFingerprint, BeatBlock, PipelineConfig};
use uuid::Uuid;

#[async_trait::async_trait]
pub trait AudioRepo: Send + Sync {
    async fn save_job(
        &self,
        job_id: Uuid,
        user_id: Uuid,
        config: &PipelineConfig,
        blocks: &[BeatBlock],
    ) -> Result<(), RepoError>;
    async fn get_job(&self, job_id: Uuid) -> Result<JobRecord, RepoError>;
    async fn list_jobs(&self, tenant_id: Uuid) -> Result<Vec<JobRecord>, RepoError>;
    async fn save_fingerprint(
        &self,
        job_id: Uuid,
        fingerprint: &AudioFingerprint,
    ) -> Result<(), RepoError>;
}

#[derive(Debug, Clone)]
pub struct JobRecord {
    pub id: Uuid,
    pub user_id: Uuid,
    pub config: serde_json::Value,
    pub blocks: serde_json::Value,
    pub status: JobStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum JobStatus {
    Pending,
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
}
