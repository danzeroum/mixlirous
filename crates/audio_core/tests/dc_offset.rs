//! Offset DC ÔÇö docs/17.1 ┬º7. Entrada com m├®dia zero n├úo pode virar sa├¡da com
//! m├®dia diferente de zero. Barato, e pega assimetria em curva de ganho,
//! clipping unilateral e erro de sinal em coeficiente de filtro ÔÇö DC n├úo se
//! ouve, consome margem de pico e faz o medidor de true peak reportar errado.
//!
//! **S├│ para os est├ígios onde a propriedade ├® garantida, n├úo onde ├®
//! plaus├¡vel.** `brickwall_limiter` e `apply_lufs_gain` aplicam um ├║nico
//! ganho linear uniforme ao buffer inteiro ÔÇö `m├®dia(g┬Àx) = g┬Àm├®dia(x)`, ent├úo
//! m├®dia zero na entrada implica m├®dia zero na sa├¡da por linearidade, sempre.
//! `fade_in`/`fade_out`/`crossfade` aplicam ganho **vari├ível no tempo**: isso
//! N├âO preserva m├®dia zero em geral (reponderar amostras de forma desigual
//! pode empurrar a m├®dia para qualquer lado, dependendo de onde as amostras
//! positivas e negativas caem dentro da janela) ÔÇö n├úo ├® bug test├ível aqui,
//! ├® a curva fazendo o que se espera dela. Incluir esses est├ígios neste
//! teste seria afirmar uma propriedade que n├úo ├® verdadeira em geral.

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
        // Aplicado ou n├úo (buffer curto demais para medir loudness), a
        // propriedade tem que valer nos dois casos ÔÇö n├úo afirmamos qual.
        let _ = apply_lufs_gain(&mut y, 44_100, -14.0);
        prop_assert!(media(&y).abs() < 1e-4, "ganho LUFS introduziu DC: {}", media(&y));
    }
}
