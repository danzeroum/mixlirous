use crate::domain::{
    AudioFingerprint, BeatBlock, BeatCandidate, BeatDetectionParams, EnergyProfile,
};
use ndarray::Array1;

pub trait AudioAnalyzer: Send + Sync {
    fn detect_beats(&self, pcm: &Array1<f32>, params: &BeatDetectionParams) -> Vec<BeatCandidate>;
    fn build_blocks(
        &self,
        pcm: &Array1<f32>,
        beats: &[BeatCandidate],
        block_size: usize,
        sample_rate: u32,
    ) -> Vec<BeatBlock>;
    fn extract_fingerprint(&self, pcm: &Array1<f32>, sample_rate: u32) -> AudioFingerprint;
    fn analyze_energy_profile(
        &self,
        pcm: &Array1<f32>,
        frame_size: usize,
        hop_size: usize,
    ) -> EnergyProfile;
    fn extract_chroma(&self, pcm: &Array1<f32>, sample_rate: u32) -> Vec<f32>;
}
