use audio_core::ports::repo_trait::{
    AudioRepo, AuditRecord, ConsentRecord, JobRecord, JobStatus, RepoError,
};
use audio_core::{AudioFingerprint, BeatBlock, PipelineConfig};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Default)]
struct InMemoryState {
    jobs: HashMap<Uuid, JobRecord>,
    audit: Vec<AuditRecord>,
    consent: HashMap<Uuid, ConsentRecord>,
}

#[derive(Default)]
pub struct InMemoryRepo {
    state: RwLock<InMemoryState>,
}

impl InMemoryRepo {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            state: RwLock::new(InMemoryState::default()),
        })
    }
}

#[async_trait::async_trait]
impl AudioRepo for InMemoryRepo {
    async fn save_job(
        &self,
        job_id: Uuid,
        tenant_id: Uuid,
        user_id: Uuid,
        config: &PipelineConfig,
        blocks: &[BeatBlock],
    ) -> Result<(), RepoError> {
        let mut state = self.state.write().await;
        let record = JobRecord {
            id: job_id,
            tenant_id,
            user_id,
            config: serde_json::to_value(config)?,
            blocks: serde_json::to_value(blocks)?,
            status: JobStatus::Queued,
            worker_id: None,
            attempts: 0,
            last_heartbeat: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        state.jobs.insert(job_id, record);
        Ok(())
    }

    async fn get_job(&self, job_id: Uuid, tenant_id: Uuid) -> Result<JobRecord, RepoError> {
        let state = self.state.read().await;
        state
            .jobs
            .get(&job_id)
            .filter(|r| r.tenant_id == tenant_id)
            .cloned()
            .ok_or(RepoError::NotFound(job_id))
    }

    async fn list_jobs(&self, tenant_id: Uuid) -> Result<Vec<JobRecord>, RepoError> {
        let state = self.state.read().await;
        Ok(state
            .jobs
            .values()
            .filter(|r| r.tenant_id == tenant_id)
            .cloned()
            .collect())
    }

    async fn save_fingerprint(
        &self,
        _job_id: Uuid,
        _fingerprint: &AudioFingerprint,
    ) -> Result<(), RepoError> {
        Ok(())
    }

    async fn transition_job(
        &self,
        job_id: Uuid,
        new_status: JobStatus,
        audit_action: &str,
    ) -> Result<(), RepoError> {
        let mut state = self.state.write().await;
        let now = Utc::now();
        let job = state
            .jobs
            .get_mut(&job_id)
            .ok_or(RepoError::NotFound(job_id))?;
        job.status = new_status.clone();
        job.updated_at = now;
        state.audit.push(AuditRecord {
            job_id,
            action: audit_action.to_string(),
            new_status,
            occurred_at: now,
        });
        Ok(())
    }

    async fn list_audit_records(&self, job_id: Uuid) -> Result<Vec<AuditRecord>, RepoError> {
        let state = self.state.read().await;
        Ok(state
            .audit
            .iter()
            .filter(|r| r.job_id == job_id)
            .cloned()
            .collect())
    }

    async fn get_consent(&self, tenant_id: Uuid) -> Result<Option<ConsentRecord>, RepoError> {
        let state = self.state.read().await;
        Ok(state.consent.get(&tenant_id).cloned())
    }

    async fn save_consent(
        &self,
        tenant_id: Uuid,
        provider: String,
    ) -> Result<ConsentRecord, RepoError> {
        let mut state = self.state.write().await;
        let record = ConsentRecord {
            tenant_id,
            assisted_mode_accepted_at: Utc::now(),
            provider_at_accept: provider,
        };
        state.consent.insert(tenant_id, record.clone());
        Ok(record)
    }

    async fn claim_next_job(&self, worker_id: Uuid) -> Result<Option<JobRecord>, RepoError> {
        let mut state = self.state.write().await;
        let now = Utc::now();
        let job_id = state
            .jobs
            .iter()
            .filter(|(_, r)| r.status == JobStatus::Queued)
            .min_by_key(|(_, r)| r.created_at)
            .map(|(id, _)| *id);

        match job_id {
            Some(id) => {
                let job = state.jobs.get_mut(&id).unwrap();
                job.status = JobStatus::Processing;
                job.worker_id = Some(worker_id);
                job.last_heartbeat = Some(now);
                job.updated_at = now;
                let job_clone = job.clone();
                state.audit.push(AuditRecord {
                    job_id: id,
                    action: "JOB_CLAIMED".to_string(),
                    new_status: JobStatus::Processing,
                    occurred_at: now,
                });
                Ok(Some(job_clone))
            },
            None => Ok(None),
        }
    }

    async fn heartbeat(&self, job_id: Uuid, worker_id: Uuid) -> Result<(), RepoError> {
        let mut state = self.state.write().await;
        let now = Utc::now();
        let job = state
            .jobs
            .get_mut(&job_id)
            .ok_or(RepoError::NotFound(job_id))?;
        match job.worker_id {
            Some(wid) if wid == worker_id => {
                job.last_heartbeat = Some(now);
                job.updated_at = now;
                Ok(())
            },
            Some(_) => Err(RepoError::AlreadyClaimed(job_id)),
            None => Err(RepoError::NotFound(job_id)),
        }
    }

    async fn fail_and_retry(&self, job_id: Uuid, max_attempts: u8) -> Result<(), RepoError> {
        let mut state = self.state.write().await;
        let now = Utc::now();
        let job = state
            .jobs
            .get_mut(&job_id)
            .ok_or(RepoError::NotFound(job_id))?;
        job.attempts += 1;
        job.worker_id = None;
        job.updated_at = now;
        if job.attempts >= max_attempts {
            job.status = JobStatus::Failed;
            state.audit.push(AuditRecord {
                job_id,
                action: "JOB_FAILED".to_string(),
                new_status: JobStatus::Failed,
                occurred_at: now,
            });
        } else {
            job.status = JobStatus::Queued;
            state.audit.push(AuditRecord {
                job_id,
                action: "JOB_RETRY".to_string(),
                new_status: JobStatus::Queued,
                occurred_at: now,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_claim_next_job_returns_oldest_queued() {
        let repo = InMemoryRepo::new();
        let worker_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let job1 = Uuid::new_v4();
        let job2 = Uuid::new_v4();
        repo.save_job(
            job1,
            tenant_id,
            Uuid::new_v4(),
            &PipelineConfig::default(),
            &[],
        )
        .await
        .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        repo.save_job(
            job2,
            tenant_id,
            Uuid::new_v4(),
            &PipelineConfig::default(),
            &[],
        )
        .await
        .unwrap();
        let claimed = repo.claim_next_job(worker_id).await.unwrap();
        assert!(claimed.is_some());
        assert_eq!(claimed.unwrap().id, job1);
    }

    #[tokio::test]
    async fn test_claim_next_job_returns_none_when_empty() {
        let repo = InMemoryRepo::new();
        assert!(repo.claim_next_job(Uuid::new_v4()).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_heartbeat_updates_last_heartbeat() {
        let repo = InMemoryRepo::new();
        let worker_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let job_id = Uuid::new_v4();
        repo.save_job(
            job_id,
            tenant_id,
            Uuid::new_v4(),
            &PipelineConfig::default(),
            &[],
        )
        .await
        .unwrap();
        repo.claim_next_job(worker_id).await.unwrap();
        let hb_before = repo
            .get_job(job_id, tenant_id)
            .await
            .unwrap()
            .last_heartbeat
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        repo.heartbeat(job_id, worker_id).await.unwrap();
        let hb_after = repo
            .get_job(job_id, tenant_id)
            .await
            .unwrap()
            .last_heartbeat
            .unwrap();
        assert!(hb_after > hb_before);
    }

    #[tokio::test]
    async fn test_heartbeat_rejects_wrong_worker() {
        let repo = InMemoryRepo::new();
        let worker1 = Uuid::new_v4();
        let worker2 = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let job_id = Uuid::new_v4();
        repo.save_job(
            job_id,
            tenant_id,
            Uuid::new_v4(),
            &PipelineConfig::default(),
            &[],
        )
        .await
        .unwrap();
        repo.claim_next_job(worker1).await.unwrap();
        assert!(matches!(
            repo.heartbeat(job_id, worker2).await.unwrap_err(),
            RepoError::AlreadyClaimed(_)
        ));
    }

    #[tokio::test]
    async fn test_fail_and_retry_requeues() {
        let repo = InMemoryRepo::new();
        let worker_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let job_id = Uuid::new_v4();
        repo.save_job(
            job_id,
            tenant_id,
            Uuid::new_v4(),
            &PipelineConfig::default(),
            &[],
        )
        .await
        .unwrap();
        repo.claim_next_job(worker_id).await.unwrap();
        repo.fail_and_retry(job_id, 3).await.unwrap();
        let job = repo.get_job(job_id, tenant_id).await.unwrap();
        assert_eq!(job.status, JobStatus::Queued);
        assert_eq!(job.attempts, 1);
    }

    #[tokio::test]
    async fn test_fail_and_retry_fails_when_max_reached() {
        let repo = InMemoryRepo::new();
        let worker_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let job_id = Uuid::new_v4();
        repo.save_job(
            job_id,
            tenant_id,
            Uuid::new_v4(),
            &PipelineConfig::default(),
            &[],
        )
        .await
        .unwrap();
        repo.claim_next_job(worker_id).await.unwrap();
        repo.fail_and_retry(job_id, 1).await.unwrap();
        let job = repo.get_job(job_id, tenant_id).await.unwrap();
        assert_eq!(job.status, JobStatus::Failed);
    }

    #[tokio::test]
    async fn test_concurrent_claims_each_job_once() {
        let repo = InMemoryRepo::new();
        let tenant_id = Uuid::new_v4();
        for _ in 0..10 {
            repo.save_job(
                Uuid::new_v4(),
                tenant_id,
                Uuid::new_v4(),
                &PipelineConfig::default(),
                &[],
            )
            .await
            .unwrap();
        }
        let mut handles = vec![];
        for _ in 0..10 {
            let repo_clone = repo.clone();
            handles.push(tokio::spawn(async move {
                repo_clone.claim_next_job(Uuid::new_v4()).await
            }));
        }
        let mut claimed = 0;
        for h in handles {
            if h.await.unwrap().unwrap().is_some() {
                claimed += 1;
            }
        }
        assert_eq!(claimed, 10);
    }

    #[tokio::test]
    async fn test_claim_emits_audit_event() {
        let repo = InMemoryRepo::new();
        let worker_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let job_id = Uuid::new_v4();
        repo.save_job(
            job_id,
            tenant_id,
            Uuid::new_v4(),
            &PipelineConfig::default(),
            &[],
        )
        .await
        .unwrap();
        repo.claim_next_job(worker_id).await.unwrap();
        let audit = repo.list_audit_records(job_id).await.unwrap();
        assert_eq!(audit.len(), 1);
        assert_eq!(audit[0].action, "JOB_CLAIMED");
    }
}
