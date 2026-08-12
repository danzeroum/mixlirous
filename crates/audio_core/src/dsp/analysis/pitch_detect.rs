/// Deteccao de frequencia fundamental (pitch) — stub simplificado.
/// TODO(Sprint B): substituir por algoritmo YIN completo.
/// O stub usa deteccao por zero-crossing + parabolizacao,
/// suficiente para validar o pipeline de afinacao em testes.
/// Limitacoes: funciona apenas para sinais monofonicos puros.

use ndarray::{Array1, s};

/// Resultado da deteccao de pitch frame-a-frame.
#[derive(Debug, Clone)]
pub struct PitchFrame {
    /// Frequencia detectada em Hz. 0.0 se nao detectado.
    pub freq: f32,
    /// Confianca da deteccao: 0.0..=1.0.
    pub confidence: f32,
    /// Indica se o frame tem sinal voiced (true) ou silencio (false).
    pub is_voiced: bool,
}

/// Detecta pitch em um frame de audio (monofonico).
/// Usa contagem de zero-crossings com interpolacao parabolica.
pub fn detect_pitch_frame(pcm: &[f32], sample_rate: u32) -> PitchFrame {
    // Verifica se o frame tem energia suficiente
    let rms: f32 = (pcm.iter().map(|&x| x * x).sum::<f32>() / pcm.len() as f32).sqrt();
    if rms < 0.01 {
        return PitchFrame {
            freq: 0.0,
            confidence: 0.0,
            is_voiced: false,
        };
    }

    // Encontra todos os zero-crossings (transicoes de sinal)
    let mut crossings: Vec<f32> = Vec::new();
    for i in 1..pcm.len() {
        if (pcm[i - 1] >= 0.0 && pcm[i] < 0.0) || (pcm[i - 1] < 0.0 && pcm[i] >= 0.0) {
            // Interpolacao linear para posicao exata do crossing
            let frac = pcm[i - 1].abs() / (pcm[i - 1].abs() + pcm[i].abs()).max(1e-10);
            let exact_pos = (i as f32 - 1.0) + frac;
            crossings.push(exact_pos);
        }
    }

    // Precisa de pelo menos 2 crossings para estimar frequencia
    if crossings.len() < 2 {
        return PitchFrame {
            freq: 0.0,
            confidence: 0.0,
            is_voiced: false,
        };
    }

    // Calcula periodos entre crossings consecutivos (meios-ciclos)
    // Frequencia = 1 / (2 * periodo medio de meio-ciclo)
    let half_periods: Vec<f32> = crossings
        .windows(2)
        .map(|w| w[1] - w[0])
        .collect();

    let avg_half_period: f32 = half_periods.iter().sum::<f32>() / half_periods.len() as f32;
    let avg_period = avg_half_period * 2.0;
    let freq = sample_rate as f32 / avg_period;

    // Confianca baseada na consistencia dos periodos
    let mean_hp = avg_half_period;
    let variance: f32 = half_periods
        .iter()
        .map(|&x| (x - mean_hp) * (x - mean_hp))
        .sum::<f32>()
        / half_periods.len() as f32;
    let std_hp = variance.sqrt();

    // Coeficiente de variacao: baixo = alta confianca
    let cv = if mean_hp > 1e-10 {
        std_hp / mean_hp
    } else {
        1.0
    };

    // Mapeia CV para confianca: CV=0 => 1.0, CV>=0.5 => 0.0
    let confidence = (1.0 - (cv / 0.5).min(1.0)).max(0.0);

    // Limites razoaveis para pitch vocal/musical: 50 Hz a 2000 Hz
    let is_valid = (50.0..=2000.0).contains(&freq);

    PitchFrame {
        freq: if is_valid { freq } else { 0.0 },
        confidence: if is_valid { confidence } else { 0.0 },
        is_voiced: is_valid && confidence > 0.3,
    }
}

/// Detecta pitch em toda a faixa, frame-a-frame com hop.
pub fn detect_pitch(
    pcm: &Array1<f32>,
    sample_rate: u32,
    frame_size: usize,
    hop_size: usize,
) -> Vec<PitchFrame> {
    let mut frames = Vec::new();
    let mut start = 0;

    while start + frame_size <= pcm.len() {
        let frame_slice = pcm.slice(s![start..start + frame_size]);
        let frame_data = frame_slice.as_slice().unwrap_or(&[]);
        frames.push(detect_pitch_frame(frame_data, sample_rate));
        start += hop_size;
    }

    frames
}

