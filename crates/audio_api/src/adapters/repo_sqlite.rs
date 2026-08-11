use audio_core::ports::repo_trait::{
    AudioRepo, AuditRecord, ConsentRecord, JobRecord, JobStatus, RepoError, TrackRecord,
    TrackStatus,
};
use audio_core::{AudioFingerprint, BeatBlock, PipelineConfig};
use chrono::{DateTime, Utc};
use sqlx::{sqlite::SqlitePoolOptions, Pool, Row, Sqlite};
use uuid::Uuid;

/// Helper: read a TEXT column as String then parse to Uuid.
/// SQLite stores UUIDs as TEXT (36 chars); sqlx would try BLOB decode otherwise.
fn uuid_from_row(row: &sqlx::sqlite::SqliteRow, col: &str) -> Result<Uuid, RepoError> {
    let s: String = row
        .try_get(col)
        .map_err(|e| RepoError::Backend(format!("parse {col}: {e}")))?;
    Uuid::parse_str(&s).map_err(|e| RepoError::Backend(format!("parse {col} uuid: {e}")))
}

fn uuid_opt_from_row(row: &sqlx::sqlite::SqliteRow, col: &str) -> Option<Uuid> {
    row.try_get::<String, _>(col)
        .ok()
        .and_then(|s| Uuid::parse_str(&s).ok())
}

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
            .map_err(|e| RepoError::Backend(format!("connect: {e}")))?;

        sqlx::query("PRAGMA journal_mode = WAL")
            .execute(&pool)
            .await
            .map_err(|e| RepoError::Backend(format!("pragma wal: {e}")))?;
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .map_err(|e| RepoError::Backend(format!("pragma fk: {e}")))?;
        sqlx::query("PRAGMA synchronous = NORMAL")
            .execute(&pool)
            .await
            .map_err(|e| RepoError::Backend(format!("pragma sync: {e}")))?;
        sqlx::query("PRAGMA busy_timeout = 5000")
            .execute(&pool)
            .await
            .map_err(|e| RepoError::Backend(format!("pragma busy: {e}")))?;

        sqlx::query(std::include_str!("migrations/001_initial.sql"))
            .execute(&pool)
            .await
            .map_err(|e| RepoError::Backend(format!("migration 001: {e}")))?;

        // 002_tracks: ALTER TABLE may fail if columns already exist (idempotent).
        let migration_002 = std::include_str!("migrations/002_tracks.sql");
        for stmt in migration_002.split(';') {
            let trimmed = stmt.trim();
            if trimmed.is_empty() {
                continue;
            }
            // ALTER TABLE ADD COLUMN fails if column exists — ignore.
            if trimmed.starts_with("ALTER TABLE") {
                if sqlx::query(trimmed).execute(&pool).await.is_err() {
                    // Column already exists — safe to continue.
                }
            } else {
                sqlx::query(trimmed)
                    .execute(&pool)
                    .await
                    .map_err(|e| RepoError::Backend(format!("migration 002: {e}")))?;
            }
        }

        Ok(Self { pool })
    }

    fn parse_job_row(row: sqlx::sqlite::SqliteRow) -> Result<JobRecord, RepoError> {
        let status_str: String = row
            .try_get("status")
            .map_err(|e| RepoError::Backend(format!("parse status: {e}")))?;
        let status = match status_str.as_str() {
            "Queued" => JobStatus::Queued,
            "Processing" => JobStatus::Processing,
            "Completed" => JobStatus::Completed,
            "Failed" => JobStatus::Failed,
            "RolledBack" => JobStatus::RolledBack,
            _ => return Err(RepoError::Backend(format!("unknown status: {status_str}"))),
        };

        let worker_id: Option<String> = row.try_get("worker_id").ok();
        let last_heartbeat: Option<String> = row.try_get("last_heartbeat").ok();
        let mode: Option<String> = row.try_get("mode").ok().filter(|v: &String| !v.is_empty());
        let user_prompt: Option<String> = row
            .try_get("user_prompt")
            .ok()
            .filter(|v: &String| !v.is_empty());
        let track_id_str: Option<String> = row.try_get("track_id").ok();
        let track_id = track_id_str.and_then(|s| Uuid::parse_str(&s).ok());

        Ok(JobRecord {
            id: uuid_from_row(&row, "id")?,
            tenant_id: uuid_from_row(&row, "tenant_id")?,
            user_id: uuid_from_row(&row, "user_id")?,
            config: row
                .try_get::<serde_json::Value, _>("config")
                .map_err(|e| RepoError::Backend(format!("parse config: {e}")))?,
            blocks: row
                .try_get::<serde_json::Value, _>("blocks")
                .map_err(|e| RepoError::Backend(format!("parse blocks: {e}")))?,
            status,
            worker_id: worker_id.and_then(|s| Uuid::parse_str(&s).ok()),
            attempts: row
                .try_get::<i64, _>("attempts")
                .map_err(|e| RepoError::Backend(format!("parse attempts: {e}")))?
                as u8,
            last_heartbeat: last_heartbeat
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            created_at: DateTime::parse_from_rfc3339(
                &row.try_get::<String, _>("created_at")
                    .map_err(|e| RepoError::Backend(format!("parse created_at: {e}")))?,
            )
            .map_err(|e| RepoError::Backend(format!("parse created_at dt: {e}")))?
            .with_timezone(&Utc),
            updated_at: DateTime::parse_from_rfc3339(
                &row.try_get::<String, _>("updated_at")
                    .map_err(|e| RepoError::Backend(format!("parse updated_at: {e}")))?,
            )
            .map_err(|e| RepoError::Backend(format!("parse updated_at dt: {e}")))?
            .with_timezone(&Utc),
            mode,
            user_prompt,
            track_id,
        })
    }

    fn parse_track_row(row: sqlx::sqlite::SqliteRow) -> Result<TrackRecord, RepoError> {
        let status_str: String = row
            .try_get("status")
            .map_err(|e| RepoError::Backend(format!("parse track status: {e}")))?;
        let status = match status_str.as_str() {
            "Uploaded" => TrackStatus::Uploaded,
            "Analyzing" => TrackStatus::Analyzing,
            "Ready" => TrackStatus::Ready,
            "Failed" => TrackStatus::Failed,
            _ => {
                return Err(RepoError::Backend(format!(
                    "unknown track status: {status_str}"
                )))
            },
        };

        let analysis_str: Option<String> = row.try_get("analysis").ok();
        let analysis = analysis_str.and_then(|s| serde_json::from_str(&s).ok());

        Ok(TrackRecord {
            id: uuid_from_row(&row, "id")?,
            tenant_id: uuid_from_row(&row, "tenant_id")?,
            project_id: uuid_opt_from_row(&row, "project_id"),
            object_key: row
                .try_get("object_key")
                .map_err(|e| RepoError::Backend(format!("parse track object_key: {e}")))?,
            display_name: row
                .try_get("display_name")
                .map_err(|e| RepoError::Backend(format!("parse track display_name: {e}")))?,
            status,
            duration_sec: row.try_get("duration_sec").ok(),
            sample_rate: row.try_get("sample_rate").ok(),
            channels: row.try_get("channels").ok(),
            sha256: row.try_get("sha256").ok(),
            analysis,
            created_at: DateTime::parse_from_rfc3339(
                &row.try_get::<String, _>("created_at")
                    .map_err(|e| RepoError::Backend(format!("parse track created_at: {e}")))?,
            )
            .map_err(|e| RepoError::Backend(format!("parse track created_at dt: {e}")))?
            .with_timezone(&Utc),
            updated_at: DateTime::parse_from_rfc3339(
                &row.try_get::<String, _>("updated_at")
                    .map_err(|e| RepoError::Backend(format!("parse track updated_at: {e}")))?,
            )
            .map_err(|e| RepoError::Backend(format!("parse track updated_at dt: {e}")))?
            .with_timezone(&Utc),
        })
    }
}

