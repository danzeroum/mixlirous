use ndarray::Array1;
use serde::{Deserialize, Serialize};

const NUM_MFCC: usize = 13;
const NUM_MEL_FILTERS: usize = 26;
const MIN_HZ: f32 = 20.0;
const MAX_HZ: f32 = 8000.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioFingerprint {
    pub mfcc: Vec<f32>,
    pub spectral_centroid: f32,
    pub rms_energy: f32,
    pub spectral_contrast: Vec<f32>,
    pub peak_ratio: f32,
    pub duration: f32,
    pub lufs: Option<f32>,
}

impl AudioFingerprint {
    pub fn from_pcm(pcm: &Array1<f32>, sample_rate: u32) -> Self {
        let mfcc = Self::extract_mfcc(pcm, sample_rate);
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
            lufs: None,
        }
    }

    /// Real MFCC extraction:
    /// 1. FFT → power spectrum
    /// 2. Mel filterbank (26 triangular filters)
    /// 3. Log of energy per filter
    /// 4. DCT-II → keep coefficients 1-13
    fn extract_mfcc(pcm: &Array1<f32>, sample_rate: u32) -> Vec<f32> {
        if pcm.len() < 2 {
            return vec![0.0; NUM_MFCC];
        }

        let mag = crate::dsp::analysis::fft::magnitude_spectrum(pcm.view());
        let n = mag.len();
        if n == 0 {
            return vec![0.0; NUM_MFCC];
        }

        // Power spectrum
        let power: Vec<f32> = mag.iter().map(|&m| m * m).collect();

        // Mel filterbank
        let mel_filters = Self::mel_filterbank(NUM_MEL_FILTERS, n, sample_rate);

        // Apply filters and take log
        let mut filter_energies = [0.0f32; NUM_MEL_FILTERS];
        for (i, filter) in mel_filters.iter().enumerate() {
            let energy: f32 = filter.iter().zip(power.iter()).map(|(f, p)| f * p).sum();
            filter_energies[i] = (energy + 1e-10).ln();
        }

        // DCT-II (keep coefficients 1-13, discard 0)
        let mut mfcc = vec![0.0f32; NUM_MFCC];
        #[allow(clippy::needless_range_loop)]
        for k in 0..NUM_MFCC {
            let mut sum = 0.0f32;
            for (n_idx, &energy) in filter_energies.iter().enumerate() {
                let angle = std::f32::consts::PI * (k + 1) as f32 * (2 * n_idx + 1) as f32
                    / (2 * NUM_MEL_FILTERS) as f32;
                sum += energy * angle.cos();
            }
            mfcc[k] = sum;
        }

        mfcc
    }

    /// Create Mel filterbank with triangular filters
    fn mel_filterbank(num_filters: usize, fft_size: usize, sample_rate: u32) -> Vec<Vec<f32>> {
        let min_mel = Self::hz_to_mel(MIN_HZ);
        let max_mel = Self::hz_to_mel(MAX_HZ.min(sample_rate as f32 / 2.0));

        // Create equally spaced points in Mel scale
        let mut mel_points = Vec::with_capacity(num_filters + 2);
        for i in 0..(num_filters + 2) {
            let mel = min_mel + (max_mel - min_mel) * i as f32 / (num_filters + 1) as f32;
            mel_points.push(Self::mel_to_hz(mel));
        }

        // Convert to FFT bin indices
        let bin_points: Vec<usize> = mel_points
            .iter()
            .map(|&hz| ((hz / (sample_rate as f32 / 2.0)) * (fft_size - 1) as f32).round() as usize)
            .collect();

        // Create triangular filters
        let mut filters = vec![vec![0.0f32; fft_size]; num_filters];
        for (i, filter) in filters.iter_mut().enumerate() {
            let left = bin_points[i];
            let center = bin_points[i + 1];
            let right = bin_points[i + 2];

            if center > left {
                #[allow(clippy::needless_range_loop)]
                for j in left..center {
                    filter[j] = (j - left) as f32 / (center - left) as f32;
                }
            }
            if right > center {
                #[allow(clippy::needless_range_loop)]
                for j in center..right {
                    filter[j] = (right - j) as f32 / (right - center) as f32;
                }
            }
        }

        filters
    }

    fn hz_to_mel(hz: f32) -> f32 {
        2595.0 * (1.0 + hz / 700.0).log10()
    }

    fn mel_to_hz(mel: f32) -> f32 {
        700.0 * (10f32.powf(mel / 2595.0) - 1.0)
    }

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
                let mean = slice.iter().sum::<f32>() / slice.len().max(1) as f32;
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

    /// Weighted distance between fingerprints with normalized components
    pub fn distance(&self, other: &Self) -> f32 {
        if self.mfcc.len() != other.mfcc.len() {
            return f32::INFINITY;
        }

        // Normalized MFCC distance (euclidean / sqrt(13))
        let mfcc_dist = Self::euclidean(&self.mfcc, &other.mfcc) / (NUM_MFCC as f32).sqrt();

        // Normalized centroid distance
        let centroid_dist = (self.spectral_centroid - other.spectral_centroid).abs()
            / self.spectral_centroid.max(other.spectral_centroid).max(1.0);

        // Normalized RMS distance
        let rms_dist = (self.rms_energy - other.rms_energy).abs()
            / self.rms_energy.max(other.rms_energy).max(1e-6);

        // Weighted combination
        (2.0 * mfcc_dist + 1.5 * centroid_dist + rms_dist) / 4.5
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
        assert!(
            (f1.distance(&f2)).abs() < 0.01,
            "distance(x,x) should be ~0"
        );
    }

    /// I10: distance is symmetric
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

        let d1 = f_silence.distance(&f_tone);
        let d2 = f_tone.distance(&f_silence);
        assert!(
            (d1 - d2).abs() < 0.01,
            "distance should be symmetric: {} vs {}",
            d1,
            d2
        );
    }

    /// MFCC should not be all zeros for non-silence
    #[test]
    fn test_mfcc_not_all_zeros_for_tone() {
        let tone = Array1::from_vec(
            (0..44100)
                .map(|i| (i as f32 / 44100.0 * 440.0 * std::f32::consts::TAU).sin() * 0.5)
                .collect(),
        );
        let fp = AudioFingerprint::from_pcm(&tone, 44100);
        let sum: f32 = fp.mfcc.iter().map(|x| x.abs()).sum();
        assert!(sum > 0.0, "MFCC should not be all zeros for non-silence");
    }

    /// Different sounds should have different fingerprints
    #[test]
    fn test_different_sounds_different_fingerprints() {
        let tone_a4 = Array1::from_vec(
            (0..44100)
                .map(|i| (i as f32 / 44100.0 * 440.0 * std::f32::consts::TAU).sin() * 0.5)
                .collect(),
        );
        let tone_c5 = Array1::from_vec(
            (0..44100)
                .map(|i| (i as f32 / 44100.0 * 523.25 * std::f32::consts::TAU).sin() * 0.5)
                .collect(),
        );

        let fp_a4 = AudioFingerprint::from_pcm(&tone_a4, 44100);
        let fp_c5 = AudioFingerprint::from_pcm(&tone_c5, 44100);

        let dist = fp_a4.distance(&fp_c5);
        assert!(dist > 0.01, "Different tones should have non-zero distance");
    }
}
