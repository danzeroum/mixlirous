//! Propriedades de emenda ÔÇö `docs/16-CORRECOES-DSP` T1.3, focado no que T2.2
//! corrige (pot├¬ncia constante). As identidades matem├íticas exatas
//! (`gain_a + gain_b = 1`, `gain_a┬▓ + gain_b┬▓ = 1`) j├í s├úo casos fixos em
//! `dsp::stitching::crossfade` ÔÇö o que s├│ propriedade pega ├® o que caso fixo
//! n├úo escreve sozinho: as bordas do espa├ºo de entrada.
//!
//! Escopo deliberadamente menor que o T1.3 completo do docs/16: sem
//! `descontinuidade_max`/I4.1 nem `queda_rms_db`/I4.2 em janela deslizante ÔÇö
//! essas dependem de infraestrutura de an├ílise que ainda n├úo existe e n├úo ├®
//! necess├íria para a fatia vertical. Cobertas aqui: I15 (finitude) e a
//! identidade de ganho constante, as duas que T2.2 precisa para ser
//! confi├ível.

mod generators;

use audio_core::dsp::stitching::crossfade::crossfade_buffers;
use audio_core::CrossfadeCurve;
use generators::{arb_degenerate, arb_noise, arb_pcm};
use proptest::prelude::*;

fn crossfade_copy(a: &[f32], b: &[f32], fade_len: usize, curve: CrossfadeCurve) -> Vec<f32> {
    let mut out = a.to_vec();
    crossfade_buffers(&mut out, 0, b, 0, fade_len, curve);
    out
}

proptest! {
    /// I15 ÔÇö pega o B1 (divis├úo por zero da curva quebrada, que fazia toda a
    /// regi├úo virar NaN/inf): nenhuma amostra de sa├¡da pode deixar de ser
    /// finita, para nenhuma curva, em nenhum ponto do espa├ºo de entrada ÔÇö
    /// incluindo os degenerados (sil├¬ncio, DC, uma amostra), pesados mais
    /// porque geradores uniformes quase nunca sorteiam essas bordas sozinhos.
    #[test]
    fn crossfade_output_is_always_finite(
        a in prop_oneof![3 => arb_degenerate(), 1 => arb_pcm(), 1 => arb_noise()],
        b in prop_oneof![3 => arb_degenerate(), 1 => arb_pcm(), 1 => arb_noise()],
        fade_len in 0usize..=3000,
        curve in prop::sample::select(vec![CrossfadeCurve::ConstantGain, CrossfadeCurve::ConstantPower]),
    ) {
        prop_assume!(!a.is_empty() && !b.is_empty());
        let out = crossfade_copy(&a, &b, fade_len, curve);
        prop_assert!(out.iter().all(|s| s.is_finite()));
    }

    /// Emendar um bloco a si mesmo com ganho constante devolve o pr├│prio
    /// bloco, para qualquer sinal gerado ÔÇö n├úo s├│ o exemplo fixo em
    /// `crossfade.rs`. ├ë identidade matem├ítica (`gain_a + gain_b = 1`,
    /// `a == b`), ent├úo n├úo deveria precisar de toler├óncia al├®m de erro de
    /// ponto flutuante.
    #[test]
    fn constant_gain_crossfade_of_signal_with_itself_is_identity(
        x in prop_oneof![3 => arb_degenerate(), 1 => arb_pcm(), 1 => arb_noise()],
        fade_len in 0usize..=3000,
    ) {
        prop_assume!(!x.is_empty());
        let out = crossfade_copy(&x, &x, fade_len, CrossfadeCurve::ConstantGain);
        prop_assert_eq!(out.len(), x.len());
        for (o, orig) in out.iter().zip(x.iter()) {
            prop_assert!((o - orig).abs() < 1e-4);
        }
    }
}