#[async_trait::async_trait]
impl AudioRepo for SqliteRepo {
    async fn save_job(
        &self,
        job_id: Uuid,
        tenant_id: Uuid,
        user_id: Uuid,
        config: &PipelineConfig,
        blocks: &[BeatBlock],
    ) -> Result<(), RepoError> {
        let config_json = serde_json::to_value(config)?;
        let blocks_json = serde_json::to_value(blocks)?;
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO jobs (id, tenant_id, user_id, config, blocks, status, worker_id, attempts, last_heartbeat, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'Queued', NULL, 0, NULL, ?6, ?6)",
        )
        .bind(job_id.to_string())
        .bind(tenant_id.to_string())
        .bind(user_id.to_string())
        .bind(config_json.to_string())
        .bind(blocks_json.to_string())
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|e| RepoError::Backend(format!("save_job: {e}")))?;
        Ok(())
    }

    async fn get_job(&self, job_id: Uuid, tenant_id: Uuid) -> Result<JobRecord, RepoError> {
        let row = sqlx::query("SELECT * FROM jobs WHERE id = ?1 AND tenant_id = ?2")
            .bind(job_id.to_string())
            .bind(tenant_id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| RepoError::Backend(format!("get_job: {e}")));
        match row {
            Ok(Some(r)) => Self::parse_job_row(r),
            Ok(None) => Err(RepoError::NotFound(job_id)),
            Err(e) => Err(e),
        }
    }

    async fn list_jobs(&self, tenant_id: Uuid) -> Result<Vec<JobRecord>, RepoError> {
        let rows = sqlx::query("SELECT * FROM jobs WHERE tenant_id = ?1 ORDER BY created_at")
            .bind(tenant_id.to_string())
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepoError::Backend(format!("list_jobs: {e}")));
        rows.and_then(|rows| rows.into_iter().map(Self::parse_job_row).collect())
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
        let tx = self
            .pool
            .begin()
            .await
            .map_err(|e| RepoError::Backend(format!("tx begin: {e}")));
        let mut tx = match tx {
            Ok(t) => t,
            Err(e) => return Err(e),
        };
        let status_str = format!("{new_status:?}");
        let now = Utc::now().to_rfc3339();

        let result = sqlx::query("UPDATE jobs SET status = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(&status_str)
            .bind(&now)
            .bind(job_id.to_string())
            .execute(&mut *tx)
            .await
            .map_err(|e| RepoError::Backend(format!("transition update: {e}")));
        let result = match result {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.rollback().await;
                return Err(e);
            },
        };

        if result.rows_affected() == 0 {
            let _ = tx.rollback().await;
            return Err(RepoError::NotFound(job_id));
        }

        if let Err(e) = sqlx::query("INSERT INTO audit_records (job_id, action, new_status, occurred_at) VALUES (?1, ?2, ?3, ?4)")
            .bind(job_id.to_string())
            .bind(audit_action)
            .bind(&status_str)
            .bind(&now)
            .execute(&mut *tx)
            .await
            .map_err(|e| RepoError::Backend(format!("audit insert: {e}")))
        {
            let _ = tx.rollback().await;
            return Err(e);
        }

        tx.commit()
            .await
            .map_err(|e| RepoError::Backend(format!("tx commit: {e}")))?;
        Ok(())
    }

    async fn list_audit_records(&self, job_id: Uuid) -> Result<Vec<AuditRecord>, RepoError> {
        let rows =
            sqlx::query("SELECT * FROM audit_records WHERE job_id = ?1 ORDER BY occurred_at")
                .bind(job_id.to_string())
                .fetch_all(&self.pool)
                .await
                .map_err(|e| RepoError::Backend(format!("list_audit: {e}")));
        rows.and_then(|rows| {
            rows.into_iter()
                .map(|row| {
                    let status_str: String = row
                        .try_get("new_status")
                        .map_err(|e| RepoError::Backend(format!("parse audit status: {e}")))?;
                    let status = match status_str.as_str() {
                        "Queued" => JobStatus::Queued,
                        "Processing" => JobStatus::Processing,
                        "Completed" => JobStatus::Completed,
                        "Failed" => JobStatus::Failed,
                        "RolledBack" => JobStatus::RolledBack,
                        _ => {
                            return Err(RepoError::Backend(format!(
                                "unknown audit status: {status_str}"
                            )))
                        },
                    };
                    Ok(AuditRecord {
                        job_id: uuid_from_row(&row, "job_id")?,
                        action: row
                            .try_get("action")
                            .map_err(|e| RepoError::Backend(format!("parse audit action: {e}")))?,
                        new_status: status,
                        occurred_at: DateTime::parse_from_rfc3339(
                            &row.try_get::<String, _>("occurred_at").map_err(|e| {
                                RepoError::Backend(format!("parse audit occurred_at: {e}"))
                            })?,
                        )
                        .map_err(|e| {
                            RepoError::Backend(format!("parse audit occurred_at dt: {e}"))
                        })?
                        .with_timezone(&Utc),
                    })
                })
                .collect()
        })
    }

    async fn get_consent(&self, tenant_id: Uuid) -> Result<Option<ConsentRecord>, RepoError> {
        let row = sqlx::query("SELECT * FROM consent_records WHERE tenant_id = ?1")
            .bind(tenant_id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| RepoError::Backend(format!("get_consent: {e}")));
        match row {
            Ok(Some(r)) => Ok(Some(ConsentRecord {
                tenant_id: uuid_from_row(&r, "tenant_id")?,
                assisted_mode_accepted_at: DateTime::parse_from_rfc3339(
                    &r.try_get::<String, _>("assisted_mode_accepted_at")
                        .map_err(|e| RepoError::Backend(format!("parse consent at: {e}")))?,
                )
                .map_err(|e| RepoError::Backend(format!("parse consent at dt: {e}")))?
                .with_timezone(&Utc),
                provider_at_accept: r
                    .try_get("provider_at_accept")
                    .map_err(|e| RepoError::Backend(format!("parse consent provider: {e}")))?,
            })),
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn save_consent(
        &self,
        tenant_id: Uuid,
        provider: String,
    ) -> Result<ConsentRecord, RepoError> {
        let now = Utc::now();
        let now_str = now.to_rfc3339();

        sqlx::query(
            "INSERT INTO consent_records (tenant_id, assisted_mode_accepted_at, provider_at_accept)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(tenant_id) DO UPDATE SET
                assisted_mode_accepted_at = excluded.assisted_mode_accepted_at,
                provider_at_accept = excluded.provider_at_accept",
        )
        .bind(tenant_id.to_string())
        .bind(&now_str)
        .bind(&provider)
        .execute(&self.pool)
        .await
        .map_err(|e| RepoError::Backend(format!("save_consent: {e}")))?;

        Ok(ConsentRecord {
            tenant_id,
            assisted_mode_accepted_at: now,
            provider_at_accept: provider,
        })
    }

    async fn claim_next_job(&self, worker_id: Uuid) -> Result<Option<JobRecord>, RepoError> {
        let tx = self
            .pool
            .begin()
            .await
            .map_err(|e| RepoError::Backend(format!("claim tx: {e}")));
        let mut tx = match tx {
            Ok(t) => t,
            Err(e) => return Err(e),
        };

        let row = sqlx::query(
            "UPDATE jobs SET status = 'Processing', worker_id = ?1, last_heartbeat = ?2, updated_at = ?2
             WHERE id = (SELECT id FROM jobs WHERE status = 'Queued' ORDER BY created_at ASC LIMIT 1)
             RETURNING *",
        )
        .bind(worker_id.to_string())
        .bind(Utc::now().to_rfc3339())
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| RepoError::Backend(format!("claim update: {e}")));
        let row = match row {
            Ok(Some(r)) => r,
            Ok(None) => {
                let _ = tx.rollback().await;
                return Ok(None);
            },
            Err(e) => {
                let _ = tx.rollback().await;
                return Err(e);
            },
        };

        let job_id = uuid_from_row(&row, "id")?;
        if let Err(e) = sqlx::query("INSERT INTO audit_records (job_id, action, new_status, occurred_at) VALUES (?1, 'JOB_CLAIMED', 'Processing', ?2)")
            .bind(job_id.to_string())
            .bind(Utc::now().to_rfc3339())
            .execute(&mut *tx)
            .await
            .map_err(|e| RepoError::Backend(format!("claim audit: {e}")))
        {
            let _ = tx.rollback().await;
            return Err(e);
        }
        tx.commit()
            .await
            .map_err(|e| RepoError::Backend(format!("claim commit: {e}")))?;
        Self::parse_job_row(row).map(Some)
    }

    async fn heartbeat(&self, job_id: Uuid, worker_id: Uuid) -> Result<(), RepoError> {
        let now = Utc::now().to_rfc3339();
        let result =
            sqlx::query("UPDATE jobs SET last_heartbeat = ?1, updated_at = ?1 WHERE id = ?2 AND worker_id = ?3 AND status = 'Processing'")
                .bind(&now)
                .bind(job_id.to_string())
                .bind(worker_id.to_string())
                .execute(&self.pool)
                .await
                .map_err(|e| RepoError::Backend(format!("heartbeat: {e}")));

        match result {
            Ok(r) if r.rows_affected() > 0 => Ok(()),
            Ok(_) => {
                // Determine if not found or already claimed by another.
                let exists_result: Result<i64, _> =
                    sqlx::query_scalar("SELECT COUNT(*) FROM jobs WHERE id = ?1")
                        .bind(job_id.to_string())
                        .fetch_one(&self.pool)
                        .await;
                let exists = match exists_result {
                    Ok(c) => c > 0,
                    Err(e) => return Err(RepoError::Backend(format!("heartbeat exists: {e}"))),
                };
                if exists {
                    Err(RepoError::AlreadyClaimed(job_id))
                } else {
                    Err(RepoError::NotFound(job_id))
                }
            },
            Err(e) => Err(e),
        }
    }

    async fn fail_and_retry(&self, job_id: Uuid, max_attempts: u8) -> Result<(), RepoError> {
        let tx = self
            .pool
            .begin()
            .await
            .map_err(|e| RepoError::Backend(format!("fail tx: {e}")));
        let mut tx = match tx {
            Ok(t) => t,
            Err(e) => return Err(e),
        };

        let row = sqlx::query("SELECT attempts FROM jobs WHERE id = ?1")
            .bind(job_id.to_string())
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| RepoError::Backend(format!("fail select: {e}")));
        let row = match row {
            Ok(Some(r)) => r,
            Ok(None) => {
                let _ = tx.rollback().await;
                return Err(RepoError::NotFound(job_id));
            },
            Err(e) => {
                let _ = tx.rollback().await;
                return Err(e);
            },
        };

        let attempts: i64 = row
            .try_get("attempts")
            .map_err(|e| RepoError::Backend(format!("fail parse attempts: {e}")))?;
        let new_attempts = attempts as u8 + 1;
        let now = Utc::now().to_rfc3339();

        let (status_str, action) = if new_attempts >= max_attempts {
            ("Failed", "JOB_FAILED")
        } else {
            ("Queued", "JOB_RETRY")
        };

        if let Err(e) = sqlx::query("UPDATE jobs SET status = ?1, attempts = ?2, worker_id = NULL, updated_at = ?3 WHERE id = ?4")
            .bind(status_str)
            .bind(new_attempts as i64)
            .bind(&now)
            .bind(job_id.to_string())
            .execute(&mut *tx)
            .await
            .map_err(|e| RepoError::Backend(format!("fail update: {e}")))
        {
            let _ = tx.rollback().await;
            return Err(e);
        }
        if let Err(e) = sqlx::query("INSERT INTO audit_records (job_id, action, new_status, occurred_at) VALUES (?1, ?2, ?3, ?4)")
            .bind(job_id.to_string())
            .bind(action)
            .bind(status_str)
            .bind(&now)
            .execute(&mut *tx)
            .await
            .map_err(|e| RepoError::Backend(format!("fail audit: {e}")))
        {
            let _ = tx.rollback().await;
            return Err(e);
        }

        tx.commit()
            .await
            .map_err(|e| RepoError::Backend(format!("fail commit: {e}")))?;
        Ok(())
    }

    // --- Tracks ---

    async fn save_track(&self, track: &TrackRecord) -> Result<(), RepoError> {
        let now = Utc::now().to_rfc3339();
        let status_str = format!("{:?}", track.status);
        let analysis_json = track
            .analysis
            .as_ref()
            .map(|v| serde_json::to_string(v).unwrap_or_default());

        sqlx::query(
            "INSERT INTO tracks (id, tenant_id, project_id, object_key, display_name, status, duration_sec, sample_rate, channels, sha256, analysis, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
        )
        .bind(track.id.to_string())
        .bind(track.tenant_id.to_string())
        .bind(track.project_id.map(|p| p.to_string()))
        .bind(&track.object_key)
        .bind(&track.display_name)
        .bind(&status_str)
        .bind(track.duration_sec)
        .bind(track.sample_rate.map(|s| s as i64))
        .bind(track.channels.map(|c| c as i64))
        .bind(&track.sha256)
        .bind(&analysis_json)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|e| RepoError::Backend(format!("save_track: {e}")))?;
        Ok(())
    }

    async fn get_track(&self, track_id: Uuid, tenant_id: Uuid) -> Result<TrackRecord, RepoError> {
        let row = sqlx::query("SELECT * FROM tracks WHERE id = ?1 AND tenant_id = ?2")
            .bind(track_id.to_string())
            .bind(tenant_id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| RepoError::Backend(format!("get_track: {e}")));
        match row {
            Ok(Some(r)) => Self::parse_track_row(r),
            Ok(None) => Err(RepoError::NotFound(track_id)),
            Err(e) => Err(e),
        }
    }

    async fn list_tracks(&self, tenant_id: Uuid) -> Result<Vec<TrackRecord>, RepoError> {
        let rows =
            sqlx::query("SELECT * FROM tracks WHERE tenant_id = ?1 ORDER BY created_at DESC")
                .bind(tenant_id.to_string())
                .fetch_all(&self.pool)
                .await
                .map_err(|e| RepoError::Backend(format!("list_tracks: {e}")));
        rows.and_then(|rows| rows.into_iter().map(Self::parse_track_row).collect())
    }

    // --- System (no tenant) ---

    async fn list_processing_jobs(&self) -> Result<Vec<JobRecord>, RepoError> {
        let rows = sqlx::query("SELECT * FROM jobs WHERE status = 'Processing'")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepoError::Backend(format!("list_processing: {e}")));
        rows.and_then(|rows| rows.into_iter().map(Self::parse_job_row).collect())
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
        repo.save_job(job_id, tenant_id, user_id, &PipelineConfig::default(), &[])
            .await
            .unwrap();
        let job = repo.get_job(job_id, tenant_id).await.unwrap();
        assert_eq!(job.id, job_id);
        assert_eq!(job.status, JobStatus::Queued);
        // New fields default to None
        assert!(job.mode.is_none());
        assert!(job.track_id.is_none());
    }

    #[tokio::test]
    async fn test_sqlite_claim_next_job() {
        let repo = setup_repo().await;
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
    async fn test_sqlite_concurrent_claims() {
        let repo = std::sync::Arc::new(setup_repo().await);
        let tenant_id = Uuid::new_v4();
        for _ in 0..5 {
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
        for _ in 0..5 {
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
        assert_eq!(claimed, 5);
    }

    #[tokio::test]
    async fn test_sqlite_heartbeat_and_fail_retry() {
        let repo = setup_repo().await;
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
        repo.heartbeat(job_id, worker_id).await.unwrap();
        repo.fail_and_retry(job_id, 3).await.unwrap();
        let job = repo.get_job(job_id, tenant_id).await.unwrap();
        assert_eq!(job.status, JobStatus::Queued);
        assert_eq!(job.attempts, 1);
    }

    #[tokio::test]
    async fn test_sqlite_save_and_get_track() {
        let repo = setup_repo().await;
        let tenant_id = Uuid::new_v4();
        let track = TrackRecord {
            id: Uuid::new_v4(),
            tenant_id,
            project_id: None,
            object_key: "tenant-abc/raw/test.wav".to_string(),
            display_name: "Test Track".to_string(),
            status: TrackStatus::Uploaded,
            duration_sec: Some(120.0),
            sample_rate: Some(44100),
            channels: Some(2),
            sha256: None,
            analysis: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        repo.save_track(&track).await.unwrap();
        let fetched = repo.get_track(track.id, tenant_id).await.unwrap();
        assert_eq!(fetched.display_name, "Test Track");
        assert_eq!(fetched.status, TrackStatus::Uploaded);
    }

    #[tokio::test]
    async fn test_sqlite_list_processing_jobs() {
        let repo = setup_repo().await;
        let tenant_id = Uuid::new_v4();
        repo.save_job(
            Uuid::new_v4(),
            tenant_id,
            Uuid::new_v4(),
            &PipelineConfig::default(),
            &[],
        )
        .await
        .unwrap();
        repo.claim_next_job(Uuid::new_v4()).await.unwrap();
        let processing = repo.list_processing_jobs().await.unwrap();
        assert_eq!(processing.len(), 1);
    }
}
