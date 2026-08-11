use ndarray::Array1;

/// Busca o zero-crossing mais pr├│ximo de um ├¡ndice alvo no buffer
/// Usado para cortes sem clicks
pub fn find_zero_crossing(
    pcm: &Array1<f32>,
    target_idx: usize,
    search_window_samples: usize,
) -> usize {
    let start = target_idx.saturating_sub(search_window_samples / 2);
    let end = (target_idx + search_window_samples / 2).min(pcm.len() - 1);

    // Procura transi├º├Áes negativo ÔåÆ positivo
    for i in start..end {
        if i > 0 && pcm[i - 1] <= 0.0 && pcm[i] > 0.0 {
            return i;
        }
    }

    // Se n├úo encontrou, retorna o target original
    target_idx
}

/// Sinal no estilo `numpy.sign`: zero mapeia para 0, n├úo para +1 como
/// `f32::signum`. A distin├º├úo importa para `zero_crossing_indices` ÔÇö um
/// trem de cliques que retorna a exatamente 0.0 entre pulsos s├│ conta como
/// cruzamento se 0.0 tiver sinal pr├│prio.
fn sign_bucket(x: f32) -> i8 {
    if x > 0.0 {
        1
    } else if x < 0.0 {
        -1
    } else {
        0
    }
}

/// ├ìndices onde o sinal muda de sinal (ou toca zero) entre uma amostra e a
/// seguinte. ├ìndice `i` significa a transi├º├úo entre `pcm[i]` e `pcm[i+1]`.
pub fn zero_crossing_indices(pcm: &Array1<f32>) -> Vec<usize> {
    let signs: Vec<i8> = pcm.iter().map(|&x| sign_bucket(x)).collect();
    (0..signs.len().saturating_sub(1))
        .filter(|&i| signs[i] != signs[i + 1])
        .collect()
}

/// Conta o total de zero-crossings no buffer inteiro (ver `zero_crossing_indices`).
pub fn count_zero_crossings(pcm: &Array1<f32>) -> usize {
    zero_crossing_indices(pcm).len()
}

/// Aplica fade-out logar├¡tmico em um buffer
pub fn fade_out_log(pcm: &mut [f32], start_sample: usize, fade_samples: usize) {
    let len = fade_samples.min(pcm.len() - start_sample);
    for i in 0..len {
        // Curva logar├¡tmica: ganho = 1 - log(1 + x) / log(1 + N)
        let x = i as f32;
        let n = len as f32;
        let gain = 1.0 - (1.0 + x).ln() / (1.0 + n).ln();
        pcm[start_sample + i] *= gain.max(0.0);
    }
}

/// Aplica fade-in logar├¡tmico
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
        let pcm = Array1::from_vec(vec![1.0f32; 200]); // sem transi├º├úo de sinal
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

    #[test]
    fn test_count_zero_crossings_of_dc_offset_is_zero() {
        let pcm = Array1::from_vec(vec![0.5f32; 1000]);
        assert_eq!(count_zero_crossings(&pcm), 0);
    }

    #[test]
    fn test_zero_crossing_indices_matches_known_alternation() {
        let pcm = Array1::from_vec(vec![1.0f32, -1.0, 1.0, -1.0, 1.0]);
        assert_eq!(zero_crossing_indices(&pcm), vec![0, 1, 2, 3]);
    }

    #[test]
    fn test_sign_bucket_treats_exact_zero_as_its_own_value() {
        // Um pulso que retorna a exatamente 0.0 entre picos positivos conta
        // como dois cruzamentos (0->+ e +->0), n├úo zero.
        let pcm = Array1::from_vec(vec![0.0f32, 0.5, 0.0]);
        assert_eq!(count_zero_crossings(&pcm), 2);
    }
}
