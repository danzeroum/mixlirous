use crate::domain::{AudioFingerprint, BeatBlock, PipelineConfig};
use crate::ports::AudioMixer;
use ndarray::{s, Array1};
use std::path::Path;

pub struct DefaultMixer;

impl AudioMixer for DefaultMixer {
    fn render_stitched(
        &self,
        blocks: &[BeatBlock],
        pcm_source: &Array1<f32>,
        _config: &PipelineConfig,
    ) -> Array1<f32> {
        // Placeholder: concatena os blocos sequencialmente
        // Na prática: aplica crossfade, fades, time-stretch
        let mut output = Vec::new();
        for block in blocks {
            if block.end_sample <= pcm_source.len() && block.start_sample < block.end_sample {
                let block_pcm = pcm_source.slice(s![block.start_sample..block.end_sample]);
                output.extend_from_slice(block_pcm.as_slice().unwrap_or(&[]));
            }
        }
        Array1::from_vec(output)
    }

    fn export_wav(
        &self,
        _pcm: &Array1<f32>,
        _path: &Path,
        _config: &PipelineConfig,
    ) -> Result<(), crate::Error> {
        // Placeholder: na prática, usa hound ou symphonia para escrever WAV
        Ok(())
    }

    fn measure_similarity(
        &self,
        fingerprint_a: &AudioFingerprint,
        fingerprint_b: &AudioFingerprint,
    ) -> f32 {
        fingerprint_a.distance(fingerprint_b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn block(start: usize, end: usize) -> BeatBlock {
        BeatBlock {
            id: Uuid::new_v4(),
            start_sample: start,
            end_sample: end,
            start_time: start as f32 / 44100.0,
            end_time: end as f32 / 44100.0,
            duration: (end - start) as f32 / 44100.0,
            rms_energy: 0.1,
            spectral_centroid: 0.0,
            chroma_vector: None,
            beat_index: 0,
            score: 0.1,
        }
    }

    #[test]
    fn test_render_stitched_concatenates_blocks() {
        let pcm = Array1::from_vec((0..1000).map(|i| i as f32).collect());
        let blocks = vec![block(0, 100), block(200, 300)];
        let mixer = DefaultMixer;
        let out = mixer.render_stitched(&blocks, &pcm, &PipelineConfig::default());
        assert_eq!(out.len(), 200);
    }
}
