use crate::domain::{AttackMs, CompressionRatio, ReleaseMs, ThresholdDb};

#[derive(Debug, Clone)]
pub struct CompressorParams {
    pub threshold_db: ThresholdDb,
    pub ratio: CompressionRatio,
    pub attack_ms: AttackMs,
    pub release_ms: ReleaseMs,
    pub makeup_gain_db: f32,
    pub knee_db: f32,
}

impl Default for CompressorParams {
    fn default() -> Self {
        Self {
            threshold_db: ThresholdDb::try_from(-18.0).unwrap(),
            ratio: CompressionRatio::try_from(2.0).unwrap(),
            attack_ms: AttackMs::try_from(30).unwrap(),
            release_ms: ReleaseMs::try_from(250).unwrap(),
            makeup_gain_db: 0.0,
            knee_db: 6.0,
        }
    }
}

pub fn apply_compression(input: &[f32], params: &CompressorParams, sample_rate: u32) -> Vec<f32> {
    if input.is_empty() {
        return Vec::new();
    }

    let sr = sample_rate as f32;
    let threshold = params.threshold_db.get();
    let ratio = params.ratio.get();
    let attack_coef = (-1000.0 / params.attack_ms.get() as f32 / sr).exp();
    let release_coef = (-1000.0 / params.release_ms.get() as f32 / sr).exp();
    let eps = 1e-10;

    let mut output = Vec::with_capacity(input.len());
    let mut envelope_db: f32 = 0.0;

    for &sample in input {
        if sample == 0.0 {
            output.push(0.0);
            continue;
        }

        let abs_sample = sample.abs();
        let level_db = 20.0 * (abs_sample + eps).log10();

        let reduction_db = if params.knee_db > 0.0 {
            let half_knee = params.knee_db / 2.0;
            if level_db < threshold - half_knee {
                0.0
            } else if level_db > threshold + half_knee {
                (level_db - threshold) * (1.0 - 1.0 / ratio)
            } else {
                let delta = level_db - (threshold - half_knee);
                delta * delta / (2.0 * params.knee_db) * (1.0 - 1.0 / ratio)
            }
        } else if level_db > threshold {
            (level_db - threshold) * (1.0 - 1.0 / ratio)
        } else {
            0.0
        };

        if reduction_db > envelope_db {
            envelope_db = reduction_db + attack_coef * (envelope_db - reduction_db);
        } else {
            envelope_db = reduction_db + release_coef * (envelope_db - reduction_db);
        }

        let gain = 10f32.powf((-envelope_db + params.makeup_gain_db) / 20.0);
        output.push(sample * gain);
    }

    output
}
