//! Distorção harmônica (THD) — docs/17.1 §3.1. Um tom puro entra, processa,
//! mede a energia nos harmônicos (2x, 3x... da fundamental) contra a energia
//! na fundamental. THD alta significa que um processo que **não deveria**
//! distorcer está distorcendo — clipping intermediário, interpolação ruim
//! ou erro de quantização.
//!
//! **Só para as funções cujo ganho é um único escalar aplicado ao buffer
//! inteiro, não uma curva no tempo.** `apply_lufs_gain` e
//! `brickwall_limiter` calculam **um** fator de ganho (da medição de LUFS ou
//! do pico global) e multiplicam todas as amostras por ele — matematicamente
//! não pode introduzir harmônicos novos, só reescalar os que já existem.
//! `fade_in`/`fade_out`/`crossfade` aplicam ganho **variável no tempo**: medir
//! THD com FFT de janela única sobre um sinal cuja envoltória está mudando
//! dentro da própria janela mistura modulação de amplitude (esperada, não é
//! distorção) com distorção harmônica de verdade — o mesmo problema de
//! "espalhamento" que descartou o chirp como sinal de teste em
//! `aliasing.rs`. Não testado aqui por esse motivo, não por lacuna.
//! `time_stretch` em fator 1,0 é bypass puro (`stretch.rs` já testa a
//! identidade) — incluído como caso trivial de referência.

use audio_core::dsp::analysis::fft::magnitude_spectrum;
use audio_core::dsp::mastering::limiter::brickwall_limiter;
use audio_core::dsp::mastering::lufs::apply_lufs_gain;
use audio_core::dsp::mastering::stretch::time_stretch;
use ndarray::Array1;

const SAMPLE_RATE: u32 = 44_100;
const FUNDAMENTAL_HZ: f32 = 1000.0;

/// Tom puro de 1 kHz, duração de 1s — número inteiro de ciclos a 44100 Hz,
/// então a FFT de janela única não borra a fundamental entre bins.
fn tom_puro() -> Vec<f32> {
    (0..SAMPLE_RATE)
        .map(|i| {
            0.5 * (2.0 * std::f32::consts::PI * FUNDAMENTAL_HZ * i as f32 / SAMPLE_RATE as f32)
                .sin()
        })
        .collect()
}

/// sqrt(energia nos harmônicos 2..=10 / energia na fundamental) — mesma
/// definição do docs/17.1 §3.1.
fn thd(pcm: &[f32]) -> f32 {
    let mag = magnitude_spectrum(Array1::from_vec(pcm.to_vec()).view());
    let bin_hz = SAMPLE_RATE as f32 / pcm.len() as f32;
    let fund_bin = (FUNDAMENTAL_HZ / bin_hz).round() as usize;
    let fund_energy = mag[fund_bin] * mag[fund_bin];

    let harm_energy: f32 = (2..=10)
        .filter_map(|h| mag.get(fund_bin * h))
        .map(|&m| m * m)
        .sum();

    (harm_energy / fund_energy).sqrt()
}

const THD_MAX: f32 = 0.001; // 0,1% — mesmo teto sugerido em docs/17.1 §3.1

#[test]
fn apply_lufs_gain_nao_distorce() {
    let mut pcm = tom_puro();
    let outcome = apply_lufs_gain(&mut pcm, SAMPLE_RATE, -14.0);
    assert!(matches!(
        outcome,
        audio_core::dsp::mastering::lufs::LufsGainOutcome::Applied { .. }
    ));
    let t = thd(&pcm);
    assert!(t < THD_MAX, "THD {:.4}% após apply_lufs_gain", t * 100.0);
}

#[test]
fn brickwall_limiter_nao_distorce_abaixo_do_teto() {
    let mut pcm = tom_puro(); // pico 0.5 (~-6 dBFS)
    brickwall_limiter(&mut pcm, -3.0); // teto acima do pico: não deveria escalar
    let t = thd(&pcm);
    assert!(
        t < THD_MAX,
        "THD {:.4}% após brickwall_limiter (abaixo do teto)",
        t * 100.0
    );
}

#[test]
fn brickwall_limiter_nao_distorce_ao_escalar() {
    let mut pcm = tom_puro(); // pico 0.5 (~-6 dBFS)
    brickwall_limiter(&mut pcm, -12.0); // teto abaixo do pico: força escalar
    let t = thd(&pcm);
    assert!(
        t < THD_MAX,
        "THD {:.4}% após brickwall_limiter (escalando)",
        t * 100.0
    );
}

#[test]
fn time_stretch_em_fator_1_nao_distorce() {
    let pcm = tom_puro();
    let duration = pcm.len() as f32 / SAMPLE_RATE as f32;
    let esticado = time_stretch(&Array1::from_vec(pcm), SAMPLE_RATE, duration).unwrap();
    let t = thd(esticado.as_slice().unwrap());
    assert!(
        t < THD_MAX,
        "THD {:.4}% após time_stretch em fator 1,0",
        t * 100.0
    );
}
