//! Geradores de sinal para os testes baseados em propriedade do
//! `docs/16-CORRECOES-DSP` Bloco 1 (T1.1).
//!
//! Gerados em código, nunca a partir de arquivo de áudio — atende
//! `docs/09-MLOPS-GOLDEN-MASTER.md` sem questão de licença, e é reprodutível
//! por semente (o `proptest` já grava o seed do caso que falhar).
//!
//! `pub` de propósito: pensado para ser reusado via `mod generators;` a
//! partir de outro arquivo em `tests/` (T1.3 e além), não só para rodar
//! sozinho.

use proptest::prelude::*;

pub const SAMPLE_RATE: f32 = 44_100.0;

/// Comprimento máximo de `arb_pcm()`/`arb_noise()`/`arb_transient()` — cerca
/// de 4,3 s a 44,1 kHz. Grande o bastante para exercitar janelas deslizantes
/// (RMS, LUFS) sem tornar o proptest lento por caso.
const MAX_LEN: usize = 192_000;

/// Duração fixa dos tons de `arb_sine()`: 0,5 s a 44,1 kHz — cobre várias
/// repetições completas mesmo no extremo grave de 20 Hz (10 ciclos).
const TONE_LEN: usize = 22_050;

/// Caso geral: comprimento `0..=192000`, amostras em `-1.0..=1.0`. Inclui
/// buffer vazio e de uma amostra — as bordas onde os bugs moram, não só o
/// meio do espaço de entrada.
pub fn arb_pcm() -> impl Strategy<Value = Vec<f32>> {
    prop::collection::vec(-1.0f32..=1.0f32, 0..=MAX_LEN)
}

/// Onda senoidal com frequência (20 Hz–20 kHz) e amplitude (0,0–1,0)
/// arbitrárias, a `SAMPLE_RATE`. Serve à verificação analítica de RMS/LUFS —
/// as duas têm valor esperado fechado para um seno, o que um buffer aleatório
/// não dá.
pub fn arb_sine() -> impl Strategy<Value = Vec<f32>> {
    (20.0f32..20_000.0f32, 0.0f32..=1.0f32).prop_map(|(freq_hz, amplitude)| {
        (0..TONE_LEN)
            .map(|i| amplitude * (i as f32 / SAMPLE_RATE * freq_hz * std::f32::consts::TAU).sin())
            .collect()
    })
}

/// Ruído não correlacionado a partir de uma semente arbitrária — o caso real
/// de crossfade (blocos vindos de trechos diferentes da faixa, não a mesma
/// amostra repetida). Um LCG simples: não precisa de qualidade
/// criptográfica, só de determinismo a partir da semente que o `proptest`
/// já grava e reduz em caso de falha.
pub fn arb_noise() -> impl Strategy<Value = Vec<f32>> {
    (any::<u64>(), 100usize..=MAX_LEN).prop_map(|(seed, len)| {
        let mut state = seed | 1;
        (0..len)
            .map(|_| {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1);
                let bits = (state >> 40) as u32; // 24 bits úteis do topo
                (bits as f32 / (1u32 << 24) as f32) * 2.0 - 1.0
            })
            .collect()
    })
}

/// As bordas onde os bugs moram: silêncio, DC (constante arbitrária,
/// incluindo zero), ±1,0 constante, e buffer de uma amostra.
///
/// Estratégia própria, não uma faixa contínua — geradores uniformes quase
/// nunca produzem silêncio puro por acaso. **Ao combinar com outros
/// geradores numa propriedade (T1.2+), sorteie esta com peso alto** (ex.:
/// `prop_oneof![3 => arb_degenerate(), 1 => arb_pcm()]`) — é aqui que B1–B6
/// moram, não no meio do espaço de entrada.
pub fn arb_degenerate() -> impl Strategy<Value = Vec<f32>> {
    prop_oneof![
        Just(vec![0.0f32; 1000]),
        (-1.0f32..=1.0f32).prop_map(|dc| vec![dc; 1000]),
        Just(vec![1.0f32; 1000]),
        Just(vec![-1.0f32; 1000]),
        (-1.0f32..=1.0f32).prop_map(|v| vec![v]),
    ]
}

/// Cliques esparsos sobre um fundo baixo — pico alto, loudness média baixa.
/// Força o conflito entre teto de pico e alvo de LUFS descrito em
/// `docs/16` §4 passo 6, que material uniforme não consegue exercitar.
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
