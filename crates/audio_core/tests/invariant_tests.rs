//! Testes de invariantes p├│s-processamento (I1 clipping, I4 estalos).
//!
//! Estes testes usam `post_process`/`check_invariants` diretamente para
//! verificar as garantias de qualidade da sa├¡da do pipeline (invariantes I1 e
//! I4 de `docs/10-TESTES-QUALIDADE.md`).

use audio_core::dsp::post_process::{check_invariants, post_process};

#[test]
fn i1_sine_wave_no_clipping_after_post_process() {
    // Senoide de amplitude 0.9 j├í est├í dentro de ┬▒1.0 ÔÇö n├úo deve gerar clipping.
    let sr = 44100u32;
    let mut pcm: Vec<f32> = (0..sr)
        .map(|i| 0.9 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sr as f32).sin())
        .collect();
    let report = post_process(&mut pcm, 0.5);
    assert_eq!(report.samples_limited, 0);
}

#[test]
fn i1_clipped_signal_is_limited() {
    let mut pcm = vec![0.0f32; 44100];
    // Cria duas regi├Áes de clipping: 100 amostras a +2.0 e 100 a -3.0.
    for i in 0..100 {
        pcm[1000 + i] = 2.0;
        pcm[2000 + i] = -3.0;
    }
    let report = post_process(&mut pcm, 0.5);
    assert_eq!(report.samples_limited, 200);
    for &s in &pcm {
        assert!(s.abs() <= 1.0 + 1e-7, "amostra fora do range: {s}");
    }
}

#[test]
fn i4_smooth_signal_no_false_clicks() {
    // Senoide suave n├úo deve ter estalos.
    let sr = 44100u32;
    let pcm: Vec<f32> = (0..sr)
        .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 220.0 * i as f32 / sr as f32).sin())
        .collect();
    let report = check_invariants(&pcm, 0.5);
    assert_eq!(report.click_corrections, 0);
}

#[test]
fn i4_impulse_is_detected_and_smoothed() {
    let mut pcm = vec![0.0f32; 1000];
    pcm[500] = 1.0;
    pcm[501] = -1.0;

    let before = check_invariants(&pcm, 0.5);
    assert!(
        before.click_corrections > 0,
        "impulso deveria ser detectado"
    );

    let after = post_process(&mut pcm, 0.5);
    assert!(after.click_corrections > 0);

    // Ap├│s suaviza├º├úo, nenhum salto deve exceder o limiar.
    let check = check_invariants(&pcm, 0.5);
    assert_eq!(check.click_corrections, 0);
}

#[test]
fn post_process_idempotent() {
    // Aplicar duas vezes n├úo deve mudar nada na segunda.
    let mut pcm = vec![1.5f32, -2.0, 0.0, 0.0, 1.0, -0.5, 3.0];
    let r1 = post_process(&mut pcm, 0.5);
    let snapshot = pcm.clone();
    let r2 = post_process(&mut pcm, 0.5);
    assert_eq!(pcm, snapshot, "segunda passagem n├úo deveria mudar nada");
    assert_eq!(r2.samples_limited, 0);
    assert_eq!(r2.click_corrections, 0);
    let _ = r1;
}
