use audio_core::dsp::selection::{select_blocks, select_continuous_window, SelectionConfig, SelectionError};
use audio_core::domain::block::BeatBlock;
use proptest::prelude::*;
use uuid::Uuid;

fn make_block(id: usize, duration: f32, score: f32, beat_index: usize) -> BeatBlock {
    BeatBlock {
        id: Uuid::new_v4(),
        start_sample: id * 44100,
        end_sample: (id + 1) * 44100,
        start_time: id as f32,
        end_time: id as f32 + duration,
        duration,
        rms_energy: score,
        spectral_centroid: 1000.0,
        chroma_vector: None,
        beat_index,
        score,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1_000))]

    /// I8: Knapsack respeita target ± tolerance ou retorna Err
    #[test]
    fn knapsack_respects_target_or_errors(
        block_duration in (1.0f32..=4.0),
        num_blocks in 5usize..=15,
    ) {
        // Create blocks with identical duration so we can always hit the target
        let blocks: Vec<BeatBlock> = (0..num_blocks)
            .map(|i| make_block(i, block_duration, 0.5 + (i as f32 * 0.05), i))
            .collect();

        let target = block_duration * 3.0; // Target = 3 blocks
        let config = SelectionConfig {
            target_duration_sec: target,
            duration_tolerance_sec: 1.0,
            ..Default::default()
        };
        
        match select_blocks(&blocks, &config) {
            Ok(selected) => {
                let total_duration: f32 = selected.iter().map(|b| b.duration).sum();
                prop_assert!(
                    (total_duration - target).abs() <= config.duration_tolerance_sec,
                    "duration {} not within {} of target {}",
                    total_duration,
                    config.duration_tolerance_sec,
                    target
                );
            }
            Err(SelectionError::CannotMeetTarget { .. }) => {
                // Valid error case - only if no combination works
            }
            Err(e) => {
                prop_assert!(false, "unexpected error: {}", e);
            }
        }
    }

    /// I9: Knapsack é determinístico (mesma entrada → mesma saída)
    #[test]
    fn knapsack_is_deterministic(
        num_blocks in 5usize..=15,
        block_duration in (1.0f32..=4.0),
    ) {
        let blocks: Vec<BeatBlock> = (0..num_blocks)
            .map(|i| make_block(i, block_duration, 0.5 + (i as f32 * 0.05), i))
            .collect();

        let config = SelectionConfig {
            target_duration_sec: block_duration * 3.0,
            duration_tolerance_sec: 1.0,
            ..Default::default()
        };
        
        let result1 = select_blocks(&blocks, &config);
        let result2 = select_blocks(&blocks, &config);
        
        match (result1, result2) {
            (Ok(r1), Ok(r2)) => {
                prop_assert_eq!(r1.len(), r2.len());
                for (a, b) in r1.iter().zip(r2.iter()) {
                    prop_assert_eq!(a.id, b.id);
                }
            }
            (Err(_), Err(_)) => {
                // Both errored - OK
            }
            (r1, r2) => {
                prop_assert!(false, "inconsistent results: {:?} vs {:?}", r1, r2);
            }
        }
    }

    /// Knapsack retorna blocos em ordem cronológica
    #[test]
    fn knapsack_returns_chronological_order(
        num_blocks in 5usize..=15,
        block_duration in (1.0f32..=4.0),
    ) {
        let blocks: Vec<BeatBlock> = (0..num_blocks)
            .map(|i| make_block(i, block_duration, 0.5 + (i as f32 * 0.05), i))
            .collect();

        let config = SelectionConfig {
            target_duration_sec: block_duration * 3.0,
            duration_tolerance_sec: 1.0,
            ..Default::default()
        };
        
        if let Ok(selected) = select_blocks(&blocks, &config) {
            for window in selected.windows(2) {
                prop_assert!(
                    window[0].beat_index <= window[1].beat_index,
                    "blocks not in chronological order: {} vs {}",
                    window[0].beat_index,
                    window[1].beat_index
                );
            }
        }
    }
}