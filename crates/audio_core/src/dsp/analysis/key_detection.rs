/// Deteccao de tonalidade via algoritmo de Krumhansl-Schmuckler.
/// Requer croma agregado temporalmente (nao frame-a-frame).
/// Referencia: Krumhansl & Schmuckler (1986).

/// Perfis de chave tonal maiores e menores (Krumhansl-Kessler).
pub const MAJOR_PROFILE: [f32; 12] = [
    6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
];

/// Perfis de chave tonal menores (Krumhansl-Kessler).
pub const MINOR_PROFILE: [f32; 12] = [
    6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
];

/// Modo da tonalidade detectada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyMode {
    Major,
    Minor,
}

/// Resultado da deteccao: tonica (0-11), modo, confianca, candidatos.
pub struct TonalContext {
    /// Tonica: 0=C, 1=C#, ..., 11=B
    pub root: u8,
    /// Modo (maior ou menor)
    pub mode: KeyMode,
    /// Coeficiente de correlacao da melhor correspondencia.
    pub confidence: f32,
    /// Top-3 candidatos com (tonica, modo, correlacao).
    pub candidates: Vec<(u8, KeyMode, f32)>,
}

/// Rotaciona um perfil de chave pela quantidade de semitons.
/// Para root=1 (C#), o perfil de C e deslocado para que a
/// tonica (valor mais alto) fique na posicao 1.
fn rotate_profile(profile: &[f32; 12], root: usize) -> [f32; 12] {
    let r = root % 12;
    let mut rotated = [0.0f32; 12];
    for i in 0..12 {
        rotated[i] = profile[(i + 12 - r) % 12];
    }
    rotated
}

/// Correlacao de Pearson entre dois vetores de 12 elementos.
pub fn pearson_correlation(a: &[f32; 12], b: &[f32; 12]) -> f32 {
    let n = 12.0f32;
    let mean_a: f32 = a.iter().sum::<f32>() / n;
    let mean_b: f32 = b.iter().sum::<f32>() / n;

    let mut cov = 0.0f32;
    let mut var_a = 0.0f32;
    let mut var_b = 0.0f32;

    for i in 0..12 {
        let da = a[i] - mean_a;
        let db = b[i] - mean_b;
        cov += da * db;
        var_a += da * da;
        var_b += db * db;
    }

    let denom = (var_a * var_b).sqrt();
    if denom < 1e-10 {
        return 0.0;
    }
    cov / denom
}

/// Detecta a tonalidade a partir de um vetor croma agregado.
/// Retorna TonalContext com a melhor correlacao.
pub fn detect_key(aggregated_chroma: &[f32]) -> TonalContext {
    // Converte slice para array fixo, preenchendo com zeros se necessario
    let chroma_arr: [f32; 12] = if aggregated_chroma.len() >= 12 {
        let mut arr = [0.0f32; 12];
        arr.copy_from_slice(&aggregated_chroma[..12]);
        arr
    } else {
        let mut arr = [0.0f32; 12];
        arr[..aggregated_chroma.len()].copy_from_slice(aggregated_chroma);
        arr
    };

    // Testa todas as 24 tonalidades (12 raizes x 2 modos)
    let mut all_scores: Vec<(u8, KeyMode, f32)> = Vec::with_capacity(24);

    for root in 0..12u8 {
        // Perfil maior rotacionado
        let major_rot = rotate_profile(&MAJOR_PROFILE, root as usize);
        let corr_major = pearson_correlation(&chroma_arr, &major_rot);
        all_scores.push((root, KeyMode::Major, corr_major));

        // Perfil menor rotacionado
        let minor_rot = rotate_profile(&MINOR_PROFILE, root as usize);
        let corr_minor = pearson_correlation(&chroma_arr, &minor_rot);
        all_scores.push((root, KeyMode::Minor, corr_minor));
    }

    // Ordena por correlacao decrescente
    all_scores.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    // Pega o melhor resultado
    let (best_root, best_mode, best_corr) = all_scores[0].clone();
    // Pega os top-3
    let candidates = all_scores.into_iter().take(3).collect();

    TonalContext {
        root: best_root,
        mode: best_mode,
        confidence: best_corr,
        candidates,
    }
}

/// Agrega uma sequencia de vetores croma em um unico vetor,
/// ponderando por RMS (energia local). Silence frames recebem peso zero.
pub fn aggregate_chroma(chroma_vectors: &[Vec<f32>], rms_values: &[f32]) -> Vec<f32> {
    if chroma_vectors.is_empty() {
        return vec![0.0; 12];
    }

    let mut weighted_sum = vec![0.0f32; 12];
    let mut total_weight = 0.0f32;

    for (i, chroma) in chroma_vectors.iter().enumerate() {
        let weight = rms_values.get(i).copied().unwrap_or(0.0);
        if weight < 1e-10 {
            // Frames silenciosos recebem peso zero
            continue;
        }
        for (j, &v) in chroma.iter().enumerate().take(12) {
            weighted_sum[j] += v * weight;
        }
        total_weight += weight;
    }

    if total_weight < 1e-10 {
        return vec![0.0; 12];
    }

    // Normaliza
    weighted_sum.iter().map(|&x| x / total_weight).collect()
}

