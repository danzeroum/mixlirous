use audio_core::ports::repo_trait::{
    AudioRepo, AuditRecord, ConsentRecord, JobRecord, JobStatus, RepoError,
};
use audio_core::{AudioFingerprint, BeatBlock, PipelineConfig};
use chrono::{DateTime, Utc};
use sqlx::{sqlite::SqlitePoolOptions, Pool, Row, Sqlite};
use uuid::Uuid;

/// SQLite adapter for AudioRepo.
/// Uses WAL mode, proper PRAGMAs, and atomic operations for queue safety.
pub struct SqliteRepo {
    pool: Pool<Sqlite>,
}

impl SqliteRepo {
    /// Create a new SqliteRepo from a database URL.
    /// Example: "sqlite:data/remix_ai.db?mode=rwc"
    pub async fn new(database_url: &str) -> Result<Self, RepoError> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await
            .map_err(|e| RepoError::Backend(e.to_string()))?;

        sqlx::query("PRAGMA journal_mode = WAL")
            .execute(&pool)
            .await
            .map_err(|e| RepoError::Backend(e.to_string()))?;
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .map_err(|e| RepoError::Backend(e.to_string()))?;
        sqlx::query("PRAGMA synchronous = NORMAL")
            .execute(&pool)
            .await
            .map_err(|e| RepoError::Backend(e.to_string()))?;
        sqlx::query("PRAGMA busy_timeout = 5000")
            .execute(&pool)
            .await
            .map_err(|e| RepoError::Backend(e.to_string()))?;

        sqlx::query(std::include_str!("migrations/001_initial.sql"))
            .execute(&pool)
            .await
            .map_err(|e| RepoError::Backend(e.to_string()))?;

        Ok(Self { pool })
    }

    fn parse_job_row(row: sqlx::sqlite::SqliteRow) -> Result<JobRecord, RepoError> {
        let status_str: String = row.try_get("status").map_err(|e| RepoError::Backend(e.to_string()))?;
        let status = match status_str.as_str() {
            "Queued" => JobStatus::Queued,
            "Processing" => JobStatus::Processing,
            "Completed" => JobStatus::Completed,
            "Failed" => JobStatus::Failed,
            "RolledBack" => JobStatus::RolledBack,
            _ => return Err(RepoError::Backend(format!("Unknown status: {status_str}"))),
        };

        let worker_id: Option<String> = row.try_get("worker_id").ok();
        let last_heartbeat: Option<String> = row.try_get("last_heartbeat").ok();

        Ok(JobRecord {
            id: row.try_get::<Uuid, _>("id").map_err(|e| RepoError::Backend(e.to_string()))?,
            tenant_id: row.try_get::<Uuid, _>("tenant_id").map_err(|e| RepoError::Backend(e.to_string()))?,
            user_id: row.try_get::<Uuid, _>("user_id").map_err(|e| RepoError::Backend(e.to_string()))?,
            config: row.try_get::<serde_json::Value, _>("config").map_err(|e| RepoError::Backend(e.to_string()))?,
            blocks: row.try_get::<serde_json::Value, _>("blocks").map_err(|e| RepoError::Backend(e.to_string()))?,
            status,
            worker_id: worker_id.and_then(|s| Uuid::parse_str(&s).ok()),
            attempts: row.try_get::<i64, _>("attempts").map_err(|e| RepoError::Backend(e.to_string()))? as u8,
            last_heartbeat: last_heartbeat.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc))),
            created_at: DateTime::parse_from_rfc3339(&row.try_get::<String, _>("created_at").map_err(|e| RepoError::Backend(e.to_string()))?).map_err(|e| RepoError::Backend(e.to_string()))?.with_timezone(&Utc),
            updated_at: DateTime::parse_from_rfc3339(&row.try_get::<String, _>("updated_at").map_err(|e| RepoError::Backend(e.to_string()))?).map_err(|e| RepoError::Backend(e.to_string()))?.with_timezone(&Utc),
        })
    }
}

