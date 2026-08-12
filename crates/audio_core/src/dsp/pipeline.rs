//! Pipeline estruturado de remix (ADR-0012).
//!
//! Orquestra as etapas B-F da cadeia de DSP definida em `docs/04-DOMINIO-DSP.md` §2:
//!
//! ```text
//! [analyze] → [segment] → [select] → [stitch] → [master]
//!    B          C          D          E          F
//! ```
//!
//! Recebe PCM decodificado (mono) + `PipelineConfig` e devolve PCM processado.
//! O decode (A) e encode (G) ficam de fora porque dependem de I/O de arquivo.

use crate::domain::{BeatBlock, BeatDetectionParams, PipelineConfig};
use crate::dsp::analysis::DefaultAnalyzer;
use crate::dsp::selection::{select_blocks, SelectionConfig as DspSelectionConfig};
use crate::ports::AudioAnalyzer;
use ndarray::Array1;

/// Dados de entrada para o pipeline.
///
/// O PCM ja deve estar decodificado e em mono (ver `io::downmix_to_mono`).
/// O `sample_rate` e o da faixa original -- o pipeline preserva.
#[derive(Debug, Clone)]
pub struct PipelineInput {
    /// PCM mono, amostras normalizadas em [-1.0, 1.0].
    pub pcm: Array1<f32>,
    /// Taxa de amostragem do PCM (ex.: 44100).
    pub sample_rate: u32,
    /// Configuracao do pipeline (parametros de selecao, crossfade, masterizacao).
    pub config: PipelineConfig,
    /// Blocos pre-selecionados (opcional). Se presente, pula as etapas B-D e
    /// vai direto para stitch + master.
    pub pre_selected_blocks: Option<Vec<BeatBlock>>,
}

/// Resultado do pipeline com metadados e avisos.
#[derive(Debug, Clone)]
pub struct PipelineResult {
    /// PCM processado (mono).
    pub pcm: Array1<f32>,
    /// Taxa de amostragem (igual a entrada).
    pub sample_rate: u32,
    /// Blocos efetivamente selecionados e emendados.
    pub blocks_used: Vec<BeatBlock>,
    /// BPM estimado (se deteccao foi executada).
    pub bpm_estimate: Option<f32>,
    /// Avisos nao-bloqueantes para publicacao via SSE.
    /// O worker publica cada aviso como evento `job.warning`.
    pub warnings: Vec<String>,
}

impl PipelineResult {
    /// Duracao do PCM de saida em segundos.
    pub fn duration_sec(&self) -> f32 {
        if self.sample_rate == 0 {
            return 0.0;
        }
        self.pcm.len() as f32 / self.sample_rate as f32
    }
}

/// Erro do pipeline. Cada variante e urn especifica o suficiente para o
/// worker montar uma mensagem de erro legivel sem deduzir contexto.
#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("buffer vazio ou muito curto ({0} amostras)")]
    BufferTooShort(usize),
    #[error("selecao: {0}")]
    Selection(String),
    #[error("emenda: {0}")]
    Stitching(String),
    #[error("masterizacao: {0}")]
    Mastering(String),
    #[error("analise: {0}")]
    Analysis(String),
}

/// Trait que define o contrato do pipeline de remix.
///
/// A implementacao padrao e `DefaultRemixPipeline`, que orquestra as etapas
/// na ordem fixa: analise → segmentacao → selecao → emenda → masterizacao.
/// Em teste, um mock pode implementar este trait para devolver PCM constante.
///
/// # Object safety
///
/// Todos os metodos recebem `&self` e devolvem tipos owned -- trait e
/// object-safe para quem precisar de despacho dinamico.
pub trait RemixPipeline: Send + Sync {
    /// Executa o pipeline completo a partir da entrada.
    fn run(&self, input: PipelineInput) -> Result<PipelineResult, PipelineError>;
}

