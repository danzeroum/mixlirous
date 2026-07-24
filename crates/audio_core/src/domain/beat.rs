use ndarray::Array1;
use realfft::RealFftPlanner;

/// Resultado da análise de onset strength em cada frame hop
#[derive(Debug, Clone)]
pub struct OnsetStrength {
    pub strengths: Vec<f32>,
    pub hop_size: usize,
    pub frame_size: usize,
    pub sample_rate: u32,
}

/// Representa um candidato a batida detectado no áudio
#[derive(Debug, Clone, PartialEq)]
pub struct BeatCandidate {
    pub sample_idx: usize,
    pub onset_strength: f32,
    pub time_sec: f32,
    pub rms_energy: f32,
}

/// Parâmetros de configuração para detecção de batidas
#[derive(Debug, Clone)]
pub struct BeatDetectionParams {
    pub min_bpm: u32,
    pub max_bpm: u32,
    pub frame_size: usize,
    pub hop_size: usize,
    pub onset_method: OnsetMethod,
    pub sample_rate: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OnsetMethod {
    SpectralFlux,
    Energy,
    Complex,
}

impl Default for BeatDetectionParams {
    fn default() -> Self {
        Self {
            min_bpm: 60,
            max_bpm: 180,
            frame_size: 2048,
            hop_size: 512,
            onset_method: OnsetMethod::SpectralFlux,
            sample_rate: 44100,
        }
    }
}

/// Extração de onset strength via fluxo espectral (Spectral Flux)
pub fn extract_onset_strength(pcm: &Array1<f32>, params: &BeatDetectionParams) -> OnsetStrength {
    if pcm.len() < params.frame_size {
        return OnsetStrength {
            strengths: Vec::new(),
            hop_size: params.hop_size,
            frame_size: params.frame_size,
            sample_rate: params.sample_rate,
        };
    }

    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(params.frame_size);

    let mut strengths = Vec::new();
    let hann = make_hann_window(params.frame_size);

    let bins = params.frame_size / 2 + 1;
    let mut prev_spectrum = vec![0.0f32; bins];
    let mut input = fft.make_input_vec();
    let mut output = fft.make_output_vec();

    for start in (0..=(pcm.len() - params.frame_size)).step_by(params.hop_size) {
        for i in 0..params.frame_size {
            input[i] = pcm[start + i] * hann[i];
        }

        if fft.process(&mut input, &mut output).is_err() {
            strengths.push(0.0);
            continue;
        }

        // Spectral flux: diferença positiva entre frames consecutivos
        let flux: f32 = output
            .iter()
            .zip(&prev_spectrum)
            .map(|(c, &p)| (c.norm() - p).max(0.0))
            .sum();

        strengths.push(flux);

        for (slot, c) in prev_spectrum.iter_mut().zip(output.iter()) {
            *slot = c.norm();
        }
    }

    OnsetStrength {
        strengths,
        hop_size: params.hop_size,
        frame_size: params.frame_size,
        sample_rate: params.sample_rate,
    }
}

/// Cria uma janela Hann pré-calculada
fn make_hann_window(size: usize) -> Vec<f32> {
    (0..size)
        .map(|i| {
            let x = (i as f64) * std::f64::consts::PI / (size - 1) as f64;
            (1.0 - x.cos()) as f32 * 0.5f32 // 0.5 * (1 - cos(x))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array1;

    #[test]
    fn test_onset_strength_returns_positive_values() {
        let pcm = Array1::from_vec(vec![0.0f32; 44100]); // 1s de silêncio
        let params = BeatDetectionParams {
            sample_rate: 44100,
            ..Default::default()
        };

        let onset = extract_onset_strength(&pcm, &params);
        // Silêncio deve gerar flux próximo de zero
        assert!(onset.strengths.iter().all(|&v| v < 1e-5));
    }
}
