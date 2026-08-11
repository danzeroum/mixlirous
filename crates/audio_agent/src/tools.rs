use serde::{Deserialize, Serialize};

/// Todas as ferramentas que o LLM pode invocar
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AudioToolDef {
    #[serde(rename = "stem_separation")]
    StemSeparation(StemSeparationParams),

    #[serde(rename = "compression")]
    Compression(CompressionParams),

    #[serde(rename = "dynamic_eq")]
    DynamicEq(DynamicEqParams),

    #[serde(rename = "crossfade")]
    Crossfade(CrossfadeParams),

    #[serde(rename = "time_stretch")]
    TimeStretch(TimeStretchParams),

    #[serde(rename = "lufs_normalization")]
    LufsNormalization(LufsNormalizationParams),

    #[serde(rename = "fade_in")]
    FadeIn(FadeParams),

    #[serde(rename = "fade_out")]
    FadeOut(FadeParams),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StemSeparationParams {
    pub model: String,
    pub stems: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionParams {
    pub ratio: f32,
    pub threshold_db: f32,
    pub attack_ms: u32,
    pub release_ms: u32,
    pub makeup_gain_db: f32,
    pub knee_db: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicEqParams {
    pub bands: Vec<EqBand>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqBand {
    pub freq_hz: f32,
    pub gain_db: f32,
    pub q: f32,
    pub type_filter: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossfadeParams {
    pub duration_ms: u32,
    /// "constant_power" | "constant_gain" ÔÇö ver `docs/03-ADENDO-R2-CONTRATOS.md`
    /// ┬º0. Vocabul├írio distinto de `FadeParams.curve`: aqui dois sinais somam,
    /// l├í um sinal vai de/para o sil├¬ncio.
    pub curve: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeStretchParams {
    pub factor: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LufsNormalizationParams {
    pub target_lufs: f32,
    pub max_true_peak_db: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FadeParams {
    pub duration_ms: u32,
    /// "linear" | "logarithmic" | "exponential" ÔÇö percep├º├úo de volume, n├úo
    /// soma de dois sinais. Ver `docs/03-ADENDO-R2-CONTRATOS.md` ┬º0.
    pub curve: String,
}
