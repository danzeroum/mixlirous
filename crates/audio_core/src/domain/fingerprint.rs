use ndarray::Array1;
use serde::{Deserialize, Serialize};

/// Assinatura acústica de um render para detecção de regressão sonora
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioFingerprint {
    pub mfcc: Vec<f32>, // 13 coeficientes MFCC
    pub spectral_centroid: f32,
    pub rms_energy: f32,
    pub spectral_contrast: Vec<f32>, // 7 bandas
    pub peak_ratio: f32,             // Peak / RMS
    pub duration: f32,
    pub lufs: Option<f32>,
}

impl AudioFingerprint {
    /// Cria uma fingerprint a partir de um buffer PCM
    pub fn from_pcm(pcm: &Array1<f32>, sample_rate: u32) -> Self {
        let mfcc = Self::extract_mfcc(pcm);
        let centroid = crate::dsp::analysis::fft::spectral_centroid(pcm.view(), sample_rate);
        let rms = crate::dsp::analysis::rms::calculate_rms(pcm.view());
        let contrast = Self::calc_spectral_contrast(pcm, sample_rate);
        let peak = Self::calc_peak(pcm);

        Self {
            mfcc,
            spectral_centroid: centroid,
            rms_energy: rms,
            spectral_contrast: contrast,
            peak_ratio: peak / rms.max(1e-10),
            duration: pcm.len() as f32 / sample_rate as f32,
            lufs: None, // Calculado com ebur128 em etapa separada (mastering::measure_lufs)
        }
    }

    /// Placeholder: uma implementação real usaria uma crate de MFCC dedicada
    /// (banco de filtros Mel + DCT sobre o log-espectro).
    fn extract_mfcc(_pcm: &Array1<f32>) -> Vec<f32> {
        vec![0.0f32; 13]
    }

    /// Contraste espectral simplificado: razão pico/média por banda log-espaçada.
    fn calc_spectral_contrast(pcm: &Array1<f32>, sample_rate: u32) -> Vec<f32> {
        const BANDS: usize = 7;
        let mag = crate::dsp::analysis::fft::magnitude_spectrum(pcm.view());
        let n = mag.len();
        if n < 2 {
            return vec![0.0; BANDS];
        }

        let nyquist = sample_rate as f32 / 2.0;
        let min_hz = 20.0f32;
        let log_min = min_hz.ln();
        let log_max = nyquist.max(min_hz + 1.0).ln();

        (0..BANDS)
            .map(|b| {
                let f_lo = (log_min + (b as f32 / BANDS as f32) * (log_max - log_min)).exp();
                let f_hi = (log_min + ((b + 1) as f32 / BANDS as f32) * (log_max - log_min)).exp();
                let bin_of = |f: f32| ((f / nyquist) * (n - 1) as f32).round() as usize;
                let start = bin_of(f_lo).min(n - 1);
                let end = bin_of(f_hi).clamp(start + 1, n);
                let slice = &mag[start..end];
                let peak = slice.iter().cloned().fold(0.0f32, f32::max);
                let mean = slice.iter().sum::<f32>() / slice.len() as f32;
                if mean > 0.0 {
                    (peak / mean).ln()
                } else {
                    0.0
                }
            })
            .collect()
    }

    fn calc_peak(pcm: &Array1<f32>) -> f32 {
        pcm.iter().map(|&x| x.abs()).fold(0.0, f32::max)
    }

    /// Calcula distância euclidiana ponderada entre fingerprints
    pub fn distance(&self, other: &Self) -> f32 {
        if self.mfcc.len() != other.mfcc.len() {
            return f32::INFINITY;
        }

        let mfcc_dist = Self::euclidean(&self.mfcc, &other.mfcc) * 2.0;
        let centroid_dist = (self.spectral_centroid - other.spectral_centroid).abs() * 1.5;
        let rms_dist = (self.rms_energy - other.rms_energy).abs();

        (mfcc_dist + centroid_dist + rms_dist) / 4.5
    }

    fn euclidean(a: &[f32], b: &[f32]) -> f32 {
        a.iter()
            .zip(b)
            .map(|(&x, &y)| (x - y) * (x - y))
            .sum::<f32>()
            .sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array1;

    /// I10 (docs/10-TESTES-QUALIDADE.md §3): distance(x, x) == 0.
    #[test]
    fn test_fingerprint_distance_zero() {
        let pcm = Array1::from_vec(vec![0.0f32; 44100]);
        let f1 = AudioFingerprint::from_pcm(&pcm, 44100);
        let f2 = AudioFingerprint::from_pcm(&pcm, 44100);

        assert_eq!(f1.distance(&f2), 0.0);
    }

    /// I10: distance é simétrica: distance(a, b) == distance(b, a).
    #[test]
    fn test_fingerprint_distance_is_symmetric() {
        let silence = Array1::from_vec(vec![0.0f32; 44100]);
        let tone = Array1::from_vec(
            (0..44100)
                .map(|i| (i as f32 / 44100.0 * 440.0 * std::f32::consts::TAU).sin())
                .collect(),
        );

        let f_silence = AudioFingerprint::from_pcm(&silence, 44100);
        let f_tone = AudioFingerprint::from_pcm(&tone, 44100);

        assert_eq!(f_silence.distance(&f_tone), f_tone.distance(&f_silence));
    }
}
