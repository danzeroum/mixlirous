use crate::domain::CrossfadeCurve;
use std::f32::consts::FRAC_PI_2;

/// Ganhos de A e B num ponto da transição (`alpha` 0..=1: 0 é só A, 1 é só
/// B). docs/16 T2.2: o crossfade linear (`ConstantGain`) mantém
/// `gain_a + gain_b = 1`, certo só para material correlacionado (o mesmo
/// bloco sobreposto a si mesmo). Para blocos de trechos diferentes — o caso
/// real do motor — o que soma é a **potência**, não a amplitude; ganho
/// constante no meio da transição (`0.5 + 0.5`) dá `√(0.5² + 0.5²) ≈ 0.707`,
/// uma queda audível de ~3 dB. `ConstantPower` mantém
/// `gain_a² + gain_b² = 1` via `cos`/`sin` do mesmo ângulo — identidade
/// trigonométrica, não calibração.
fn compute_gains(alpha: f32, curve: CrossfadeCurve) -> (f32, f32) {
    let a = alpha.clamp(0.0, 1.0);
    match curve {
        CrossfadeCurve::ConstantGain => (1.0 - a, a),
        CrossfadeCurve::ConstantPower => {
            let angle = a * FRAC_PI_2;
            (angle.cos(), angle.sin())
        }
    }
}

