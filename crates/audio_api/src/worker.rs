use crate::state::AppState;
use audio_agent::{AudioToolDef, LlmError, ProposalDecision, ReActCallbacks, ReActOutput};
use audio_core::decode_to_pcm;
use audio_core::domain::{AudioCodec, CompressionRatio, CrossfadeCurve, CrossfadeMs, LufsTarget};
use audio_core::downmix_to_mono;
use audio_core::ports::repo_trait::JobStatus;
use audio_core::{DefaultRemixPipeline, PipelineConfig, PipelineInput, RemixPipeline};
use serde_json::json;
use std::time::Duration;
use tokio::time::interval;

/// Traduz a `ReActOutput` (lista de `AudioToolDef` escolhidas pelo agente) em
/// overrides sobre um `PipelineConfig` base. Item B1 do mapa de ação: o worker
/// não pode mais descartar a receita e rodar `PipelineConfig::default()` — o
/// agente "pensa" mas nada mudava. Aqui cada ferramenta suportada mapeia para
/// um campo concreto do `PipelineConfig`.
///
/// Regras:
/// - Ferramentas não-mapeadas (`FadeIn`/`FadeOut`/`DynamicEq`/`StemSeparation`)
///   são registradas via tracing mas não causam erro — o pipeline ainda roda.
/// - Valores fora de limite são rejeitados silenciosamente pelo newtype
///   (`TryFrom`), e o fallback é o valor anterior do `PipelineConfig`.
/// - `TimeStretch` exige pós-processamento (não é parte do PipelineConfig) —
///   emitido como aviso, aplicação fica para sprint futura.
fn apply_recipe_to_config(recipe: &ReActOutput, config: &mut PipelineConfig) {
    for tool in &recipe.tool_calls {
        match tool {
            AudioToolDef::Compression(p) => {
                if let Ok(r) = CompressionRatio::try_from(p.ratio) {
                    config.mastering.compression_ratio = r;
                }
                // threshold_db/attack_ms/release_ms/makeup_gain_db/knee_db são
                // parâmetros que o DefaultRemixPipeline ainda não consome
                // diretamente; registrados para a próxima iteração.
                tracing::info!(
                    ratio = p.ratio,
                    threshold_db = p.threshold_db,
                    "aplicando receita de compression ao config"
                );
            },
            AudioToolDef::Crossfade(p) => {
                if let Ok(ms) = CrossfadeMs::try_from(p.duration_ms) {
                    config.crossfade.max_duration_ms = ms;
                }
                config.crossfade.curve = match p.curve.as_str() {
                    "constant_gain" => CrossfadeCurve::ConstantGain,
                    _ => CrossfadeCurve::ConstantPower,
                };
                config.crossfade.enabled = p.duration_ms > 0;
            },
            AudioToolDef::LufsNormalization(p) => {
                if let Ok(t) = LufsTarget::try_from(p.target_lufs) {
                    config.mastering.lufs_target = t;
                }
                config.mastering.peak_db = p.max_true_peak_db;
                config.mastering.enable_limiting = true;
            },
            AudioToolDef::TimeStretch(p) => {
                // TimeStretch é pós-processamento (aplicado depois do pipeline
                // com `time_stretch()` do dsp::mastering). Não há campo direto
                // no PipelineConfig. Registramos como aviso; aplicação fica para
                // a próxima iteração.
                tracing::info!(
                    factor = p.factor,
                    "time_stretch na receita — aplicação pós-pipeline pendente"
                );
            },
            AudioToolDef::FadeIn(_)
            | AudioToolDef::FadeOut(_)
            | AudioToolDef::DynamicEq(_)
            | AudioToolDef::StemSeparation(_) => {
                tracing::info!(
                    tool = ?tool,
                    "ferramenta na receita não mapeada para PipelineConfig — registrada mas não aplicada"
                );
            },
        }
    }
    if recipe.llm_call_failed {
        tracing::warn!(
            "receita gerada em modo fallback (LLM indisponível) — usando PipelineConfig base"
        );
    }
}

use uuid::Uuid;

/// Liga os callbacks do loop ReAct ao `EventHub` (task 3.9 — streaming de
/// raciocínio via SSE). `tool_requires_proposal` retorna `false` porque o
/// ciclo HITL é acionado pelo usuário via `POST /proposals/{id}/approve` —
/// o worker automático segue a receita; a pausa explícita do job fica para a
/// Sprint 4 (detecção de job travado).
struct HubCallbacks {
    job_id: Uuid,
    hub: std::sync::Arc<crate::sse::hub::EventHub>,
}

