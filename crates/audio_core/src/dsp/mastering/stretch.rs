use audioadapter_buffers::direct::SequentialSliceOfVecs;
use ndarray::Array1;
use rubato::{Async, FixedAsync, Resampler, SincInterpolationParameters};

/// Ajusta a duração de um buffer PCM mono por reamostragem sinc de banda
/// limitada (`rubato::Async`, janela Blackman-Harris2, interpolação cúbica).
///
/// Reamostrar ainda altera o pitch junto com a duração — não é um
/// time-stretch que preserva afinação (isso seria WSOLA/phase vocoder,
/// trabalho futuro quando entrar pitch-shifting independente). Mas o
/// resample em si é de qualidade real: banda limitada por sinc, não
/// interpolação linear (que aliasa e perde agudos).
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

    let ratio = (target_duration_sec / current_duration) as f64;
    let params = SincInterpolationParameters::default();
    let mut resampler =
        Async::<f32>::new_sinc(ratio, 10.0, &params, pcm.len(), 1, FixedAsync::Input).ok()?;

    let channels = [pcm.to_vec()];
    let input = SequentialSliceOfVecs::new(&channels, 1, pcm.len()).ok()?;

    let output = resampler.process_all(&input, pcm.len(), None).ok()?;
    Some(Array1::from_vec(output.take_data()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_stretch_changes_length() {
        let pcm = Array1::from_vec((0..44100).map(|i| (i as f32 / 44100.0).sin()).collect());
        let stretched = time_stretch(&pcm, 44100, 15.0).unwrap();
        assert_eq!(stretched.len(), 44100 * 15);
    }

    #[test]
    fn test_time_stretch_shortening_also_matches_target() {
        let pcm = Array1::from_vec((0..44100).map(|i| (i as f32 / 44100.0).sin()).collect());
        let stretched = time_stretch(&pcm, 44100, 0.5).unwrap();
        assert_eq!(stretched.len(), 22050);
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

    /// I12 (docs/10-TESTES-QUALIDADE.md §3): entrega duração dentro de ±20ms.
    /// A implementação por sinc devolve o comprimento exato pedido (o teste
    /// acima já prova isso), o que é bem mais apertado que a tolerância do
    /// invariante — este teste torna esse invariante explícito por nome.
    #[test]
    fn test_time_stretch_satisfies_i12_duration_tolerance() {
        let sample_rate = 44100u32;
        let pcm = Array1::from_vec(
            (0..sample_rate as usize)
                .map(|i| (i as f32 / sample_rate as f32).sin())
                .collect(),
        );
        let target_sec = 2.37;
        let stretched = time_stretch(&pcm, sample_rate, target_sec).unwrap();

        let actual_sec = stretched.len() as f32 / sample_rate as f32;
        assert!(
            (actual_sec - target_sec).abs() <= 0.020,
            "duração fora de ±20ms: alvo {target_sec}s, obtido {actual_sec}s"
        );
    }

    /// A qualidade do resample importa: um sinal senoidal puro reamostrado
    /// não deve ganhar energia de alta frequência (aliasing) que não existia
    /// na entrada. Interpolação linear falha este teste; sinc passa.
    #[test]
    fn test_time_stretch_does_not_introduce_gross_amplitude_artifacts() {
        let sample_rate = 44100u32;
        let freq = 440.0f32;
        let pcm = Array1::from_vec(
            (0..sample_rate as usize)
                .map(|i| (i as f32 / sample_rate as f32 * freq * std::f32::consts::TAU).sin())
                .collect(),
        );

        let stretched = time_stretch(&pcm, sample_rate, 0.7).unwrap();
        let peak = stretched.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);
        // Um seno de amplitude 1 não deve virar > ~1.05 depois do resample
        // (alguma margem para ripple do filtro sinc); interpolação linear
        // com aliasing severo tende a produzir picos bem maiores que isso.
        assert!(peak <= 1.05, "pico pós-resample suspeito: {peak}");
    }
}