/// Implementacao padrao do pipeline de remix.
///
/// Orquestra as etapas na ordem fixa definida em `docs/04-DOMINIO-DSP.md` §8:
/// 1. Analise: deteccao de batidas e construcao de blocos.
/// 2. Selecao: knapsack para atingir duracao alvo.
/// 3. Emenda: crossfade entre blocos com zero-crossing e fades de borda.
/// 4. Masterizacao: LUFS + limiter (e compressor se habilitado).
///
/// Se `pre_selected_blocks` estiver presente em `PipelineInput`, pula
/// analise e selecao e usa os blocos fornecidos diretamente.
pub struct DefaultRemixPipeline;

impl DefaultRemixPipeline {
    pub fn new() -> Self {
        Self
    }

    /// Etapa B -- Analise: deteccao de batidas e construcao de blocos.
    fn analyze(
        &self,
        pcm: &Array1<f32>,
        sample_rate: u32,
        config: &PipelineConfig,
    ) -> Result<(Vec<BeatBlock>, f32), PipelineError> {
        let analyzer = DefaultAnalyzer;
        let params = BeatDetectionParams {
            sample_rate,
            ..Default::default()
        };

        // B.1 + B.2: onset strength e deteccao de batidas
        let beats = analyzer.detect_beats(pcm, &params);
        let bpm = crate::dsp::analysis::beat_tracking::estimate_bpm(
            &crate::dsp::analysis::beat_tracking::onset_strength(
                pcm,
                params.frame_size,
                params.hop_size,
            ),
            sample_rate,
            params.hop_size,
        );

        // C: segmentacao em blocos
        let block_size_beats = config.selection.block_size_beats.get();
        let blocks = analyzer.build_blocks(pcm, &beats, block_size_beats, sample_rate);

        Ok((blocks, bpm))
    }

    /// Etapa D -- Selecao: knapsack para atingir a duracao alvo.
    fn select(
        &self,
        blocks: &[BeatBlock],
        config: &PipelineConfig,
    ) -> Result<Vec<BeatBlock>, PipelineError> {
        let target_sec = config.target_duration.as_secs_f32();
        let tolerance_sec = 2.0;

        let sel_config = DspSelectionConfig {
            target_duration_sec: target_sec,
            duration_tolerance_sec: tolerance_sec,
            preserve_intro_ms: config.selection.preserve_intro_ms,
            preserve_outro_ms: config.selection.preserve_outro_ms,
            require_strong_beat_start: false,
            allow_repeats: false,
        };

        select_blocks(blocks, &sel_config).map_err(|e| PipelineError::Selection(e.to_string()))
    }

