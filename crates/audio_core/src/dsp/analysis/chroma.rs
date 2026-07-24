use ndarray::ArrayView1;

/// Extração de Chroma Vector (12 classes de pitch)
/// Implementação simplificada — na prática, usar CQT (Constant-Q Transform)
pub fn chroma_vector(pcm: ArrayView1<f32>, sample_rate: u32) -> Vec<f32> {
    // Placeholder: implementação real usaria CQT para mapear frequências
    // para 12 classes de pitch (C, C#, D, ... B)
    // Aqui usamos uma aproximação via FFT + mapeamento logarítmico

    let mag = super::fft::magnitude_spectrum(pcm);
    let n = mag.len();
    if n == 0 {
        return vec![0.0; 12];
    }

    // Frequência de cada bin
    let bin_freq = |i: usize| (i as f32 * sample_rate as f32) / (n as f32 * 2.0);

    // Cria 12 buckets (uma por semitom, oitava 2-8)
    let mut chroma = vec![0.0f32; 12];

    for (i, &magnitude) in mag.iter().enumerate().skip(1) {
        let freq = bin_freq(i);
        if !(65.0..=2000.0).contains(&freq) {
            continue; // Faixa de interesse
        }

        let note_index = ((12.0 * (freq.log2() - 6.0)) % 12.0).round() as i32; // A440 reference

        if (0..12).contains(&note_index) {
            chroma[note_index as usize] += magnitude;
        }
    }

    // Normaliza
    let sum: f32 = chroma.iter().sum();
    if sum > 0.0 {
        for c in &mut chroma {
            *c /= sum;
        }
    }

    chroma
}
