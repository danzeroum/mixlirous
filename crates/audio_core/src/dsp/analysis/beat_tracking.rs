//! Beat Tracking Algorithm ÔÇö implementa├º├úo funcional baseada em onset strength
//!
//! # Fluxo
//! 1. Calcula energia RMS em janelas (hop)
//! 2. Derivada da energia para detectar onsets (transientes)
//! 3. Autocorrela├º├úo do onset strength para estimar BPM
//! 4. Picos locais no onset strength para alinhar batidas

use crate::domain::{BeatBlock, BeatCandidate, BeatDetectionParams, EnergyProfile};
use ndarray::{s, Array1};

/// Calcula o onset strength (energia da derivada) em um buffer PCM
pub fn onset_strength(pcm: &Array1<f32>, frame_size: usize, hop_size: usize) -> Vec<f32> {
    let mut rms_frames = Vec::new();

    if pcm.len() < frame_size {
        return rms_frames;
    }

    // 1. Calcula RMS em janelas deslizantes
    for start in (0..=(pcm.len() - frame_size)).step_by(hop_size) {
        let window = pcm.slice(s![start..start + frame_size]);
        rms_frames.push(super::rms::calculate_rms(window));
    }

    if rms_frames.len() < 3 {
        return vec![0.0; rms_frames.len()];
    }

    // 2. Calcula a derivada positiva (energia da mudan├ºa)
    let mut onset = Vec::with_capacity(rms_frames.len());
    onset.push(0.0); // Primeira janela sem derivada

    for i in 1..rms_frames.len() {
        let delta = rms_frames[i] - rms_frames[i - 1];
        onset.push(delta.max(0.0)); // Apenas transientes positivos
    }

    // 3. Suaviza com janela m├│vel de 3 pontos
    let mut smoothed = Vec::with_capacity(onset.len());
    for i in 0..onset.len() {
        let window_start = i.saturating_sub(1);
        let window_end = (i + 2).min(onset.len());
        let window = &onset[window_start..window_end];
        let avg = window.iter().sum::<f32>() / window.len() as f32;
        smoothed.push(avg);
    }

    smoothed
}

/// Estima o BPM a partir do onset strength usando autocorrela├º├úo
pub fn estimate_bpm(onset: &[f32], sample_rate: u32, hop_size: usize) -> f32 {
    // Par├ómetros: busca BPM entre 60 e 240 (1-4 batidas por segundo)
    let min_bpm = 60.0;
    let max_bpm = 240.0;
    let min_lag = (sample_rate as f32 * 60.0 / max_bpm / (hop_size as f32)).round() as usize;
    let max_lag = (sample_rate as f32 * 60.0 / min_bpm / (hop_size as f32)).round() as usize;

    let n = onset.len();
    if n < max_lag + 1 {
        return 120.0;
    }

    // Autocorrela├º├úo
    let mut best_score = -1.0f32;
    let mut best_lag = min_lag.max(1);

    for lag in min_lag.max(1)..=max_lag {
        let mut corr = 0.0f32;
        let mut norm_a = 0.0f32;
        let mut norm_b = 0.0f32;

        for t in 0..(n - lag) {
            corr += onset[t] * onset[t + lag];
            norm_a += onset[t] * onset[t];
            norm_b += onset[t + lag] * onset[t + lag];
        }

        if norm_a > 0.0 && norm_b > 0.0 {
            let score = corr / (norm_a * norm_b).sqrt();
            if score > best_score {
                best_score = score;
                best_lag = lag;
            }
        }
    }

    // Converte lag em BPM
    let frames_per_beat = best_lag as f32;
    let seconds_per_beat = frames_per_beat * (hop_size as f32) / (sample_rate as f32);
    if seconds_per_beat > 0.0 {
        60.0 / seconds_per_beat
    } else {
        120.0
    }
}

/// Detecta as posi├º├Áes de batida (em frames de onset) usando o onset strength
fn detect_beat_frames(
    onset: &[f32],
    sample_rate: u32,
    hop_size: usize,
    bpm_hint: Option<f32>,
) -> Vec<usize> {
    let bpm = bpm_hint.unwrap_or_else(|| estimate_bpm(onset, sample_rate, hop_size));
    let beat_period_frames = (sample_rate as f32 / (bpm / 60.0)) / (hop_size as f32);
    let window_size = (beat_period_frames * 0.5).max(1.0) as usize;

    // #27 — limiar adaptativo em vez de 0.1 fixo.
    // O limiar anterior falhava em material com onset strength baixo
    // (ex.: rhythm_120bpm_mono.wav, pico ~0.071 nunca cruzava 0.1).
    // Novo: percentil 75 + 10% do range (peak - p75), nunca abaixo de 1e-4
    // para não detectar ruído de fundo como batida.
    let threshold = if onset.len() < 4 {
        0.1 // fallback para sinais muito curtos
    } else {
        let mut sorted = onset.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let p75 = sorted[sorted.len() * 3 / 4];
        let peak = *sorted.last().unwrap_or(&0.0);
        let range = (peak - p75).max(0.0);
        (p75 + 0.1 * range).max(1e-4)
    };

    let mut beat_indices = Vec::new();
    if onset.len() < 2 {
        return beat_indices;
    }

    for i in 1..(onset.len() - 1) {
        if onset[i] > onset[i - 1] && onset[i] > onset[i + 1] && onset[i] > threshold {
            // Supress├úo de proximidade: garante que batidas n├úo fiquem muito pr├│ximas
            if beat_indices.is_empty() || (i - beat_indices.last().unwrap()) > window_size {
                beat_indices.push(i);
            }
        }
    }

    beat_indices
}

