use ndarray::Array1;

/// Ajusta a duração de um buffer PCM por reamostragem linear.
///
/// Implementação simplificada: reamostrar altera duração e afeta o pitch
/// junto (não é um time-stretch preservando afinação). Um phase vocoder ou
/// WSOLA fica para uma sprint futura quando a qualidade de áudio virar foco;
/// aqui o objetivo é ter uma função íntegra e testável para a Sprint 0.
pub fn time_stretch(
    pcm: &Array1<f32>,
    sample_rate: u32,
    target_duration_sec: f32,
) -> Option<Array1<f32>> {
    if pcm.is_empty() || sample_rate == 0 || target_duration_sec <= 0.0 {
        return None;
    }

    let current_duration = pcm.len() as f32 / sample_rate as f32;
    if (current_duration - target_duration_sec).abs() < 0.05 {
        return Some(pcm.clone());
    }

    let target_len = (sample_rate as f32 * target_duration_sec).round() as usize;
    if target_len == 0 {
        return None;
    }

    let mut output = Vec::with_capacity(target_len);
    let ratio = (pcm.len() - 1).max(1) as f32 / target_len.max(1) as f32;

    for i in 0..target_len {
        let src_pos = i as f32 * ratio;
        let idx0 = src_pos.floor() as usize;
        let idx1 = (idx0 + 1).min(pcm.len() - 1);
        let frac = src_pos - idx0 as f32;
        let sample = pcm[idx0] * (1.0 - frac) + pcm[idx1] * frac;
        output.push(sample);
    }

    Some(Array1::from_vec(output))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_stretch_changes_length() {
        let pcm = Array1::from_vec((0..44100).map(|i| (i as f32 / 44100.0).sin()).collect());
        let stretched = time_stretch(&pcm, 44100, 15.0).unwrap();
        assert!((stretched.len() as i64 - 44100 * 15).abs() <= 1);
    }

    #[test]
    fn test_time_stretch_returns_none_for_empty_input() {
        let pcm = Array1::from_vec(Vec::new());
        assert!(time_stretch(&pcm, 44100, 5.0).is_none());
    }

    #[test]
    fn test_time_stretch_within_tolerance_returns_clone() {
        let pcm = Array1::from_vec(vec![0.1f32; 44100]);
        let result = time_stretch(&pcm, 44100, 1.0).unwrap();
        assert_eq!(result.len(), pcm.len());
    }
}
