// Geradores de sinais sinteticos para testes de afinacao e pipeline.
// Cada gerador e deterministico: mesmos parametros = mesmos bytes.

use ndarray::Array1;

/// Seno puro em frequencia e amplitude especificadas.
pub fn generate_sine(
    freq: f32,
    duration_sec: f32,
    sample_rate: u32,
    amplitude: f32,
) -> Array1<f32> {
    let n_samples = (duration_sec * sample_rate as f32) as usize;
    let two_pi = std::f32::consts::TAU;
    Array1::from_iter((0..n_samples).map(|i| {
        let t = i as f32 / sample_rate as f32;
        amplitude * (two_pi * freq * t).sin()
    }))
}

/// Seno com drift linear de frequencia em cents.
/// A frequencia varia de base_freq ate base_freq * 2^(drift_cents/1200)
/// ao longo da duracao, usando integracao numerica da fase.
pub fn generate_drifted_sine(
    base_freq: f32,
    drift_cents: f32,
    duration_sec: f32,
    sample_rate: u32,
    amplitude: f32,
) -> Array1<f32> {
    let n_samples = (duration_sec * sample_rate as f32) as usize;
    // Frequencia final alvo
    let end_freq = base_freq * 2.0f32.powf(drift_cents / 1200.0);
    // Taxa de variacao linear da frequencia (Hz por segundo)
    let freq_rate = (end_freq - base_freq) / duration_sec;
    // Integracao numerica da fase: phi(t) = 2*pi * integral( f(t') dt' )
    // f(t) = base_freq + freq_rate * t
    // phi(t) = 2*pi * (base_freq*t + freq_rate*t^2/2)
    let two_pi = std::f32::consts::TAU;
    Array1::from_iter((0..n_samples).map(|i| {
        let t = i as f32 / sample_rate as f32;
        let phase = two_pi * (base_freq * t + freq_rate * t * t / 2.0);
        amplitude * phase.sin()
    }))
}

/// Seno com bend de frequencia em formato de arco seno.
/// A frequencia sobe e volta ao valor original durante bend_duration.
pub fn generate_bend(
    base_freq: f32,
    bend_cents: f32,
    bend_duration: f32,
    total_duration: f32,
    sample_rate: u32,
    amplitude: f32,
) -> Array1<f32> {
    let n_samples = (total_duration * sample_rate as f32) as usize;
    let two_pi = std::f32::consts::TAU;
    let mut phase = 0.0f32;
    let dt = 1.0 / sample_rate as f32;

    let mut samples = Vec::with_capacity(n_samples);
    for i in 0..n_samples {
        let t = i as f32 / sample_rate as f32;
        // Envelope de bend: arco seno durante bend_duration, senao base_freq
        let freq_multiplier = if t < bend_duration {
            // O bend segue sin(pi * t / bend_duration), que vai de 0 a 1 e volta a 0
            2.0f32.powf(bend_cents / 1200.0 * (std::f32::consts::PI * t / bend_duration).sin())
        } else {
            1.0
        };
        let freq = base_freq * freq_multiplier;
        phase += two_pi * freq * dt;
        samples.push(amplitude * phase.sin());
    }

    Array1::from_vec(samples)
}

/// Acorde: soma de multiplos senos com frequencias e amplitudes dadas.
pub fn generate_chord(notes: &[(f32, f32)], duration_sec: f32, sample_rate: u32) -> Array1<f32> {
    let n_samples = (duration_sec * sample_rate as f32) as usize;
    let two_pi = std::f32::consts::TAU;
    let num_notes = notes.len() as f32;

    // Soma normalizada para nao clipar: divide pelo numero de notas
    Array1::from_iter((0..n_samples).map(|i| {
        let t = i as f32 / sample_rate as f32;
        let sum: f32 = notes
            .iter()
            .map(|&(freq, amp)| amp * (two_pi * freq * t).sin())
            .sum();
        sum / num_notes
    }))
}

