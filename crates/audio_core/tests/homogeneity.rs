//! Homogeneidade dos processos lineares — docs/17.1 §8.
//! `processar(k*x) == k*processar(x)` para tudo que é (e deveria ser)
//! linear. Se divergir, há não linearidade escondida: um clamp, uma
//! saturação, ou um limiar absoluto onde não deveria haver.
//!
//! **Só para as funções onde a propriedade é garantida por construção, não
//! plausível.** `apply_fade_in`/`apply_fade_out`/`crossfade_buffers`
//! multiplicam cada amostra por um ganho que depende só da posição (curva
//! de tempo, nunca do valor da amostra) — homogêneas por definição.
//! `time_stretch` interpola com coeficientes fixos por posição (o filtro
//! sinc do `rubato`), também independentes da amplitude de entrada —
//! homogênea. `brickwall_limiter` e `apply_lufs_gain` são **deliberadamente
//! não homogêneos**: o primeiro só escala quando o pico ultrapassa um
//! limiar absoluto (escalar `x` por 0,5 pode desativar o gatilho que
//! disparava para `x`); o segundo normaliza para um alvo de LUFS absoluto,
//! então dobrar a entrada não dobra a saída — ele ativamente contra-ajusta
//! a mudança de escala, que é o ponto de existir. Afirmar homogeneidade
//! nesses dois seria afirmar algo falso por design, não lacuna de
//! cobertura.

mod generators;

use audio_core::domain::CrossfadeCurve;
use audio_core::dsp::mastering::stretch::time_stretch;
use audio_core::dsp::stitching::crossfade::crossfade_buffers;
use audio_core::dsp::stitching::fades::{apply_fade_in, apply_fade_out, FadeCurve};
use generators::arb_pcm;
use ndarray::Array1;
use proptest::prelude::*;

/// Escala usada em todo o arquivo: 0,5, não 2,0, para não esbarrar em fundo
/// de escala ao dobrar amostras já perto de ±1.0 (docs/17.1 §8).
const ESCALA: f32 = 0.5;

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

fn escalar(x: &[f32], k: f32) -> Vec<f32> {
    x.iter().map(|&v| v * k).collect()
}

proptest! {
    #[test]
    fn fade_in_out_sao_homogeneos(x in arb_pcm()) {
        for curve in [FadeCurve::Linear, FadeCurve::Logarithmic, FadeCurve::Exponential] {
            let len = x.len();

            let mut a = escalar(&x, ESCALA);
            apply_fade_in(&mut a, 0, len, &curve);

            let mut b = x.clone();
            apply_fade_in(&mut b, 0, len, &curve);
            let b_escalado = escalar(&b, ESCALA);

            prop_assert!(
                residual_db(&a, &b_escalado) <= -80.0,
                "{curve:?} fade_in não homogêneo"
            );

            let mut c = escalar(&x, ESCALA);
            apply_fade_out(&mut c, 0, len, &curve);

            let mut d = x.clone();
            apply_fade_out(&mut d, 0, len, &curve);
            let d_escalado = escalar(&d, ESCALA);

            prop_assert!(
                residual_db(&c, &d_escalado) <= -80.0,
                "{curve:?} fade_out não homogêneo"
            );
        }
    }

    #[test]
    fn crossfade_e_homogeneo(a in arb_pcm(), b_seed in arb_pcm()) {
        prop_assume!(!a.is_empty());
        let b: Vec<f32> = b_seed.iter().cycle().take(a.len()).copied().collect();
        let fade_samples = a.len() / 2;

        for curve in [CrossfadeCurve::ConstantGain, CrossfadeCurve::ConstantPower] {
            let mut esc_depois = escalar(&a, ESCALA);
            let b_esc = escalar(&b, ESCALA);
            crossfade_buffers(&mut esc_depois, 0, &b_esc, 0, fade_samples, curve);

            let mut antes = a.clone();
            crossfade_buffers(&mut antes, 0, &b, 0, fade_samples, curve);
            let antes_escalado = escalar(&antes, ESCALA);

            prop_assert!(
                residual_db(&esc_depois, &antes_escalado) <= -80.0,
                "{curve:?} crossfade não homogêneo"
            );
        }
    }

    // Faixa de tamanho própria, bem menor que `arb_pcm()` (até 192_000): o
    // `rubato::Async` reconstrói o filtro sinc do zero em cada chamada, e
    // este teste chama `time_stretch` duas vezes por caso — com o tamanho
    // completo de `arb_pcm()`, 256 casos padrão do proptest levaram **648
    // segundos** (medido, não estimado) só para esta propriedade. A
    // homogeneidade não depende do tamanho do buffer para se manifestar;
    // 200..=4000 amostras já exercita a propriedade e roda em segundos.
    #[test]
    fn time_stretch_e_homogeneo(x in prop::collection::vec(-1.0f32..=1.0f32, 200..=4000)) {
        let sample_rate = 44_100u32;
        let target_sec = (x.len() as f32 / sample_rate as f32) * 0.7;

        let a = time_stretch(&Array1::from_vec(escalar(&x, ESCALA)), sample_rate, target_sec).unwrap();

        let b = time_stretch(&Array1::from_vec(x), sample_rate, target_sec).unwrap();
        let b_escalado = escalar(b.as_slice().unwrap(), ESCALA);

        prop_assert_eq!(a.len(), b_escalado.len());
        prop_assert!(
            residual_db(a.as_slice().unwrap(), &b_escalado) <= -80.0,
            "time_stretch não homogêneo"
        );
    }
}
