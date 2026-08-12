/// Metricas objetivas de qualidade auditiva.
/// Fontes: ITU-R BS.1770 (LUFS), DAFX Zolzer (THD),
/// literatura de avaliacao de pitch (jitter).

use ndarray::{Array1, s};

/// Distorcao harmonica total: sqrt(sum A_k^2 for k>=2) / A_1
/// Usa FFT para encontrar harmonicos. Retorna 0.0 para silencio.
pub fn total_harmonic_distortion(
    signal: &Array1<f32>,
    sample_rate: u32,
    fundamental_freq: f32,
) -> f32 {
    // Verifica se o sinal tem energia suficiente
    let signal_power: f32 = signal.iter().map(|&x| x * x).sum::<f32>() / signal.len() as f32;
    if signal_power < 1e-20 {
        return 0.0;
    }

    // Usa a maior janela potencia-de-2 que couber no sinal
    let exp = (signal.len() as f32).log2().floor() as u32;
    let fft_len = 2usize.pow(exp).min(signal.len()).max(4);
    let frame = signal.slice(s![..fft_len]);
    let mag = super::fft::magnitude_spectrum(frame);

    if mag.is_empty() {
        return 0.0;
    }

    let nyquist = sample_rate as f32 / 2.0;
    let bin_width = nyquist / mag.len() as f32;

    // Encontra o bin mais proximo da fundamental
    let fund_bin = (fundamental_freq / bin_width).round() as usize;
    if fund_bin == 0 || fund_bin >= mag.len() {
        return 0.0;
    }

    // Amplitude da fundamental (usando vizinhos para robustez)
    let a1 = mag.get(fund_bin).copied().unwrap_or(0.0);
    if a1 < 1e-10 {
        return 0.0;
    }

    // Soma das amplitudes dos harmonicos 2..=N
    let num_harmonics = ((nyquist / fundamental_freq) as usize).min(20);
    let mut harmonic_sum_sq = 0.0f32;
    for k in 2..=num_harmonics {
        let bin = (fund_bin as f32 * k as f32).round() as usize;
        if bin < mag.len() {
            let ak = mag[bin];
            harmonic_sum_sq += ak * ak;
        }
    }

    (harmonic_sum_sq.sqrt()) / a1
}

/// Razao de jitter: stddev(pitch_processado) / stddev(pitch_original)
/// Se o original tem vibrato, o processado deve preservar a variacao.
/// Retorna 1.0 se ambos forem silenciosos.
pub fn jitter_ratio(
    original: &Array1<f32>,
    processed: &Array1<f32>,
    sample_rate: u32,
    frame_size: usize,
) -> f32 {
    let pitches_orig = super::pitch_detect::detect_pitch(original, sample_rate, frame_size, frame_size / 2);
    let pitches_proc = super::pitch_detect::detect_pitch(processed, sample_rate, frame_size, frame_size / 2);

    let voiced_orig: Vec<f32> = pitches_orig.iter().filter(|p| p.is_voiced).map(|p| p.freq).collect();
    let voiced_proc: Vec<f32> = pitches_proc.iter().filter(|p| p.is_voiced).map(|p| p.freq).collect();

    // Se ambos vazios (silencio), retorna 1.0
    if voiced_orig.is_empty() && voiced_proc.is_empty() {
        return 1.0;
    }

    let std_orig = stddev(&voiced_orig);
    let std_proc = stddev(&voiced_proc);

    if std_orig < 1e-10 {
        return if std_proc < 1e-10 { 1.0 } else { f32::MAX };
    }

    std_proc / std_orig
}

/// Diferenca de envelope: L1 norm da diferenca entre envelopes RMS.
/// Janelas de window_ms. Retorna 0.0 para sinais identicos.
pub fn envelope_difference(
    original: &Array1<f32>,
    processed: &Array1<f32>,
    sample_rate: u32,
    window_ms: f32,
) -> f32 {
    if original.is_empty() || processed.is_empty() {
        return 0.0;
    }

    let window_size = (window_ms / 1000.0 * sample_rate as f32) as usize;
    let hop = window_size / 2;
    if window_size < 2 || hop < 1 {
        return 0.0;
    }

    let rms_orig = super::rms::sliding_rms(original.view(), window_size, hop);
    let rms_proc = super::rms::sliding_rms(processed.view(), window_size, hop);

    let min_len = rms_orig.len().min(rms_proc.len());
    if min_len == 0 {
        return 0.0;
    }

    let l1_diff: f32 = rms_orig[..min_len]
        .iter()
        .zip(&rms_proc[..min_len])
        .map(|(a, b)| (a - b).abs())
        .sum();

    l1_diff / min_len as f32
}

/// Relacao sinal-ruido entre original e processado.
/// SNR = 10 * log10(var_original / var_residuo)
/// Retorna f32::MAX se residuo for zero (sinais identicos).
pub fn signal_to_noise(original: &Array1<f32>, processed: &Array1<f32>) -> f32 {
    if original.is_empty() || original.len() != processed.len() {
        return f32::MAX;
    }

    let var_orig: f32 = original.iter().map(|&x| x * x).sum::<f32>() / original.len() as f32;
    let residual: Array1<f32> = original - processed;
    let var_res: f32 = residual.iter().map(|&x| x * x).sum::<f32>() / residual.len() as f32;

    if var_res < 1e-20 {
        return f32::MAX;
    }

    10.0 * (var_orig / var_res).log10()
}

