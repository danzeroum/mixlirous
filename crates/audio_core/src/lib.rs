//! Core Domain + DSP Engine for Remix AI
//!
//! # Architecture
//! - `domain/`: Entidades de negócio (BeatBlock, EnergyProfile, SourceTrack)
//! - `dsp/`: Algoritmos de processamento de áudio (FFT, RMS, Crossfade)
//! - `ports/`: Abstrações de I/O (Storage, Analyzer, Repo)

pub mod domain;
pub mod dsp;
pub mod error;
pub mod ports;

pub use error::Error;

// Re-exports seletivos para conveniência do API layer
pub use domain::{
    AttackMs, AudioCodec, AudioFingerprint, AudioFormat, BeatBlock, BeatCandidate,
    BeatDetectionParams, BlockEnergy, BlockSizeBeats, CompressionRatio, CrossfadeConfig,
    CrossfadeCurve, CrossfadeMs, EnergyProfile, EqGainDb, LufsTarget, MasteringConfig, OnsetMethod,
    OnsetStrength, Percentile, PipelineConfig, ReleaseMs, SelectionConfig, ThresholdDb,
    TimeStretchFactor,
};
pub use dsp::stitching::FadeCurve;
pub use ports::{AudioAnalyzer, AudioMixer, AudioRepo};