#[async_trait::async_trait]
impl ReActCallbacks for HubCallbacks {
    async fn on_thought(&self, delta: &str) {
        self.hub
            .publish(self.job_id, "agent.thought", json!({ "delta": delta }))
            .await;
    }
    async fn on_tool_call(&self, tool: &audio_agent::AudioToolDef) {
        self.hub
            .publish(
                self.job_id,
                "agent.tool",
                json!({ "tool": format!("{tool:?}") }),
            )
            .await;
    }
    async fn on_validation_error(&self, error: &str) {
        self.hub
            .publish(
                self.job_id,
                "agent.error",
                json!({ "type": "validation", "message": error }),
            )
            .await;
    }
    async fn on_replan(&self, reason: &str) {
        self.hub
            .publish(self.job_id, "agent.replan", json!({ "reason": reason }))
            .await;
    }
    async fn on_llm_error(&self, error: &LlmError) {
        self.hub
            .publish(
                self.job_id,
                "agent.error",
                json!({ "type": "llm", "message": error.to_string() }),
            )
            .await;
    }
    async fn on_finished(&self, output: &ReActOutput) {
        self.hub
            .publish(
                self.job_id,
                "agent.finished",
                json!({
                    "llm_call_failed": output.llm_call_failed,
                    "tools_used": output.tool_calls.len(),
                }),
            )
            .await;
    }
    fn tool_requires_proposal(&self, _tool: &audio_agent::AudioToolDef) -> bool {
        false
    }
    async fn await_proposal_decision(&self) -> ProposalDecision {
        ProposalDecision::Approved
    }
    async fn on_proposal_created(&self, _proposal: &serde_json::Value) {}
}

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
                // Item B4: download_url deve apontar para a rota REST
                // documentada em docs/03-CONTRATOS-API.md §3.3
                // (`GET /api/v1/jobs/{id}/artifact`) — não para o path
                // interno de storage. O handler faz o 302 para a URL real.
                self.state
                    .hub
                    .publish(
                        job_id,
                        "job.completed",
                        json!({
                            "job_id": job_id.to_string(),
                            "status": "completed",
                            "download_url": format!("/api/v1/jobs/{job_id}/artifact"),
                            "artifact_object_key": artifact_key,
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
        // Item B1: a receita não é mais descartada — `apply_recipe_to_config`
        // traduz cada `AudioToolDef` em overrides sobre o `PipelineConfig`
        // base, aplicados ao pipeline DSP logo abaixo.
        let mut recipe_opt: Option<ReActOutput> = None;
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

                let callbacks = HubCallbacks {
                    job_id: job.id,
                    hub: self.state.hub.clone(),
                };

                match self
                    .state
                    .orchestrator
                    .run(prompt, &context, &callbacks)
                    .await
                {
                    Ok(r) => {
                        tracing::info!(
                            job_id = %job.id,
                            tools_used = r.tool_calls.len(),
                            llm_call_failed = r.llm_call_failed,
                            "agent recipe generated"
                        );
                        recipe_opt = Some(r);
                    },
                    Err(e) => {
                        // Item C4: erro estruturado — não aborta o job,
                        // mas publica `agent.error` com flag de fallback
                        // e segue com `PipelineConfig::default()`.
                        tracing::warn!(
                            job_id = %job.id,
                            error = %e,
                            "agent error non-fatal — falling back to manual config"
                        );
                        self.state
                            .hub
                            .publish(
                                job.id,
                                "agent.error",
                                json!({
                                    "type": "agent_run",
                                    "message": e.to_string(),
                                    "will_replan": false,
                                    "fallback": "manual_config",
                                }),
                            )
                            .await;
                    },
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
        // Item B1: serializa a receita para JSON antes do spawn_blocking
        // (não podemos mover `ReActOutput` entre threads sem `Send`, mas o
        // `serde_json::Value` correspondente é `Send`). Aplicamos no closure.
        let recipe_value = recipe_opt
            .as_ref()
            .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
            .unwrap_or(serde_json::Value::Null);
        let recipe_tool_calls_count = recipe_opt.as_ref().map(|r| r.tool_calls.len()).unwrap_or(0);
        let pipeline_result = tokio::task::spawn_blocking(move || {
            // Downmix para mono -- o pipeline opera em mono.
            let mono_pcm = downmix_to_mono(&decoded);

            // Monta a configuracao do pipeline, forçando mono para encode.
            let mut config = PipelineConfig::default();
            config.format.sample_rate = sample_rate;
            config.format.channels = 1;
            config.format.bit_depth = 32;
            config.format.codec = AudioCodec::WAV;

            // Item B1: aplica a receita do agente (se houver) sobre o config
            // base. Os overrides respeitam os newtypes (valores fora de limite
            // são rejeitados silenciosamente pelo `TryFrom`).
            if let Ok(recipe) = serde_json::from_value::<ReActOutput>(recipe_value) {
                apply_recipe_to_config(&recipe, &mut config);
            }

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

        tracing::info!(
            job_id = %job.id,
            recipe_tool_calls = recipe_tool_calls_count,
            blocks_used = pipeline_result.blocks_used.len(),
            "pipeline finished"
        );

        // Item M7: publica avisos do pipeline com o schema completo do
        // contrato (code/severity/message_ptbr/hint_ptbr) — não só `message`.
        for warning in &pipeline_result.warnings {
            self.state
                .hub
                .publish(
                    job.id,
                    "job.warning",
                    json!({
                        "job_id": job.id.to_string(),
                        "code": "pipeline_warning",
                        "severity": "warning",
                        "at_sec": null,
                        "message_ptbr": warning,
                        "hint_ptbr": null,
                        "measured": null,
                    }),
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

        let wav_bytes = audio_core::dsp::DefaultMixer
            .encode_wav_to_vec(&pipeline_result.pcm, &export_config)
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

#[cfg(test)]
mod recipe_tests {
    use super::*;
    use audio_agent::{
        tools::{CompressionParams, CrossfadeParams, LufsNormalizationParams},
        ReActOutput,
    };

    fn empty_output() -> ReActOutput {
        ReActOutput {
            thoughts: Vec::new(),
            tool_calls: Vec::new(),
            final_context: serde_json::Value::Null,
            llm_call_failed: false,
        }
    }

    #[test]
    fn aplica_crossfade_da_receita() {
        let mut config = PipelineConfig::default();
        let initial_curve = config.crossfade.curve;
        let mut recipe = empty_output();
        recipe
            .tool_calls
            .push(AudioToolDef::Crossfade(CrossfadeParams {
                duration_ms: 1500,
                curve: "constant_gain".to_string(),
            }));
        apply_recipe_to_config(&recipe, &mut config);
        assert_eq!(config.crossfade.max_duration_ms.get(), 1500);
        assert!(config.crossfade.enabled);
        assert_ne!(config.crossfade.curve, initial_curve);
        assert_eq!(config.crossfade.curve, CrossfadeCurve::ConstantGain);
    }

    #[test]
    fn ignora_crossfade_fora_de_limite() {
        let mut config = PipelineConfig::default();
        let original_ms = config.crossfade.max_duration_ms.get();
        let mut recipe = empty_output();
        recipe
            .tool_calls
            .push(AudioToolDef::Crossfade(CrossfadeParams {
                duration_ms: 50000, // acima de CrossfadeMs::MAX
                curve: "constant_power".to_string(),
            }));
        apply_recipe_to_config(&recipe, &mut config);
        // O newtype rejeita — valor original permanece.
        assert_eq!(config.crossfade.max_duration_ms.get(), original_ms);
    }

    #[test]
    fn aplica_lufs_e_compression() {
        let mut config = PipelineConfig::default();
        let mut recipe = empty_output();
        recipe
            .tool_calls
            .push(AudioToolDef::LufsNormalization(LufsNormalizationParams {
                target_lufs: -16.0,
                max_true_peak_db: -0.5,
            }));
        recipe
            .tool_calls
            .push(AudioToolDef::Compression(CompressionParams {
                ratio: 4.0,
                threshold_db: -14.5,
                attack_ms: 30,
                release_ms: 250,
                makeup_gain_db: 0.0,
                knee_db: 6.0,
            }));
        apply_recipe_to_config(&recipe, &mut config);
        // LufsTarget::try_from(-16.0) deve funcionar (dentro do range).
        // O teste real é que o campo foi alterado de -14 para -16.
        assert!((config.mastering.lufs_target.get() - (-16.0)).abs() < 0.01);
        assert!((config.mastering.peak_db - (-0.5)).abs() < 0.01);
        // CompressionRatio::try_from(4.0) deve funcionar.
        assert!((config.mastering.compression_ratio.get() - 4.0).abs() < 0.01);
    }

    #[test]
    fn receita_vazia_nao_altera_config() {
        let mut config = PipelineConfig::default();
        let original = config.clone();
        apply_recipe_to_config(&empty_output(), &mut config);
        // Sem tool_calls, nada muda (exceto campos derivados de clone).
        assert_eq!(
            config.mastering.lufs_target.get(),
            original.mastering.lufs_target.get()
        );
        assert_eq!(
            config.crossfade.max_duration_ms.get(),
            original.crossfade.max_duration_ms.get()
        );
    }
}
