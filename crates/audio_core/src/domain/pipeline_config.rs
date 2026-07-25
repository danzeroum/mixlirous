use crate::domain::{BlockSizeBeats, CompressionRatio, CrossfadeMs, LufsTarget, Percentile};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    pub target_duration: Duration,
    pub crossfade: CrossfadeConfig,
    pub mastering: MasteringConfig,
    pub selection: SelectionConfig,
    pub format: AudioFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossfadeConfig {
    pub enabled: bool,
    /// Newtype validado na desserialização (T0.0, I14) — um `PipelineConfig`
    /// recuperado do banco depois de um crash não pode reconstruir um
    /// crossfade fora da faixa 0–3000 ms; o erro acontece aqui, não seis
    /// passos depois num DSP que assume o valor já é válido.
    pub max_duration_ms: CrossfadeMs,
    pub curve: CrossfadeCurve,
}

/// Curva de crossfade: dois sinais somando durante a sobreposição.
/// Distinta de `FadeCurve` (`dsp::stitching::fades`), que rege fade de
/// entrada/saída — um sinal só, indo de/para o silêncio. Ver
/// `docs/03-ADENDO-R2-CONTRATOS.md` §0.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossfadeCurve {
    /// gain_a + gain_b = 1. Material correlacionado.
    ConstantGain,
    /// gain_a² + gain_b² = 1. Padrão — blocos de origens diferentes.
    ConstantPower,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasteringConfig {
    /// Newtype validado na desserialização (T0.0, I14) — ver
    /// `crate::LufsTarget`.
    pub lufs_target: LufsTarget,
    pub peak_db: f32,
    pub enable_limiting: bool,
    /// Newtype validado na desserialização (T0.0, I14) — ver
    /// `crate::CompressionRatio`.
    pub compression_ratio: CompressionRatio,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionConfig {
    /// Newtype validado na desserialização (T0.0, I14) — ver
    /// `crate::Percentile`.
    pub min_strong_beat_percentile: Percentile,
    /// Newtype validado na desserialização (T0.0, I14) — ver
    /// `crate::BlockSizeBeats`.
    pub block_size_beats: BlockSizeBeats,
    pub preserve_intro_ms: u32,
    pub preserve_outro_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub bit_depth: u8,
    pub codec: AudioCodec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AudioCodec {
    WAV,
    MP3,
    AAC,
    FLAC,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            target_duration: Duration::from_secs(30),
            crossfade: CrossfadeConfig {
                enabled: true,
                max_duration_ms: CrossfadeMs::try_from(3000)
                    .expect("3000 está dentro de CrossfadeMs::MIN..=MAX por construção"),
                curve: CrossfadeCurve::ConstantPower,
            },
            mastering: MasteringConfig {
                lufs_target: LufsTarget::try_from(-14.0)
                    .expect("-14.0 está dentro de LufsTarget::MIN..=MAX por construção"),
                peak_db: -1.0,
                enable_limiting: true,
                compression_ratio: CompressionRatio::try_from(2.0)
                    .expect("2.0 está dentro de CompressionRatio::MIN..=MAX por construção"),
            },
            selection: SelectionConfig {
                min_strong_beat_percentile: Percentile::try_from(0.8)
                    .expect("0.8 está dentro de Percentile::MIN..=MAX por construção"),
                block_size_beats: BlockSizeBeats::try_from(4)
                    .expect("4 está dentro de BlockSizeBeats::MIN..=MAX por construção"),
                preserve_intro_ms: 3000,
                preserve_outro_ms: 3000,
            },
            format: AudioFormat {
                sample_rate: 44100,
                channels: 2,
                bit_depth: 24,
                codec: AudioCodec::WAV,
            },
        }
    }
}
