use crate::domain::block::BeatBlock;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SelectionError {
    #[error("cannot meet target duration: target={target}s, tolerance={tolerance}s")]
    CannotMeetTarget { target: f32, tolerance: f32 },
    #[error("no blocks available")]
    NoBlocks,
    #[error("intro/outro preservation exceeds target duration")]
    PreservationTooLarge,
}

#[derive(Debug, Clone)]
pub struct SelectionConfig {
    pub target_duration_sec: f32,
    pub duration_tolerance_sec: f32,
    pub preserve_intro_ms: u32,
    pub preserve_outro_ms: u32,
    pub require_strong_beat_start: bool,
    pub allow_repeats: bool,
}

impl Default for SelectionConfig {
    fn default() -> Self {
        Self {
            target_duration_sec: 30.0,
            duration_tolerance_sec: 2.0,
            preserve_intro_ms: 0,
            preserve_outro_ms: 0,
            require_strong_beat_start: true,
            allow_repeats: false,
        }
    }
}

pub fn select_blocks(
    blocks: &[BeatBlock],
    config: &SelectionConfig,
) -> Result<Vec<BeatBlock>, SelectionError> {
    if blocks.is_empty() {
        return Err(SelectionError::NoBlocks);
    }

    let target = config.target_duration_sec;
    let tolerance = config.duration_tolerance_sec;

    let preservation_sec = (config.preserve_intro_ms + config.preserve_outro_ms) as f32 / 1000.0;
    let available_target = target - preservation_sec;

    if available_target <= 0.0 {
        return Err(SelectionError::PreservationTooLarge);
    }

    let candidates: Vec<&BeatBlock> = blocks.iter().collect();

    if candidates.is_empty() {
        return Err(SelectionError::NoBlocks);
    }

    let step_ms = 10.0;
    let step_sec = step_ms / 1000.0;
    let max_steps = (available_target / step_sec) as usize + 1;

    let n = candidates.len();
    let mut dp = vec![vec![0.0f32; max_steps + 1]; n + 1];
    let mut keep = vec![vec![false; max_steps + 1]; n + 1];

    for i in 1..=n {
        let block = candidates[i - 1];
        let block_steps = (block.duration / step_sec) as usize;
        let score = block.score;

        for t in 0..=max_steps {
            dp[i][t] = dp[i - 1][t];
            keep[i][t] = false;

            if block_steps <= t {
                let candidate_score = dp[i - 1][t - block_steps] + score;
                if candidate_score > dp[i][t] {
                    dp[i][t] = candidate_score;
                    keep[i][t] = true;
                }
            }
        }
    }

    let mut best_t = 0;
    let mut best_score = 0.0f32;

    #[allow(clippy::needless_range_loop)]
    for t in 0..=max_steps {
        let duration = t as f32 * step_sec;
        if (duration - available_target).abs() <= tolerance && dp[n][t] > best_score {
            best_score = dp[n][t];
            best_t = t;
        }
    }

    if best_t == 0 && best_score == 0.0 {
        return Err(SelectionError::CannotMeetTarget { target, tolerance });
    }

    let mut selected = Vec::new();
    let mut t = best_t;
    for i in (1..=n).rev() {
        if keep[i][t] {
            selected.push(candidates[i - 1].clone());
            let block_steps = (candidates[i - 1].duration / step_sec) as usize;
            t -= block_steps;
        }
    }

    selected.sort_by_key(|b| b.beat_index);

    Ok(selected)
}

pub fn select_continuous_window(
    blocks: &[BeatBlock],
    config: &SelectionConfig,
) -> Result<Vec<BeatBlock>, SelectionError> {
    if blocks.is_empty() {
        return Err(SelectionError::NoBlocks);
    }

    let target = config.target_duration_sec;

    let mut prefix_duration = vec![0.0f32; blocks.len() + 1];
    let mut prefix_energy = vec![0.0f32; blocks.len() + 1];

    for (i, block) in blocks.iter().enumerate() {
        prefix_duration[i + 1] = prefix_duration[i] + block.duration;
        prefix_energy[i + 1] = prefix_energy[i] + block.rms_energy * block.duration;
    }

    let mut best_start = 0;
    let mut best_end = 0;
    let mut best_avg_energy = 0.0f32;

    for start in 0..blocks.len() {
        for end in (start + 1)..=blocks.len() {
            let window_duration = prefix_duration[end] - prefix_duration[start];

            if (window_duration - target).abs() <= config.duration_tolerance_sec {
                let window_energy = prefix_energy[end] - prefix_energy[start];
                let avg_energy = window_energy / window_duration;

                if avg_energy > best_avg_energy {
                    best_avg_energy = avg_energy;
                    best_start = start;
                    best_end = end;
                }
            }
        }
    }

    if best_end == 0 {
        return Err(SelectionError::CannotMeetTarget {
            target,
            tolerance: config.duration_tolerance_sec,
        });
    }

    Ok(blocks[best_start..best_end].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
    fn test_select_blocks_empty() {
        let config = SelectionConfig::default();
        let result = select_blocks(&[], &config);
        assert!(matches!(result, Err(SelectionError::NoBlocks)));
    }

    #[test]
    fn test_select_blocks_basic() {
        let blocks = vec![
            make_block(0, 4.0, 0.8, 0),
            make_block(1, 4.0, 0.6, 1),
            make_block(2, 4.0, 0.9, 2),
            make_block(3, 4.0, 0.7, 3),
        ];
        let config = SelectionConfig {
            target_duration_sec: 8.0,
            duration_tolerance_sec: 1.0,
            ..Default::default()
        };
        let result = select_blocks(&blocks, &config).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|b| b.score == 0.9));
        assert!(result.iter().any(|b| b.score == 0.8));
    }

    #[test]
    fn test_select_blocks_deterministic() {
        let blocks = vec![
            make_block(0, 4.0, 0.8, 0),
            make_block(1, 4.0, 0.6, 1),
            make_block(2, 4.0, 0.9, 2),
        ];
        let config = SelectionConfig {
            target_duration_sec: 8.0,
            duration_tolerance_sec: 1.0,
            ..Default::default()
        };
        let result1 = select_blocks(&blocks, &config).unwrap();
        let result2 = select_blocks(&blocks, &config).unwrap();
        assert_eq!(result1.len(), result2.len());
        for (a, b) in result1.iter().zip(result2.iter()) {
            assert_eq!(a.id, b.id);
        }
    }

    #[test]
    fn test_select_blocks_chronological_order() {
        let blocks = vec![
            make_block(0, 4.0, 0.5, 0),
            make_block(1, 4.0, 0.9, 1),
            make_block(2, 4.0, 0.3, 2),
            make_block(3, 4.0, 0.8, 3),
        ];
        let config = SelectionConfig {
            target_duration_sec: 8.0,
            duration_tolerance_sec: 1.0,
            ..Default::default()
        };
        let result = select_blocks(&blocks, &config).unwrap();
        for window in result.windows(2) {
            assert!(window[0].beat_index <= window[1].beat_index);
        }
    }

    #[test]
    fn test_select_continuous_window() {
        let blocks = vec![
            make_block(0, 4.0, 0.5, 0),
            make_block(1, 4.0, 0.9, 1),
            make_block(2, 4.0, 0.3, 2),
            make_block(3, 4.0, 0.8, 3),
        ];
        let config = SelectionConfig {
            target_duration_sec: 8.0,
            duration_tolerance_sec: 1.0,
            ..Default::default()
        };
        let result = select_continuous_window(&blocks, &config).unwrap();
        assert_eq!(result.len(), 2);
    }
}
