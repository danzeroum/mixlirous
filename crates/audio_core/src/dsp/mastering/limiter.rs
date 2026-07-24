/// Aplica brickwall limiting para garantir pico máximo
pub fn brickwall_limiter(pcm: &mut [f32], max_peak_db: f32) {
    let max_peak_linear = 10f32.powf(max_peak_db / 20.0);
    let mut current_peak = 0.0f32;

    // Primeiro passo: encontrar o pico
    for &sample in pcm.iter() {
        let abs = sample.abs();
        if abs > current_peak {
            current_peak = abs;
        }
    }

    if current_peak > max_peak_linear && current_peak > 0.0 {
        let gain = max_peak_linear / current_peak;
        for sample in pcm.iter_mut() {
            *sample *= gain;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_limiter_clamps_peak_to_target() {
        let mut pcm = vec![0.0f32, 1.0, -0.8, 0.5];
        brickwall_limiter(&mut pcm, -6.0); // -6 dBFS ~= 0.501 linear
        let target = 10f32.powf(-6.0 / 20.0);
        let peak = pcm.iter().cloned().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!((peak - target).abs() < 1e-4);
    }

    #[test]
    fn test_limiter_leaves_quiet_signal_untouched() {
        let mut pcm = vec![0.1f32, -0.2, 0.05];
        let before = pcm.clone();
        brickwall_limiter(&mut pcm, -1.0);
        assert_eq!(pcm, before);
    }
}
