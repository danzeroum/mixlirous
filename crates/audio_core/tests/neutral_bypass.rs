//! Teste de nulo em ajuste neutro ÔÇö docs/17.1 ┬º1.1, o item #1 da "Ordem
//! sugerida" (o de maior alcance: uma regra, v├írias ferramentas).
//!
//! Ferramenta em ajuste neutro que altera o sinal tem bug ÔÇö quase sempre
//! arredondamento indevido, ganho aplicado duas vezes, ou convers├úo de tipo
//! perdendo precis├úo.
//!
//! **Escopo real, n├úo o da tabela inteira.** `compression` e `dynamic_eq`
//! n├úo t├¬m DSP nenhum por tr├ís ainda (issue #8 ÔÇö ferramenta anunciada no
//! schema sem implementa├º├úo) ÔÇö n├úo h├í o que testar. Cobertas aqui: as
//! quatro que existem e t├¬m um "ajuste neutro" bem definido: `crossfade`,
//! `fade_in`/`fade_out`, `time_stretch`, e o ganho de normaliza├º├úo LUFS.

mod generators;

use audio_core::domain::CrossfadeCurve;
use audio_core::dsp::mastering::lufs::{apply_lufs_gain, measure_lufs, LufsGainOutcome};
use audio_core::dsp::mastering::stretch::time_stretch;
use audio_core::dsp::stitching::crossfade::crossfade_buffers;
use audio_core::dsp::stitching::fades::{apply_fade_in, apply_fade_out, FadeCurve};
use generators::arb_pcm;
use ndarray::Array1;
use proptest::prelude::*;

/// Pico da diferen├ºa amostra a amostra, em dB. `-inf` para diferen├ºa zero
/// (residual perfeito) ÔÇö n├úo "muito negativo", porque `log10(0)` n├úo existe.
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
    /// concatena├º├úo simples: nada de A se sobrep├Áe, B substitui integralmente
    /// a partir do ponto de emenda. Buffers do mesmo tamanho para n├úo
    /// disputar espa├ºo com a sem├óntica de truncamento de
    /// `crossfade_buffers` quando os dois lados t├¬m tamanhos diferentes ÔÇö
    /// isso ├® uma pergunta separada, n├úo o que este teste cobre.
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

    /// `time_stretch` com o alvo igual ├á dura├º├úo atual devolve a entrada ÔÇö
    /// j├í ├® o comportamento hoje (curto-circuito expl├¡cito em `stretch.rs`
    /// quando `|atual - alvo| < 0.05s`). Fixture do pr├│prio docs/17.1: "se a
    /// implementa├º├úo sempre passar pelo reamostrador, o residual n├úo vai ser
    /// zero ÔÇö isso ├ë o achado". Aqui n├úo ├®: o caminho neutro ├® curto-circuitado.
    #[test]
    fn time_stretch_same_duration_is_identity(x in arb_pcm()) {
        prop_assume!(!x.is_empty());
        let sample_rate = 44_100u32;
        let pcm = Array1::from_vec(x.clone());
        let current_duration = x.len() as f32 / sample_rate as f32;

        let out = time_stretch(&pcm, sample_rate, current_duration).unwrap();
        prop_assert_eq!(residual_db(&x, out.as_slice().unwrap()), f32::NEG_INFINITY);
    }

    /// Normalizar para o pr├│prio LUFS medido (alvo == atual) n├úo move nada.
    /// Dois casos, os dois preservam a entrada: se `atual` ├® finito,
    /// `gain_db = alvo - atual = 0` exato, `gain_linear = 10^0 = 1.0`. Se
    /// `atual` ├® `-inf` (buffer curto/silencioso demais para formar bloco de
    /// gating ÔÇö `x` vazio incluso, `arb_pcm()` sorteia isso), `atual - atual`
    /// seria `NaN` (forma indeterminada `-inf - (-inf)`, N├âO zero ÔÇö cancelar
    /// "algebricamente" ├® falso aqui), e ├® exatamente por isso que
    /// `apply_lufs_gain` verifica `current.is_finite()` antes de calcular
    /// `gain_db` ÔÇö devolve `UnmeasurableLoudness` sem tocar no buffer.
    #[test]
    fn lufs_normalization_to_own_measurement_is_identity(x in arb_pcm()) {
        let sample_rate = 44_100u32;
        let atual = measure_lufs(&Array1::from_vec(x.clone()), sample_rate);

        let mut y = x.clone();
        let outcome = apply_lufs_gain(&mut y, sample_rate, atual);
        let outcome_esperado = match outcome {
            LufsGainOutcome::Applied { gain_db, .. } => gain_db == 0.0,
            LufsGainOutcome::UnmeasurableLoudness => true,
        };
        prop_assert!(outcome_esperado, "outcome inesperado: {outcome:?}");
        prop_assert_eq!(residual_db(&x, &y), f32::NEG_INFINITY);
    }
}
