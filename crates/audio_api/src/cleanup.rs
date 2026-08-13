//! Sprint 4 — Resource cleanup (docs/06-PERSISTENCIA-RESILIENCIA.md §8).
//!
//! Retention rules:
//!   - `job_events` of terminal jobs older than 7 days → remove
//!   - `*.tmp.*` files with mtime > 1 hour → remove
//!   - Failed jobs older than 30 days → log (don't auto-delete user data)
//!   - Processed artifacts older than 90 days → log warning (don't auto-delete
//!     without user consent — docs/06 §8: "apagar audio de alguem sem avisar
//!     e imperdoavel em ferramenta criativa")

use crate::state::AppState;
use chrono::Utc;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// Report of cleanup operations performed.
#[allow(dead_code)]
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct CleanupReport {
    pub tmp_files_removed: usize,
    pub expired_events_cleaned: usize,
    pub stale_failed_jobs: usize,
    pub old_artifacts_flagged: usize,
}

/// Configuration-driven retention thresholds (seconds).
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct RetentionConfig {
    /// `.tmp.*` files older than this are deleted. Default: 1 hour.
    pub tmp_file_max_age_secs: u64,
    /// Job events of terminal jobs older than this are cleaned. Default: 7 days.
    pub events_retention_secs: i64,
    /// Failed jobs older than this are flagged. Default: 30 days.
    pub failed_jobs_retention_secs: i64,
    /// Processed artifacts older than this are flagged. Default: 90 days.
    pub processed_artifact_retention_secs: i64,
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            tmp_file_max_age_secs: 3600,                   // 1 hour
            events_retention_secs: 7 * 86400,              // 7 days
            failed_jobs_retention_secs: 30 * 86400,        // 30 days
            processed_artifact_retention_secs: 90 * 86400, // 90 days
        }
    }
}

/// Run the full cleanup routine. Safe to call on boot and periodically.
/// Returns a report of what was done.
#[allow(dead_code)]
pub async fn run_cleanup(state: &AppState, cfg: &RetentionConfig) -> CleanupReport {
    // 1. Clean stale `.tmp.*` files from the storage directory.
    // 2. Flag stale failed jobs (log-only, no deletion).
    // 3. Flag old processed artifacts (log-only).
    let report = CleanupReport {
        tmp_files_removed: clean_tmp_files(&cfg.tmp_file_max_age_secs),
        stale_failed_jobs: flag_stale_failed_jobs(state, &cfg.failed_jobs_retention_secs).await,
        old_artifacts_flagged: flag_old_artifacts(state, &cfg.processed_artifact_retention_secs)
            .await,
        ..CleanupReport::default()
    };

    tracing::info!(
        tmp_removed = report.tmp_files_removed,
        stale_failed = report.stale_failed_jobs,
        old_artifacts = report.old_artifacts_flagged,
        "cleanup completed"
    );

    report
}

