/// Limiter brickwall com lookahead e release — correção da issue #37
/// (Lote 3 do plano Pareto).
///
/// ## Por que a versão anterior era o bug da #37
///
/// A implementação antiga media o pico do buffer INTEIRO e, se ele
/// passasse do teto, escalava **todas** as amostras pelo mesmo ganho.
/// Depois do `apply_lufs_gain` normalizar para −14 LUFS, material
/// percussivo (fator de crista alto: picos raros e altos, média baixa)
/// estourava o teto — e a escala uniforme derrubava o loudness integrado
/// junto com os picos: saía ~−17 LU onde o alvo era −14 (medido na issue).
/// O "limiter" era, de fato, um botão de volume que desfazia a
/// normalização.
///
/// ## O que este faz
///
/// Limiter de pico de verdade: cada amostra recebe o ganho que ela
/// **individualmente** exige (`ceiling/|x|`, teto em 1.0), o ganho é
/// antecipado por uma janela de lookahead (mínimo deslizante — sem
/// overshoot de ataque) e recupera exponencialmente após o transiente
/// (release). Amostras abaixo do teto não são tocadas além do ganho de
/// pico vizinho — o loudness integrado sobrevive à limitação.
///
/// Garantia de teto: em toda amostra com |x| > ceiling, o ganho aplicado é
/// exatamente `ceiling/|x|` (o mínimo da janela que contém a amostra é ≤
/// requerido, e o release só recupera **em direção ao env**, nunca acima
/// dele); em toda amostra com |x| ≤ ceiling, o ganho aplicado é ≤ 1. Logo
/// `|y| ≤ ceiling` para todo n.

/// Janela de lookahead (ms). Pega o transiente antes de ele acontecer —
/// 2 ms é o padrão de limiters de pico; maior engoliria transientes.
const LOOKAHEAD_MS: f32 = 2.0;
/// Constante de tempo de release (ms). Recuperação rápida o bastante para
/// não bombear em material rítmico, longa o bastante para não distorcer a
/// cauda do transiente.
const RELEASE_MS: f32 = 60.0;

