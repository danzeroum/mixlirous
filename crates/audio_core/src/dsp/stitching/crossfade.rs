use super::FadeCurve;

/// Realiza o crossfade entre dois buffers de áudio
/// O segundo buffer é sobreposto ao final do primeiro
pub fn crossfade_buffers(
    buffer_a: &mut [f32],
    start_a: usize,
    buffer_b: &[f32],
    start_b: usize,
    fade_samples: usize,
    curve: FadeCurve,
) {
    let overlap_len = fade_samples
        .min(buffer_a.len() - start_a)
        .min(buffer_b.len() - start_b);

    // Aplica fade-out em A (não temos, então fazemos fade-in reverso em A e fade-in normal em B)
    // Técnica: ganhoA(t) = 1 - gainB(t)

    for i in 0..overlap_len {
        // Gain de transição: de 0 (início) para 1 (fim)
        let alpha = (i as f32) / (overlap_len as f32 - 1.0).max(1.0);

        let gain_b = match curve {
            FadeCurve::Linear => alpha,
            // ln(1) = 0 e ln(e) = 1, então isto varre 0..1 sem divisão por zero
            FadeCurve::Logarithmic => (1.0 + alpha * (std::f32::consts::E - 1.0)).ln(),
            FadeCurve::Exponential => alpha.exp2() - 1.0,
        };
        let gain_a = 1.0 - gain_b;

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

    #[test]
    fn test_crossfade_preserves_amplitude_bounds() {
        for curve in [
            FadeCurve::Linear,
            FadeCurve::Logarithmic,
            FadeCurve::Exponential,
        ] {
            let mut a = vec![1.0f32; 100];
            let b = vec![1.0f32; 100];
            crossfade_buffers(&mut a, 0, &b, 0, 50, curve);
            assert!(a
                .iter()
                .all(|&v| v.is_finite() && (-0.01..=1.01).contains(&v)));
        }
    }

    #[test]
    fn test_crossfade_copies_remainder_of_b() {
        let mut a = vec![0.0f32; 200];
        let b = vec![1.0f32; 150];
        crossfade_buffers(&mut a, 0, &b, 0, 50, FadeCurve::Linear);
        assert_eq!(a[100], 1.0);
    }
}
