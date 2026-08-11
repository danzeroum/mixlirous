use audio_core::dsp::mastering::compressor::{apply_compression, CompressorParams};
use audio_core::domain::{AttackMs, CompressionRatio, ReleaseMs, ThresholdDb};
use proptest::prelude::*;

fn arb_sample() -> impl Strategy<Value = f32> {
    (-1.0f32..=1.0).prop_map(|x| x)
}

fn arb_pcm() -> impl Strategy<Value = Vec<f32>> {
    prop::collection::vec(arb_sample(), 1..=48000)
}

fn arb_params() -> impl Strategy<Value = CompressorParams> {
    (
        (-60.0f32..=0.0).prop_map(|x| ThresholdDb::try_from(x).unwrap()),
        (1.0f32..=10.0).prop_map(|x| CompressionRatio::try_from(x).unwrap()),
        (0u32..=500).prop_map(|x| AttackMs::try_from(x).unwrap()),
        (10u32..=5000).prop_map(|x| ReleaseMs::try_from(x).unwrap()),
    )
    .prop_map(|(threshold, ratio, attack, release)| CompressorParams {
        threshold_db: threshold,
        ratio,
        attack_ms: attack,
        release_ms: release,
        makeup_gain_db: 0.0,
        knee_db: 6.0,
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    /// I1: Compressor com makeup ≤ 0 nunca aumenta o pico
    #[test]
    fn compressor_never_increases_peak_with_zero_makeup(
        input in arb_pcm(),
        params in arb_params(),
    ) {
        let sr = 44100u32;
        let output = apply_compression(&input, &params, sr);
        let input_peak = input.iter().fold(0.0f32, |m, &x| m.max(x.abs()));
        let output_peak = output.iter().fold(0.0f32, |m, &x| m.max(x.abs()));
        prop_assert!(
            output_peak <= input_peak + 1e-6,
            "input_peak={} output_peak={}",
            input_peak,
            output_peak
        );
    }

    /// I1b: ratio = 1.0 é identidade (sem makeup)
    #[test]
    fn compressor_ratio_one_is_identity(
        input in arb_pcm(),
    ) {
        let params = CompressorParams {
            threshold_db: ThresholdDb::try_from(-18.0).unwrap(),
            ratio: CompressionRatio::try_from(1.0).unwrap(),
            attack_ms: AttackMs::try_from(30).unwrap(),
            release_ms: ReleaseMs::try_from(250).unwrap(),
            makeup_gain_db: 0.0,
            knee_db: 6.0,
        };
        let sr = 44100u32;
        let output = apply_compression(&input, &params, sr);
        for (i, (inp, out)) in input.iter().zip(output.iter()).enumerate() {
            prop_assert!(
                (inp - out).abs() < 1e-5,
                "diff at {}: inp={} out={}",
                i, inp, out
            );
        }
    }

    /// I1c: Silêncio permanece silêncio
    #[test]
    fn compressor_silence_stays_silent(
        params in arb_params(),
    ) {
        let input = vec![0.0f32; 1000];
        let sr = 44100u32;
        let output = apply_compression(&input, &params, sr);
        for (i, &sample) in output.iter().enumerate() {
            prop_assert!(
                sample.abs() < 1e-10,
                "non-zero at {}: {}",
                i, sample
            );
        }
    }
}