use ebur128::{EbuR128, Mode};
use ndarray::Array1;

/// Mede o LUFS integrado de um buffer PCM mono
pub fn measure_lufs(pcm: &Array1<f32>, sample_rate: u32) -> f32 {
    let Ok(mut meter) = EbuR128::new(1, sample_rate, Mode::I) else {
        return -99.0;
    };

    if meter.add_frames_f32(pcm.as_slice().unwrap_or(&[])).is_err() {
        return -99.0;
    }

    meter.loudness_global().unwrap_or(-99.0) as f32
}

/// Mede o true peak em dBTP (sobreamostragem ITU-R BS.1770, `Mode::TRUE_PEAK`
/// do ebur128) — não confundir com pico de amostra (ver B5 em docs/17).
/// `pcm` é intercalado por frame quando `channels > 1`.
pub fn measure_true_peak(pcm: &[f32], channels: u32, sample_rate: u32) -> f32 {
    let Ok(mut meter) = EbuR128::new(channels, sample_rate, Mode::TRUE_PEAK) else {
        return f32::NEG_INFINITY;
    };

    if meter.add_frames_f32(pcm).is_err() {
        return f32::NEG_INFINITY;
    }

    let peak = (0..channels)
        .filter_map(|ch| meter.true_peak(ch).ok())
        .fold(0.0f64, f64::max);

    if peak > 0.0 {
        (20.0 * peak.log10()) as f32
    } else {
        f32::NEG_INFINITY
    }
}

/// Aplica ganho para atingir target LUFS
pub fn apply_lufs_gain(pcm: &mut [f32], sample_rate: u32, target_lufs: f32) {
    let current = measure_lufs(&Array1::from_vec(pcm.to_vec()), sample_rate);
    let gain_db = target_lufs - current;
    let gain_linear = 10f32.powf(gain_db / 20.0);

    for sample in pcm.iter_mut() {
        *sample *= gain_linear;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_measure_lufs_of_silence_is_very_low() {
        let pcm = Array1::from_vec(vec![0.0f32; 44100]);
        let lufs = measure_lufs(&pcm, 44100);
        assert!(lufs < -60.0);
    }

    /// I11 (docs/10-TESTES-QUALIDADE.md §3): após normalização, |lufs-alvo| <= 0.5 LU.
    #[test]
    fn test_apply_lufs_gain_satisfies_i11_tolerance() {
        let mut pcm = vec![0.05f32; 44100];
        apply_lufs_gain(&mut pcm, 44100, -14.0);
        let result = measure_lufs(&Array1::from_vec(pcm), 44100);
        assert!(
            (result - -14.0).abs() <= 0.5,
            "fora do invariante I11: {result} LUFS"
        );
    }

    #[test]
    fn test_measure_true_peak_of_full_scale_sine_is_near_zero_dbtp() {
        let sr = 44100;
        let pcm: Vec<f32> = (0..sr)
            .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sr as f32).sin())
            .collect();
        let tp = measure_true_peak(&pcm, 1, sr);
        assert!((tp - 0.0).abs() <= 0.2, "esperado ~0 dBTP, obtido {tp}");
    }

    #[test]
    fn test_measure_true_peak_of_silence_is_negative_infinity() {
        let pcm = vec![0.0f32; 44100];
        assert_eq!(measure_true_peak(&pcm, 1, 44100), f32::NEG_INFINITY);
    }
}
