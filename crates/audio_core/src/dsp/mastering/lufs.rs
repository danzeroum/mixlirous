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

    #[test]
    fn test_apply_lufs_gain_moves_toward_target() {
        let mut pcm = vec![0.05f32; 44100];
        apply_lufs_gain(&mut pcm, 44100, -14.0);
        let result = measure_lufs(&Array1::from_vec(pcm), 44100);
        assert!((result - -14.0).abs() < 1.0);
    }
}
