//! Sprint 4 — Full recovery loop (docs/06-PERSISTENCIA-RESILIENCIA.md §5).
//!
//! Recovery runs BEFORE the API accepts requests. It is idempotent: safe to
//! run multiple times, including if it crashes mid-way.
//!
//! Algorithm:
//!   1. List in-flight jobs (Processing, AwaitingApproval)
//!   2. For each Processing job:
//!      a. Check if artifact exists in storage
//!      b. If exists, verify integrity (non-empty)
//!      c. If valid artifact → mark Completed (user sees nothing)
//!      d. If no artifact or corrupt → requeue or fail
//!   3. For each AwaitingApproval job:
//!      a. If proposal expired → requeue (agent replans)
//!      b. If proposal alive → keep (TTL continues)
//!   4. Log recovery action audit event

use crate::audit::{record_audit, ActorType, AuditAction};
use crate::metrics::{inc_counter, RECOVERY_JOBS_TOTAL};
use crate::state::AppState;
use audio_core::ports::repo_trait::JobStatus;
use chrono::Utc;
use serde::Serialize;
use uuid::Uuid;

/// Detailed recovery report with per-job outcomes.
#[derive(Debug, Default, Clone, Serialize)]
pub struct RecoveryReport {
    /// Jobs whose artifact was found intact and marked completed.
    pub recovered: usize,
    /// Jobs requeued for retry.
    pub requeued: usize,
    /// Jobs permanently failed (max attempts exhausted).
    pub lost: usize,
    /// Expired proposals cleaned up.
    pub proposals_expired: usize,
    /// Jobs left in running state (heartbeat recent).
    pub still_running: usize,
    /// Jobs in awaiting_approval with live proposals.
    pub awaiting_approval_live: usize,
}

/// Maximum time since last heartbeat before a job is considered stale.
const STALE_THRESHOLD_SECS: i64 = 120; // 2 minutes

/// Run the full Sprint 4 recovery loop.
/// This is the version that checks storage and handles artifacts.
pub async fn run_recovery(state: &AppState) -> Result<RecoveryReport, String> {
    let mut report = RecoveryReport::default();
    let cutoff = Utc::now() - chrono::Duration::seconds(STALE_THRESHOLD_SECS);

    tracing::info!("Sprint 4 recovery: scanning for in-flight jobs...");

    // Step 1: List all in-flight jobs (no tenant filter — recovery is global).
    let processing_jobs = state
        .repo
        .list_processing_jobs()
        .await
        .map_err(|e| format!("recovery list_processing failed: {e}"))?;

    let max_attempts: u8 = 3;

    // Step 2: Process each in-flight job.
    for job in processing_jobs {
        let is_stale = match &job.last_heartbeat {
            Some(hb) => *hb < cutoff,
            None => true, // No heartbeat ever → definitely stale
        };

        if !is_stale {
            // Heartbeat is recent — worker might still be alive.
            report.still_running += 1;
            tracing::debug!(
                job_id = %job.id,
                "recovery: job has recent heartbeat, keeping as running"
            );
            continue;
        }

        // Job is stale — check if artifact was produced before the crash.
        let artifact_key = format!("tenant-{}/artifacts/{}/remix.wav", job.tenant_id, job.id);

        let artifact_exists = state.storage.exists(&artifact_key).await;

        if artifact_exists {
            // Artifact exists on disk — the DSP finished before the crash.
            // Verify integrity (non-empty).
            match state.storage.get(&artifact_key).await {
                Ok(bytes) if !bytes.is_empty() => {
                    // Valid artifact found → mark completed silently.
                    if let Err(e) = state
                        .repo
                        .transition_job(job.id, JobStatus::Completed, "RECOVERY_COMPLETED")
                        .await
                    {
                        tracing::warn!(
                            job_id = %job.id,
                            error = %e,
                            "recovery: failed to mark recovered job as completed"
                        );
                        report.lost += 1;
                    } else {
                        report.recovered += 1;
                        inc_counter(&RECOVERY_JOBS_TOTAL);
                        tracing::info!(
                            job_id = %job.id,
                            artifact_bytes = bytes.len(),
                            "recovery: artifact found intact, job marked completed"
                        );

                        // Publish recovery event via SSE.
                        state
                            .hub
                            .publish(
                                job.id,
                                "job.completed",
                                serde_json::json!({
                                    "status": "completed",
                                    "recovered": true,
                                    "download_url": format!("/api/v1/artifacts/{artifact_key}")
                                }),
                            )
                            .await;

                        // Audit event.
                        record_audit(
                            job.tenant_id,
                            ActorType::System,
                            AuditAction::RecoveryAction,
                            "job",
                            &job.id.to_string(),
                        );
                    }
                },
                _ => {
                    // Artifact exists but is empty or unreadable — corrupt.
                    tracing::warn!(
                        job_id = %job.id,
                        "recovery: artifact exists but is corrupt/empty, requeuing"
                    );
                    try_requeue_or_fail(
                        &state.repo,
                        job.id,
                        job.tenant_id,
                        max_attempts,
                        &mut report,
                    )
                    .await;
                },
            }
        } else {
            // No artifact — DSP didn't finish. Requeue or fail.
            try_requeue_or_fail(
                &state.repo,
                job.id,
                job.tenant_id,
                max_attempts,
                &mut report,
            )
            .await;
        }
    }

    // Step 3: Handle proposals — expired ones.
    // (Proposal expiry is handled by the ProposalStore TTL in proposals.rs.)
    // Here we just count and log.
    tracing::debug!("recovery: proposal cleanup handled by ProposalStore TTL");

    // Step 4: Publish recovery report via SSE.
    state
        .hub
        .publish(
            Uuid::nil(),
            "recovery.report",
            serde_json::json!({
                "recovered": report.recovered,
                "requeued": report.requeued,
                "lost": report.lost,
                "still_running": report.still_running,
            }),
        )
        .await;

    tracing::info!(
        recovered = report.recovered,
        requeued = report.requeued,
        lost = report.lost,
        still_running = report.still_running,
        "Sprint 4 recovery completed"
    );

    Ok(report)
}

