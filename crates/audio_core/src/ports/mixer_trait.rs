use crate::domain::{AudioFingerprint, BeatBlock, PipelineConfig};
use ndarray::Array1;
use std::path::Path;

pub trait AudioMixer: Send + Sync {
    fn render_stitched(
        &self,
        blocks: &[BeatBlock],
        pcm_source: &Array1<f32>,
        config: &PipelineConfig,
    ) -> Array1<f32>;

    fn export_wav(
        &self,
        pcm: &Array1<f32>,
        path: &Path,
        config: &PipelineConfig,
    ) -> Result<(), crate::Error>;
    fn measure_similarity(
        &self,
        fingerprint_a: &AudioFingerprint,
        fingerprint_b: &AudioFingerprint,
    ) -> f32;
}
