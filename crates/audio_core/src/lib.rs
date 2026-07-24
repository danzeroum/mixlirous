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
    AudioCodec, AudioFingerprint, AudioFormat, BeatBlock, BeatCandidate, BeatDetectionParams,
    BlockEnergy, CrossfadeConfig, CrossfadeCurve, EnergyProfile, MasteringConfig, OnsetMethod,
    OnsetStrength, PipelineConfig, SelectionConfig,
};
pub use dsp::stitching::FadeCurve;
pub use ports::{AudioAnalyzer, AudioMixer, AudioRepo};
