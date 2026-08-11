//! Lat├¬ncia declarada ÔÇö docs/17.1 ┬º2. "Escreva este teste agora, antes do
//! T3.2": um limiter com look-ahead atrasa o sinal pelo tamanho da janela de
//! antecipa├º├úo, e esse atraso, se n├úo for compensado, sai deslocado no
//! tempo ÔÇö inaud├¡vel isolado, mas destr├│i o alinhamento em emenda e
//! compara├º├úo A/B. Passa em todo teste de LUFS, pico e finitude, ent├úo s├│
//! aparece aqui.
//!
//! **Escopo de hoje.** Nenhum est├ígio atual usa linha de atraso ÔÇö todos
//! processam amostra `i` da entrada para amostra `i` da sa├¡da, sem deslocar
//! nada no tempo (ganho uniforme ou por curva, nunca um FIFO). Os quatro
//! testes abaixo confirmam lat├¬ncia zero nos est├ígios que existem hoje;
//! quando o T3.2 (limiter com look-ahead, `docs/16`) for implementado, ele
//! precisa declarar sua janela de antecipa├º├úo como lat├¬ncia real, e o teste
//! correspondente aqui deixa de valer zero ÔÇö ├® o que for├ºa a compensa├º├úo a
//! n├úo ser esquecida.
//!
//! `time_stretch` fica de fora: mudar a dura├º├úo desloca todo o sinal
//! proporcionalmente por design (n├úo ├® um atraso fixo, ├® retiming), ent├úo
//! "posi├º├úo do pico n├úo muda" n├úo ├® a propriedade certa para ele.

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

/// Lat├¬ncia declarada de cada est├ígio hoje: zero. Ver coment├írio do m├│dulo ÔÇö
/// ├® aqui que o T3.2 precisa atualizar quando o limiter ganhar look-ahead.
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
        let b = vec![0.0f32; SAMPLE_RATE]; // sil├¬ncio: s├│ o impulso de A pode ser o pico
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
    brickwall_limiter(&mut x, -6.0); // abaixo do pico de 0.9 (~-0.9 dBFS): for├ºa a escalar
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