/// Realiza o crossfade entre dois buffers de áudio
/// O segundo buffer é sobreposto ao final do primeiro
pub fn crossfade_buffers(
    buffer_a: &mut [f32],
    start_a: usize,
    buffer_b: &[f32],
    start_b: usize,
    fade_samples: usize,
    curve: CrossfadeCurve,
) {
    let overlap_len = fade_samples
        .min(buffer_a.len() - start_a)
        .min(buffer_b.len() - start_b);

    for i in 0..overlap_len {
        // Posição na transição: de 0 (início) para 1 (fim)
        let alpha = (i as f32) / (overlap_len as f32 - 1.0).max(1.0);
        let (gain_a, gain_b) = compute_gains(alpha, curve);

        buffer_a[start_a + i] *= gain_a;
        if start_b + i < buffer_b.len() {
            buffer_a[start_a + i] += buffer_b[start_b + i] * gain_b;
        }
    }

    // O que sobrar do buffer B (se for maior que o overlap) copia direto
    let copy_start = start_a + overlap_len;
    let copy_len = buffer_b
        .len()
        .saturating_sub(overlap_len)
        .min(buffer_a.len() - copy_start);
    buffer_a[copy_start..copy_start + copy_len]
        .copy_from_slice(&buffer_b[overlap_len..overlap_len + copy_len]);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// I15 — nenhuma amostra pode ser NaN/infinita, para nenhuma curva.
    /// Não confunde com "dentro de -1..=1": potência constante aplicada a
    /// sinal correlacionado (abaixo) legitimamente ultrapassa 1.0 — é outra
    /// propriedade, testada à parte.
    #[test]
    fn test_crossfade_output_is_always_finite() {
        for curve in [CrossfadeCurve::ConstantGain, CrossfadeCurve::ConstantPower] {
            let mut a = vec![1.0f32; 100];
            let b = vec![1.0f32; 100];
            crossfade_buffers(&mut a, 0, &b, 0, 50, curve);
            assert!(
                a.iter().all(|v| v.is_finite()),
                "{curve:?} produziu amostra não finita"
            );
        }
    }

    /// `gain_a + gain_b = 1` é combinação convexa: para `a == b` (sinal
    /// correlacionado consigo mesmo), o resultado fica dentro dos limites de
    /// `a`/`b` por construção — não é uma propriedade de `ConstantPower`
    /// (ver `test_constant_power_gains_satisfy_pythagorean_identity`, onde
    /// `gain_a + gain_b` pode chegar a √2 no meio da transição).
    #[test]
    fn test_constant_gain_crossfade_of_identical_signal_stays_within_bounds() {
        let mut a = vec![1.0f32; 100];
        let b = vec![1.0f32; 100];
        crossfade_buffers(&mut a, 0, &b, 0, 50, CrossfadeCurve::ConstantGain);
        assert!(a.iter().all(|&v| (-0.01..=1.01).contains(&v)));
    }

    #[test]
    fn test_crossfade_copies_remainder_of_b() {
        let mut a = vec![0.0f32; 200];
        let b = vec![1.0f32; 150];
        crossfade_buffers(&mut a, 0, &b, 0, 50, CrossfadeCurve::ConstantGain);
        assert_eq!(a[100], 1.0);
    }

    /// docs/16 T2.2, a identidade que fecha o argumento: emendar um bloco a
    /// si mesmo com ganho constante tem que devolver o próprio bloco. Não é
    /// valor medido — é `gain_a + gain_b = 1` aplicado a `a == b`.
    #[test]
    fn test_constant_gain_crossfade_of_signal_with_itself_is_identity() {
        let x = vec![0.3f32, -0.6, 0.9, -0.2, 0.5];
        let mut a = x.clone();
        crossfade_buffers(&mut a, 0, &x, 0, x.len(), CrossfadeCurve::ConstantGain);

        for (out, orig) in a.iter().zip(x.iter()) {
            assert!((out - orig).abs() < 1e-5, "esperado {orig}, obtido {out}");
        }
    }

    /// Par do teste acima, e o que fecha a semântica dos dois curvas: para
    /// sinal correlacionado consigo mesmo, potência constante **soma**
    /// amplitude no meio — `cos(45°) + sin(45°) = 0,707 + 0,707 ≈ 1,414`,
    /// um bump de +3 dB. Não é bug: é a razão de `ConstantGain` existir para
    /// este caso. Os dois testes juntos travam a escolha contra alguém
    /// "consertar" o bump daqui a seis meses.
    #[test]
    fn test_constant_power_crossfade_of_signal_with_itself_produces_3db_bump_at_midpoint() {
        let x = vec![1.0f32; 101];
        let mut out = x.clone();
        crossfade_buffers(&mut out, 0, &x, 0, 101, CrossfadeCurve::ConstantPower);

        assert!(
            (out[50] - std::f32::consts::SQRT_2).abs() < 1e-3,
            "potência constante em sinal correlacionado deveria somar a ~1.414 (+3 dB) no meio, obtido {}",
            out[50]
        );
    }

    #[test]
    fn test_constant_gain_gains_sum_to_one() {
        for i in 0..=10 {
            let alpha = i as f32 / 10.0;
            let (a, b) = compute_gains(alpha, CrossfadeCurve::ConstantGain);
            assert!((a + b - 1.0).abs() < 1e-5, "alpha={alpha}: a+b={}", a + b);
        }
    }

    #[test]
    fn test_constant_power_gains_satisfy_pythagorean_identity() {
        for i in 0..=10 {
            let alpha = i as f32 / 10.0;
            let (a, b) = compute_gains(alpha, CrossfadeCurve::ConstantPower);
            assert!(
                (a * a + b * b - 1.0).abs() < 1e-5,
                "alpha={alpha}: a²+b²={}",
                a * a + b * b
            );
        }
    }

    /// docs/16 T2.2, a queda de ~3 dB que motiva a correção, na matemática
    /// em si (não num buffer — ver o módulo para por que sinal correlacionado
    /// não a demonstra). No meio da transição, ganho constante dá
    /// `gain_a = gain_b = 0.5`: soma de amplitude 1.0, mas para sinais **não
    /// correlacionados** a potência resultante é `0.5² + 0.5² = 0.5`, RMS
    /// caindo a √0,5 ≈ 0,707 (~3 dB). Potência constante mantém
    /// `a² + b² = 1` no mesmo ponto — é a identidade que corrige a queda.
    #[test]
    fn test_constant_gain_underpowers_relative_to_constant_power_at_midpoint() {
        let (gain_a, gain_b) = compute_gains(0.5, CrossfadeCurve::ConstantGain);
        let (power_a, power_b) = compute_gains(0.5, CrossfadeCurve::ConstantPower);

        let gain_power_sum = gain_a * gain_a + gain_b * gain_b;
        let power_power_sum = power_a * power_a + power_b * power_b;

        assert!(
            (gain_power_sum - 0.5).abs() < 1e-5,
            "ganho constante no meio deveria somar potência 0.5, obtido {gain_power_sum}"
        );
        assert!(
            (power_power_sum - 1.0).abs() < 1e-5,
            "potência constante no meio deveria somar potência 1.0, obtido {power_power_sum}"
        );
    }
}