/// Converte frequencia para cents relativo a uma referencia.
pub fn freq_to_cents(freq: f32, reference: f32) -> f32 {
    if freq <= 0.0 || reference <= 0.0 {
        return 0.0;
    }
    1200.0 * (freq / reference).log2()
}

/// Estima o drift medio em cents ao longo da faixa.
/// Usa regressao linear simples sobre os pitches detectados.
pub fn detect_drift(pitch_frames: &[PitchFrame]) -> f32 {
    // Filtra apenas frames voiced
    let voiced: Vec<(usize, f32)> = pitch_frames
        .iter()
        .enumerate()
        .filter(|(_, p)| p.is_voiced)
        .map(|(i, p)| (i, p.freq))
        .collect();

    if voiced.len() < 2 {
        return 0.0;
    }

    // Converte frequencias para cents relativos a primeira frequencia
    let ref_freq = voiced[0].1;
    let cents: Vec<(f32, f32)> = voiced
        .iter()
        .map(|&(t, f)| {
            let t_norm = t as f32 / voiced.len() as f32;
            let c = freq_to_cents(f, ref_freq);
            (t_norm, c)
        })
        .collect();

    // Regressao linear simples: y = a + b*x
    let n = cents.len() as f32;
    let sum_x: f32 = cents.iter().map(|(x, _)| *x).sum();
    let sum_y: f32 = cents.iter().map(|(_, y)| *y).sum();
    let sum_xy: f32 = cents.iter().map(|(x, y)| x * y).sum();
    let sum_x2: f32 = cents.iter().map(|(x, _)| x * x).sum();

    let denom = n * sum_x2 - sum_x * sum_x;
    if denom.abs() < 1e-10 {
        return 0.0;
    }

    // Inclinacao (cents por tempo normalizado)
    let slope = (n * sum_xy - sum_x * sum_y) / denom;
    // Drift total e a diferenca de cents entre inicio e fim
    slope
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::analysis::test_fixtures::{generate_drifted_sine, generate_sine};

    #[test]
    fn test_detect_pitch_a4_sine() {
        let sr = 44100u32;
        let sine = generate_sine(440.0, 0.5, sr, 0.9);
        // Frame de 4096 amostras
        let frame_slice = sine.slice(s![..4096.min(sine.len())]);
        let frame = frame_slice.as_slice().unwrap_or(&[]);
        let result = detect_pitch_frame(frame, sr);

        assert!(
            result.is_voiced,
            "Deveria detectar pitch voiced para seno 440 Hz"
        );
        assert!(
            (result.freq - 440.0).abs() < 2.0,
            "Frequencia detectada {} Hz, esperava 440 Hz (tol 2 Hz)",
            result.freq
        );
    }

    #[test]
    fn test_detect_pitch_silence() {
        let sr = 44100u32;
        let silence = vec![0.0f32; 4096];
        let result = detect_pitch_frame(&silence, sr);

        assert!(
            !result.is_voiced,
            "Silencio nao deveria ser detectado como voiced"
        );
        assert_eq!(result.freq, 0.0);
    }

    #[test]
    fn test_freq_to_cents_known() {
        // 445 Hz relativo a 440 Hz: 1200 * log2(445/440)
        let cents = freq_to_cents(445.0, 440.0);
        // Valor esperado: ~19.56 cents
        assert!(
            (cents - 19.56).abs() < 1.0,
            "Cents para 445/440 Hz deveria ser ~19.6, obteve {cents}"
        );
    }

    #[test]
    fn test_detect_drift_no_drift() {
        let sr = 44100u32;
        let sine = generate_sine(440.0, 2.0, sr, 0.9);
        let frames = detect_pitch(&sine, sr, 4096, 2048);
        let drift = detect_drift(&frames);

        assert!(
            drift.abs() < 1.0,
            "Drift para seno constante deveria ser < 1 cent, obteve {drift}"
        );
    }

    #[test]
    fn test_detect_drift_positive() {
        let sr = 44100u32;
        // Drift de +100 cents ao longo de 2 segundos
        let sine = generate_drifted_sine(440.0, 100.0, 2.0, sr, 0.9);
        let frames = detect_pitch(&sine, sr, 4096, 2048);
        let drift = detect_drift(&frames);

        // O drift deve ser positivo e razoavelmente proximo de 100 cents
        assert!(
            drift > 0.0,
            "Drift deveria ser positivo, obteve {drift}"
        );
        // Tolerancia de 30%: 100 * 0.3 = 30
        assert!(
            (drift - 100.0).abs() < 30.0,
            "Drift {drift} cents fora da tolerancia de 30% para 100 cents esperados"
        );
    }
}