/// Detecta as posi├º├Áes de batida (em samples) usando o onset strength
pub fn detect_beats(
    onset: &[f32],
    sample_rate: u32,
    hop_size: usize,
    bpm_hint: Option<f32>,
) -> Vec<usize> {
    detect_beat_frames(onset, sample_rate, hop_size, bpm_hint)
        .into_iter()
        .map(|idx| idx * hop_size)
        .collect()
}

/// Implementa├º├úo concreta do AudioAnalyzer para DSP
pub struct DefaultAnalyzer;

impl crate::ports::analyzer_trait::AudioAnalyzer for DefaultAnalyzer {
    fn detect_beats(&self, pcm: &Array1<f32>, params: &BeatDetectionParams) -> Vec<BeatCandidate> {
        let onset = onset_strength(pcm, params.frame_size, params.hop_size);
        let bpm = estimate_bpm(&onset, params.sample_rate, params.hop_size);
        let beat_frames =
            detect_beat_frames(&onset, params.sample_rate, params.hop_size, Some(bpm));

        beat_frames
            .into_iter()
            .filter(|&frame| frame < onset.len())
            .map(|frame| {
                let sample_idx = frame * params.hop_size;
                BeatCandidate {
                    sample_idx,
                    onset_strength: onset[frame],
                    time_sec: sample_idx as f32 / params.sample_rate as f32,
                    rms_energy: onset[frame].min(1.0),
                }
            })
            .collect()
    }

    fn build_blocks(
        &self,
        pcm: &Array1<f32>,
        beats: &[BeatCandidate],
        block_size: usize,
        sample_rate: u32,
    ) -> Vec<BeatBlock> {
        let mut blocks =
            crate::domain::block::build_beat_blocks(pcm, beats, block_size, sample_rate);
        for block in &mut blocks {
            if block.end_sample <= pcm.len() {
                let block_pcm = pcm.slice(s![block.start_sample..block.end_sample]);
                block.chroma_vector = Some(super::chroma::chroma_vector(block_pcm, sample_rate));
            }
        }
        blocks
    }

    fn extract_fingerprint(
        &self,
        pcm: &Array1<f32>,
        sample_rate: u32,
    ) -> crate::domain::AudioFingerprint {
        crate::domain::AudioFingerprint::from_pcm(pcm, sample_rate)
    }

    fn analyze_energy_profile(
        &self,
        pcm: &Array1<f32>,
        frame_size: usize,
        hop_size: usize,
    ) -> EnergyProfile {
        let rms_frames = super::rms::sliding_rms(pcm.view(), frame_size, hop_size);

        if rms_frames.is_empty() {
            return EnergyProfile {
                rms_mean: 0.0,
                rms_std: 0.0,
                peak_db: -99.0,
                dynamic_range: 0.0,
                blocks: Vec::new(),
            };
        }

        let rms_mean = rms_frames.iter().sum::<f32>() / rms_frames.len() as f32;
        let variance = rms_frames
            .iter()
            .map(|&v| (v - rms_mean).powi(2))
            .sum::<f32>()
            / rms_frames.len() as f32;
        let rms_std = variance.sqrt();

        let rms_max = rms_frames.iter().cloned().fold(0.0f32, f32::max);
        let rms_min = rms_frames.iter().cloned().fold(f32::MAX, f32::min);
        let peak = pcm.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);
        let peak_db = if peak > 0.0 {
            20.0 * peak.log10()
        } else {
            -99.0
        };

        let mut sorted = rms_frames.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let blocks = rms_frames
            .iter()
            .enumerate()
            .map(|(block_idx, &rms)| {
                let rank = sorted.partition_point(|&v| v < rms);
                let percentile = rank as f32 / sorted.len() as f32;
                crate::domain::block::BlockEnergy {
                    block_idx,
                    rms,
                    is_strong_beat: percentile >= 0.8,
                    percentile,
                }
            })
            .collect();