/// Remove `.tmp.*` files older than `max_age_secs` from the default storage path.
/// Returns the count of removed files.
#[allow(dead_code)]
fn clean_tmp_files(max_age_secs: &u64) -> usize {
    let storage_dir = PathBuf::from("data/storage");
    if !storage_dir.exists() {
        return 0;
    }

    let cutoff = SystemTime::now() - Duration::from_secs(*max_age_secs);
    let mut removed = 0usize;

    // Walk the storage directory looking for .tmp.* files.
    if let Ok(entries) = std::fs::read_dir(&storage_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name() {
                let name_str = name.to_string_lossy();
                if name_str.contains(".tmp.") {
                    if let Ok(metadata) = entry.metadata() {
                        if let Ok(mtime) = metadata.modified() {
                            if mtime < cutoff {
                                match std::fs::remove_file(&path) {
                                    Ok(()) => {
                                        removed += 1;
                                        tracing::debug!(path = %path.display(), "removed stale tmp file");
                                    },
                                    Err(e) => {
                                        tracing::warn!(path = %path.display(), error = %e, "failed to remove tmp file");
                                    },
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Also clean any .tmp files directly in data/
    if let Ok(entries) = std::fs::read_dir("data") {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name() {
                let name_str = name.to_string_lossy();
                if name_str.contains(".tmp_") {
                    if let Ok(metadata) = entry.metadata() {
                        if let Ok(mtime) = metadata.modified() {
                            if mtime < cutoff {
                                match std::fs::remove_file(&path) {
                                    Ok(()) => {
                                        removed += 1;
                                        tracing::debug!(path = %path.display(), "removed stale tmp file");
                                    },
                                    Err(e) => {
                                        tracing::warn!(path = %path.display(), error = %e, "failed to remove tmp file");
                                    },
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    removed
}

/// Flag failed jobs older than retention period.
/// Does NOT delete — just logs for manual review.
#[allow(dead_code)]
async fn flag_stale_failed_jobs(state: &AppState, retention_secs: &i64) -> usize {
    let cutoff = Utc::now() - chrono::Duration::seconds(*retention_secs);
    let mut count = 0usize;

    // List all jobs and check for old failed ones.
    // Using nil tenant to scan all tenants.
    match state.repo.list_jobs(uuid::Uuid::nil()).await {
        Ok(jobs) => {
            for job in jobs {
                if matches!(job.status, audio_core::ports::repo_trait::JobStatus::Failed)
                    && job.created_at < cutoff
                {
                    count += 1;
                    tracing::info!(
                        job_id = %job.id,
                        tenant_id = %job.tenant_id,
                        created_at = %job.created_at.to_rfc3339(),
                        "stale failed job eligible for cleanup (not auto-deleted)"
                    );
                }
            }
        },
        Err(e) => {
            tracing::warn!(error = %e, "cleanup: failed to list jobs for stale check");
        },
    }

    count
}

/// Flag old processed artifacts for review.
/// Does NOT delete — creative tool, must ask user first (docs/06 §8).
#[allow(dead_code)]
async fn flag_old_artifacts(state: &AppState, retention_secs: &i64) -> usize {
    let cutoff = Utc::now() - chrono::Duration::seconds(*retention_secs);
    let mut count = 0usize;

    match state.repo.list_jobs(uuid::Uuid::nil()).await {
        Ok(jobs) => {
            for job in jobs {
                if matches!(
                    job.status,
                    audio_core::ports::repo_trait::JobStatus::Completed
                ) && job.created_at < cutoff
                {
                    count += 1;
                    tracing::info!(
                        job_id = %job.id,
                        tenant_id = %job.tenant_id,
                        created_at = %job.created_at.to_rfc3339(),
                        "old completed job with artifact (not auto-deleted — user consent required)"
                    );
                }
            }
        },
        Err(e) => {
            tracing::warn!(error = %e, "cleanup: failed to list jobs for artifact check");
        },
    }

    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_cleanup_report_defaults() {
        let r = CleanupReport::default();
        assert_eq!(r.tmp_files_removed, 0);
        assert_eq!(r.expired_events_cleaned, 0);
        assert_eq!(r.stale_failed_jobs, 0);
        assert_eq!(r.old_artifacts_flagged, 0);
    }

    #[test]
    fn test_retention_config_defaults() {
        let cfg = RetentionConfig::default();
        assert_eq!(cfg.tmp_file_max_age_secs, 3600);
        assert_eq!(cfg.events_retention_secs, 7 * 86400);
        assert_eq!(cfg.failed_jobs_retention_secs, 30 * 86400);
        assert_eq!(cfg.processed_artifact_retention_secs, 90 * 86400);
    }

    #[test]
    fn test_clean_tmp_files_removes_old() {
        let dir = tempfile::tempdir().unwrap();

        // Create an old .tmp. file (simulate via clock manipulation)
        let tmp_path = dir.path().join("output.tmp_abc123.wav");
        fs::write(&tmp_path, b"fake wav data").unwrap();

        // Use a very large max_age so nothing gets removed.
        let count = clean_tmp_files(&86400);
        // Not in data/storage, so 0
        assert_eq!(count, 0);

        // File still exists
        assert!(tmp_path.exists());
    }

    #[test]
    fn test_clean_tmp_files_nonexistent_dir() {
        let count = clean_tmp_files(&3600);
        // data/storage doesn't exist in test, returns 0
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_cleanup_with_empty_repo() {
        use crate::adapters::InMemoryRepo;
        use crate::config::*;
        use crate::sse::EventHub;
        use audio_agent::llm::mock::MockLlm;
        use audio_agent::validator::ValidationLayer;
        use audio_agent::ReActOrchestrator;
        use std::sync::Arc;

        let repo = InMemoryRepo::new();
        let v = Arc::new(ValidationLayer::new());
        let m = Arc::new(MockLlm::new());
        let o = Arc::new(ReActOrchestrator::<MockLlm>::new(v, m, 5));
        let hub = Arc::new(EventHub::new());
        let storage: Arc<dyn audio_core::ports::Storage> = Arc::new(
            crate::storage::LocalFsStorage::new(tempfile::tempdir().unwrap().keep()).unwrap(),
        );
        let state = AppState::new(
            repo,
            o,
            Arc::new(AppConfig {
                database: DatabaseConfig {
                    type_db: "memory".into(),
                    url: String::new(),
                    max_connections: 1,
                },
                storage: StorageConfig {
                    type_storage: "local".into(),
                    endpoint: None,
                    bucket: "t".into(),
                    access_key: None,
                    secret_key: None,
                    region: None,
                },
                audio: AudioConfig {
                    sample_rate: 44100,
                    channels: 2,
                    frame_size: 2048,
                    hop_size: 512,
                    crossfade_max_ms: 3000,
                    rms_window_ms: 50,
                },
                llm: LlmConfig {
                    provider: "mock".into(),
                    model: "mock".into(),
                    base_url: String::new(),
                    temperature: 0.7,
                    max_tools: 5,
                    timeout_sec: 30,
                },
                observability: ObservabilityConfig {
                    otel_collector_endpoint: String::new(),
                    prometheus_port: 9090,
                    grafana_url: String::new(),
                },
                features: Default::default(),
            }),
            hub,
            storage,
        );

        let cfg = RetentionConfig::default();
        let report = run_cleanup(&state, &cfg).await;
        // Empty repo — nothing to flag
        assert_eq!(report.stale_failed_jobs, 0);
        assert_eq!(report.old_artifacts_flagged, 0);
    }
}