/// Adiciona ruido branco com SNR alvo em dB.
pub fn add_white_noise(signal: &Array1<f32>, snr_db: f32) -> Array1<f32> {
    // Potencia do sinal
    let signal_power: f32 = signal.iter().map(|&x| x * x).sum::<f32>() / signal.len() as f32;
    if signal_power < 1e-20 {
        return signal.clone();
    }

    // Potencia do ruido desejada: SNR = 10*log10(Ps/Pn) => Pn = Ps / 10^(SNR/10)
    let noise_power = signal_power / 10.0f32.powf(snr_db / 10.0);

    // Gerador de ruido deterministico com LCG simples (semente fixa)
    let mut seed: u64 = 42;
    let mut next_rand = move || -> f32 {
        // LCG: x_{n+1} = (a * x_n + c) mod m
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        // Normaliza para -1..1
        (seed >> 33) as i32 as f32 / (i32::MAX as f32)
    };

    // Gera ruido base e calcula sua potencia real para escalar corretamente
    let raw_noise: Vec<f32> = signal.iter().map(|_| next_rand()).collect();
    let raw_power: f32 = raw_noise.iter().map(|&x| x * x).sum::<f32>() / raw_noise.len() as f32;

    if raw_power < 1e-20 {
        return signal.clone();
    }

    // Escala fator para atingir a potencia de ruido alvo
    let scale = (noise_power / raw_power).sqrt();

    signal
        .iter()
        .zip(raw_noise.iter())
        .map(|(&s, &n)| s + n * scale)
        .collect()
}

/// Impulso percussivo com envelope Attack-Decay.
pub fn generate_percussive(attack_ms: f32, decay_ms: f32, sample_rate: u32) -> Array1<f32> {
    let total_ms = attack_ms + decay_ms;
    let n_samples = (total_ms / 1000.0 * sample_rate as f32) as usize;
    let attack_samples = (attack_ms / 1000.0 * sample_rate as f32) as usize;

    Array1::from_iter((0..n_samples).map(|i| {
        let t = i as f32 / sample_rate as f32;
        let env = if i < attack_samples {
            // Fase de ataque: rampa linear de 0 a 1
            i as f32 / attack_samples as f32
        } else {
            // Fase de decaimento: exponencial
            let decay_t = (i - attack_samples) as f32 / sample_rate as f32;
            let decay_duration = decay_ms / 1000.0;
            (-decay_t / decay_duration * 4.0).exp()
        };
        // Ruido de percussao deterministico
        env * (2.0 * std::f32::consts::PI * 3.0 * t).sin() * 0.5
    }))
}

/// Mistura de N sinais, normalizada para nao clipar.
pub fn mix_signals(signals: &[&Array1<f32>]) -> Array1<f32> {
    if signals.is_empty() {
        return Array1::zeros(0);
    }

    let len = signals.iter().map(|s| s.len()).max().unwrap_or(0);
    let n = signals.len();

    let mut mixed = vec![0.0f32; len];
    for sig in signals {
        for (i, &v) in sig.iter().enumerate() {
            mixed[i] += v;
        }
    }

    // Normaliza pelo numero de sinais
    let norm = n as f32;
    for v in mixed.iter_mut() {
        *v /= norm;
    }

    Array1::from_vec(mixed)
}

