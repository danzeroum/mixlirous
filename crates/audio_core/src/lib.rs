//! Core Domain + DSP Engine for Remix AI
//!
//! # Architecture
//! - `domain/`: Entidades de negócio (BeatBlock, EnergyProfile, SourceTrack)
//! - `dsp/`: Algoritmos de processamento de áudio (FFT, RMS, Crossfade)
//! - `io/`: Decodificação de arquivo de áudio para PCM
//! - `ports/`: Abstrações de I/O (Storage, Analyzer, Repo)

pub mod domain;
pub mod dsp;
pub mod error;
pub mod io;
pub mod ports;

pub use error::Error;

/// Re-export do `ndarray` para quem consome este crate.
///
/// Toda a API de DSP fala `Array1<f32>`. Sem isto, cada crate consumidor
/// precisaria declarar a própria dependência de `ndarray` — e uma divergência
/// de versão faria os tipos deixarem de unificar, com erro de "expected
/// `Array1`, found `Array1`". Aqui só existe uma versão possível.
pub use ndarray;

// Re-exports seletivos para conveniência do API layer
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
