//! Offset DC — docs/17.1 §7. Entrada com média zero não pode virar saída com
//! média diferente de zero. Barato, e pega assimetria em curva de ganho,
//! clipping unilateral e erro de sinal em coeficiente de filtro — DC não se
//! ouve, consome margem de pico e faz o medidor de true peak reportar errado.
//!
//! **Só para os estágios onde a propriedade é garantida, não onde é
//! plausível.** `brickwall_limiter` e `apply_lufs_gain` aplicam um único
//! ganho linear uniforme ao buffer inteiro — `média(g·x) = g·média(x)`, então
//! média zero na entrada implica média zero na saída por linearidade, sempre.
//! `fade_in`/`fade_out`/`crossfade` aplicam ganho **variável no tempo**: isso
//! NÃO preserva média zero em geral (reponderar amostras de forma desigual
//! pode empurrar a média para qualquer lado, dependendo de onde as amostras
//! positivas e negativas caem dentro da janela) — não é bug testável aqui,
//! é a curva fazendo o que se espera dela. Incluir esses estágios neste
//! teste seria afirmar uma propriedade que não é verdadeira em geral.

mod generators;

use audio_core::dsp::mastering::limiter::brickwall_limiter;
use audio_core::dsp::mastering::lufs::apply_lufs_gain;
use generators::arb_pcm;
use proptest::prelude::*;

fn media(x: &[f32]) -> f32 {
    if x.is_empty() {
        0.0
    } else {
        x.iter().sum::<f32>() / x.len() as f32
    }
}

fn de_media(x: &mut [f32]) {
    let m = media(x);
    for s in x.iter_mut() {
        *s -= m;
    }
}

proptest! {
    #[test]
    fn limiter_preserva_media_zero(mut x in arb_pcm()) {
        prop_assume!(!x.is_empty());
        de_media(&mut x);
        let mut y = x.clone();
        brickwall_limiter(&mut y, -3.0);
        prop_assert!(media(&y).abs() < 1e-4, "limiter introduziu DC: {}", media(&y));
    }

    #[test]
    fn lufs_gain_preserva_media_zero(mut x in arb_pcm()) {
        prop_assume!(!x.is_empty());
        de_media(&mut x);
        let mut y = x.clone();
        apply_lufs_gain(&mut y, 44_100, -14.0);
        prop_assert!(media(&y).abs() < 1e-4, "ganho LUFS introduziu DC: {}", media(&y));
    }
}
