use crate::state::AppState;
use audio_core::decode_to_pcm;
use audio_core::downmix_to_mono;
use audio_core::ports::repo_trait::JobStatus;
use audio_core::{DefaultRemixPipeline, PipelineConfig, PipelineInput, RemixPipeline};
use audio_core::domain::AudioCodec;
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

        // Inicia tarefa de heartbeat em background.
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
        // 1. Carrega audio do storage via track_id/object_key.
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

        // 2. Decodifica para PCM (CPU-bound, spawn_blocking).
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

        // 3. Se modo assistido, executa o orquestrador ReAct para obter a receita.
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

                let _recipe = self
                    .state
                    .orchestrator
                    .run(prompt, &context)
                    .await
                    .map_err(|e| format!("agent error (non-fatal, falling back to manual): {e}"))
                    .ok();

                if let Some(ref r) = _recipe {
                    tracing::info!(
                        job_id = %job.id,
                        tools_used = r.tool_calls.len(),
                        "agent recipe generated"
                    );
                }
            }
        }

        // 4. Pipeline DSP (CPU-bound, spawn_blocking).
        // Usa o pipeline estruturado (ADR-0012) em vez de operacoes ad-hoc.
        self.state
            .hub
            .publish(
                job.id,
                "job.state",
                json!({ "status": "processing", "stage": "dsp_processing" }),
            )
            .await;

        let sample_rate = decoded.sample_rate;
        let pipeline_result = tokio::task::spawn_blocking(move || {
            // Downmix para mono -- o pipeline opera em mono.
            let mono_pcm = downmix_to_mono(&decoded);

            // Monta a configuracao do pipeline, forçando mono para encode.
            let mut config = PipelineConfig::default();
            config.format.sample_rate = sample_rate;
            config.format.channels = 1;
            config.format.bit_depth = 32;
            config.format.codec = AudioCodec::WAV;

            let pipeline = DefaultRemixPipeline::new();
            let input = PipelineInput {
                pcm: mono_pcm,
                sample_rate,
                config,
                pre_selected_blocks: None,
            };
            pipeline.run(input)
        })
        .await
        .map_err(|e| format!("pipeline join: {e}"))?;

        let pipeline_result = pipeline_result.map_err(|e| format!("pipeline: {e}"))?;

        // Publica avisos do pipeline como eventos SSE.
        for warning in &pipeline_result.warnings {
            self.state
                .hub
                .publish(
                    job.id,
                    "job.warning",
                    json!({ "message": warning }),
                )
                .await;
        }

        tracing::info!(
            job_id = %job.id,
            blocks_used = pipeline_result.blocks_used.len(),
            duration_sec = %pipeline_result.duration_sec(),
            bpm = ?pipeline_result.bpm_estimate,
            "pipeline completed"
        );

        // 5. Codifica para WAV e armazena o artefato.
        self.state
            .hub
            .publish(
                job.id,
                "job.state",
                json!({ "status": "processing", "stage": "storing_artifact" }),
            )
            .await;

        let mut export_config = PipelineConfig::default();
        export_config.format.sample_rate = sample_rate;
        export_config.format.channels = 1;
        export_config.format.bit_depth = 32;
        export_config.format.codec = AudioCodec::WAV;

        let wav_bytes = audio_core::dsp::DefaultMixer.encode_wav_to_vec(
            &pipeline_result.pcm,
            &export_config,
        )
        .map_err(|e| format!("encode: {e}"))?;

        let artifact_key = format!("tenant-{}/artifacts/{}/remix.wav", job.tenant_id, job.id);

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
