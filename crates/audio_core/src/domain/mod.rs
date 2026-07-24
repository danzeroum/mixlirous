pub mod beat;
pub mod block;
pub mod fingerprint;
pub mod pipeline_config;

pub use beat::{BeatCandidate, BeatDetectionParams, OnsetMethod, OnsetStrength};
pub use block::{build_beat_blocks, BeatBlock, BlockEnergy, EnergyProfile};
pub use fingerprint::AudioFingerprint;
pub use pipeline_config::{
    AudioCodec, AudioFormat, CrossfadeConfig, CrossfadeCurve, MasteringConfig, PipelineConfig,
    SelectionConfig,
};
