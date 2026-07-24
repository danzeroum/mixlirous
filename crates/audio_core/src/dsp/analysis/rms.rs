use ndarray::{s, ArrayView1};

/// Calcula o RMS (Root Mean Square) de um buffer de áudio
pub fn calculate_rms(pcm: ArrayView1<f32>) -> f32 {
    if pcm.len() < 100 {
        return 0.0;
    } // Janela muito curta
    let sum_sq: f32 = pcm.iter().map(|&x| x * x).sum();
    (sum_sq / pcm.len() as f32).sqrt()
}

/// Calcula RMS em janelas deslizantes para análise de energia ao longo do tempo
pub fn sliding_rms(pcm: ArrayView1<f32>, frame_size: usize, hop_size: usize) -> Vec<f32> {
    let mut rms_values = Vec::new();

    if pcm.len() < frame_size {
        return rms_values;
    }

    for start in (0..=(pcm.len() - frame_size)).step_by(hop_size) {
        let window = pcm.slice(s![start..start + frame_size]);
        rms_values.push(calculate_rms(window));
    }

    rms_values
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array1;

    #[test]
    fn test_calculate_rms_of_silence_is_zero() {
        let pcm = Array1::from_vec(vec![0.0f32; 1000]);
        assert_eq!(calculate_rms(pcm.view()), 0.0);
    }

    #[test]
    fn test_calculate_rms_of_short_buffer_is_zero() {
        let pcm = Array1::from_vec(vec![1.0f32; 10]);
        assert_eq!(calculate_rms(pcm.view()), 0.0);
    }

    #[test]
    fn test_sliding_rms_produces_expected_frame_count() {
        let pcm = Array1::from_vec(vec![0.5f32; 4096]);
        let frames = sliding_rms(pcm.view(), 1024, 512);
        assert_eq!(frames.len(), 7);
        assert!(frames.iter().all(|&v| (v - 0.5).abs() < 1e-5));
    }
}
