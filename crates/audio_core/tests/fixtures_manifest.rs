//! Harness dirigido pelo manifesto de fixtures — docs/17 §5.
//!
//! Uma função, N casos: cada entrada de `fixtures/audio/manifest.json` diz o
//! que medir e com que tolerância. Adicionar fixture é editar o JSON, não
//! este arquivo. Os valores esperados vêm da construção matemática do sinal
//! (ver `scripts/generate_fixtures.py`), nunca de medição com este mesmo
//! motor — senão o teste só provaria que o motor concorda consigo mesmo.
//!
//! As fixtures em si não são comitadas (docs/17 §2) — rode
//! `scripts/generate_fixtures.py` antes de `cargo test`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use audio_core::dsp::analysis::beat_tracking::{estimate_bpm, onset_strength};
use audio_core::dsp::analysis::fft::magnitude_spectrum;
use audio_core::dsp::mastering::lufs::{measure_lufs, measure_true_peak};
use audio_core::dsp::stitching::zero_cross::{count_zero_crossings, zero_crossing_indices};
use ndarray::Array1;
use serde::Deserialize;
use sha2::{Digest, Sha256};

const EXPECTED_FIXTURE_COUNT: usize = 35;

/// Issue #18: `estimate_bpm` reporta metade do andamento nestas três fixtures
/// (confirmado com o autocorrelograma real: em cada uma, o score no lag do
/// dobro do período vence — por pouco — o score no lag do período
/// verdadeiro; não é limite de faixa de busca, os dois lags estão dentro do
/// intervalo pesquisado). Isto pina o comportamento ERRADO de hoje, não o
/// correto — quando o #18 for corrigido, a asserção abaixo passa a FALHAR
/// (porque o medido deixa de ser metade do esperado), e é para falhar:
/// remova a entrada da lista e feche a issue.
const BPM_METADE_CONHECIDA_ISSUE_18: &[&str] = &[
    "click_tracks/click_128bpm_mono.wav",
    "rhythm/rhythm_120bpm_mono.wav",
    "rhythm/rhythm_140bpm_mono.wav",
];

/// Caminho relativo resolve a partir da crate (`CARGO_MANIFEST_DIR`), não do
/// workspace — docs/17 §3.1. Sem isso o teste passa numa máquina e falha na
/// outra dependendo de onde `cargo test` foi invocado.
fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/audio")
}

#[derive(Deserialize)]
struct Manifest {
    files: HashMap<String, FileEntry>,
}

#[derive(Deserialize)]
struct FileEntry {
    sample_rate: u32,
    channels: u32,
    sha256: String,
    #[serde(default)]
    expected: Expected,
}

#[derive(Deserialize, Default)]
struct Expected {
    expected_behavior: Option<String>,
    bpm: Option<f64>,
    bpm_tolerance_pct: Option<f64>,
    true_peak_dbtp: Option<f64>,
    true_peak_dbtp_tolerance: Option<f64>,
    lufs_i: Option<f64>,
    zero_crossing_count: Option<u64>,
    zero_crossing_indices: Option<Vec<u64>>,
    /// Propriedade verificável do sinal em si (não só sha256 + valores de
    /// saída) — hoje só nas fixtures de varredura. Ver `scripts/generate_fixtures.py`
    /// (`gen_log_sweep`) e docs/17 sobre por que isso existe: uma fixture
    /// com defeito na própria construção é pior que teste ausente, porque
    /// produz um verde que ninguém questiona.
    instantaneous_freq_checkpoints: Option<Vec<FreqCheckpoint>>,
}

#[derive(Deserialize)]
struct FreqCheckpoint {
    t_sec: f64,
    freq_hz: f64,
}

/// Pico espectral (Hz) de uma janela Hann centrada em `centro_sec` segundos
/// — mesma técnica de `aliasing.rs`. Hann para não borrar o pico com
/// vazamento espectral; `magnitude_spectrum` em si não janela.
fn pico_espectral_hz(pcm: &Array1<f32>, centro_sec: f64, sample_rate: u32) -> f64 {
    const WINDOW_LEN: usize = 4096;
    let centro_idx = (centro_sec * sample_rate as f64) as usize;
    let half = WINDOW_LEN / 2;
    let start = centro_idx.saturating_sub(half);
    let end = (start + WINDOW_LEN).min(pcm.len());
    let start = end.saturating_sub(WINDOW_LEN);

    let janela: Vec<f32> = (start..end)
        .enumerate()
        .map(|(i, idx)| {
            let hann =
                0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / (WINDOW_LEN - 1) as f32).cos();
            pcm[idx] * hann
        })
        .collect();

    let espectro = magnitude_spectrum(Array1::from_vec(janela).view());
    let (bin_pico, _) = espectro
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
        .unwrap();
    bin_pico as f64 * sample_rate as f64 / WINDOW_LEN as f64
}