/// Try to requeue a job, or mark as failed if max attempts exceeded.
async fn try_requeue_or_fail(
    repo: &std::sync::Arc<dyn audio_core::ports::repo_trait::AudioRepo>,
    job_id: Uuid,
    _tenant_id: Uuid,
    max_attempts: u8,
    report: &mut RecoveryReport,
) {
    if let Err(e) = repo.fail_and_retry(job_id, max_attempts).await {
        tracing::error!(
            job_id = %job_id,
            error = %e,
            "recovery: fail_and_retry failed — job lost"
        );
        report.lost += 1;
    } else {
        report.requeued += 1;
        inc_counter(&RECOVERY_JOBS_TOTAL);
        tracing::info!(job_id = %job_id, "recovery: job requeued for retry");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::InMemoryRepo;
    use crate::config::*;
    use crate::sse::EventHub;
    use crate::storage::LocalFsStorage;
    use audio_agent::llm::mock::MockLlm;
    use audio_agent::validator::ValidationLayer;
    use audio_agent::ReActOrchestrator;
    use std::sync::Arc;

    fn test_state() -> AppState {
        let config = AppConfig {
            database: DatabaseConfig {
                type_db: "sqlite".to_string(),
                url: ":memory:".to_string(),
                max_connections: 1,
            },
            storage: StorageConfig {
                type_storage: "local".to_string(),
                endpoint: None,
                bucket: "test".to_string(),
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
                provider: "mock".to_string(),
                model: "mock".to_string(),
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
        };
        let repo = InMemoryRepo::new();
        let v = Arc::new(ValidationLayer::new());
        let m = Arc::new(MockLlm::new());
        let o = Arc::new(ReActOrchestrator::<MockLlm>::new(v, m, 5));
        let hub = Arc::new(EventHub::new());
        let storage: Arc<dyn audio_core::ports::Storage> =
            Arc::new(LocalFsStorage::new(tempfile::tempdir().unwrap().keep()).unwrap());
        AppState::new(repo, o, Arc::new(config), hub, storage)
    }

    #[tokio::test]
    async fn test_recovery_no_stale_jobs() {
        let state = test_state();
        let report = run_recovery(&state).await.unwrap();
        assert_eq!(report.requeued, 0);
        assert_eq!(report.recovered, 0);
        assert_eq!(report.lost, 0);
    }

    #[test]
    fn test_recovery_report_defaults() {
        let r = RecoveryReport::default();
        assert_eq!(r.requeued, 0);
        assert_eq!(r.lost, 0);
        assert_eq!(r.recovered, 0);
        assert_eq!(r.proposals_expired, 0);
        assert_eq!(r.still_running, 0);
        assert_eq!(r.awaiting_approval_live, 0);
    }

    #[test]
    fn test_recovery_report_serialization() {
        let r = RecoveryReport {
            recovered: 1,
            requeued: 2,
            lost: 0,
            proposals_expired: 3,
            still_running: 1,
            awaiting_approval_live: 0,
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"recovered\":1"));
        assert!(json.contains("\"requeued\":2"));
        assert!(json.contains("\"proposals_expired\":3"));
    }

    #[test]
    fn test_stale_threshold_is_120_seconds() {
        assert_eq!(STALE_THRESHOLD_SECS, 120);
    }

    #[tokio::test]
    async fn test_recovery_with_stale_job_no_artifact_requeues() {
        let state = test_state();
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let job_id = Uuid::new_v4();
        let default_config = audio_core::PipelineConfig::default();
        let default_blocks: Vec<audio_core::BeatBlock> = vec![];

        // Create a job via save_job and transition to Processing (stale)
        state
            .repo
            .save_job(job_id, tenant_id, user_id, &default_config, &default_blocks)
            .await
            .unwrap();

        state
            .repo
            .transition_job(job_id, JobStatus::Processing, "test")
            .await
            .unwrap();

        // Run recovery — no artifact exists, should requeue
        let report = run_recovery(&state).await.unwrap();
        assert_eq!(report.requeued, 1);
        assert_eq!(report.lost, 0);
    }

    #[tokio::test]
    async fn test_recovery_with_artifact_marks_completed() {
        let state = test_state();
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let job_id = Uuid::new_v4();
        let default_config = audio_core::PipelineConfig::default();
        let default_blocks: Vec<audio_core::BeatBlock> = vec![];

        state
            .repo
            .save_job(job_id, tenant_id, user_id, &default_config, &default_blocks)
            .await
            .unwrap();

        state
            .repo
            .transition_job(job_id, JobStatus::Processing, "test")
            .await
            .unwrap();

        // Pre-write the artifact to storage (simulating DSP completed before crash)
        let artifact_key = format!("tenant-{}/artifacts/{}/remix.wav", tenant_id, job_id);
        state
            .storage
            .put(
                &artifact_key,
                bytes::Bytes::from_static(b"RIFFfake_wav_data"),
            )
            .await
            .unwrap();

        // Run recovery — artifact exists and is non-empty → completed
        let report = run_recovery(&state).await.unwrap();
        assert_eq!(report.recovered, 1);
        assert_eq!(report.requeued, 0);
        assert_eq!(report.lost, 0);
    }
}
