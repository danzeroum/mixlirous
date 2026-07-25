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

/// Resultado de `apply_lufs_gain`. `#[must_use]` de propósito: a variante
/// `UnmeasurableLoudness` significa que o buffer **não foi tocado** — se o
/// chamador ignorar o retorno, o áudio sai sem normalizar e ninguém sabe,
/// exatamente o padrão de falha silenciosa que este projeto vem eliminando
/// em outras camadas (ex.: `loudness_target_conflict`). Quando a cadeia de
/// `warnings[]` existir (`docs/03-ADENDO-R2-CONTRATOS.md` §1, T3.3 do
/// `docs/16`), esta variante vira o código `unmeasurable_loudness` — mesma
/// família, mesma regra: aviso não bloqueante, não muda o estado do job.
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LufsGainOutcome {
    /// Ganho aplicado; `gain_db` é o quanto foi ajustado.
    Applied { gain_db: f32 },
    /// `measure_lufs` devolveu um valor não finito (buffer curto ou
    /// silencioso demais para formar um bloco de gating da BS.1770 — retorno
    /// válido de `loudness_global()`, não erro). Sem loudness mensurável não
    /// há ganho coerente a calcular; o buffer não foi tocado.
    UnmeasurableLoudness,
}

/// Aplica ganho para atingir target LUFS. Ver `LufsGainOutcome`.
pub fn apply_lufs_gain(pcm: &mut [f32], sample_rate: u32, target_lufs: f32) -> LufsGainOutcome {
    let current = measure_lufs(&Array1::from_vec(pcm.to_vec()), sample_rate);
    if !current.is_finite() {
        return LufsGainOutcome::UnmeasurableLoudness;
    }

    let gain_db = target_lufs - current;
    let gain_linear = 10f32.powf(gain_db / 20.0);
    // `target - current` pode ser não finito por outras vias além de
    // `current` (ex.: `target_lufs` não finito, responsabilidade do
    // chamador) — mesma guarda, mesmo motivo: `0.0 * inf = NaN`.
    if !gain_linear.is_finite() {
        return LufsGainOutcome::UnmeasurableLoudness;
    }

    for sample in pcm.iter_mut() {
        *sample *= gain_linear;
    }
    LufsGainOutcome::Applied { gain_db }
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

    /// Regressão: um buffer curto o bastante para não formar um bloco de
    /// gating da BS.1770 mede -inf LUFS (retorno válido de `loudness_global`,
    /// não sentinela). `target - (-inf) = +inf`; sem a guarda de
    /// `is_finite()`, multiplicar por ganho infinito produzia +-inf e, em
    /// qualquer amostra exatamente 0.0, NaN — contaminando o buffer inteiro.
    /// Achado pelo teste de offset DC de docs/17.1 §7 (`dc_offset.rs`), com
    /// um caso de 17 amostras.
    #[test]
    fn test_apply_lufs_gain_does_not_corrupt_buffer_with_unmeasurable_loudness() {
        let mut pcm = vec![-0.2f32, 0.3, -0.1, 0.15, 0.0];
        assert!(!measure_lufs(&Array1::from_vec(pcm.clone()), 44100).is_finite());

        let outcome = apply_lufs_gain(&mut pcm, 44100, -14.0);

        assert_eq!(outcome, LufsGainOutcome::UnmeasurableLoudness);
        assert!(
            pcm.iter().all(|s| s.is_finite()),
            "buffer contaminado com NaN/Inf: {pcm:?}"
        );
    }

    /// I11 (docs/10-TESTES-QUALIDADE.md §3): após normalização, |lufs-alvo| <= 0.5 LU.
    #[test]
    fn test_apply_lufs_gain_satisfies_i11_tolerance() {
        let mut pcm = vec![0.05f32; 44100];
        let outcome = apply_lufs_gain(&mut pcm, 44100, -14.0);
        assert!(matches!(outcome, LufsGainOutcome::Applied { .. }));
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
