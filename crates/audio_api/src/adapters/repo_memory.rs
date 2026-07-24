use audio_core::ports::repo_trait::{AudioRepo, JobRecord, JobStatus, RepoError};
use audio_core::{AudioFingerprint, BeatBlock, PipelineConfig};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Implementação em memória do `AudioRepo`, usada como padrão local/MVP
/// (ver ADR-0003). Uma implementação real sobre SQLite/Postgres é trabalho de
/// Sprint 1+; esta adapter mantém a Sprint 0 compilando e testável sem
/// depender de um banco de verdade.
#[derive(Default)]
pub struct InMemoryRepo {
    jobs: RwLock<HashMap<Uuid, JobRecord>>,
}

impl InMemoryRepo {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            jobs: RwLock::new(HashMap::new()),
        })
    }
}

#[async_trait::async_trait]
impl AudioRepo for InMemoryRepo {
    async fn save_job(
        &self,
        job_id: Uuid,
        user_id: Uuid,
        config: &PipelineConfig,
        blocks: &[BeatBlock],
    ) -> Result<(), RepoError> {
        let mut jobs = self.jobs.write().await;
        let record = JobRecord {
            id: job_id,
            user_id,
            config: serde_json::to_value(config)?,
            blocks: serde_json::to_value(blocks)?,
            status: JobStatus::Pending,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        jobs.insert(job_id, record);
        Ok(())
    }

    async fn get_job(&self, job_id: Uuid) -> Result<JobRecord, RepoError> {
        let jobs = self.jobs.read().await;
        jobs.get(&job_id)
            .cloned()
            .ok_or(RepoError::NotFound(job_id))
    }

    async fn list_jobs(&self, tenant_id: Uuid) -> Result<Vec<JobRecord>, RepoError> {
        let jobs = self.jobs.read().await;
        Ok(jobs
            .values()
            .filter(|r| r.user_id == tenant_id)
            .cloned()
            .collect())
    }

    async fn save_fingerprint(
        &self,
        _job_id: Uuid,
        _fingerprint: &AudioFingerprint,
    ) -> Result<(), RepoError> {
        // Placeholder: a Sprint 0 não persiste fingerprints; ver docs/09-MLOPS-GOLDEN-MASTER.md
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_save_then_get_job_roundtrip() {
        let repo = InMemoryRepo::new();
        let job_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        repo.save_job(job_id, user_id, &PipelineConfig::default(), &[])
            .await
            .unwrap();
        let job = repo.get_job(job_id).await.unwrap();

        assert_eq!(job.id, job_id);
        assert_eq!(job.user_id, user_id);
    }

    #[tokio::test]
    async fn test_get_unknown_job_returns_not_found() {
        let repo = InMemoryRepo::new();
        let err = repo.get_job(Uuid::new_v4()).await.unwrap_err();
        assert!(matches!(err, RepoError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_list_jobs_scopes_by_tenant() {
        let repo = InMemoryRepo::new();
        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();

        repo.save_job(Uuid::new_v4(), tenant_a, &PipelineConfig::default(), &[])
            .await
            .unwrap();
        repo.save_job(Uuid::new_v4(), tenant_b, &PipelineConfig::default(), &[])
            .await
            .unwrap();

        let jobs_a = repo.list_jobs(tenant_a).await.unwrap();
        assert_eq!(jobs_a.len(), 1);
        assert_eq!(jobs_a[0].user_id, tenant_a);
    }
}
