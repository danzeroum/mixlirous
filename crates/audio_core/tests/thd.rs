//! Distor├º├úo harm├┤nica (THD) ÔÇö docs/17.1 ┬º3.1. Um tom puro entra, processa,
//! mede a energia nos harm├┤nicos (2x, 3x... da fundamental) contra a energia
//! na fundamental. THD alta significa que um processo que **n├úo deveria**
//! distorcer est├í distorcendo ÔÇö clipping intermedi├írio, interpola├º├úo ruim
//! ou erro de quantiza├º├úo.
//!
//! **S├│ para as fun├º├Áes cujo ganho ├® um ├║nico escalar aplicado ao buffer
//! inteiro, n├úo uma curva no tempo.** `apply_lufs_gain` e
//! `brickwall_limiter` calculam **um** fator de ganho (da medi├º├úo de LUFS ou
//! do pico global) e multiplicam todas as amostras por ele ÔÇö matematicamente
//! n├úo pode introduzir harm├┤nicos novos, s├│ reescalar os que j├í existem.
//! `fade_in`/`fade_out`/`crossfade` aplicam ganho **vari├ível no tempo**: medir
//! THD com FFT de janela ├║nica sobre um sinal cuja envolt├│ria est├í mudando
//! dentro da pr├│pria janela mistura modula├º├úo de amplitude (esperada, n├úo ├®
//! distor├º├úo) com distor├º├úo harm├┤nica de verdade ÔÇö o mesmo problema de
//! "espalhamento" que descartou o chirp como sinal de teste em
//! `aliasing.rs`. N├úo testado aqui por esse motivo, n├úo por lacuna.
//! `time_stretch` em fator 1,0 ├® bypass puro (`stretch.rs` j├í testa a
//! identidade) ÔÇö inclu├¡do como caso trivial de refer├¬ncia.

use audio_core::dsp::analysis::fft::magnitude_spectrum;
use audio_core::dsp::mastering::limiter::brickwall_limiter;
use audio_core::dsp::mastering::lufs::apply_lufs_gain;
use audio_core::dsp::mastering::stretch::time_stretch;
use ndarray::Array1;

const SAMPLE_RATE: u32 = 44_100;
const FUNDAMENTAL_HZ: f32 = 1000.0;

/// Tom puro de 1 kHz, dura├º├úo de 1s ÔÇö n├║mero inteiro de ciclos a 44100 Hz,
/// ent├úo a FFT de janela ├║nica n├úo borra a fundamental entre bins.
fn tom_puro() -> Vec<f32> {
    (0..SAMPLE_RATE)
        .map(|i| {
            0.5 * (2.0 * std::f32::consts::PI * FUNDAMENTAL_HZ * i as f32 / SAMPLE_RATE as f32)
                .sin()
        })
        .collect()
}

/// sqrt(energia nos harm├┤nicos 2..=10 / energia na fundamental) ÔÇö mesma
/// defini├º├úo do docs/17.1 ┬º3.1.
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

const THD_MAX: f32 = 0.001; // 0,1% ÔÇö mesmo teto sugerido em docs/17.1 ┬º3.1

#[test]
fn apply_lufs_gain_nao_distorce() {
    let mut pcm = tom_puro();
    let outcome = apply_lufs_gain(&mut pcm, SAMPLE_RATE, -14.0);
    assert!(matches!(
        outcome,
        audio_core::dsp::mastering::lufs::LufsGainOutcome::Applied { .. }
    ));
    let t = thd(&pcm);
    assert!(t < THD_MAX, "THD {:.4}% ap├│s apply_lufs_gain", t * 100.0);
}

#[test]
fn brickwall_limiter_nao_distorce_abaixo_do_teto() {
    let mut pcm = tom_puro(); // pico 0.5 (~-6 dBFS)
    brickwall_limiter(&mut pcm, -3.0); // teto acima do pico: n├úo deveria escalar
    let t = thd(&pcm);
    assert!(
        t < THD_MAX,
        "THD {:.4}% ap├│s brickwall_limiter (abaixo do teto)",
        t * 100.0
    );
}

#[test]
fn brickwall_limiter_nao_distorce_ao_escalar() {
    let mut pcm = tom_puro(); // pico 0.5 (~-6 dBFS)
    brickwall_limiter(&mut pcm, -12.0); // teto abaixo do pico: for├ºa escalar
    let t = thd(&pcm);
    assert!(
        t < THD_MAX,
        "THD {:.4}% ap├│s brickwall_limiter (escalando)",
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
        "THD {:.4}% ap├│s time_stretch em fator 1,0",
        t * 100.0
    );
}
