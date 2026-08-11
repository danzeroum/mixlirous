use crate::state::AppState;
use audio_core::decode_to_pcm;
use audio_core::ports::repo_trait::JobStatus;
use serde_json::json;
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
            Err(e) => return Err(format!("claim: {e}")),
        };
        let job_id = job.id;

        // Start heartbeat task
        let repo = self.state.repo.clone();
        let hb_job = job_id;
        let hb_wid = worker_id;
        let hb_task = tokio::spawn(async move {
            let mut iv = tokio::time::interval(Duration::from_secs(10));
            loop {
                iv.tick().await;
                if repo.heartbeat(hb_job, hb_wid).await.is_err() {
                    break;
                }
            }
        });

        let result = self.execute_job(&job).await;

        hb_task.abort();

        match result {
            Ok(artifact_key) => {
                self.state
                    .repo
                    .transition_job(job_id, JobStatus::Completed, "JOB_COMPLETED")
                    .await
                    .map_err(|e| format!("transition: {e}"))?;
                self.state
                    .hub
                    .publish(
                        job_id,
                        "job.completed",
                        json!({
                            "status": "completed",
                            "download_url": format!("/api/v1/artifacts/{artifact_key}")
                        }),
                    )
                    .await;
            },
            Err(e) => {
                tracing::error!(worker_id = %self.id, job_id = %job_id, error = %e, "job failed");
                let _ = self.state.repo.fail_and_retry(job_id, 3).await;
                self.state
                    .hub
                    .publish(
                        job_id,
                        "job.failed",
                        json!({ "status": "failed", "error": e }),
                    )
                    .await;
            },
        }
        Ok(())
    }

    async fn execute_job(
        &self,
        job: &audio_core::ports::repo_trait::JobRecord,
    ) -> Result<String, String> {
        // 1. Load audio from storage via track_id/object_key
        let object_key = if let Some(track_id) = job.track_id {
            self.state
                .repo
                .get_track(track_id, job.tenant_id)
                .await
                .ok()
                .map(|t| t.object_key)
                .unwrap_or_default()
        } else {
            String::new()
        };

        if object_key.is_empty() {
            return Err("no track_id/object_key associated with job".to_string());
        }

        self.state
            .hub
            .publish(
                job.id,
                "job.state",
                json!({ "status": "processing", "stage": "loading_audio" }),
            )
            .await;

        let audio_bytes = self
            .state
            .storage
            .get(&object_key)
            .await
            .map_err(|e| format!("storage get: {e}"))?;

        // 2. Decode to PCM (CPU-bound, spawn_blocking)
        self.state
            .hub
            .publish(
                job.id,
                "job.state",
                json!({ "status": "processing", "stage": "decoding" }),
            )
            .await;

        let decoded = tokio::task::spawn_blocking(move || decode_to_pcm(&audio_bytes))
            .await
            .map_err(|e| format!("decode join: {e}"))?
            .map_err(|e| format!("decode: {e}"))?;

        // 3. If assisted mode, run ReAct orchestrator to get recipe
        if job.mode.as_deref() == Some("assisted") {
            if let Some(ref prompt) = job.user_prompt {
                self.state
                    .hub
                    .publish(
                        job.id,
                        "job.state",
                        json!({ "status": "processing", "stage": "agent_planning" }),
                    )
                    .await;

                let context = json!({
                    "track_info": {
                        "duration_sec": decoded.duration_sec(),
                        "sample_rate": decoded.sample_rate,
                        "channels": decoded.channels,
                        "frames": decoded.frames(),
                    }
                });

                // The orchestrator type is ReActOrchestrator<MockLlm> at compile time.
                // In a real deploy, the state would hold a dyn dispatch or generic.
                // For now, we attempt to run and if it errors, we fall back to manual.
                let _recipe = self
                    .state
                    .orchestrator
                    .run(prompt, &context)
                    .await
                    .map_err(|e| format!("agent error (non-fatal, falling back to manual): {e}"))
                    .ok();

                // The recipe's tool_calls will be consumed by the DSP pipeline.
                // For now, the recipe is logged for visibility.
                if let Some(ref r) = _recipe {
                    tracing::info!(
                        job_id = %job.id,
                        tools_used = r.tool_calls.len(),
                        "agent recipe generated"
                    );
                }
            }
        }

        // 4. DSP pipeline (CPU-bound, spawn_blocking)
        self.state
            .hub
            .publish(
                job.id,
                "job.state",
                json!({ "status": "processing", "stage": "dsp_processing" }),
            )
            .await;

        let artifact_key = format!("tenant-{}/artifacts/{}/remix.wav", job.tenant_id, job.id);

        // The full DSP chain from fatia_vertical goes here.
        // For now, produce a placeholder WAV using the decoded audio.
        let wav_bytes = tokio::task::spawn_blocking(move || {
            // Placeholder: encode the decoded audio back to WAV.
            // The real pipeline would be: beats -> blocks -> stitch -> fades -> master -> encode.
            // This is a no-op pass-through to validate the end-to-end flow.
            audio_core::dsp::DefaultMixer.encode_wav_to_vec(
                &audio_core::ndarray::Array1::from(decoded.interleaved),
                &audio_core::PipelineConfig::default(),
            )
        })
        .await
        .map_err(|e| format!("dsp join: {e}"))?
        .map_err(|e| format!("dsp: {e}"))?;

        // 5. Store artifact
        self.state
            .hub
            .publish(
                job.id,
                "job.state",
                json!({ "status": "processing", "stage": "storing_artifact" }),
            )
            .await;

        self.state
            .storage
            .put(&artifact_key, bytes::Bytes::from(wav_bytes))
            .await
            .map_err(|e| format!("artifact storage: {e}"))?;

        Ok(artifact_key)
    }
}

pub async fn start_worker(state: AppState) {
    Worker::new(state).run().await;
}