/// Downsamplo de Array1<f32> para f64 necessario por algumas APIs.
pub fn to_f64(signal: &Array1<f32>) -> Vec<f64> {
    signal.iter().map(|&x| x as f64).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::analysis::fft::magnitude_spectrum;

    /// Encontra a frequencia de pico no espectro de magnitude.
    fn find_peak_freq(mag: &[f32], sample_rate: u32, _fft_len: usize) -> f32 {
        let nyquist = sample_rate as f32 / 2.0;
        let bin_width = nyquist / mag.len() as f32;
        let (max_idx, _) = mag
            .iter()
            .skip(1) // pula DC
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap_or((0, &0.0));
        (max_idx + 1) as f32 * bin_width
    }

    #[test]
    fn test_generate_sine_produces_correct_frequency() {
        let freq = 440.0f32;
        let sr = 44100u32;
        let sine = generate_sine(freq, 1.0, sr, 0.9);
        // Janela de FFT de 4096 amostras (potencia de 2)
        let frame = sine.slice(ndarray::s![..4096.min(sine.len())]);
        let mag = magnitude_spectrum(frame);
        let peak = find_peak_freq(&mag, sr, 4096);
        // Tolerancia de 5 Hz para frequencia de pico
        assert!(
            (peak - freq).abs() < 5.0,
            "Frequencia de pico {peak} Hz, esperava {freq} Hz"
        );
    }

    #[test]
    fn test_generate_drifted_sine_frequency_ramp() {
        let base_freq = 220.0f32;
        let drift_cents = 1200.0; // 1 oitava acima
        let sr = 44100u32;
        let duration = 2.0f32;
        let sine = generate_drifted_sine(base_freq, drift_cents, duration, sr, 0.9);

        // Verifica frequencia no inicio: primeiro frame de 4096
        let frame_start = sine.slice(ndarray::s![..4096]);
        let mag_start = magnitude_spectrum(frame_start);
        let peak_start = find_peak_freq(&mag_start, sr, 4096);
        assert!(
            (peak_start - base_freq).abs() < 10.0,
            "Inicio: pico {peak_start} Hz, esperava ~{base_freq} Hz"
        );

        // Verifica frequencia no final: ultimo frame de 4096
        let start_idx = sine.len() - 4096;
        let frame_end = sine.slice(ndarray::s![start_idx..]);
        let mag_end = magnitude_spectrum(frame_end);
        let expected_end = base_freq * 2.0; // 1200 cents = 1 oitava
        let peak_end = find_peak_freq(&mag_end, sr, 4096);
        assert!(
            (peak_end - expected_end).abs() < 30.0,
            "Fim: pico {peak_end} Hz, esperava ~{expected_end} Hz"
        );
    }

    #[test]
    fn test_generate_bend_returns_to_base() {
        let base_freq = 440.0f32;
        let bend_cents = 100.0;
        let bend_duration = 0.5f32;
        let total_duration = 1.0f32;
        let sr = 44100u32;
        let sine = generate_bend(
            base_freq,
            bend_cents,
            bend_duration,
            total_duration,
            sr,
            0.9,
        );

        // Verifica que apos o bend, a frequencia volta ao valor base
        let start_idx = sine.len() - 4096;
        let frame_end = sine.slice(ndarray::s![start_idx..]);
        let mag_end = magnitude_spectrum(frame_end);
        let peak_end = find_peak_freq(&mag_end, sr, 4096);
        assert!(
            (peak_end - base_freq).abs() < 10.0,
            "Apos bend: pico {peak_end} Hz, esperava ~{base_freq} Hz"
        );
    }

    #[test]
    fn test_generate_chord_has_multiple_peaks() {
        let sr = 44100u32;
        // Acorde de La menor: A=220, C=261.63, E=329.63
        let notes = [(220.0, 1.0), (261.63, 1.0), (329.63, 1.0)];
        let chord = generate_chord(&notes, 2.0, sr);

        let frame = chord.slice(ndarray::s![4096..8192]);
        let mag = magnitude_spectrum(frame);

        // Verifica se ha energia significativa nas frequencias esperadas
        let bin_width = sr as f32 / (2.0 * mag.len() as f32);
        for &(expected_freq, _) in &notes {
            let expected_bin = (expected_freq / bin_width) as usize;
            // Verifica se o bin da frequencia esperada tem magnitude significativa
            let mag_at_bin = mag.get(expected_bin).copied().unwrap_or(0.0);
            let max_mag = mag.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            let ratio = mag_at_bin / max_mag.max(1e-10);
            assert!(
                ratio > 0.05,
                "Frequencia {expected_freq} Hz nao encontrada no acorde (ratio={ratio})"
            );
        }
    }

    #[test]
    fn test_add_white_noise_achieves_target_snr() {
        let sr = 44100u32;
        let signal = generate_sine(440.0, 1.0, sr, 0.9);
        let target_snr = 20.0f32; // 20 dB
        let noisy = add_white_noise(&signal, target_snr);

        // Calcula SNR real
        let signal_power: f32 = signal.iter().map(|&x| x * x).sum::<f32>() / signal.len() as f32;
        let residual: Array1<f32> = &noisy - &signal;
        let noise_power: f32 = residual.iter().map(|&x| x * x).sum::<f32>() / residual.len() as f32;
        let measured_snr = 10.0 * (signal_power / noise_power.max(1e-20)).log10();

        // Tolerancia de 3 dB
        assert!(
            (measured_snr - target_snr).abs() < 3.0,
            "SNR medido {measured_snr:.1} dB, esperava {target_snr} dB (tol 3 dB)"
        );
    }

    #[test]
    fn test_mix_signals_no_clipping() {
        let sr = 44100u32;
        let s1 = generate_sine(440.0, 0.5, sr, 1.0);
        let s2 = generate_sine(550.0, 0.5, sr, 1.0);
        let s3 = generate_sine(660.0, 0.5, sr, 1.0);

        let mixed = mix_signals(&[&s1, &s2, &s3]);
        let max_amp = mixed.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let min_amp = mixed.iter().cloned().fold(f32::INFINITY, f32::min);

        // Amplitude maxima absoluta deve estar <= 1.0
        let max_abs = max_amp.abs().max(min_amp.abs());
        assert!(max_abs <= 1.0, "Amplitude maxima {max_abs} excede 1.0");
    }
}