#[async_trait::async_trait]
impl AudioRepo for SqliteRepo {
    async fn save_job(&self, job_id: Uuid, tenant_id: Uuid, user_id: Uuid, config: &PipelineConfig, blocks: &[BeatBlock]) -> Result<(), RepoError> {
        let config_json = serde_json::to_value(config)?;
        let blocks_json = serde_json::to_value(blocks)?;
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO jobs (id, tenant_id, user_id, config, blocks, status, worker_id, attempts, last_heartbeat, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'Queued', NULL, 0, NULL, ?6, ?6)"
        )
        .bind(job_id.to_string()).bind(tenant_id.to_string()).bind(user_id.to_string())
        .bind(config_json.to_string()).bind(blocks_json.to_string()).bind(&now)
        .execute(&self.pool).await.map_err(|e| RepoError::Backend(e.to_string()))?;
        Ok(())
    }

    async fn get_job(&self, job_id: Uuid, tenant_id: Uuid) -> Result<JobRecord, RepoError> {
        let row = sqlx::query("SELECT * FROM jobs WHERE id = ?1 AND tenant_id = ?2")
            .bind(job_id.to_string()).bind(tenant_id.to_string())
            .fetch_optional(&self.pool).await.map_err(|e| RepoError::Backend(e.to_string()))?;
        match row {
            Some(r) => Self::parse_job_row(r),
            None => Err(RepoError::NotFound(job_id)),
        }
    }

    async fn list_jobs(&self, tenant_id: Uuid) -> Result<Vec<JobRecord>, RepoError> {
        let rows = sqlx::query("SELECT * FROM jobs WHERE tenant_id = ?1 ORDER BY created_at")
            .bind(tenant_id.to_string()).fetch_all(&self.pool).await.map_err(|e| RepoError::Backend(e.to_string()))?;
        rows.into_iter().map(Self::parse_job_row).collect()
    }

    async fn save_fingerprint(&self, _job_id: Uuid, _fingerprint: &AudioFingerprint) -> Result<(), RepoError> { Ok(()) }

    async fn transition_job(&self, job_id: Uuid, new_status: JobStatus, audit_action: &str) -> Result<(), RepoError> {
        let mut tx = self.pool.begin().await.map_err(|e| RepoError::Backend(e.to_string()))?;
        let status_str = format!("{:?}", new_status);
        let now = Utc::now().to_rfc3339();

        let result = sqlx::query("UPDATE jobs SET status = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(&status_str).bind(&now).bind(job_id.to_string())
            .execute(&mut *tx).await.map_err(|e| RepoError::Backend(e.to_string()))?;

        if result.rows_affected() == 0 {
            tx.rollback().await.map_err(|e| RepoError::Backend(e.to_string()))?;
            return Err(RepoError::NotFound(job_id));
        }

        sqlx::query("INSERT INTO audit_records (job_id, action, new_status, occurred_at) VALUES (?1, ?2, ?3, ?4)")
            .bind(job_id.to_string()).bind(audit_action).bind(&status_str).bind(&now)
            .execute(&mut *tx).await.map_err(|e| RepoError::Backend(e.to_string()))?;

        tx.commit().await.map_err(|e| RepoError::Backend(e.to_string()))?;
        Ok(())
    }

    async fn list_audit_records(&self, job_id: Uuid) -> Result<Vec<AuditRecord>, RepoError> {
        let rows = sqlx::query("SELECT * FROM audit_records WHERE job_id = ?1 ORDER BY occurred_at")
            .bind(job_id.to_string()).fetch_all(&self.pool).await.map_err(|e| RepoError::Backend(e.to_string()))?;
        rows.into_iter()
            .map(|row| {
                let status_str: String = row.try_get("new_status").map_err(|e| RepoError::Backend(e.to_string()))?;
                let status = match status_str.as_str() {
                    "Queued" => JobStatus::Queued,
                    "Processing" => JobStatus::Processing,
                    "Completed" => JobStatus::Completed,
                    "Failed" => JobStatus::Failed,
                    "RolledBack" => JobStatus::RolledBack,
                    _ => return Err(RepoError::Backend(format!("Unknown status: {status_str}"))),
                };
                Ok(AuditRecord {
                    job_id: row.try_get::<Uuid, _>("job_id").map_err(|e| RepoError::Backend(e.to_string()))?,
                    action: row.try_get("action").map_err(|e| RepoError::Backend(e.to_string()))?,
                    new_status: status,
                    occurred_at: DateTime::parse_from_rfc3339(&row.try_get::<String, _>("occurred_at").map_err(|e| RepoError::Backend(e.to_string()))?).map_err(|e| RepoError::Backend(e.to_string()))?.with_timezone(&Utc),
                })
            })
            .collect()
    }

    async fn get_consent(&self, tenant_id: Uuid) -> Result<Option<ConsentRecord>, RepoError> {
        let row = sqlx::query("SELECT * FROM consent_records WHERE tenant_id = ?1")
            .bind(tenant_id.to_string()).fetch_optional(&self.pool).await.map_err(|e| RepoError::Backend(e.to_string()))?;
        match row {
            Some(r) => Ok(Some(ConsentRecord {
                tenant_id: r.try_get::<Uuid, _>("tenant_id").map_err(|e| RepoError::Backend(e.to_string()))?,
                assisted_mode_accepted_at: DateTime::parse_from_rfc3339(&r.try_get::<String, _>("assisted_mode_accepted_at").map_err(|e| RepoError::Backend(e.to_string()))?).map_err(|e| RepoError::Backend(e.to_string()))?.with_timezone(&Utc),
                provider_at_accept: r.try_get("provider_at_accept").map_err(|e| RepoError::Backend(e.to_string()))?,
            })),
            None => Ok(None),
        }
    }

    async fn save_consent(&self, tenant_id: Uuid, provider: String) -> Result<ConsentRecord, RepoError> {
        let now = Utc::now();
        let now_str = now.to_rfc3339();

        sqlx::query(
            "INSERT INTO consent_records (tenant_id, assisted_mode_accepted_at, provider_at_accept)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(tenant_id) DO UPDATE SET
                assisted_mode_accepted_at = excluded.assisted_mode_accepted_at,
                provider_at_accept = excluded.provider_at_accept"
        )
        .bind(tenant_id.to_string()).bind(&now_str).bind(&provider)
        .execute(&self.pool).await.map_err(|e| RepoError::Backend(e.to_string()))?;

        Ok(ConsentRecord { tenant_id, assisted_mode_accepted_at: now, provider_at_accept: provider })
    }

    async fn claim_next_job(&self, worker_id: Uuid) -> Result<Option<JobRecord>, RepoError> {
        let mut tx = self.pool.begin().await.map_err(|e| RepoError::Backend(e.to_string()))?;

        let row = sqlx::query(
            "UPDATE jobs SET status = 'Processing', worker_id = ?1, last_heartbeat = ?2, updated_at = ?2
             WHERE id = (SELECT id FROM jobs WHERE status = 'Queued' ORDER BY created_at ASC LIMIT 1)
             RETURNING *"
        )
        .bind(worker_id.to_string()).bind(Utc::now().to_rfc3339())
        .fetch_optional(&mut *tx).await.map_err(|e| RepoError::Backend(e.to_string()))?;

        match row {
            Some(r) => {
                let job_id: Uuid = r.try_get::<Uuid, _>("id").map_err(|e| RepoError::Backend(e.to_string()))?;
                sqlx::query("INSERT INTO audit_records (job_id, action, new_status, occurred_at) VALUES (?1, 'JOB_CLAIMED', 'Processing', ?2)")
                    .bind(job_id.to_string()).bind(Utc::now().to_rfc3339())
                    .execute(&mut *tx).await.map_err(|e| RepoError::Backend(e.to_string()))?;
                tx.commit().await.map_err(|e| RepoError::Backend(e.to_string()))?;
                Self::parse_job_row(r).map(Some)
            }
            None => {
                tx.rollback().await.map_err(|e| RepoError::Backend(e.to_string()))?;
                Ok(None)
            }
        }
    }

    async fn heartbeat(&self, job_id: Uuid, worker_id: Uuid) -> Result<(), RepoError> {
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query("UPDATE jobs SET last_heartbeat = ?1, updated_at = ?1 WHERE id = ?2 AND worker_id = ?3 AND status = 'Processing'")
            .bind(&now).bind(job_id.to_string()).bind(worker_id.to_string())
            .execute(&self.pool).await.map_err(|e| RepoError::Backend(e.to_string()))?;

        if result.rows_affected() == 0 {
            let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM jobs WHERE id = ?1)")
                .bind(job_id.to_string()).fetch_one(&self.pool).await.map_err(|e| RepoError::Backend(e.to_string()))?;
            if exists { Err(RepoError::AlreadyClaimed(job_id)) } else { Err(RepoError::NotFound(job_id)) }
        } else {
            Ok(())
        }
    }

    async fn fail_and_retry(&self, job_id: Uuid, max_attempts: u8) -> Result<(), RepoError> {
        let mut tx = self.pool.begin().await.map_err(|e| RepoError::Backend(e.to_string()))?;
        let row = sqlx::query("SELECT attempts FROM jobs WHERE id = ?1")
            .bind(job_id.to_string()).fetch_optional(&mut *tx).await.map_err(|e| RepoError::Backend(e.to_string()))?;

        let attempts: i64 = match row {
            Some(r) => r.try_get("attempts").map_err(|e| RepoError::Backend(e.to_string()))?,
            None => { tx.rollback().await.map_err(|e| RepoError::Backend(e.to_string()))?; return Err(RepoError::NotFound(job_id)); }
        };

        let new_attempts = attempts as u8 + 1;
        let now = Utc::now().to_rfc3339();

        if new_attempts >= max_attempts {
            sqlx::query("UPDATE jobs SET status = 'Failed', attempts = ?1, worker_id = NULL, updated_at = ?2 WHERE id = ?3")
                .bind(new_attempts as i64).bind(&now).bind(job_id.to_string())
                .execute(&mut *tx).await.map_err(|e| RepoError::Backend(e.to_string()))?;
            sqlx::query("INSERT INTO audit_records (job_id, action, new_status, occurred_at) VALUES (?1, 'JOB_FAILED', 'Failed', ?2)")
                .bind(job_id.to_string()).bind(&now).execute(&mut *tx).await.map_err(|e| RepoError::Backend(e.to_string()))?;
        } else {
            sqlx::query("UPDATE jobs SET status = 'Queued', attempts = ?1, worker_id = NULL, updated_at = ?2 WHERE id = ?3")
                .bind(new_attempts as i64).bind(&now).bind(job_id.to_string())
                .execute(&mut *tx).await.map_err(|e| RepoError::Backend(e.to_string()))?;
            sqlx::query("INSERT INTO audit_records (job_id, action, new_status, occurred_at) VALUES (?1, 'JOB_RETRY', 'Queued', ?2)")
                .bind(job_id.to_string()).bind(&now).execute(&mut *tx).await.map_err(|e| RepoError::Backend(e.to_string()))?;
        }

        tx.commit().await.map_err(|e| RepoError::Backend(e.to_string()))?;
        Ok(())
    }
}


