//! Latência declarada — docs/17.1 §2. "Escreva este teste agora, antes do
//! T3.2": um limiter com look-ahead atrasa o sinal pelo tamanho da janela de
//! antecipação, e esse atraso, se não for compensado, sai deslocado no
//! tempo — inaudível isolado, mas destrói o alinhamento em emenda e
//! comparação A/B. Passa em todo teste de LUFS, pico e finitude, então só
//! aparece aqui.
//!
//! **Escopo de hoje.** Nenhum estágio atual usa linha de atraso — todos
//! processam amostra `i` da entrada para amostra `i` da saída, sem deslocar
//! nada no tempo (ganho uniforme ou por curva, nunca um FIFO). Os quatro
//! testes abaixo confirmam latência zero nos estágios que existem hoje;
//! quando o T3.2 (limiter com look-ahead, `docs/16`) for implementado, ele
//! precisa declarar sua janela de antecipação como latência real, e o teste
//! correspondente aqui deixa de valer zero — é o que força a compensação a
//! não ser esquecida.
//!
//! `time_stretch` fica de fora: mudar a duração desloca todo o sinal
//! proporcionalmente por design (não é um atraso fixo, é retiming), então
//! "posição do pico não muda" não é a propriedade certa para ele.

use audio_core::domain::CrossfadeCurve;
use audio_core::dsp::mastering::limiter::brickwall_limiter;
use audio_core::dsp::mastering::lufs::{apply_lufs_gain, LufsGainOutcome};
use audio_core::dsp::stitching::crossfade::crossfade_buffers;
use audio_core::dsp::stitching::fades::{apply_fade_in, apply_fade_out, FadeCurve};

const SAMPLE_RATE: usize = 44_100;
const IMPULSE_IDX: usize = 1000;

fn impulso() -> Vec<f32> {
    let mut x = vec![0.0f32; SAMPLE_RATE];
    x[IMPULSE_IDX] = 0.9;
    x
}

fn argmax_abs(x: &[f32]) -> usize {
    x.iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.abs().partial_cmp(&b.abs()).unwrap())
        .map(|(i, _)| i)
        .unwrap()
}

/// Latência declarada de cada estágio hoje: zero. Ver comentário do módulo —
/// é aqui que o T3.2 precisa atualizar quando o limiter ganhar look-ahead.
const LATENCIA_FADE_IN: usize = 0;
const LATENCIA_FADE_OUT: usize = 0;
const LATENCIA_CROSSFADE: usize = 0;
const LATENCIA_LIMITER: usize = 0;
const LATENCIA_LUFS_GAIN: usize = 0;

#[test]
fn fade_in_nao_desloca_o_impulso() {
    for curve in [
        FadeCurve::Linear,
        FadeCurve::Logarithmic,
        FadeCurve::Exponential,
    ] {
        let mut x = impulso();
        let len = x.len();
        apply_fade_in(&mut x, 0, len, &curve);
        assert_eq!(
            argmax_abs(&x) as isize - IMPULSE_IDX as isize,
            LATENCIA_FADE_IN as isize,
            "{curve:?}: atraso real diverge do declarado"
        );
    }
}

#[test]
fn fade_out_nao_desloca_o_impulso() {
    for curve in [
        FadeCurve::Linear,
        FadeCurve::Logarithmic,
        FadeCurve::Exponential,
    ] {
        let mut x = impulso();
        let len = x.len();
        apply_fade_out(&mut x, 0, len, &curve);
        assert_eq!(
            argmax_abs(&x) as isize - IMPULSE_IDX as isize,
            LATENCIA_FADE_OUT as isize,
            "{curve:?}: atraso real diverge do declarado"
        );
    }
}

#[test]
fn crossfade_nao_desloca_o_impulso() {
    for curve in [CrossfadeCurve::ConstantGain, CrossfadeCurve::ConstantPower] {
        let mut a = impulso();
        let b = vec![0.0f32; SAMPLE_RATE]; // silêncio: só o impulso de A pode ser o pico
        let len = a.len();
        crossfade_buffers(&mut a, 0, &b, 0, len, curve);
        assert_eq!(
            argmax_abs(&a) as isize - IMPULSE_IDX as isize,
            LATENCIA_CROSSFADE as isize,
            "{curve:?}: atraso real diverge do declarado"
        );
    }
}

#[test]
fn limiter_nao_desloca_o_impulso() {
    let mut x = impulso();
    brickwall_limiter(&mut x, -6.0); // abaixo do pico de 0.9 (~-0.9 dBFS): força a escalar
    assert_eq!(
        argmax_abs(&x) as isize - IMPULSE_IDX as isize,
        LATENCIA_LIMITER as isize,
        "atraso real diverge do declarado"
    );
}

#[test]
fn lufs_gain_nao_desloca_o_impulso() {
    let mut x = impulso();
    let outcome = apply_lufs_gain(&mut x, SAMPLE_RATE as u32, -10.0);
    assert!(matches!(outcome, LufsGainOutcome::Applied { .. }));
    assert_eq!(
        argmax_abs(&x) as isize - IMPULSE_IDX as isize,
        LATENCIA_LUFS_GAIN as isize,
        "atraso real diverge do declarado"
    );
}
