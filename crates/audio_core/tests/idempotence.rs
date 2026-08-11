//! Idempot├¬ncia da normaliza├º├úo ÔÇö docs/17.1 ┬º1.2. Normalizar para o mesmo
//! alvo de LUFS duas vezes tem que dar o mesmo que uma vez. Se divergir, o
//! normalizador tem estado ou o medidor n├úo ├® est├ível.
//!
//! Matematicamente esperado ser exato, n├úo s├│ pr├│ximo: LUFS escala em dB
//! com a amplitude linear, ent├úo `apply_lufs_gain` na primeira chamada leva
//! a medi├º├úo a `target_lufs` (a menos de arredondamento de ponto flutuante);
//! a segunda chamada mede isso, calcula `gain_db ~= 0`, e n├úo move nada.

mod generators;

use audio_core::dsp::mastering::lufs::apply_lufs_gain;
use generators::arb_pcm;
use proptest::prelude::*;

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
    #[test]
    fn lufs_normalization_e_idempotente(x in arb_pcm()) {
        let sample_rate = 44_100u32;

        let mut uma = x.clone();
        let _ = apply_lufs_gain(&mut uma, sample_rate, -14.0);

        let mut duas = uma.clone();
        let _ = apply_lufs_gain(&mut duas, sample_rate, -14.0);

        prop_assert!(
            residual_db(&uma, &duas) <= -100.0,
            "segunda normaliza├º├úo moveu o sinal ÔÇö residual {} dB",
            residual_db(&uma, &duas)
        );
    }
}
