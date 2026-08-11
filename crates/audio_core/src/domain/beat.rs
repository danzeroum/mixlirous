use ndarray::Array1;
use realfft::RealFftPlanner;

/// Resultado da an├ílise de onset strength em cada frame hop
#[derive(Debug, Clone)]
pub struct OnsetStrength {
    pub strengths: Vec<f32>,
    pub hop_size: usize,
    pub frame_size: usize,
    pub sample_rate: u32,
}

/// Representa um candidato a batida detectado no ├íudio
#[derive(Debug, Clone, PartialEq)]
pub struct BeatCandidate {
    pub sample_idx: usize,
    pub onset_strength: f32,
    pub time_sec: f32,
    pub rms_energy: f32,
}

/// Par├ómetros de configura├º├úo para detec├º├úo de batidas
#[derive(Debug, Clone)]
pub struct BeatDetectionParams {
    pub min_bpm: u32,
    pub max_bpm: u32,
    pub frame_size: usize,
    pub hop_size: usize,
    pub onset_method: OnsetMethod,
    pub sample_rate: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OnsetMethod {
    SpectralFlux,
    Energy,
    Complex,
}

impl Default for BeatDetectionParams {
    fn default() -> Self {
        Self {
            min_bpm: 60,
            max_bpm: 180,
            frame_size: 2048,
            hop_size: 512,
            onset_method: OnsetMethod::SpectralFlux,
            sample_rate: 44100,
        }
    }
}

/// Extra├º├úo de onset strength via fluxo espectral (Spectral Flux)
pub fn extract_onset_strength(pcm: &Array1<f32>, params: &BeatDetectionParams) -> OnsetStrength {
    if pcm.len() < params.frame_size {
        return OnsetStrength {
            strengths: Vec::new(),
            hop_size: params.hop_size,
            frame_size: params.frame_size,
            sample_rate: params.sample_rate,
        };
    }

    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(params.frame_size);

    let mut strengths = Vec::new();
    let hann = make_hann_window(params.frame_size);

    let bins = params.frame_size / 2 + 1;
    let mut prev_spectrum = vec![0.0f32; bins];
    let mut input = fft.make_input_vec();
    let mut output = fft.make_output_vec();
    // O primeiro quadro n├úo tem antecessor: comparar contra um espectro
    // zerado transformaria a energia espectral inteira em "fluxo", e o
    // quadro 0 viraria o maior onset da faixa por constru├º├úo (issue #32 ÔÇö
    // medido em `sine_8khz_mono.wav`: 2304 no quadro 0 contra pico de 182 no
    // resto). Fluxo ├® uma diferen├ºa; sem antecessor ele ├® zero, n├úo infinito.
    let mut primeiro_quadro = true;

    for start in (0..=(pcm.len() - params.frame_size)).step_by(params.hop_size) {
        for i in 0..params.frame_size {
            input[i] = pcm[start + i] * hann[i];
        }

        if fft.process(&mut input, &mut output).is_err() {
            strengths.push(0.0);
            continue;
        }

        // Spectral flux: diferen├ºa positiva entre frames consecutivos
        let flux: f32 = if primeiro_quadro {
            primeiro_quadro = false;
            0.0
        } else {
            output
                .iter()
                .zip(&prev_spectrum)
                .map(|(c, &p)| (c.norm() - p).max(0.0))
                .sum()
        };

        strengths.push(flux);

        for (slot, c) in prev_spectrum.iter_mut().zip(output.iter()) {
            *slot = c.norm();
        }
    }

    OnsetStrength {
        strengths,
        hop_size: params.hop_size,
        frame_size: params.frame_size,
        sample_rate: params.sample_rate,
    }
}

/// Cria uma janela de Hann pr├®-calculada: `w[i] = 0.5┬À(1 ÔêÆ cos(2¤Çi/(NÔêÆ1)))`.
///
/// O fator ├® **2¤Ç**, n├úo ¤Ç (issue #32). Com ¤Ç o argumento percorre s├│ meio
/// per├¡odo de cosseno e a sa├¡da vira uma rampa monot├┤nica de 0 a 1 ÔÇö
/// `w[NÔêÆ1] = 1.0` em vez de 0. Uma janela que termina em ganho pleno n├úo
/// atenua o fim do quadro, ent├úo a descontinuidade na emenda entre quadros
/// consecutivos permanece, que ├® exatamente o vazamento espectral que a
/// janela existe para eliminar.
fn make_hann_window(size: usize) -> Vec<f32> {
    if size <= 1 {
        return vec![0.0; size];
    }
    (0..size)
        .map(|i| {
            let x = 2.0 * (i as f64) * std::f64::consts::PI / (size - 1) as f64;
            (1.0 - x.cos()) as f32 * 0.5f32
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array1;

    #[test]
    fn test_onset_strength_returns_positive_values() {
        let pcm = Array1::from_vec(vec![0.0f32; 44100]); // 1s de sil├¬ncio
        let params = BeatDetectionParams {
            sample_rate: 44100,
            ..Default::default()
        };

        let onset = extract_onset_strength(&pcm, &params);
        // Sil├¬ncio deve gerar flux pr├│ximo de zero
        assert!(onset.strengths.iter().all(|&v| v < 1e-5));
    }

    /// Issue #32. O teste anterior (`positive_values`) passava tanto para uma
    /// Hann quanto para a rampa monot├┤nica que estava no lugar dela ÔÇö
    /// positividade n├úo distingue as duas. Este afirma a **forma**.
    #[test]
    fn hann_e_simetrica_com_zeros_nas_bordas_e_maximo_no_centro() {
        let n = 2048;
        let w = make_hann_window(n);

        assert!(w[0].abs() < 1e-6, "borda esquerda: {}", w[0]);
        assert!(
            w[n - 1].abs() < 1e-6,
            "borda direita: {} (era 1.0 com ¤Ç)",
            w[n - 1]
        );
        assert!(
            (w[(n - 1) / 2] - 1.0).abs() < 1e-3,
            "centro: {} (era ~0.5 com ¤Ç)",
            w[(n - 1) / 2]
        );

        for i in 0..n {
            assert!(
                (w[i] - w[n - 1 - i]).abs() < 1e-6,
                "assimetria em {i}: {} vs {}",
                w[i],
                w[n - 1 - i]
            );
        }
    }

    #[test]
    fn hann_nao_estoura_com_tamanho_degenerado() {
        assert!(make_hann_window(0).is_empty());
        assert_eq!(make_hann_window(1), vec![0.0]);
    }

    /// Issue #32. Fluxo ├® uma diferen├ºa; o primeiro quadro n├úo tem
    /// antecessor. Comparar contra espectro zerado fazia o quadro 0 reportar
    /// a energia espectral inteira e virar o maior onset da faixa por
    /// constru├º├úo ÔÇö vis├¡vel em sinal estacion├írio, invis├¡vel em sinal que
    /// come├ºa em sil├¬ncio.
    #[test]
    fn primeiro_quadro_nao_reporta_energia_como_fluxo() {
        // Senoide estacion├íria: depois do primeiro quadro o fluxo real ├®
        // quase nulo, ent├úo qualquer valor grande em [0] ├® o artefato.
        let pcm = Array1::from_vec(
            (0..44100)
                .map(|i| (i as f32 * 0.3).sin() * 0.5)
                .collect::<Vec<_>>(),
        );
        let params = BeatDetectionParams {
            sample_rate: 44100,
            ..Default::default()
        };

        let onset = extract_onset_strength(&pcm, &params);
        assert!(onset.strengths.len() > 2);
        assert_eq!(onset.strengths[0], 0.0, "quadro 0 deve ser fluxo zero");

        let pico_resto = onset.strengths[1..].iter().cloned().fold(0.0f32, f32::max);
        assert!(
            onset.strengths[0] <= pico_resto,
            "quadro 0 ({}) n├úo pode dominar o resto ({pico_resto})",
            onset.strengths[0]
        );
    }
}