    /// Etapa E -- Emenda: crossfade entre blocos com zero-crossing e fades.
    fn stitch(
        &self,
        source_pcm: &Array1<f32>,
        blocks: &[BeatBlock],
        config: &PipelineConfig,
        sample_rate: u32,
    ) -> Result<(Vec<f32>, Vec<String>), PipelineError> {
        if blocks.is_empty() {
            // Fallback: se nenhum bloco foi selecionado, devolve o PCM original.
            // Nao e erro -- o resultado e o proprio sinal.
            return Ok((source_pcm.to_vec(), Vec::new()));
        }

        let fade_ms = 20.0f32;
        let fade_samples_alvo = ((fade_ms / 1000.0) * sample_rate as f32) as usize;
        let mut montado: Vec<f32> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();

        for (i, block) in blocks.iter().enumerate() {
            let pcm_slice = source_pcm.as_slice().unwrap_or(&[]);
            if block.start_sample >= pcm_slice.len() || block.end_sample > pcm_slice.len() {
                continue;
            }
            let trecho = &pcm_slice[block.start_sample..block.end_sample];

            if i == 0 {
                montado.extend_from_slice(trecho);
                continue;
            }

            // E.1: ajuste para zero-crossing
            let _zc =
                crate::dsp::stitching::find_zero_crossing(source_pcm, block.start_sample, 200);

            let fade_samples = fade_samples_alvo.min(montado.len()).min(trecho.len());

            if fade_samples == 0 {
                // Nao ha sobreposicao possivel -- concatena sem crossfade.
                montado.extend_from_slice(trecho);
                continue;
            }

            let start_a = montado.len() - fade_samples;
            montado.resize(start_a + trecho.len(), 0.0);

            crate::dsp::stitching::crossfade_buffers(
                &mut montado,
                start_a,
                trecho,
                0,
                fade_samples,
                config.crossfade.curve,
            );

            // E.3: politica de emenda -- avisa se a diferenca de nivel for brusca.
            // (simplificada: calcula o pico RMS do trecho e compara com o ultimo
            // bloco emendado).
            if i > 0 && i < blocks.len() {
                let prev_block = &blocks[i - 1];
                let prev_pcm = if prev_block.end_sample <= pcm_slice.len()
                    && prev_block.start_sample < prev_block.end_sample
                {
                    &pcm_slice[prev_block.start_sample..prev_block.end_sample]
                } else {
                    &[][..]
                };

                if !prev_pcm.is_empty() && !trecho.is_empty() {
                    let rms_a = crate::dsp::analysis::rms::calculate_rms(
                        ndarray::ArrayView1::from(prev_pcm),
                    );
                    let rms_b =
                        crate::dsp::analysis::rms::calculate_rms(ndarray::ArrayView1::from(trecho));
                    if rms_a > 1e-6 && rms_b > 1e-6 {
                        let diff_db = 20.0 * (rms_b / rms_a).abs().log10();
                        if diff_db.abs() > 8.0 {
                            warnings.push(format!(
                                "enenda brusca entre blocos {i} e {}: diff {:.1} dB",
                                i + 1,
                                diff_db
                            ));
                        }
                    }
                }
            }
        }

        // E.2: fades de borda
        let fade_borda_samples = (0.02 * sample_rate as f32) as usize;
        let fim = montado.len();
        crate::dsp::stitching::apply_fade_in(
            &mut montado,
            0,
            fade_borda_samples.min(fim),
            &crate::dsp::stitching::FadeCurve::Logarithmic,
        );
        crate::dsp::stitching::apply_fade_out(
            &mut montado,
            fim.saturating_sub(fade_borda_samples),
            fade_borda_samples.min(fim),
            &crate::dsp::stitching::FadeCurve::Logarithmic,
        );

        Ok((montado, warnings))
    }

    /// Etapa F -- Masterizacao: LUFS + limiter (e compressor se habilitado).
    /// Ordem fixa por `docs/04-DOMINIO-DSP.md` §8: compressor → limiter → LUFS.
    fn master(
        &self,
        pcm: &mut Vec<f32>,
        sample_rate: u32,
        config: &PipelineConfig,
        warnings: &mut Vec<String>,
    ) -> Result<(), PipelineError> {
        if pcm.is_empty() {
            return Ok(());
        }

        // F.1: compressor (se habilitado via enable_limiting)
        if config.mastering.enable_limiting {
            let params = crate::dsp::mastering::CompressorParams::default();
            let compressed = crate::dsp::mastering::apply_compression(pcm, &params, sample_rate);
            pcm.clone_from(&compressed);
        }

        // F.2: limiter brickwall
        crate::dsp::mastering::brickwall_limiter(pcm, config.mastering.peak_db);

        // F.3: normalizacao LUFS
        let target_lufs = config.mastering.lufs_target.get();
        match crate::dsp::mastering::apply_lufs_gain(pcm, sample_rate, target_lufs) {
            crate::dsp::mastering::LufsGainOutcome::Applied { gain_db } => {
                // Aviso se o ganho for > 3 dB (sinal muito comprimido na origem).
                if gain_db.abs() > 3.0 {
                    warnings.push(format!(
                        "ganho LUFS de {:.1} dB -- sinal pode estar muito comprimido",
                        gain_db
                    ));
                }
            },
            crate::dsp::mastering::LufsGainOutcome::UnmeasurableLoudness => {
                warnings.push(
                    "loudness nao mensuravel (buffer curto/silencioso) -- sem normalizacao LUFS"
                        .to_string(),
                );
            },
        }

        // F.2 novamente apos LUFS: o ganho pode ter estourado o teto.
        crate::dsp::mastering::brickwall_limiter(pcm, config.mastering.peak_db);

        Ok(())
    }
}

