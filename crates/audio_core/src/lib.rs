//! Core Domain + DSP Engine for Remix AI
//!
//! # Architecture
//! - `domain/`: Entidades de neg├│cio (BeatBlock, EnergyProfile, SourceTrack)
//! - `dsp/`: Algoritmos de processamento de ├íudio (FFT, RMS, Crossfade)
//! - `io/`: Decodifica├º├úo de arquivo de ├íudio para PCM
//! - `ports/`: Abstra├º├Áes de I/O (Storage, Analyzer, Repo)

pub mod domain;
pub mod dsp;
pub mod error;
pub mod io;
pub mod ports;

pub use error::Error;

/// Re-export do `ndarray` para quem consome este crate.
///
/// Toda a API de DSP fala `Array1<f32>`. Sem isto, cada crate consumidor
/// precisaria declarar a pr├│pria depend├¬ncia de `ndarray` ÔÇö e uma diverg├¬ncia
/// de vers├úo faria os tipos deixarem de unificar, com erro de "expected
/// `Array1`, found `Array1`". Aqui s├│ existe uma vers├úo poss├¡vel.
pub use ndarray;

// Re-exports seletivos para conveni├¬ncia do API layer
pub use domain::{
    AttackMs, AudioCodec, AudioFingerprint, AudioFormat, BeatBlock, BeatCandidate,
    BeatDetectionParams, BlockEnergy, BlockSizeBeats, CompressionRatio, CrossfadeConfig,
    CrossfadeCurve, CrossfadeMs, EnergyProfile, EqGainDb, LufsTarget, MasteringConfig, OnsetMethod,
    OnsetStrength, Percentile, PipelineConfig, ReleaseMs, SelectionConfig, ThresholdDb,
    TimeStretchFactor,
};
pub use dsp::stitching::FadeCurve;
pub use io::{decode_to_pcm, DecodedAudio};
pub use ports::{AudioAnalyzer, AudioMixer, AudioRepo};
