use ndarray::{s, Array1};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Um bloco at├┤mico de ├íudio alinhado a batidas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeatBlock {
    pub id: Uuid,
    pub start_sample: usize,
    pub end_sample: usize,
    pub start_time: f32,
    pub end_time: f32,
    pub duration: f32, // em segundos
    pub rms_energy: f32,
    pub spectral_centroid: f32,
    pub chroma_vector: Option<Vec<f32>>, // 12 classes de pitch
    pub beat_index: usize,               // Posi├º├úo na grade de batidas
    pub score: f32,                      // Score de sele├º├úo (energia ├ù prioridade)
}

/// Perfil de energia para decis├úo heur├¡stica de sele├º├úo
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyProfile {
    pub rms_mean: f32,
    pub rms_std: f32,
    pub peak_db: f32,
    pub dynamic_range: f32, // RMS max - RMS min
    pub blocks: Vec<BlockEnergy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockEnergy {
    pub block_idx: usize,
    pub rms: f32,
    pub is_strong_beat: bool,
    pub percentile: f32, // P80, P90, etc.
}

/// Constr├│i a grade de BeatBlocks a partir de candidatos de batida
pub fn build_beat_blocks(
    pcm: &Array1<f32>,
    beat_candidates: &[crate::domain::beat::BeatCandidate],
    block_size_beats: usize,
    sample_rate: u32,
) -> Vec<BeatBlock> {
    if block_size_beats == 0 || beat_candidates.len() < block_size_beats {
        return Vec::new();
    }

    let mut blocks = Vec::new();
    let window = block_size_beats;

    // Agrupa batidas em blocos de N batidas
    for i in (0..=(beat_candidates.len() - window)).step_by(window) {
        let start_cand = &beat_candidates[i];
        let end_cand = &beat_candidates[i + window - 1];

        let start_sample = start_cand.sample_idx;
        let end_sample = end_cand.sample_idx;

        if end_sample <= start_sample || end_sample > pcm.len() {
            continue;
        }

        let block_pcm = pcm.slice(s![start_sample..end_sample]);
        let rms = crate::dsp::analysis::rms::calculate_rms(block_pcm);
        let spectral_centroid =
            crate::dsp::analysis::fft::spectral_centroid(block_pcm, sample_rate);

        blocks.push(BeatBlock {
            id: Uuid::new_v4(),
            start_sample,
            end_sample,
            start_time: start_cand.time_sec,
            end_time: end_cand.time_sec,
            duration: end_cand.time_sec - start_cand.time_sec,
            rms_energy: rms,
            spectral_centroid,
            chroma_vector: None, // Ser├í preenchido depois via AudioAnalyzer::extract_chroma
            beat_index: i / window,
            score: rms, // Pode ser refinado com heur├¡sticas
        });
    }

    blocks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::beat::BeatCandidate;

    fn candidate(sample_idx: usize, time_sec: f32) -> BeatCandidate {
        BeatCandidate {
            sample_idx,
            onset_strength: 0.5,
            time_sec,
            rms_energy: 0.1,
        }
    }

    #[test]
    fn test_build_beat_blocks_groups_by_window() {
        let pcm = Array1::from_vec(vec![0.1f32; 44100]);
        let candidates: Vec<BeatCandidate> = (0..8)
            .map(|i| candidate(i * 5000, i as f32 * 0.1))
            .collect();

        let blocks = build_beat_blocks(&pcm, &candidates, 4, 44100);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].beat_index, 0);
        assert_eq!(blocks[1].beat_index, 1);
    }

    #[test]
    fn test_build_beat_blocks_empty_when_not_enough_candidates() {
        let pcm = Array1::from_vec(vec![0.1f32; 44100]);
        let candidates = vec![candidate(0, 0.0), candidate(1000, 0.1)];
        assert!(build_beat_blocks(&pcm, &candidates, 4, 44100).is_empty());
    }
}