impl Default for DefaultRemixPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl RemixPipeline for DefaultRemixPipeline {
    fn run(&self, input: PipelineInput) -> Result<PipelineResult, PipelineError> {
        let PipelineInput {
            ref pcm,
            sample_rate,
            ref config,
            ref pre_selected_blocks,
        } = input;

        if pcm.len() < 100 {
            return Err(PipelineError::BufferTooShort(pcm.len()));
        }

        let mut warnings: Vec<String> = Vec::new();
        let mut bpm_estimate = None;

        // Etapas B-D: analise + segmentacao + selecao
        // (ou usa blocos pre-selecionados se fornecidos)
        let selected_blocks = if let Some(ref blocks) = pre_selected_blocks {
            blocks.clone()
        } else {
            let (all_blocks, bpm) = self.analyze(pcm, sample_rate, config)?;
            bpm_estimate = Some(bpm);

            if all_blocks.is_empty() {
                warnings
                    .push("nenhum bloco detectado -- usando PCM bruto como fallback".to_string());
                // Fallback: nem tenta selecionar, vai direto para master.
                Vec::new()
            } else {
                match self.select(&all_blocks, config) {
                    Ok(sel) => sel,
                    Err(e) => {
                        warnings.push(format!(
                            "selecao falhou ({}), usando todos os blocos: {e}",
                            e
                        ));
                        all_blocks
                    },
                }
            }
        };

        // Etapa E: emenda
        let (mut pcm_out, stitch_warnings) =
            self.stitch(pcm, &selected_blocks, config, sample_rate)?;
        warnings.extend(stitch_warnings);

        // Etapa F: masterizacao
        self.master(&mut pcm_out, sample_rate, config, &mut warnings)?;

        // I15: nenhuma amostra NaN ou infinita apos o pipeline.
        if pcm_out.iter().any(|s| !s.is_finite()) {
            // Limpa -- algo no caminho produziu NaN. Substitui por 0.0 e avisa.
            let count = pcm_out.iter().filter(|s| !s.is_finite()).count();
            for s in pcm_out.iter_mut() {
                if !s.is_finite() {
                    *s = 0.0;
                }
            }
            warnings.push(format!(
                "I15: {count} amostras NaN/Inf limpas apos masterizacao"
            ));
        }

        Ok(PipelineResult {
            pcm: Array1::from_vec(pcm_out),
            sample_rate,
            blocks_used: selected_blocks,
            bpm_estimate,
            warnings,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::AudioCodec;

    fn mono_config(sample_rate: u32) -> PipelineConfig {
        let mut config = PipelineConfig::default();
        config.format.sample_rate = sample_rate;
        config.format.channels = 1;
        config.format.bit_depth = 32;
        config.format.codec = AudioCodec::WAV;
        config
    }

    #[test]
    fn pipeline_rejeita_buffer_vazio() {
        let pipeline = DefaultRemixPipeline::new();
        let input = PipelineInput {
            pcm: Array1::from_vec(Vec::new()),
            sample_rate: 44100,
            config: PipelineConfig::default(),
            pre_selected_blocks: None,
        };
        let err = pipeline.run(input).unwrap_err();
        assert!(matches!(err, PipelineError::BufferTooShort(0)));
    }

    #[test]
    fn pipeline_rejeita_buffer_curto() {
        let pipeline = DefaultRemixPipeline::new();
        let input = PipelineInput {
            pcm: Array1::from_vec(vec![0.0f32; 50]),
            sample_rate: 44100,
            config: PipelineConfig::default(),
            pre_selected_blocks: None,
        };
        let err = pipeline.run(input).unwrap_err();
        assert!(matches!(err, PipelineError::BufferTooShort(50)));
    }

    #[test]
    fn pipeline_produz_saida_finita() {
        // 1 segundo de seno 440 Hz a 44100 Hz.
        let pcm: Vec<f32> = (0..44100)
            .map(|i| (i as f32 / 44100.0 * 440.0 * std::f32::consts::TAU).sin())
            .collect();
        let pipeline = DefaultRemixPipeline::new();
        let input = PipelineInput {
            pcm: Array1::from_vec(pcm),
            sample_rate: 44100,
            config: mono_config(44100),
            pre_selected_blocks: None,
        };
        let result = pipeline.run(input).unwrap();
        assert!(
            result.pcm.iter().all(|s| s.is_finite()),
            "I15: pipeline produziu amostra nao finita"
        );
    }

    #[test]
    fn pipeline_com_blocos_pre_selecionados_pula_analise() {
        // Silencio puro -- sem batidas. Com blocos pre-selecionados,
        // o pipeline deve funcionar sem erro.
        let pcm = vec![0.1f32; 44100];
        let blocks = vec![BeatBlock {
            id: uuid::Uuid::new_v4(),
            start_sample: 0,
            end_sample: 44100,
            start_time: 0.0,
            end_time: 1.0,
            duration: 1.0,
            rms_energy: 0.1,
            spectral_centroid: 1000.0,
            chroma_vector: None,
            beat_index: 0,
            score: 0.5,
        }];

        let pipeline = DefaultRemixPipeline::new();
        let input = PipelineInput {
            pcm: Array1::from_vec(pcm),
            sample_rate: 44100,
            config: mono_config(44100),
            pre_selected_blocks: Some(blocks),
        };
        let result = pipeline.run(input).unwrap();
        assert!(!result.pcm.is_empty());
        // Nao deveria ter BPM estimate porque a analise foi pulada.
        assert!(result.bpm_estimate.is_none());
    }

    #[test]
    fn pipeline_silencio_fallback_sem_panic() {
        // Silencio puro -- nao deve entrar em panic nem retornar erro.
        let pipeline = DefaultRemixPipeline::new();
        let input = PipelineInput {
            pcm: Array1::from_vec(vec![0.0f32; 44100]),
            sample_rate: 44100,
            config: mono_config(44100),
            pre_selected_blocks: None,
        };
        let result = pipeline.run(input).unwrap();
        assert!(!result.pcm.is_empty(), "fallback nao deve ser vazio");
        assert!(
            result.pcm.iter().all(|s| s.is_finite()),
            "I15: silencio produziu NaN"
        );
    }

    #[test]
    fn pipeline_duration_sec_retorna_duracao_correta() {
        let pcm = vec![0.1f32; 22050]; // 0.5s a 44100 Hz
        let block = BeatBlock {
            id: uuid::Uuid::new_v4(),
            start_sample: 0,
            end_sample: 22050,
            start_time: 0.0,
            end_time: 0.5,
            duration: 0.5,
            rms_energy: 0.1,
            spectral_centroid: 1000.0,
            chroma_vector: None,
            beat_index: 0,
            score: 0.5,
        };
        let pipeline = DefaultRemixPipeline::new();
        let input = PipelineInput {
            pcm: Array1::from_vec(pcm),
            sample_rate: 44100,
            config: mono_config(44100),
            pre_selected_blocks: Some(vec![block]),
        };
        let result = pipeline.run(input).unwrap();
        // Com 1 bloco e sem emenda, a duracao deve ser ~0.5s.
        assert!((result.duration_sec() - 0.5).abs() < 0.05);
    }
}
