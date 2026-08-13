//! Sprint 4 — Audio fingerprint with normalized distance (task 4.10).
//!
//! Real MFCC extraction (13 coefficients, 26 mel filters), spectral contrast,
//! spectral centroid, RMS energy, peak ratio, duration, optional LUFS.
//!
//! Sprint 4 improvement: distance() now uses **fully normalized components**.
//! Each component is scaled to [0, 1] before weighting, preventing any single
//! component from dominating the distance.
//!
//! Normalization strategy:
//!   - MFCC: Euclidean / sqrt(NUM_MFCC) — bounded by construction
//!   - Spectral centroid: |a-b| / max(|a|,|b|,1.0) — relative difference
//!   - RMS energy: |a-b| / max(a,b,1e-6) — relative difference
//!   - Spectral contrast: cosine distance — naturally in [0, 2]
//!   - Peak ratio: |a-b| / max(a,b,0.01) — relative difference
//!   - Duration: |a-b| / max(a,b,1.0) — relative difference

use ndarray::Array1;
use serde::{Deserialize, Serialize};

const NUM_MFCC: usize = 13;
const NUM_MEL_FILTERS: usize = 26;
const MIN_HZ: f32 = 20.0;
const MAX_HZ: f32 = 8000.0;

/// Per-component weights for the distance function.
/// Sum must equal 1.0 for the distance to be in [0, 1].
const WEIGHT_MFCC: f32 = 0.40;
const WEIGHT_CENTROID: f32 = 0.15;
const WEIGHT_RMS: f32 = 0.10;
const WEIGHT_CONTRAST: f32 = 0.15;
const WEIGHT_PEAK_RATIO: f32 = 0.10;
const WEIGHT_DURATION: f32 = 0.10;

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
    /// 1. FFT -> power spectrum
    /// 2. Mel filterbank (26 triangular filters)
    /// 3. Log of energy per filter
    /// 4. DCT-II -> keep coefficients 1-13
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

    /// Cosine distance between two vectors. Returns value in [0, 2].
    /// 0 = identical direction, 2 = opposite direction.
    fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return f32::INFINITY;
        }
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        let denom = norm_a * norm_b;
        if denom < 1e-10 {
            return if norm_a < 1e-10 && norm_b < 1e-10 {
                0.0 // Both zero vectors
            } else {
                1.0 // One is zero
            };
        }
        let cosine = (dot / denom).clamp(-1.0, 1.0);
        1.0 - cosine // [0, 2]
    }

    /// Normalized relative difference, clamped to [0, 1].
    /// |a - b| / max(|a|, |b|, epsilon)
    fn normalized_relative_diff(a: f32, b: f32, epsilon: f32) -> f32 {
        let denom = a.abs().max(b.abs()).max(epsilon);
        ((a - b).abs() / denom).min(1.0)
    }

    /// Sprint 4 weighted distance with fully normalized components.
    /// Each component is scaled to approximately [0, 1] before weighting.
    /// Final distance is in approximately [0, 1].
    ///
    /// Returns f32::INFINITY if fingerprints are incompatible (different
    /// MFCC dimensions).
    pub fn distance(&self, other: &Self) -> f32 {
        if self.mfcc.len() != other.mfcc.len() {
            return f32::INFINITY;
        }

        // 1. MFCC: Euclidean distance normalized by sqrt(N).
        //    This is bounded and well-scaled.
        let mfcc_dist = Self::euclidean(&self.mfcc, &other.mfcc) / (NUM_MFCC as f32).sqrt();
        // Clamp to [0, 1] — in practice this can exceed 1 for very different signals,
        // so we soft-clamp.
        let mfcc_norm = (mfcc_dist / 5.0).min(1.0); // 5.0 is an empirical max

        // 2. Spectral centroid: relative difference.
        let centroid_norm =
            Self::normalized_relative_diff(self.spectral_centroid, other.spectral_centroid, 1.0);

        // 3. RMS energy: relative difference.
        let rms_norm = Self::normalized_relative_diff(self.rms_energy, other.rms_energy, 1e-6);

        // 4. Spectral contrast: cosine distance (naturally in [0, 2], divide by 2).
        let contrast_dist =
            Self::cosine_distance(&self.spectral_contrast, &other.spectral_contrast);
        let contrast_norm = (contrast_dist / 2.0).min(1.0);

        // 5. Peak ratio: relative difference.
        let peak_norm = Self::normalized_relative_diff(self.peak_ratio, other.peak_ratio, 0.01);

        // 6. Duration: relative difference.
        let duration_norm = Self::normalized_relative_diff(self.duration, other.duration, 1.0);

        // Weighted combination — weights sum to 1.0.
        WEIGHT_MFCC * mfcc_norm
            + WEIGHT_CENTROID * centroid_norm
            + WEIGHT_RMS * rms_norm
            + WEIGHT_CONTRAST * contrast_norm
            + WEIGHT_PEAK_RATIO * peak_norm
            + WEIGHT_DURATION * duration_norm
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

    /// I10 (docs/10-TESTES-QUALIDADE.md S3): distance(x, x) == 0.
    #[test]
    fn test_fingerprint_distance_zero() {
        let pcm = Array1::from_vec(vec![0.0f32; 44100]);
        let f1 = AudioFingerprint::from_pcm(&pcm, 44100);
        let f2 = AudioFingerprint::from_pcm(&pcm, 44100);
        assert!(
            (f1.distance(&f2)).abs() < 0.01,
            "distance(x,x) should be ~0, got {}",
            f1.distance(&f2)
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
        assert!(
            dist > 0.01,
            "Different tones should have non-zero distance, got {}",
            dist
        );
    }

    /// Sprint 4: distance should be bounded in [0, 1] for normalized comparison.
    #[test]
    fn test_distance_bounded() {
        let tone_a4 = Array1::from_vec(
            (0..44100)
                .map(|i| (i as f32 / 44100.0 * 440.0 * std::f32::consts::TAU).sin() * 0.8)
                .collect(),
        );
        let tone_c5 = Array1::from_vec(
            (0..44100)
                .map(|i| (i as f32 / 44100.0 * 523.25 * std::f32::consts::TAU).sin() * 0.2)
                .collect(),
        );
        let noise = Array1::from_vec(
            (0..44100)
                .map(|i| ((i * 7919 + 1234) % 10000) as f32 / 10000.0 * 2.0 - 1.0)
                .collect(),
        );

        let fp_a4 = AudioFingerprint::from_pcm(&tone_a4, 44100);
        let fp_c5 = AudioFingerprint::from_pcm(&tone_c5, 44100);
        let fp_noise = AudioFingerprint::from_pcm(&noise, 44100);

        // All distances should be finite and in a reasonable range.
        let d1 = fp_a4.distance(&fp_c5);
        let d2 = fp_a4.distance(&fp_noise);
        let d3 = fp_c5.distance(&fp_noise);

        for (name, d) in [("a4_c5", d1), ("a4_noise", d2), ("c5_noise", d3)] {
            assert!(
                d.is_finite(),
                "distance({}) should be finite, got {}",
                name,
                d
            );
            assert!(
                d >= 0.0,
                "distance({}) should be non-negative, got {}",
                name,
                d
            );
            assert!(d <= 1.5, "distance({}) should be bounded, got {}", name, d);
        }
    }

    /// Sprint 4: similar signals should have smaller distance than dissimilar.
    #[test]
    fn test_distance_ordering() {
        let tone_440 = Array1::from_vec(
            (0..44100)
                .map(|i| (i as f32 / 44100.0 * 440.0 * std::f32::consts::TAU).sin() * 0.5)
                .collect(),
        );
        // 442 Hz is very close to 440 Hz
        let tone_442 = Array1::from_vec(
            (0..44100)
                .map(|i| (i as f32 / 44100.0 * 442.0 * std::f32::consts::TAU).sin() * 0.5)
                .collect(),
        );
        // 1000 Hz is very different from 440 Hz
        let tone_1000 = Array1::from_vec(
            (0..44100)
                .map(|i| (i as f32 / 44100.0 * 1000.0 * std::f32::consts::TAU).sin() * 0.5)
                .collect(),
        );

        let fp_440 = AudioFingerprint::from_pcm(&tone_440, 44100);
        let fp_442 = AudioFingerprint::from_pcm(&tone_442, 44100);
        let fp_1000 = AudioFingerprint::from_pcm(&tone_1000, 44100);

        let d_near = fp_440.distance(&fp_442);
        let d_far = fp_440.distance(&fp_1000);

        assert!(
            d_near < d_far,
            "Near tones (440 vs 442) should be closer than far tones (440 vs 1000): {} vs {}",
            d_near,
            d_far
        );
    }

    /// Sprint 4: test cosine_distance helper.
    #[test]
    fn test_cosine_distance() {
        let a = [1.0, 0.0, 0.0];
        let b = [1.0, 0.0, 0.0];
        assert!((AudioFingerprint::cosine_distance(&a, &b)).abs() < 1e-6);

        let c = [0.0, 1.0, 0.0];
        let d = AudioFingerprint::cosine_distance(&a, &c);
        assert!((d - 1.0).abs() < 1e-6, "orthogonal = 1.0, got {}", d);

        let e = [-1.0, 0.0, 0.0];
        let f = AudioFingerprint::cosine_distance(&a, &e);
        assert!((f - 2.0).abs() < 1e-6, "opposite = 2.0, got {}", f);
    }

    /// Sprint 4: test normalized_relative_diff.
    #[test]
    fn test_normalized_relative_diff() {
        assert!((AudioFingerprint::normalized_relative_diff(10.0, 10.0, 1.0)).abs() < 1e-6);
        assert!((AudioFingerprint::normalized_relative_diff(0.0, 0.0, 1.0)).abs() < 1e-6);
        assert!((AudioFingerprint::normalized_relative_diff(10.0, 20.0, 1.0) - 0.5).abs() < 1e-6);
        assert!((AudioFingerprint::normalized_relative_diff(0.0, 10.0, 1e-6) - 1.0).abs() < 1e-3);
    }

    /// Sprint 4: weights sum to 1.0.
    #[test]
    fn test_weights_sum_to_one() {
        let sum = WEIGHT_MFCC
            + WEIGHT_CENTROID
            + WEIGHT_RMS
            + WEIGHT_CONTRAST
            + WEIGHT_PEAK_RATIO
            + WEIGHT_DURATION;
        assert!(
            (sum - 1.0).abs() < 1e-6,
            "weights should sum to 1.0, got {}",
            sum
        );
    }

    /// Sprint 4: incompatible fingerprints return infinity.
    #[test]
    fn test_incompatible_fingerprints() {
        let fp1 = AudioFingerprint {
            mfcc: vec![1.0, 2.0, 3.0],
            spectral_centroid: 1000.0,
            rms_energy: 0.5,
            spectral_contrast: vec![0.1, 0.2],
            peak_ratio: 1.5,
            duration: 5.0,
            lufs: None,
        };
        let fp2 = AudioFingerprint {
            mfcc: vec![1.0, 2.0], // Different length!
            spectral_centroid: 1000.0,
            rms_energy: 0.5,
            spectral_contrast: vec![0.1, 0.2],
            peak_ratio: 1.5,
            duration: 5.0,
            lufs: None,
        };
        assert_eq!(fp1.distance(&fp2), f32::INFINITY);
    }
}
