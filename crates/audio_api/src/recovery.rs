use crate::state::AppState;
use audio_core::ports::repo_trait::JobStatus;
use chrono::Utc;

#[derive(Debug, Default)]
pub struct RecoveryReport {
    pub recovered: usize,
    pub requeued: usize,
    pub lost: usize,
    pub proposals_expired: usize,
}

pub async fn run_recovery(state: &AppState) -> Result<RecoveryReport, String> {
    let mut report = RecoveryReport::default();
    let cutoff = Utc::now() - chrono::Duration::minutes(2);

    let all_jobs = state
        .repo
        .list_jobs(uuid::Uuid::nil())
        .await
        .map_err(|e| format!("recovery list failed: {e}"))?;

    for job in all_jobs {
        if job.status != JobStatus::Processing {
            continue;
        }
        let is_stale = match &job.last_heartbeat {
            Some(hb) => *hb < cutoff,
            None => true,
        };
        if is_stale {
            if let Err(e) = state.repo.fail_and_retry(job.id, 3).await {
                tracing::warn!(job_id = %job.id, error = %e, "recovery: failed");
                report.lost += 1;
            } else {
                report.requeued += 1;
            }
        }
    }

    state
        .hub
        .publish(
            uuid::Uuid::nil(),
            "recovery.report",
            serde_json::json!({"requeued": report.requeued}),
        )
        .await;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::InMemoryRepo;
    use crate::config::{
        AppConfig, AudioConfig, DatabaseConfig, LlmConfig, ObservabilityConfig, StorageConfig,
    };
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
                base_url: "".to_string(),
                temperature: 0.7,
                max_tools: 5,
                timeout_sec: 30,
            },
            observability: ObservabilityConfig {
                otel_collector_endpoint: "".to_string(),
                prometheus_port: 9090,
                grafana_url: "".to_string(),
            },
            features: Default::default(),
        };
        let repo = InMemoryRepo::new();
        let v = Arc::new(ValidationLayer::new());
        let m = Arc::new(MockLlm::new());
        let o = Arc::new(ReActOrchestrator::<MockLlm>::new(v, m, 5));
        let hub = Arc::new(crate::sse::EventHub::new());
        AppState::new(repo, o, Arc::new(config), hub)
    }

    #[tokio::test]
    async fn test_recovery_no_stale_jobs() {
        let state = test_state();
        let report = run_recovery(&state).await.unwrap();
        assert_eq!(report.requeued, 0);
    }

    #[test]
    fn test_recovery_report_defaults() {
        let r = RecoveryReport::default();
        assert_eq!(r.requeued, 0);
        assert_eq!(r.lost, 0);
    }
}