#[cfg(test)]
mod sqlite_tests {
    use super::*;
    use audio_core::PipelineConfig;

    async fn setup_repo() -> SqliteRepo {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = format!("file:memdb_{id}?mode=memory&cache=shared");
        SqliteRepo::new(&path).await.unwrap()
    }

    #[tokio::test]
    async fn test_sqlite_save_and_get_job() {
        let repo = setup_repo().await;
        let job_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        repo.save_job(job_id, tenant_id, user_id, &PipelineConfig::default(), &[]).await.unwrap();
        let job = repo.get_job(job_id, tenant_id).await.unwrap();
        assert_eq!(job.id, job_id);
        assert_eq!(job.status, JobStatus::Queued);
    }

    #[tokio::test]
    async fn test_sqlite_claim_next_job() {
        let repo = setup_repo().await;
        let worker_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let job1 = Uuid::new_v4();
        let job2 = Uuid::new_v4();
        repo.save_job(job1, tenant_id, Uuid::new_v4(), &PipelineConfig::default(), &[]).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        repo.save_job(job2, tenant_id, Uuid::new_v4(), &PipelineConfig::default(), &[]).await.unwrap();
        let claimed = repo.claim_next_job(worker_id).await.unwrap();
        assert!(claimed.is_some());
        assert_eq!(claimed.unwrap().id, job1);
    }