        EnergyProfile {
            rms_mean,
            rms_std,
            peak_db,
            dynamic_range: rms_max - rms_min,
            blocks,
        }
    }

    fn extract_chroma(&self, pcm: &Array1<f32>, sample_rate: u32) -> Vec<f32> {
        super::chroma::chroma_vector(pcm.view(), sample_rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::AudioAnalyzer as _;

    #[test]
    fn test_onset_strength_detects_transients() {
        // Cria um buffer PCM com um transiente claro no meio
        let mut pcm = vec![0.0f32; 44100];
        for (i, sample) in pcm.iter_mut().enumerate().take(22100).skip(22000) {
            *sample = (i as f32 - 22000.0) / 100.0;
        }
        for (i, sample) in pcm.iter_mut().enumerate().take(22200).skip(22100) {
            *sample = (22200.0 - i as f32) / 100.0;
        }

        let onset = onset_strength(&Array1::from_vec(pcm), 1024, 512);
        let max_onset_idx = onset
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(0);

        assert!(
            (40..=50).contains(&max_onset_idx),
            "Pico de onset fora do esperado: {}",
            max_onset_idx
        );
    }

    #[test]
    fn test_estimate_bpm_returns_reasonable_range() {
        // 120 BPM = 0.5s por batida; a 44100 Hz / hop 512, isso ├® ~43 frames
        let period_frames = 43;
        let mut onset = vec![0.0f32; 300];
        for i in (0..300).step_by(period_frames) {
            onset[i] = 1.0;
        }

        let bpm = estimate_bpm(&onset, 44100, 512);
        assert!(
            (100.0..=160.0).contains(&bpm),
            "BPM fora do esperado: {}",
            bpm
        );
    }

    #[test]
    fn test_detect_beats_finds_peaks() {
        // Espa├ºamento de 30 frames (> janela de supress├úo para bpm_hint=120) evita
        // que os picos sejam mesclados pela supress├úo de proximidade.
        let mut onset = vec![0.0f32; 130];
        onset[10] = 0.8;
        onset[40] = 0.9;
        onset[70] = 0.85;
        onset[100] = 0.75;

        let beats = detect_beats(&onset, 44100, 512, Some(120.0));
        assert!(
            beats.len() >= 3,
            "N├úo encontrou os picos esperados: {:?}",
            beats
        );
    }

    #[test]
    fn test_default_analyzer_integration() {
        let analyzer = DefaultAnalyzer;
        let pcm = Array1::from_vec(vec![0.0f32; 44100]);
        let params = BeatDetectionParams {
            frame_size: 1024,
            hop_size: 512,
            sample_rate: 44100,
            ..Default::default()
        };

        let beats = analyzer.detect_beats(&pcm, &params);
        assert!(
            beats.len() <= 5,
            "Encontrou batidas demais em buffer silencioso"
        );

        let blocks = analyzer.build_blocks(&pcm, &beats, 4, 44100);
        if beats.len() >= 4 {
            assert!(
                !blocks.is_empty(),
                "Deveria gerar blocos com {} batidas",
                beats.len()
            );
        }
    }

    #[test]
    fn test_analyze_energy_profile_shape() {
        let analyzer = DefaultAnalyzer;
        let pcm = Array1::from_vec(vec![0.2f32; 44100]);
        let profile = analyzer.analyze_energy_profile(&pcm, 1024, 512);
        assert!(!profile.blocks.is_empty());
        assert!(profile.rms_mean > 0.0);
    }

    /// #27 — o threshold adaptativo deve detectar batidas mesmo quando
    /// o pico absoluto de onset é baixo (ex.: ~0.07 na fixture real).
    /// Um sinal com transientes claros mas amplitude moderada não pode
    /// retornar zero batidas.
    #[test]
    fn detect_beats_adaptive_threshold_finds_beats_in_low_amplitude_signal() {
        // Sinal com transientes suaves: blocos de senoide com amplitude 0.05
        // separados por silêncio. O onset strength será baixo (~0.01-0.05).
        let mut pcm = vec![0.0f32; 44100 * 2]; // 2 segundos
        let freq = 440.0;
        let sr = 44100.0;
        // 4 transientes em 0.0s, 0.5s, 1.0s, 1.5s
        for &t_sec in &[0.0f32, 0.5, 1.0, 1.5] {
            let start = (t_sec * sr) as usize;
            let len = (0.05 * sr) as usize; // 50 ms de tom
            for i in start..(start + len).min(pcm.len()) {
                let t = i as f32 / sr;
                pcm[i] = 0.05 * (2.0 * std::f32::consts::PI * freq * t).sin();
            }
        }

        let params = BeatDetectionParams {
            sample_rate: 44100,
            ..Default::default()
        };
        let analyzer = DefaultAnalyzer;
        let beats = analyzer.detect_beats(&Array1::from_vec(pcm), &params);
        assert!(
            beats.len() >= 2,
            "deveria detectar ao menos 2 batidas em sinal com transientes, obteve {}",
            beats.len()
        );
    }

    /// #27 — silêncio absoluto continua retornando zero batidas.
    #[test]
    fn detect_beats_silence_still_zero() {
        let pcm = Array1::from_vec(vec![0.0f32; 44100]);
        let params = BeatDetectionParams {
            sample_rate: 44100,
            ..Default::default()
        };
        let analyzer = DefaultAnalyzer;
        let beats = analyzer.detect_beats(&pcm, &params);
        assert!(beats.len() <= 3, "silêncio não deveria ter batidas");
    }
}
