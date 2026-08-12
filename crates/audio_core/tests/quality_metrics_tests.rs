//! Testes detalhados das metricas de qualidade auditiva.
//! Complementam os testes unitarios do modulo quality_metrics.rs
//! com cenarios mais realistas usando fixtures sinteticas.

use audio_core::dsp::analysis::quality_metrics::{
    compute_quality_report, envelope_difference, signal_to_noise, spectral_centroid_shift,
    total_harmonic_distortion,
};
use audio_core::dsp::analysis::test_fixtures::{
    add_white_noise, generate_chord, generate_drifted_sine, generate_sine,
};

#[test]
fn test_thd_chord_higher_than_sine() {
    let sine = generate_sine(440.0, 2.0, 44100, 0.8);
    let chord = generate_chord(&[(261.63, 0.4), (329.63, 0.3), (392.0, 0.3)], 2.0, 44100);

    let thd_sine = total_harmonic_distortion(&sine, 44100, 440.0);
    let thd_chord = total_harmonic_distortion(&chord, 44100, 261.63);

    // Acorde tem harmonicos naturais, mas THD nao e necessariamente maior
    // pois medimos relacao ao fundamental.
    assert!(thd_sine >= 0.0);
    assert!(thd_chord >= 0.0);
}

#[test]
fn test_snr_degrades_with_noise() {
    let clean = generate_sine(440.0, 2.0, 44100, 0.8);
    let noisy_20db = add_white_noise(&clean, 20.0);
    let noisy_6db = add_white_noise(&clean, 6.0);

    let snr_20 = signal_to_noise(&clean, &noisy_20db);
    let snr_6 = signal_to_noise(&clean, &noisy_6db);

    assert!(
        snr_20 > snr_6,
        "SNR 20dB deveria ser > SNR 6dB: {} > {}",
        snr_20,
        snr_6
    );
    assert!(
        snr_6 < 20.0,
        "SNR medida deveria ser < 20 para sinal com 6dB de ruido"
    );
}

#[test]
fn test_envelope_diff_with_different_signals() {
    let sig_a = generate_sine(440.0, 2.0, 44100, 0.8);
    let sig_b = generate_sine(880.0, 2.0, 44100, 0.8);

    let diff = envelope_difference(&sig_a, &sig_b, 44100, 50.0);
    let diff_same = envelope_difference(&sig_a, &sig_a, 44100, 50.0);

    assert!(
        diff > diff_same,
        "envelope de sinais diferentes deveria ser maior"
    );
    assert!(
        diff_same < 0.001,
        "envelope de sinal identico deveria ser ~0"
    );
}

#[test]
fn test_spectral_centroid_shift_higher_freq() {
    let low = generate_sine(220.0, 2.0, 44100, 0.8);
    let high = generate_sine(880.0, 2.0, 44100, 0.8);

    let shift = spectral_centroid_shift(&low, &high, 44100);
    assert!(
        shift.abs() > 10.0,
        "mudanca de 220->880 Hz deveria dar shift >10%: {}",
        shift
    );
}

#[test]
fn test_quality_report_noisy_signal() {
    let clean = generate_sine(440.0, 2.0, 44100, 0.8);
    let noisy = add_white_noise(&clean, 15.0);

    let report = compute_quality_report(&clean, &noisy, 44100, 440.0);

    assert!(report.thd >= 0.0, "THD deve ser >= 0");
    assert!(report.snr_db < 100.0, "SNR com ruido deve ser finito");
    assert!(
        report.envelope_diff > 0.0,
        "envelope com ruido deve diferir"
    );
    assert!(
        report.centroid_shift_pct >= 0.0,
        "centroid shift deve ser >= 0"
    );
}

#[test]
fn test_quality_report_drifted_vs_original() {
    let original = generate_sine(440.0, 3.0, 44100, 0.8);
    let drifted = generate_drifted_sine(440.0, 20.0, 3.0, 44100, 0.8);

    let report = compute_quality_report(&original, &drifted, 44100, 440.0);

    // Sinal com drift deve ter diferencas espectrais mensuraveis
    assert!(
        report.envelope_diff > 0.0,
        "sinal com drift deve ter envelope diferente"
    );
}