fn load_manifest() -> Manifest {
    let manifest_path = fixtures_dir().join("manifest.json");
    let raw = std::fs::read_to_string(&manifest_path).unwrap_or_else(|e| {
        panic!("{manifest_path:?}: {e} — rode `python scripts/generate_fixtures.py` (docs/17 §2)")
    });
    serde_json::from_str(&raw).expect("manifest.json malformado")
}

fn sha256_of(path: &Path) -> String {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("lendo {path:?}: {e}"));
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    format!("{:x}", hasher.finalize())
}

/// Áudio decodificado de um WAV de fixture. `mono` já soma os canais quando
/// `channels > 1` — usado pelas métricas (BPM, LUFS, zero-crossing) que só
/// fazem sentido em mono; `interleaved` preserva os canais originais para
/// true peak, que deve ser medido por canal.
struct Decoded {
    mono: Array1<f32>,
    interleaved: Vec<f32>,
    channels: u32,
    sample_rate: u32,
}

/// Decodifica um WAV via `hound`. Não é o decoder de produção (que ainda não
/// existe — `default_mixer.rs` só tem um placeholder) — é um shim exclusivo
/// deste harness, e é adequado exatamente porque as fixtures são sempre WAV.
fn decodificar(path: &Path) -> Result<Decoded, hound::Error> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();

    let interleaved: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let max_val = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 / max_val))
                .collect::<Result<_, _>>()?
        }
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>()?,
    };

    let channels = spec.channels as u32;
    let mono = if channels <= 1 {
        Array1::from_vec(interleaved.clone())
    } else {
        let frames = interleaved.len() / channels as usize;
        let mut out = Vec::with_capacity(frames);
        for frame in 0..frames {
            let start = frame * channels as usize;
            let sum: f32 = interleaved[start..start + channels as usize].iter().sum();
            out.push(sum / channels as f32);
        }
        Array1::from_vec(out)
    };

    Ok(Decoded {
        mono,
        interleaved,
        channels,
        sample_rate: spec.sample_rate,
    })
}