/// Agregacao simplificada: media aritmetica dos vetores croma.
pub fn aggregate_chroma_simple(chroma_vectors: &[Vec<f32>]) -> Vec<f32> {
    if chroma_vectors.is_empty() {
        return vec![0.0; 12];
    }

    let mut sum = vec![0.0f32; 12];
    for chroma in chroma_vectors {
        for (j, &v) in chroma.iter().enumerate().take(12) {
            sum[j] += v;
        }
    }

    let n = chroma_vectors.len() as f32;
    sum.iter().map(|&x| x / n).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::analysis::chroma::chroma_vector;
    use crate::dsp::analysis::test_fixtures::generate_chord;
    use ndarray::Array1;

    /// Gera vetores croma a partir de um sinal PCM usando janelas deslizantes.
    fn extract_chroma_sequence(
        pcm: &Array1<f32>,
        sample_rate: u32,
        frame_size: usize,
        hop_size: usize,
    ) -> Vec<Vec<f32>> {
        let mut chromas = Vec::new();
        let mut start = 0;
        while start + frame_size <= pcm.len() {
            let frame = pcm.slice(ndarray::s![start..start + frame_size]);
            chromas.push(chroma_vector(frame, sample_rate));
            start += hop_size;
        }
        chromas
    }

    #[test]
    fn test_detect_key_c_major_sine() {
        let sr = 44100u32;
        // Triade de Do maior: C4=261.63, E4=329.63, G4=392.0
        let chord = generate_chord(&[(261.63, 1.0), (329.63, 1.0), (392.0, 1.0)], 2.0, sr);
        let chromas = extract_chroma_sequence(&chord, sr, 4096, 2048);
        let aggregated = aggregate_chroma_simple(&chromas);
        let result = detect_key(&aggregated);

        // Deve detectar C (root=0) como Maior
        assert_eq!(
            result.root, 0,
            "Esperava tonica C (0), obteve {}",
            result.root
        );
        assert_eq!(
            result.mode,
            KeyMode::Major,
            "Esperava modo Major, obteve {:?}",
            result.mode
        );
        // Triade com perfil major deve ter confianca significativa
        assert!(
            result.confidence > 0.5,
            "Confianca deveria ser > 0.5 para triade major, obteve {}",
            result.confidence
        );
    }

    #[test]
    fn test_detect_key_a_minor_chord() {
        let sr = 44100u32;
        // Acorde de La menor: A3=220, C4=261.63, E4=329.63
        let chord = generate_chord(&[(220.0, 1.0), (261.63, 1.0), (329.63, 1.0)], 2.0, sr);
        let chromas = extract_chroma_sequence(&chord, sr, 4096, 2048);
        let aggregated = aggregate_chroma_simple(&chromas);
        let result = detect_key(&aggregated);

        // Deve detectar A menor (root=9=A, mode=Minor)
        assert_eq!(
            result.root, 9,
            "Esperava tonica A (9), obteve {}",
            result.root
        );
        assert_eq!(
            result.mode,
            KeyMode::Minor,
            "Esperava modo Minor, obteve {:?}",
            result.mode
        );
    }

    #[test]
    fn test_aggregate_chroma_weights_by_rms() {
        // Cria 3 vetores croma simulados
        let chromas = vec![
            vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        ];
        // RMS: frame 0 tem energia, frame 1 e silencioso, frame 2 tem energia
        let rms_values = vec![1.0, 0.0, 1.0];

        let result = aggregate_chroma(&chromas, &rms_values);

        // Frame 1 (silencioso) e ignorado, entao a media e entre frames 0 e 2
        // Ambos tem energia na posicao 0, entao resultado[0] = 1.0
        assert!(
            result[0] > 0.9,
            "Posicao 0 deveria ter energia alta apos ponderacao, obteve {}",
            result[0]
        );
        // Posicao 1 deveria ser 0 pois o unico frame com energia la era silencioso
        assert!(
            result[1] < 0.01,
            "Posicao 1 deveria ser 0 (frame silencioso ignorado), obteve {}",
            result[1]
        );
    }

    #[test]
    fn test_pearson_correlation_identical_is_one() {
        let a = [
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0,
        ];
        let corr = pearson_correlation(&a, &a);
        assert!(
            (corr - 1.0).abs() < 1e-6,
            "Correlacao de vetor identico deveria ser 1.0, obteve {corr}"
        );
    }

    #[test]
    fn test_pearson_correlation_orthogonal_is_zero() {
        // Vetores ortogonais: energia em posicoes diferentes
        let a = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let b = [0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let corr = pearson_correlation(&a, &b);
        // Um vetor e constante-zero apos centralizacao, entao denom=0 => retorna 0
        assert!(
            corr.abs() < 0.1,
            "Correlacao de vetores ortogonais deveria ser ~0, obteve {corr}"
        );
    }
}
