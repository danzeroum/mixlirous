/// Tipo de curva de fade
#[derive(Debug, Clone, PartialEq)]
pub enum FadeCurve {
    Linear,
    Logarithmic,
    Exponential,
}

/// Aplica um fade-out no buffer PCM
pub fn apply_fade_out(pcm: &mut [f32], start: usize, duration_samples: usize, curve: &FadeCurve) {
    let len = duration_samples.min(pcm.len() - start);
    match curve {
        FadeCurve::Linear => {
            for i in 0..len {
                let alpha = 1.0 - (i as f32) / (len as f32);
                pcm[start + i] *= alpha;
            }
        },
        FadeCurve::Logarithmic => {
            let n = len as f32;
            for i in 0..len {
                let x = i as f32;
                let gain = 1.0 - (1.0 + x).ln() / (1.0 + n).ln();
                pcm[start + i] *= gain.max(0.0);
            }
        },
        FadeCurve::Exponential => {
            let n = (len as f32 - 1.0).max(1.0);
            for i in 0..len {
                let alpha = i as f32 / n;
                // 2^alpha varia em [1,2]; "2 - 2^alpha" varre 1..0 sem saturar
                let gain = 2.0 - alpha.exp2();
                pcm[start + i] *= gain.clamp(0.0, 1.0);
            }
        },
    }
}

/// Aplica um fade-in no buffer PCM
pub fn apply_fade_in(pcm: &mut [f32], start: usize, duration_samples: usize, curve: &FadeCurve) {
    let len = duration_samples.min(pcm.len() - start);
    match curve {
        FadeCurve::Linear => {
            for i in 0..len {
                let alpha = (i as f32) / (len as f32);
                pcm[start + i] *= alpha;
            }
        },
        FadeCurve::Logarithmic => {
            let n = len as f32;
            for i in 0..len {
                let x = (len - 1 - i) as f32;
                let gain = 1.0 - (1.0 + x).ln() / (1.0 + n).ln();
                pcm[start + i] *= gain.max(0.0);
            }
        },
        FadeCurve::Exponential => {
            let n = (len as f32 - 1.0).max(1.0);
            for i in 0..len {
                let alpha = i as f32 / n;
                // 2^alpha - 1 varre 0..1 sem saturar
                let gain = alpha.exp2() - 1.0;
                pcm[start + i] *= gain.clamp(0.0, 1.0);
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fade_out_ends_near_silence_for_every_curve() {
        for curve in [
            FadeCurve::Linear,
            FadeCurve::Logarithmic,
            FadeCurve::Exponential,
        ] {
            let mut pcm = vec![1.0f32; 100];
            apply_fade_out(&mut pcm, 0, 100, &curve);
            assert!(
                pcm[0] > 0.9,
                "{curve:?} deveria come├ºar pr├│ximo do volume cheio"
            );
            assert!(
                pcm[99] < 0.15,
                "{curve:?} deveria terminar pr├│ximo do sil├¬ncio, got {}",
                pcm[99]
            );
        }
    }

    #[test]
    fn test_fade_in_starts_near_silence_for_every_curve() {
        for curve in [
            FadeCurve::Linear,
            FadeCurve::Logarithmic,
            FadeCurve::Exponential,
        ] {
            let mut pcm = vec![1.0f32; 100];
            apply_fade_in(&mut pcm, 0, 100, &curve);
            assert!(
                pcm[0] < 0.15,
                "{curve:?} deveria come├ºar pr├│ximo do sil├¬ncio, got {}",
                pcm[0]
            );
            assert!(
                pcm[99] > 0.9,
                "{curve:?} deveria terminar pr├│ximo do volume cheio"
            );
        }
    }
}
