use crate::state::AppState;
use audio_core::ports::repo_trait::JobStatus;
use std::time::Duration;
use tokio::time::interval;
use uuid::Uuid;

pub struct Worker {
    id: Uuid,
    state: AppState,
}

impl Worker {
    pub fn new(state: AppState) -> Self {
        Self {
            id: Uuid::new_v4(),
            state,
        }
    }

    pub async fn run(&self) {
        let mut ticker = interval(Duration::from_secs(5));
        tracing::info!(worker_id = %self.id, "worker started");

        loop {
            ticker.tick().await;
            if let Err(e) = self.process_next_job().await {
                tracing::error!(worker_id = %self.id, error = %e, "worker error");
            }
        }
    }

    async fn process_next_job(&self) -> Result<(), String> {
        let worker_id = Uuid::new_v4();

        let job = match self.state.repo.claim_next_job(worker_id).await {
            Ok(Some(job)) => job,
            Ok(None) => return Ok(()),
            Err(e) => return Err(format!("claim failed: {e}")),
        };

        let job_id = job.id;
        tracing::info!(worker_id = %self.id, job_id = %job_id, "job claimed");

        self.state.hub.publish(
            job_id,
            "job.state",
            serde_json::json!({ "job_id": job_id, "status": "processing" }),
        ).await;

        // TODO: Execute DSP pipeline (Sprint 2)
        tokio::time::sleep(Duration::from_secs(2)).await;

        self.state.repo
            .transition_job(job_id, JobStatus::Completed, "JOB_COMPLETED")
            .await
            .map_err(|e| format!("transition failed: {e}"))?;

        self.state.hub.publish(
            job_id,
            "job.completed",
            serde_json::json!({ "job_id": job_id, "status": "completed" }),
        ).await;

        tracing::info!(worker_id = %self.id, job_id = %job_id, "job completed");
        Ok(())
    }
}

pub async fn start_worker(state: AppState) {
    let worker = Worker::new(state);
    worker.run().await;
}