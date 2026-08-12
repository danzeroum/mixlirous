//! P├│s-processamento e verifica├º├úo de invariantes da sa├¡da do pipeline.
//!
//! Verifica invariantes cr├¡ticas que devem valer para qualquer sa├¡da do motor
//! DSP (`docs/10-TESTES-QUALIDADE.md`). Separadas do pipeline para que possam
//! ser chamadas tanto pelo pr├│prio pipeline quanto por testes de integra├º├úo
//! diretos, sem passar por toda a cadeia.

/// Relat├│rio do p├│s-processamento: o que foi encontrado e corrigido.
#[derive(Debug, Clone, Default)]
pub struct PostProcessReport {
    /// Amostras que excediam ┬▒1.0 e foram limitadas (I1).
    pub samples_limited: usize,
    /// Saltos entre amostras adjacentes suavizados (I4).
    pub click_corrections: usize,
}

/// Aplica p├│s-processamento seguro ao buffer de sa├¡da.
///
/// 1. Limita amostras a ┬▒1.0 (I1) ÔÇö necess├írio para formatos inteiros.
/// 2. Suaviza saltos entre amostras adjacentes > limiar (I4).
///
/// O limiar de clique (0.5 por padr├úo) corresponde a ~6 dB de salto entre
/// amostras consecutivas ÔÇö aud├¡vel como estalo em material de qualidade.
///
/// # Garantia de I4
/// O passo 2 aproxima cada amostra do vizinho esquerdo at├® a diferen├ºa cair
/// no limiar, numa ├║nica varredura da esquerda para a direita. Depois dela,
/// toda diferen├ºa entre amostras consecutivas ├® `<= click_threshold` ÔÇö
/// inclusive em impulsos de dois ou mais samples de largura (caso em que uma
/// m├®dia de 3 samples no ponto central deixaria a borda esquerda do impulso
/// intacta e o salto de quase 2x o limiar permaneceria).
pub fn post_process(pcm: &mut [f32], click_threshold: f32) -> PostProcessReport {
    let mut report = PostProcessReport::default();

    // I1: limita clipping
    for sample in pcm.iter_mut() {
        if *sample > 1.0 {
            *sample = 1.0;
            report.samples_limited += 1;
        } else if *sample < -1.0 {
            *sample = -1.0;
            report.samples_limited += 1;
        }
    }

    // I4: suaviza estalos (diferen├ºa adjacente > limiar).
    // Uma ├║nica varredura garante `|pcm[i] - pcm[i-1]| <= limiar` para todo i:
    // ao tocar em `pcm[i]`, a dupla (i-1, i) j├í foi resolvida e a dupla
    // seguinte (i, i+1) ser├í resolvida na pr├│xima itera├º├úo da varredura.
    for i in 1..pcm.len() {
        let diff = pcm[i] - pcm[i - 1];
        if diff.abs() > click_threshold {
            pcm[i] = pcm[i - 1] + click_threshold.copysign(diff);
            report.click_corrections += 1;
        }
    }

    report
}

/// Verifica os invariantes sem modificar o buffer.
///
/// ├Ütil para testes e para emitir warnings via SSE sem alterar a sa├¡da.
pub fn check_invariants(pcm: &[f32], click_threshold: f32) -> PostProcessReport {
    let samples_limited = pcm.iter().filter(|&&s| s.abs() > 1.0).count();

    let click_corrections = pcm
        .windows(2)
        .filter(|w| (w[1] - w[0]).abs() > click_threshold)
        .count();

    PostProcessReport {
        samples_limited,
        click_corrections,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_signal_passes_unchanged() {
        let mut pcm = vec![0.1f32, 0.2, 0.3, 0.2, 0.1];
        let before = pcm.clone();
        let report = post_process(&mut pcm, 0.5);
        assert_eq!(report.samples_limited, 0);
        assert_eq!(report.click_corrections, 0);
        assert_eq!(pcm, before);
    }

    #[test]
    fn clipping_is_limited() {
        let mut pcm = vec![0.5f32, 1.5, -1.8, 0.3];
        let report = post_process(&mut pcm, 0.5);
        assert_eq!(report.samples_limited, 2);
        assert!(
            pcm[1].abs() <= 1.0,
            "pcm[1] deveria ter sido limitado: {}",
            pcm[1]
        );
        assert!(
            pcm[2].abs() <= 1.0,
            "pcm[2] deveria ter sido limitado: {}",
            pcm[2]
        );
        // O clamp de +1.0 para -1.0 entre índices vizinhos também é um estalo
        // (I4): além de limitar, o pós-processamento tem que eliminar o salto.
        for w in pcm.windows(2) {
            assert!(
                (w[1] - w[0]).abs() <= 0.5 + 1e-6,
                "salto remanescente: {}",
                (w[1] - w[0]).abs()
            );
        }
    }

    #[test]
    fn click_is_smoothed() {
        // Salto de 0.9 entre amostras adjacentes
        let mut pcm = vec![0.0f32, 0.0, 0.9, 0.0, 0.0];
        let report = post_process(&mut pcm, 0.5);
        assert_eq!(report.click_corrections, 1);
        // A m├®dia de 3 deve reduzir o pico
        assert!(
            pcm[2].abs() < 0.9,
            "estalo deveria ser suavizado: {}",
            pcm[2]
        );
    }

    #[test]
    fn empty_buffer_is_ok() {
        let mut pcm: Vec<f32> = vec![];
        let report = post_process(&mut pcm, 0.5);
        assert_eq!(report.samples_limited, 0);
        assert_eq!(report.click_corrections, 0);
    }

    #[test]
    fn single_sample_buffer_is_ok() {
        let mut pcm = vec![2.0f32];
        let report = post_process(&mut pcm, 0.5);
        assert_eq!(report.samples_limited, 1);
        assert_eq!(pcm[0], 1.0);
    }

    #[test]
    fn check_invariants_does_not_modify() {
        let pcm = vec![1.5f32, 0.0, -1.2];
        let original = pcm.clone();
        let report = check_invariants(&pcm, 0.5);
        assert_eq!(report.samples_limited, 2);
        assert_eq!(pcm, original);
    }

    // I1: invariante de clipping ÔÇö ap├│s post_process, nenhuma amostra excede ┬▒1.0
    #[test]
    fn invariant_i1_no_clipping_after_post_process() {
        let mut pcm = vec![0.0f32; 1000];
        pcm[10] = 3.0;
        pcm[500] = -2.5;
        pcm[999] = 1.001;
        post_process(&mut pcm, 0.5);
        for &s in &pcm {
            assert!(s.abs() <= 1.0 + 1e-7, "I1 violado: amostra = {s}");
        }
    }

    // I4: invariante de continuidade ÔÇö ap├│s post_process, nenhum salto > limiar
    #[test]
    fn invariant_i4_no_clicks_after_post_process() {
        let mut pcm = vec![0.0f32; 1000];
        pcm[100] = 1.5; // cria salto
        pcm[101] = -1.5;
        post_process(&mut pcm, 0.5);
        for w in pcm.windows(2) {
            assert!(
                (w[1] - w[0]).abs() <= 0.5 + 1e-6,
                "I4 violado: salto de {} entre amostras adjacentes",
                (w[1] - w[0]).abs()
            );
        }
    }
}
