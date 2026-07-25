//! Teste de nulo em ajuste neutro — docs/17.1 §1.1, o item #1 da "Ordem
//! sugerida" (o de maior alcance: uma regra, várias ferramentas).
//!
//! Ferramenta em ajuste neutro que altera o sinal tem bug — quase sempre
//! arredondamento indevido, ganho aplicado duas vezes, ou conversão de tipo
//! perdendo precisão.
//!
//! **Escopo real, não o da tabela inteira.** `compression` e `dynamic_eq`
//! não têm DSP nenhum por trás ainda (issue #8 — ferramenta anunciada no
//! schema sem implementação) — não há o que testar. Cobertas aqui: as
//! quatro que existem e têm um "ajuste neutro" bem definido: `crossfade`,
//! `fade_in`/`fade_out`, `time_stretch`, e o ganho de normalização LUFS.

mod generators;

use audio_core::domain::CrossfadeCurve;
use audio_core::dsp::mastering::lufs::{apply_lufs_gain, measure_lufs, LufsGainOutcome};
use audio_core::dsp::mastering::stretch::time_stretch;
use audio_core::dsp::stitching::crossfade::crossfade_buffers;
use audio_core::dsp::stitching::fades::{apply_fade_in, apply_fade_out, FadeCurve};
use generators::arb_pcm;
use ndarray::Array1;
use proptest::prelude::*;

/// Pico da diferença amostra a amostra, em dB. `-inf` para diferença zero
/// (residual perfeito) — não "muito negativo", porque `log10(0)` não existe.
fn residual_db(a: &[f32], b: &[f32]) -> f32 {
    let pico = a
        .iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0f32, f32::max);
    if pico == 0.0 {
        f32::NEG_INFINITY
    } else {
        20.0 * pico.log10()
    }
}

proptest! {
    /// `fade_in`/`fade_out` com `duration_samples: 0` devolvem a entrada.
    #[test]
    fn fade_in_out_zero_duration_is_identity(x in arb_pcm()) {
        for curve in [FadeCurve::Linear, FadeCurve::Logarithmic, FadeCurve::Exponential] {
            let mut y_in = x.clone();
            apply_fade_in(&mut y_in, 0, 0, &curve);
            prop_assert_eq!(residual_db(&x, &y_in), f32::NEG_INFINITY);

            let mut y_out = x.clone();
            apply_fade_out(&mut y_out, 0, 0, &curve);
            prop_assert_eq!(residual_db(&x, &y_out), f32::NEG_INFINITY);
        }
    }

    /// `crossfade` com `duration_ms: 0` (fade_samples=0) devolve a
    /// concatenação simples: nada de A se sobrepõe, B substitui integralmente
    /// a partir do ponto de emenda. Buffers do mesmo tamanho para não
    /// disputar espaço com a semântica de truncamento de
    /// `crossfade_buffers` quando os dois lados têm tamanhos diferentes —
    /// isso é uma pergunta separada, não o que este teste cobre.
    #[test]
    fn crossfade_zero_duration_is_simple_concatenation(
        a in arb_pcm(), b_seed in arb_pcm()
    ) {
        prop_assume!(!a.is_empty());
        let b: Vec<f32> = b_seed.iter().cycle().take(a.len()).copied().collect();

        for curve in [CrossfadeCurve::ConstantGain, CrossfadeCurve::ConstantPower] {
            let mut out = a.clone();
            crossfade_buffers(&mut out, 0, &b, 0, 0, curve);
            prop_assert_eq!(residual_db(&out, &b), f32::NEG_INFINITY);
        }
    }

    /// `time_stretch` com o alvo igual à duração atual devolve a entrada —
    /// já é o comportamento hoje (curto-circuito explícito em `stretch.rs`
    /// quando `|atual - alvo| < 0.05s`). Fixture do próprio docs/17.1: "se a
    /// implementação sempre passar pelo reamostrador, o residual não vai ser
    /// zero — isso É o achado". Aqui não é: o caminho neutro é curto-circuitado.
    #[test]
    fn time_stretch_same_duration_is_identity(x in arb_pcm()) {
        prop_assume!(!x.is_empty());
        let sample_rate = 44_100u32;
        let pcm = Array1::from_vec(x.clone());
        let current_duration = x.len() as f32 / sample_rate as f32;

        let out = time_stretch(&pcm, sample_rate, current_duration).unwrap();
        prop_assert_eq!(residual_db(&x, out.as_slice().unwrap()), f32::NEG_INFINITY);
    }

    /// Normalizar para o próprio LUFS medido (alvo == atual) não move nada.
    /// Dois casos, os dois preservam a entrada: se `atual` é finito,
    /// `gain_db = alvo - atual = 0` exato, `gain_linear = 10^0 = 1.0`. Se
    /// `atual` é `-inf` (buffer curto/silencioso demais para formar bloco de
    /// gating — `x` vazio incluso, `arb_pcm()` sorteia isso), `atual - atual`
    /// seria `NaN` (forma indeterminada `-inf - (-inf)`, NÃO zero — cancelar
    /// "algebricamente" é falso aqui), e é exatamente por isso que
    /// `apply_lufs_gain` verifica `current.is_finite()` antes de calcular
    /// `gain_db` — devolve `UnmeasurableLoudness` sem tocar no buffer.
    #[test]
    fn lufs_normalization_to_own_measurement_is_identity(x in arb_pcm()) {
        let sample_rate = 44_100u32;
        let atual = measure_lufs(&Array1::from_vec(x.clone()), sample_rate);

        let mut y = x.clone();
        let outcome = apply_lufs_gain(&mut y, sample_rate, atual);
        let outcome_esperado = match outcome {
            LufsGainOutcome::Applied { gain_db } => gain_db == 0.0,
            LufsGainOutcome::UnmeasurableLoudness => true,
        };
        prop_assert!(outcome_esperado, "outcome inesperado: {outcome:?}");
        prop_assert_eq!(residual_db(&x, &y), f32::NEG_INFINITY);
    }
}
