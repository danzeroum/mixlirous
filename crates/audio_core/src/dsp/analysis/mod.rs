pub mod beat_tracking;
pub mod chroma;
pub mod fft;
pub mod key_detection;
pub mod pitch_detect;
pub mod quality_metrics;
pub mod rms;
pub mod test_fixtures;

pub use beat_tracking::DefaultAnalyzer;
pub use chroma::*;
pub use fft::*;
pub use key_detection::{
    aggregate_chroma, aggregate_chroma_simple, detect_key, KeyMode, TonalContext,
};
pub use pitch_detect::{detect_drift, detect_pitch, PitchFrame};
pub use quality_metrics::{compute_quality_report, QualityReport};
pub use rms::*;
