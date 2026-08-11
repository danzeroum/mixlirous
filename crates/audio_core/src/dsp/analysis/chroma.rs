use ndarray::ArrayView1;

/// chroma vector extraction (12 pitch classes)
/// 
/// Implementation follows the spec in docs/04-DOMINIO-DSP.md §B.5:
/// 1. FFT per frame → magnitude
/// 2. Map each frequency bin to pitch class:
///    c = floor(12 * log2(f / 440)) mod 12
/// 3. Sum magnitudes per class → 12-value vector, L2-normalized
pub fn chroma_vector(pcm: ArrayView1<f32>, sample_rate: u32) -> Vec<f32> {
    let mag = super::fft::magnitude_spectrum(pcm);
    let n = mag.len();
    if n == 0 {
        return vec![0.0; 12];
    }

    let bin_freq = |i: usize| -> f32 {
        i as f32 * sample_rate as f32 / (n as f32 * 2.0)
    };

    let mut chroma = vec![0.0f32; 12];

    for (i, &magnitude) in mag.iter().enumerate().skip(1) {
        let freq = bin_freq(i);
        
        // Focus on musically relevant range (C2 to C8, ~65 Hz to ~4186 Hz)
        if !(65.0..=4186.0).contains(&freq) {
            continue;
        }

        // Map frequency to pitch class (12 semitones)
        // A4 = 440 Hz is class 9
        let semitone = (12.0 * (freq / 440.0).log2() + 9.0).round();
        let class = ((semitone % 12.0 + 12.0) % 12.0) as usize;
        
        if class < 12 {
            chroma[class] += magnitude;
        }
    }

    // L2 normalize
    let norm: f32 = chroma.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 1e-10 {
        for c in chroma.iter_mut() {
            *c /= norm;
        }
    }

    chroma
}

/// Compute similarity between two chroma vectors (cosine similarity)
pub fn chroma_similarity(a: &[f32; 12], b: &[f32; 12]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// Build similarity matrix from a sequence of chroma vectors
pub fn similarity_matrix(chroma_vectors: &[[f32; 12]]) -> Vec<Vec<f32>> {
    let n = chroma_vectors.len();
    let mut matrix = vec![vec![0.0f32; n]; n];
    
    for i in 0..n {
        for j in i..n {
            let sim = chroma_similarity(&chroma_vectors[i], &chroma_vectors[j]);
            matrix[i][j] = sim;
            matrix[j][i] = sim;
        }
    }
    
    matrix
}

/// Detect sections using novelty curve from similarity matrix
pub fn detect_sections(
    chroma_vectors: &[[f32; 12]],
    sample_rate: u32,
    hop_size: usize,
) -> Vec<(f32, f32, String)> {
    if chroma_vectors.len() < 2 {
        return vec![(0.0, 0.0, "unknown".to_string())];
    }

    let matrix = similarity_matrix(chroma_vectors);
    let n = matrix.len();
    
    // Compute novelty curve (diagonal differences)
    let kernel_size = 4.min(n / 2);
    let mut novelty = vec![0.0f32; n];
    
    for i in kernel_size..(n - kernel_size) {
        let mut score = 0.0f32;
        for k in 1..=kernel_size {
            score += matrix[i - k][i + k];
        }
        novelty[i] = score / kernel_size as f32;
    }
    
    // Find peaks in novelty curve
    let threshold = 0.5;
    let mut peaks = vec![0]; // Start with first frame
    
    for i in 1..(n - 1) {
        if novelty[i] > novelty[i - 1] 
            && novelty[i] > novelty[i + 1] 
            && novelty[i] > threshold 
        {
            peaks.push(i);
        }
    }
    peaks.push(n - 1); // End with last frame
    
    // Convert peaks to sections
    let time_per_frame = hop_size as f32 / sample_rate as f32;
    let mut sections = Vec::new();
    
    for window in peaks.windows(2) {
        let start_frame = window[0];
        let end_frame = window[1];
        let start_time = start_frame as f32 * time_per_frame;
        let end_time = end_frame as f32 * time_per_frame;
        
        // Simple heuristic for section labels
        let label = if start_time < 5.0 {
            "intro"
        } else if end_time - start_time < 3.0 {
            "transition"
        } else {
            "section"
        };
        
        sections.push((start_time, end_time, label.to_string()));
    }
    
    sections
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chroma_pure_sine_a4() {
        // A4 = 440 Hz should concentrate > 80% energy in class 9 (A)
        let sr = 44100;
        let duration_secs = 1.0;
        let num_samples = (sr as f32 * duration_secs) as usize;
        let pcm: Vec<f32> = (0..num_samples)
            .map(|i| {
                let t = i as f32 / sr as f32;
                (2.0 * std::f32::consts::PI * 440.0 * t).sin()
            })
            .collect();
        
        let chroma = chroma_vector(ndarray::ArrayView1::from(&pcm), sr);
        
        // Find the class with most energy
        let (max_class, max_energy) = chroma.iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(i, &v)| (i, v))
            .unwrap();
        
        // Class 9 = A should dominate
        assert_eq!(max_class, 9, "Expected class 9 (A), got {}", max_class);
        assert!(
            max_energy > 0.8,
            "Expected > 80% energy in class 9, got {}",
            max_energy
        );
    }

    #[test]
    fn test_chroma_similarity_identical() {
        let a = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let sim = chroma_similarity(&a, &a);
        assert!((sim - 1.0).abs() < 0.01, "Similarity of identical vectors should be ~1.0");
    }

    #[test]
    fn test_chroma_similarity_orthogonal() {
        let a = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let b = [0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let sim = chroma_similarity(&a, &b);
        assert!(sim < 0.01, "Similarity of orthogonal vectors should be ~0.0");
    }

    #[test]
    fn test_similarity_matrix_symmetric() {
        let vectors = [
            [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        ];
        
        let matrix = similarity_matrix(&vectors);
        for i in 0..3 {
            for j in 0..3 {
                assert!(
                    (matrix[i][j] - matrix[j][i]).abs() < 0.01,
                    "Matrix should be symmetric at ({}, {})",
                    i,
                    j
                );
            }
        }
    }

    #[test]
    fn test_detect_sections_empty() {
        let sections = detect_sections(&[], 44100, 512);
        assert_eq!(sections.len(), 1);
    }

    #[test]
    fn test_detect_sections_minimal() {
        let vectors = [
            [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        ];
        let sections = detect_sections(&vectors, 44100, 512);
        assert!(sections.len() >= 1);
    }
}