use ndarray::ArrayView1;
use realfft::RealFftPlanner;

/// Calcula o espectro de magnitude de um frame de áudio
pub fn magnitude_spectrum(pcm: ArrayView1<f32>) -> Vec<f32> {
    if pcm.len() < 2 {
        return Vec::new();
    }

    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(pcm.len());

    let mut input = fft.make_input_vec();
    for (dst, &src) in input.iter_mut().zip(pcm.iter()) {
        *dst = src;
    }

    let mut output = fft.make_output_vec();
    if fft.process(&mut input, &mut output).is_err() {
        return vec![0.0; output.len()];
    }

    output.iter().map(|c| c.norm()).collect()
}

/// Calcula o centroide espectral (medida de brilho)
pub fn spectral_centroid(pcm: ArrayView1<f32>, sample_rate: u32) -> f32 {
    let mag = magnitude_spectrum(pcm);
    let n = mag.len();
    if n == 0 {
        return 0.0;
    }

    let total_energy: f32 = mag.iter().sum();
    if total_energy <= 0.0 {
        return 0.0;
    }

    let freq_weighted: f32 = mag.iter().enumerate().map(|(i, &m)| m * (i as f32)).sum();

    // n bins cobrem 0..=Nyquist em (n - 1) passos
    (freq_weighted / total_energy) * (sample_rate as f32 / (2.0 * (n - 1).max(1) as f32))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array1;

    #[test]
    fn test_magnitude_spectrum_silence_is_zero() {
        let pcm = Array1::from_vec(vec![0.0f32; 1024]);
        let mag = magnitude_spectrum(pcm.view());
        assert!(mag.iter().all(|&v| v.abs() < 1e-6));
    }

    #[test]
    fn test_spectral_centroid_of_silence_is_zero() {
        let pcm = Array1::from_vec(vec![0.0f32; 1024]);
        assert_eq!(spectral_centroid(pcm.view(), 44100), 0.0);
    }
}
