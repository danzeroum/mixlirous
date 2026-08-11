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
    pub fn new(state: AppState) -> Self { Self { id: Uuid::new_v4(), state } }

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
            Err(e) => return Err(format!("claim: {e}")),
        };
        let job_id = job.id;

        let repo = self.state.repo.clone();
        let hb_job = job_id;
        let hb_wid = worker_id;
        let hb_task = tokio::spawn(async move {
            let mut iv = tokio::time::interval(Duration::from_secs(10));
            loop { iv.tick().await; if repo.heartbeat(hb_job, hb_wid).await.is_err() { break; } }
        });

        self.state.hub.publish(job_id, "job.state", serde_json::json!({"status":"processing"})).await;
        tokio::time::sleep(Duration::from_secs(2)).await;
        hb_task.abort();

        self.state.repo.transition_job(job_id, JobStatus::Completed, "JOB_COMPLETED").await
            .map_err(|e| format!("transition: {e}"))?;
        self.state.hub.publish(job_id, "job.completed", serde_json::json!({"status":"completed"})).await;
        Ok(())
    }
}

pub async fn start_worker(state: AppState) {
    Worker::new(state).run().await;
}