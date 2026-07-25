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
    pub max_duration_ms: u32,
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
    pub lufs_target: f32,
    pub peak_db: f32,
    pub enable_limiting: bool,
    pub compression_ratio: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionConfig {
    pub min_strong_beat_percentile: f32,
    pub block_size_beats: usize,
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
                max_duration_ms: 3000,
                curve: CrossfadeCurve::ConstantPower,
            },
            mastering: MasteringConfig {
                lufs_target: -14.0,
                peak_db: -1.0,
                enable_limiting: true,
                compression_ratio: 2.0,
            },
            selection: SelectionConfig {
                min_strong_beat_percentile: 0.8,
                block_size_beats: 4,
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