    #[tokio::test]
    async fn test_sqlite_concurrent_claims() {
        let repo = std::sync::Arc::new(setup_repo().await);
        let tenant_id = Uuid::new_v4();
        for _ in 0..5 { repo.save_job(Uuid::new_v4(), tenant_id, Uuid::new_v4(), &PipelineConfig::default(), &[]).await.unwrap(); }
        let mut handles = vec![];
        for _ in 0..5 { let repo_clone = repo.clone(); handles.push(tokio::spawn(async move { repo_clone.claim_next_job(Uuid::new_v4()).await })); }
        let mut claimed = 0;
        for h in handles { if h.await.unwrap().unwrap().is_some() { claimed += 1; } }
        assert_eq!(claimed, 5);
    }

    #[tokio::test]
    async fn test_sqlite_heartbeat_and_fail_retry() {
        let repo = setup_repo().await;
        let worker_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let job_id = Uuid::new_v4();
        repo.save_job(job_id, tenant_id, Uuid::new_v4(), &PipelineConfig::default(), &[]).await.unwrap();
        repo.claim_next_job(worker_id).await.unwrap();
        repo.heartbeat(job_id, worker_id).await.unwrap();
        repo.fail_and_retry(job_id, 3).await.unwrap();
        let job = repo.get_job(job_id, tenant_id).await.unwrap();
        assert_eq!(job.status, JobStatus::Queued);
        assert_eq!(job.attempts, 1);
    }
}