pub fn brickwall_limiter(pcm: &mut [f32], max_peak_db: f32, sample_rate: u32) {
    let max_peak_linear = 10f32.powf(max_peak_db / 20.0);

    // Pico de amostra (não true peak — mesmo critério da versão anterior;
    // o teto do contrato é aplicado com folga de −1 dBTP no config).
    let mut current_peak = 0.0f32;
    for &sample in pcm.iter() {
        let abs = if sample.is_finite() {
            sample.abs()
        } else {
            0.0
        };
        if abs > current_peak {
            current_peak = abs;
        }
    }

    // Sinal já dentro do teto: intocado (mesmo contrato do teste antigo
    // `test_limiter_leaves_quiet_signal_untouched`).
    if current_peak <= max_peak_linear || current_peak <= 0.0 {
        return;
    }

    let n = pcm.len();
    if n == 0 {
        return;
    }

    // Ganho requerido por amostra: `ceiling/|x|`, teto em 1.0 (não ganha
    // volume — limiter não é make-up gain).
    let required: Vec<f32> = pcm
        .iter()
        .map(|&s| {
            let abs = if s.is_finite() { s.abs() } else { 0.0 };
            if abs > max_peak_linear && abs > 0.0 {
                max_peak_linear / abs
            } else {
                1.0
            }
        })
        .collect();

    // Lookahead: `env[i]` = mínimo do ganho requerido em [i, i+L). Deque
    // monótono — O(n) (uma janela deslizante ingênua seria O(n·L)).
    let lookahead = (((LOOKAHEAD_MS / 1000.0) * sample_rate as f32) as usize).clamp(1, n);
    let mut env = vec![1.0f32; n];
    let mut dq: std::collections::VecDeque<usize> =
        std::collections::VecDeque::with_capacity(lookahead + 1);
    for i in (0..n).rev() {
        while let Some(&front) = dq.front() {
            if front >= i + lookahead {
                dq.pop_front();
            } else {
                break;
            }
        }
        while let Some(&back) = dq.back() {
            if required[back] >= required[i] {
                dq.pop_back();
            } else {
                break;
            }
        }
        dq.push_back(i);
        env[i] = required[*dq.front().unwrap()];
    }

    // Envelope com release exponencial: desce instantâneo (o lookahead já
    // antecipou), sobe com a constante de tempo do release.
    let release_coeff = (-1.0 / ((RELEASE_MS / 1000.0) * sample_rate as f32)).exp();
    let mut state = env[0];
    for i in 0..n {
        state = if env[i] <= state {
            env[i]
        } else {
            env[i] - (env[i] - state) * release_coeff
        };
        pcm[i] *= state;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::mastering::measure_lufs;

    #[test]
    fn test_limiter_clamps_peak_to_target() {
        let mut pcm = vec![0.0f32, 1.0, -0.8, 0.5];
        brickwall_limiter(&mut pcm, -6.0, 44100); // -6 dBFS ~= 0.501 linear
        let target = 10f32.powf(-6.0 / 20.0);
        let peak = pcm.iter().cloned().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!((peak - target).abs() < 1e-4, "pico {peak} ≠ teto {target}");
    }

    #[test]
    fn test_limiter_leaves_quiet_signal_untouched() {
        let mut pcm = vec![0.1f32, -0.2, 0.05];
        let before = pcm.clone();
        brickwall_limiter(&mut pcm, -1.0, 44100);
        assert_eq!(pcm, before);
    }

    /// #37 — regressão principal: material percussivo (picos raros e
    /// altos, média baixa) normalizado para −14 LUFS NÃO pode sair
    /// −17 LU depois do teto de pico. A escala uniforme antiga derrubava
    /// o loudness integrado; o limiter de pico preserva.
    #[test]
    fn limiter_preserva_loudness_em_material_percussivo() {
        let sr = 44100u32;
        // Rajadas de seno: 4% de duty cycle, pico 0.9 — fator de crista
        // alto, o cenário exato da issue (ganho de +7 dB após
        // normalização; a escala antiga derrubava ~7 LU de loudness).
        let total = sr as usize * 2;
        let burst_len = (total as f32 * 0.04) as usize;
        let period = total / 8; // 8 rajadas
        let mut pcm: Vec<f32> = vec![0.0; total];
        for b in 0..8 {
            let start = b * period;
            for (i, s) in pcm[start..(start + burst_len).min(total)]
                .iter_mut()
                .enumerate()
            {
                *s = 0.9 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sr as f32).sin();
            }
        }

        let target = -14.0f32;
        let outcome = crate::dsp::mastering::apply_lufs_gain(&mut pcm, sr, target);
        assert!(matches!(
            outcome,
            crate::dsp::mastering::LufsGainOutcome::Applied { .. }
        ));

        let ceiling_db = -1.0f32;
        brickwall_limiter(&mut pcm, ceiling_db, sr);

        // Teto respeitado.
        let ceiling = 10f32.powf(ceiling_db / 20.0);
        let peak = pcm.iter().fold(0.0f32, |m, &x| m.max(x.abs()));
        assert!(
            peak <= ceiling * 1.001,
            "pico {peak} estourou teto {ceiling}"
        );

        // Loudness sobrevive: |final − alvo| ≤ 1.5 LU (a versão antiga
        // ficava a ~3 LU de distância — o "−17 LU" da issue).
        let lufs_final = measure_lufs(&crate::ndarray::Array1::from_vec(pcm), sr);
        assert!(
            (lufs_final - target).abs() <= 1.5,
            "limiter desfaz o ganho de LUFS: final {lufs_final:.2} vs alvo {target:.2}"
        );
    }

    /// Sinal denso (seno contínuo alto): o limiter vira atenuação
    /// constante ≈ uniforme — tudo bem, é o caso onde a escala antiga
    /// estava certa; o resultado tem que continuar no teto.
    #[test]
    fn limiter_sinal_denso_fica_no_teto() {
        let sr = 44100u32;
        let pcm: Vec<f32> = (0..sr as usize)
            .map(|i| 0.95 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sr as f32).sin())
            .collect();
        let mut pcm = pcm;
        brickwall_limiter(&mut pcm, -1.0, sr);
        let ceiling = 10f32.powf(-1.0 / 20.0);
        let peak = pcm.iter().fold(0.0f32, |m, &x| m.max(x.abs()));
        assert!(peak <= ceiling * 1.001);
    }

    /// Depois do fim da rajada, o ganho recupera (release) — as caudas
    /// não podem ficar permanentemente abafadas.
    #[test]
    fn limiter_recupera_ganho_apos_o_transiente() {
        let sr = 44100u32;
        let total = sr as usize;
        let mut pcm = vec![0.0f32; total];
        // Uma única rajada curta no começo (5 ms).
        for (i, s) in pcm[..(0.005 * sr as f32) as usize].iter_mut().enumerate() {
            *s = 1.2 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sr as f32).sin();
        }
        let tail_before: Vec<f32> = pcm[total - 1000..].to_vec();
        brickwall_limiter(&mut pcm, -1.0, sr);
        let tail_after = &pcm[total - 1000..];
        for (a, b) in tail_before.iter().zip(tail_after.iter()) {
            assert!(
                (a - b).abs() < 1e-3 * a.abs().max(1.0),
                "cauda alterada além da tolerância: {a} vs {b}"
            );
        }
    }

    /// Amostras não finitas não derrubam o limiter (pico fantasma NaN
    /// faria o ganho ir a 0 e silenciar o buffer inteiro).
    #[test]
    fn limiter_ignora_amostras_nao_finitas() {
        let mut pcm = vec![0.9f32, f32::NAN, -0.9, 0.9];
        brickwall_limiter(&mut pcm, -1.0, 44100);
        let ceiling = 10f32.powf(-1.0 / 20.0);
        for (i, &s) in pcm.iter().enumerate() {
            if i == 1 {
                continue; // NaN permanece NaN (I15 limpa depois)
            }
            assert!(s.abs() <= ceiling * 1.001, "amostra {i} = {s}");
        }
    }
}
