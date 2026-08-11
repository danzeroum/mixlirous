//! Geradores de sinal para os testes baseados em propriedade do
//! `docs/16-CORRECOES-DSP` Bloco 1 (T1.1).
//!
//! Gerados em c├│digo, nunca a partir de arquivo de ├íudio ÔÇö atende
//! `docs/09-MLOPS-GOLDEN-MASTER.md` sem quest├úo de licen├ºa, e ├® reprodut├¡vel
//! por semente (o `proptest` j├í grava o seed do caso que falhar).
//!
//! `pub` de prop├│sito: pensado para ser reusado via `mod generators;` a
//! partir de outro arquivo em `tests/` (T1.3 e al├®m), n├úo s├│ para rodar
//! sozinho.

use proptest::prelude::*;

pub const SAMPLE_RATE: f32 = 44_100.0;

/// Comprimento m├íximo de `arb_pcm()`/`arb_noise()`/`arb_transient()` ÔÇö cerca
/// de 4,3 s a 44,1 kHz. Grande o bastante para exercitar janelas deslizantes
/// (RMS, LUFS) sem tornar o proptest lento por caso.
const MAX_LEN: usize = 192_000;

/// Dura├º├úo fixa dos tons de `arb_sine()`: 0,5 s a 44,1 kHz ÔÇö cobre v├írias
/// repeti├º├Áes completas mesmo no extremo grave de 20 Hz (10 ciclos).
const TONE_LEN: usize = 22_050;

/// Caso geral: comprimento `0..=192000`, amostras em `-1.0..=1.0`. Inclui
/// buffer vazio e de uma amostra ÔÇö as bordas onde os bugs moram, n├úo s├│ o
/// meio do espa├ºo de entrada.
pub fn arb_pcm() -> impl Strategy<Value = Vec<f32>> {
    prop::collection::vec(-1.0f32..=1.0f32, 0..=MAX_LEN)
}

/// Onda senoidal com frequ├¬ncia (20 HzÔÇô20 kHz) e amplitude (0,0ÔÇô1,0)
/// arbitr├írias, a `SAMPLE_RATE`. Serve ├á verifica├º├úo anal├¡tica de RMS/LUFS ÔÇö
/// as duas t├¬m valor esperado fechado para um seno, o que um buffer aleat├│rio
/// n├úo d├í.
pub fn arb_sine() -> impl Strategy<Value = Vec<f32>> {
    (20.0f32..20_000.0f32, 0.0f32..=1.0f32).prop_map(|(freq_hz, amplitude)| {
        (0..TONE_LEN)
            .map(|i| amplitude * (i as f32 / SAMPLE_RATE * freq_hz * std::f32::consts::TAU).sin())
            .collect()
    })
}

/// Ru├¡do n├úo correlacionado a partir de uma semente arbitr├íria ÔÇö o caso real
/// de crossfade (blocos vindos de trechos diferentes da faixa, n├úo a mesma
/// amostra repetida). Um LCG simples: n├úo precisa de qualidade
/// criptogr├ífica, s├│ de determinismo a partir da semente que o `proptest`
/// j├í grava e reduz em caso de falha.
pub fn arb_noise() -> impl Strategy<Value = Vec<f32>> {
    (any::<u64>(), 100usize..=MAX_LEN).prop_map(|(seed, len)| {
        let mut state = seed | 1;
        (0..len)
            .map(|_| {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1);
                let bits = (state >> 40) as u32; // 24 bits ├║teis do topo
                (bits as f32 / (1u32 << 24) as f32) * 2.0 - 1.0
            })
            .collect()
    })
}

/// As bordas onde os bugs moram: sil├¬ncio, DC (constante arbitr├íria,
/// incluindo zero), ┬▒1,0 constante, e buffer de uma amostra.
///
/// Estrat├®gia pr├│pria, n├úo uma faixa cont├¡nua ÔÇö geradores uniformes quase
/// nunca produzem sil├¬ncio puro por acaso. **Ao combinar com outros
/// geradores numa propriedade (T1.2+), sorteie esta com peso alto** (ex.:
/// `prop_oneof![3 => arb_degenerate(), 1 => arb_pcm()]`) ÔÇö ├® aqui que B1ÔÇôB6
/// moram, n├úo no meio do espa├ºo de entrada.
pub fn arb_degenerate() -> impl Strategy<Value = Vec<f32>> {
    prop_oneof![
        Just(vec![0.0f32; 1000]),
        (-1.0f32..=1.0f32).prop_map(|dc| vec![dc; 1000]),
        Just(vec![1.0f32; 1000]),
        Just(vec![-1.0f32; 1000]),
        (-1.0f32..=1.0f32).prop_map(|v| vec![v]),
    ]
}

/// Cliques esparsos sobre um fundo baixo ÔÇö pico alto, loudness m├®dia baixa.
/// For├ºa o conflito entre teto de pico e alvo de LUFS descrito em
/// `docs/16` ┬º4 passo 6, que material uniforme n├úo consegue exercitar.
pub fn arb_transient() -> impl Strategy<Value = Vec<f32>> {
    (
        1000usize..=MAX_LEN,
        prop::collection::vec(0usize..1000, 1..20),
    )
        .prop_map(|(len, click_offsets)| {
            let mut pcm = vec![0.01f32; len];
            for offset in click_offsets {
                pcm[offset % len] = 1.0;
            }
            pcm
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    proptest! {
        #[test]
        fn arb_pcm_respects_bounds(pcm in arb_pcm()) {
            prop_assert!(pcm.len() <= MAX_LEN);
            prop_assert!(pcm.iter().all(|&s| (-1.0..=1.0).contains(&s)));
        }

        #[test]
        fn arb_sine_is_finite_and_bounded(pcm in arb_sine()) {
            prop_assert_eq!(pcm.len(), TONE_LEN);
            prop_assert!(pcm.iter().all(|s| s.is_finite() && (-1.0..=1.0).contains(s)));
        }

        #[test]
        fn arb_noise_respects_bounds(pcm in arb_noise()) {
            prop_assert!(!pcm.is_empty());
            prop_assert!(pcm.iter().all(|&s| s.is_finite() && (-1.0..=1.0).contains(&s)));
        }

        #[test]
        fn arb_degenerate_is_finite(pcm in arb_degenerate()) {
            prop_assert!(!pcm.is_empty());
            prop_assert!(pcm.iter().all(|s| s.is_finite()));
        }

        #[test]
        fn arb_transient_has_at_least_one_click(pcm in arb_transient()) {
            prop_assert!(pcm.contains(&1.0));
            prop_assert!(pcm.iter().all(|&s| (0.0..=1.0).contains(&s)));
        }
    }
}
