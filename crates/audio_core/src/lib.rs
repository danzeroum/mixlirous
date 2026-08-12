//! Core Domain + DSP Engine for Remix AI
//!
//! # Architecture
//! - `domain/`: Entidades de negocio (BeatBlock, EnergyProfile, SourceTrack)
//! - `dsp/`: Algoritmos de processamento de audio (FFT, RMS, Crossfade, Pipeline)
//! - `io/`: Decodificacao de arquivo de audio para PCM
//! - `ports/`: Abstracoes de I/O (Storage, Analyzer, Repo)

pub mod domain;
pub mod dsp;
pub mod error;
pub mod io;
pub mod ports;

pub use error::Error;

/// Re-export do `ndarray` para quem consome este crate.
///
/// Toda a API de DSP fala `Array1<f32>`. Sem isto, cada crate consumidor
/// precisaria declarar a propria dependencia de `ndarray` -- e uma divergencia
/// de versao faria os tipos deixarem de unificar, com erro de "expected
/// `Array1`, found `Array1`". Aqui so existe uma versao possivel.
pub use ndarray;

// Re-exports seletivos para conveniencia do API layer
pub use domain::{
    AttackMs, AudioCodec, AudioFingerprint, AudioFormat, BeatBlock, BeatCandidate,
    BeatDetectionParams, BlockEnergy, BlockSizeBeats, CompressionRatio, CrossfadeConfig,
    CrossfadeCurve, CrossfadeMs, EnergyProfile, EqGainDb, LufsTarget, MasteringConfig,
    MaxCorrectionCents, MinConfidence, OnsetMethod, OnsetStrength, Percentile, PipelineConfig,
    ReleaseMs, SelectionConfig, ThresholdDb, TimeStretchFactor, TuningConfig, TuningMode,
};
pub use dsp::pipeline::{DefaultRemixPipeline, PipelineInput, PipelineResult, RemixPipeline};
pub use dsp::post_process::{check_invariants, post_process, PostProcessReport};
pub use dsp::stitching::FadeCurve;
pub use dsp::{
    aggregate_chroma, compute_quality_report, detect_drift, detect_key, detect_pitch, KeyMode,
    PitchFrame, QualityReport, TonalContext,
};
pub use io::{decode_to_pcm, downmix_to_mono, DecodedAudio};
pub use ports::{AudioAnalyzer, AudioMixer, AudioRepo};
