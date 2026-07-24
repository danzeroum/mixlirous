use ndarray::Array1;

/// Busca o zero-crossing mais próximo de um índice alvo no buffer
/// Usado para cortes sem clicks
pub fn find_zero_crossing(
    pcm: &Array1<f32>,
    target_idx: usize,
    search_window_samples: usize,
) -> usize {
    let start = target_idx.saturating_sub(search_window_samples / 2);
    let end = (target_idx + search_window_samples / 2).min(pcm.len() - 1);

    // Procura transições negativo → positivo
    for i in start..end {
        if i > 0 && pcm[i - 1] <= 0.0 && pcm[i] > 0.0 {
            return i;
        }
    }

    // Se não encontrou, retorna o target original
    target_idx
}

/// Aplica fade-out logarítmico em um buffer
pub fn fade_out_log(pcm: &mut [f32], start_sample: usize, fade_samples: usize) {
    let len = fade_samples.min(pcm.len() - start_sample);
    for i in 0..len {
        // Curva logarítmica: ganho = 1 - log(1 + x) / log(1 + N)
        let x = i as f32;
        let n = len as f32;
        let gain = 1.0 - (1.0 + x).ln() / (1.0 + n).ln();
        pcm[start_sample + i] *= gain.max(0.0);
    }
}

/// Aplica fade-in logarítmico
pub fn fade_in_log(pcm: &mut [f32], start_sample: usize, fade_samples: usize) {
    let len = fade_samples.min(pcm.len() - start_sample);
    for i in 0..len {
        let x = (len - i) as f32;
        let n = len as f32;
        let gain = 1.0 - (1.0 + x).ln() / (1.0 + n).ln();
        pcm[start_sample + i] *= gain.max(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array1;

    #[test]
    fn test_find_zero_crossing_locates_sign_change() {
        let mut samples = vec![0.0f32; 200];
        for (i, s) in samples.iter_mut().enumerate() {
            *s = if i < 100 { -1.0 } else { 1.0 };
        }
        let pcm = Array1::from_vec(samples);
        let idx = find_zero_crossing(&pcm, 100, 40);
        assert_eq!(idx, 100);
    }

    #[test]
    fn test_find_zero_crossing_falls_back_to_target() {
        let pcm = Array1::from_vec(vec![1.0f32; 200]); // sem transição de sinal
        assert_eq!(find_zero_crossing(&pcm, 50, 20), 50);
    }

    #[test]
    fn test_fade_out_log_reaches_near_silence() {
        let mut pcm = vec![1.0f32; 100];
        fade_out_log(&mut pcm, 0, 100);
        assert!(pcm[99] < 0.2);
    }

    #[test]
    fn test_fade_in_log_starts_near_silence() {
        let mut pcm = vec![1.0f32; 100];
        fade_in_log(&mut pcm, 0, 100);
        assert!(pcm[0] < 0.2);
    }
}