/// Mudanca relativa do centroide espectral (%).
/// |centroid_proc - centroid_orig| / centroid_orig * 100
pub fn spectral_centroid_shift(
    original: &Array1<f32>,
    processed: &Array1<f32>,
    sample_rate: u32,
) -> f32 {
    // Usa a menor janela potencia-de-2 que couber em ambos
    let max_len = original.len().min(processed.len());
    if max_len < 4 {
        return 0.0;
    }
    let exp = (max_len as f32).log2().floor() as u32;
    let fft_len = 2usize.pow(exp).max(4);

    let centroid_orig = super::fft::spectral_centroid(original.slice(s![..fft_len]), sample_rate);
    let centroid_proc = super::fft::spectral_centroid(processed.slice(s![..fft_len]), sample_rate);

    if centroid_orig < 1e-10 {
        return 0.0;
    }

    (centroid_proc - centroid_orig).abs() / centroid_orig * 100.0
}

/// Estrutura com todas as metricas calculadas.
pub struct QualityReport {
    pub thd: f32,
    pub jitter_ratio: f32,
    pub envelope_diff: f32,
    pub snr_db: f32,
    pub centroid_shift_pct: f32,
}

/// Calcula todas as metricas de uma vez.
pub fn compute_quality_report(
    original: &Array1<f32>,
    processed: &Array1<f32>,
    sample_rate: u32,
    fundamental_freq: f32,
) -> QualityReport {
    QualityReport {
        thd: total_harmonic_distortion(processed, sample_rate, fundamental_freq),
        jitter_ratio: jitter_ratio(original, processed, sample_rate, 4096),
        envelope_diff: envelope_difference(original, processed, sample_rate, 50.0),
        snr_db: signal_to_noise(original, processed),
        centroid_shift_pct: spectral_centroid_shift(original, processed, sample_rate),
    }
}

/// Calcula o desvio padrao de um slice de f32.
fn stddev(values: &[f32]) -> f32 {
    if values.len() < 2 {
        return 0.0;
    }
    let mean = values.iter().sum::<f32>() / values.len() as f32;
    let variance = values.iter().map(|&x| (x - mean) * (x - mean)).sum::<f32>() / values.len() as f32;
    variance.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::analysis::test_fixtures::generate_sine;

    #[test]
    fn test_thd_pure_sine_is_zero() {
        let sr = 44100u32;
        let sine = generate_sine(440.0, 1.0, sr, 0.9);
        let thd = total_harmonic_distortion(&sine, sr, 440.0);
        // Seno puro ideal: THD deve ser proximo de zero
        assert!(
            thd < 0.05,
            "THD de seno puro deveria ser ~0, obteve {thd}"
        );
    }

    #[test]
    fn test_thd_clipped_sine_is_high() {
        let sr = 44100u32;
        let sine = generate_sine(440.0, 1.0, sr, 2.0); // amplitude 2.0 sera clipada
        // Clipa manualmente para simular distorcao
        let clipped: Array1<f32> = sine.iter().map(|&x| x.clamp(-1.0, 1.0)).collect();
        let thd = total_harmonic_distortion(&clipped, sr, 440.0);
        assert!(
            thd > 0.1,
            "THD de seno clipada deveria ser > 0.1, obteve {thd}"
        );
    }

    #[test]
    fn test_envelope_difference_identical_is_zero() {
        let sr = 44100u32;
        let signal = generate_sine(440.0, 1.0, sr, 0.8);
        let diff = envelope_difference(&signal, &signal, sr, 50.0);
        assert!(
            diff.abs() < 1e-10,
            "Diferenca de envelope de sinal identico deveria ser 0.0, obteve {diff}"
        );
    }

    #[test]
    fn test_snr_identical_is_max() {
        let sr = 44100u32;
        let signal = generate_sine(440.0, 1.0, sr, 0.8);
        let snr = signal_to_noise(&signal, &signal);
        assert_eq!(
            snr, f32::MAX,
            "SNR de sinal identico deveria ser f32::MAX, obteve {snr}"
        );
    }

    #[test]
    fn test_spectral_centroid_shift_high_freq() {
        let sr = 44100u32;
        // Sinal de baixa frequencia
        let low_freq = generate_sine(100.0, 0.5, sr, 0.9);
        // Sinal de alta frequencia
        let high_freq = generate_sine(2000.0, 0.5, sr, 0.9);

        let shift = spectral_centroid_shift(&low_freq, &high_freq, sr);
        // Mudanca deve ser significativa: > 40%
        assert!(
            shift > 40.0,
            "Mudanca de centroide entre 100 Hz e 2000 Hz deveria ser > 40%, obteve {shift:.1}%"
        );
    }

    #[test]
    fn test_quality_report_pure_sine() {
        let sr = 44100u32;
        let signal = generate_sine(440.0, 1.0, sr, 0.8);
        let report = compute_quality_report(&signal, &signal, sr, 440.0);

        // THD de seno puro deve ser baixo
        assert!(
            report.thd < 0.05,
            "THD deveria ser ~0 para seno puro, obteve {}",
            report.thd
        );
        // SNR identico = MAX
        assert_eq!(report.snr_db, f32::MAX, "SNR deveria ser MAX para sinal identico");
        // Envelope identico = 0
        assert!(
            report.envelope_diff.abs() < 1e-10,
            "Diferenca de envelope deveria ser 0, obteve {}",
            report.envelope_diff
        );
        // Centroide identico = 0%
        assert!(
            report.centroid_shift_pct.abs() < 0.1,
            "Mudanca de centroide deveria ser 0%, obteve {}%",
            report.centroid_shift_pct
        );
        // Jitter de sinal identico = 1.0
        assert!(
            (report.jitter_ratio - 1.0).abs() < 0.1,
            "Jitter ratio deveria ser ~1.0 para sinal identico, obteve {}",
            report.jitter_ratio
        );
    }
}