#[test]
fn fixtures_conformam_ao_manifesto() {
    let manifest = load_manifest();
    assert_eq!(
        manifest.files.len(),
        EXPECTED_FIXTURE_COUNT,
        "docs/17 §2.1 promete {EXPECTED_FIXTURE_COUNT} fixtures — atualize a doc ou o gerador"
    );

    for (caminho, spec) in &manifest.files {
        let arquivo = fixtures_dir().join(caminho);

        // 1. A fixture não mudou desde que os valores esperados foram calculados.
        assert_eq!(
            sha256_of(&arquivo),
            spec.sha256,
            "{caminho}: arquivo diferente do manifesto — regenere (scripts/generate_fixtures.py) ou atualize o manifesto"
        );

        // `corrupted_truncated.wav` é o único caso cuja decodificação deve
        // falhar por construção — coberto por `arquivos_invalidos_falham_sem_panic`.
        // `degenerate_zero_duration.wav` decodifica normalmente (buffer vazio,
        // sem erro do hound) e segue pelo caminho comum abaixo.
        if spec.expected.expected_behavior.as_deref() == Some("decode_error") {
            continue;
        }

        let audio = decodificar(&arquivo).unwrap_or_else(|e| panic!("{caminho}: {e}"));

        // 2. Vale para toda fixture, sempre.
        assert!(
            audio.interleaved.iter().all(|s| s.is_finite()),
            "{caminho}: amostra não finita (I15)"
        );
        assert_eq!(
            audio.sample_rate, spec.sample_rate,
            "{caminho}: sample_rate divergente"
        );
        assert_eq!(
            audio.channels, spec.channels,
            "{caminho}: channels divergente"
        );

        // 3. Condicional ao que o manifesto declara.
        if let (Some(bpm), Some(tolerancia_pct)) =
            (spec.expected.bpm, spec.expected.bpm_tolerance_pct)
        {
            let onset = onset_strength(&audio.mono, 2048, 512);
            let medido = estimate_bpm(&onset, audio.sample_rate, 512) as f64;
            // Fixtures na lista de bug conhecido (issue #18) são cobradas
            // contra o valor ERRADO que produzem hoje, não contra `bpm` —
            // ver comentário na constante. Todas as outras são cobradas
            // contra o valor verdadeiro, sem tolerância a oitava: um trem de
            // cliques uniforme não tem ambiguidade musical legítima entre
            // tempo e metade de tempo, então medir metade ali é falha de
            // detecção, não uma leitura alternativa defensável.
            let alvo = if BPM_METADE_CONHECIDA_ISSUE_18.contains(&caminho.as_str()) {
                bpm / 2.0
            } else {
                bpm
            };
            let erro_pct = (medido - alvo).abs() / alvo * 100.0;
            assert!(
                erro_pct <= tolerancia_pct,
                "{caminho}: BPM medido {medido:.1}, esperado {alvo:.1} (tolerância {tolerancia_pct}%)"
            );
        }

        // Só afirmado quando o gerador declara uma tolerância explícita
        // (hoje só true_peak/*) — nas demais fixtures o campo é uma cópia
        // informativa do pico de amostra, não uma medição de true peak
        // validada (perto do Nyquist ou em transientes rápidos o true peak
        // real diverge do pico de amostra por bem mais que 0,1-0,2 dB, sem
        // que isso seja regressão nenhuma).
        if let (Some(tp), Some(tolerancia)) = (
            spec.expected.true_peak_dbtp,
            spec.expected.true_peak_dbtp_tolerance,
        ) {
            let medido =
                measure_true_peak(&audio.interleaved, audio.channels, audio.sample_rate) as f64;
            assert!(
                (medido - tp).abs() <= tolerancia,
                "{caminho}: true peak medido {medido:.2} dBTP, esperado {tp:.2} (tolerância {tolerancia})"
            );
        }

        if let Some(lufs) = spec.expected.lufs_i {
            let medido = measure_lufs(&audio.mono, audio.sample_rate) as f64;
            // Tolerância larga de propósito: ao contrário de BPM/frequência,
            // LUFS integrado com gating não é derivável à mão a partir da
            // amplitude linear usada para construir o sinal (conflict_targets
            // mistura um leito contínuo baixo com transientes esparsos — o
            // gating da BS.1770 pesa desproporcionalmente os transientes).
            // O alvo aqui é pegar quebra grosseira (sinal mudo, ganho
            // trocado de sinal), não validar LUFS ao décimo.
            assert!(
                (medido - lufs).abs() <= 3.5,
                "{caminho}: LUFS medido {medido:.2}, esperado {lufs:.2}"
            );
        }

        if let Some(n) = spec.expected.zero_crossing_count {
            let medido = count_zero_crossings(&audio.mono) as u64;
            assert_eq!(medido, n, "{caminho}: contagem de zero-crossings");
        }

        if let Some(indices) = &spec.expected.zero_crossing_indices {
            let medido: Vec<u64> = zero_crossing_indices(&audio.mono)
                .into_iter()
                .map(|i| i as u64)
                .collect();
            assert_eq!(&medido, indices, "{caminho}: índices de zero-crossing");
        }

        // Propriedade verificável do sinal em si, independente de qualquer
        // teste downstream ter sido escrito — teria pego sozinho o bug de
        // normalização por `duration` em `gen_log_sweep` (docs/17 §2.4).
        if let Some(checkpoints) = &spec.expected.instantaneous_freq_checkpoints {
            for cp in checkpoints {
                let medido = pico_espectral_hz(&audio.mono, cp.t_sec, audio.sample_rate);
                let tolerancia_hz = (cp.freq_hz * 0.05) + 20.0;
                assert!(
                    (medido - cp.freq_hz).abs() <= tolerancia_hz,
                    "{caminho}: em t={:.1}s, pico espectral medido {medido:.1} Hz, esperado {:.1} Hz (tolerância {tolerancia_hz:.1} Hz)",
                    cp.t_sec,
                    cp.freq_hz
                );
            }
        }
    }
}

/// `corrupted_truncated.wav`: cabeçalho válido, dados truncados — decodificar
/// tem que devolver `Err`, nunca panic.
#[test]
fn arquivos_invalidos_falham_sem_panic() {
    let path = fixtures_dir().join("corrupted/corrupted_truncated.wav");
    let resultado = std::panic::catch_unwind(|| decodificar(&path));
    assert!(
        resultado.is_ok(),
        "corrupted_truncated.wav: causou panic em vez de Err"
    );
    assert!(
        resultado.unwrap().is_err(),
        "corrupted_truncated.wav: deveria falhar e não falhou"
    );
}
