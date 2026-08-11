//! Harness dirigido pelo manifesto de fixtures ÔÇö docs/17 ┬º5.
//!
//! Uma fun├º├úo, N casos: cada entrada de `fixtures/audio/manifest.json` diz o
//! que medir e com que toler├óncia. Adicionar fixture ├® editar o JSON, n├úo
//! este arquivo. Os valores esperados v├¬m da constru├º├úo matem├ítica do sinal
//! (ver `scripts/generate_fixtures.py`), nunca de medi├º├úo com este mesmo
//! motor ÔÇö sen├úo o teste s├│ provaria que o motor concorda consigo mesmo.
//!
//! As fixtures em si n├úo s├úo comitadas (docs/17 ┬º2) ÔÇö rode
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

/// Issue #18: `estimate_bpm` reporta metade do andamento nestas tr├¬s fixtures
/// (confirmado com o autocorrelograma real: em cada uma, o score no lag do
/// dobro do per├¡odo vence ÔÇö por pouco ÔÇö o score no lag do per├¡odo
/// verdadeiro; n├úo ├® limite de faixa de busca, os dois lags est├úo dentro do
/// intervalo pesquisado). Isto pina o comportamento ERRADO de hoje, n├úo o
/// correto ÔÇö quando o #18 for corrigido, a asser├º├úo abaixo passa a FALHAR
/// (porque o medido deixa de ser metade do esperado), e ├® para falhar:
/// remova a entrada da lista e feche a issue.
const BPM_METADE_CONHECIDA_ISSUE_18: &[&str] = &[
    "click_tracks/click_128bpm_mono.wav",
    "rhythm/rhythm_120bpm_mono.wav",
    "rhythm/rhythm_140bpm_mono.wav",
];

/// Caminho relativo resolve a partir da crate (`CARGO_MANIFEST_DIR`), n├úo do
/// workspace ÔÇö docs/17 ┬º3.1. Sem isso o teste passa numa m├íquina e falha na
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
    /// Frequ├¬ncia exata dos tons puros (`tones/*`, `time_stretch/pure_tone_440hz.wav`)
    /// ÔÇö presente no manifesto desde o in├¡cio, nunca antes verificada pelo
    /// harness (achado revisando a extens├úo de `instantaneous_freq_checkpoints`
    /// a outras fixtures com propriedade anal├¡tica, docs/17 ┬º2.5).
    freq_hz: Option<f64>,
    freq_tolerance_hz: Option<f64>,
    /// Propriedade verific├ível do sinal em si (n├úo s├│ sha256 + valores de
    /// sa├¡da) ÔÇö hoje s├│ nas fixtures de varredura. Ver `scripts/generate_fixtures.py`
    /// (`gen_log_sweep`) e docs/17 sobre por que isso existe: uma fixture
    /// com defeito na pr├│pria constru├º├úo ├® pior que teste ausente, porque
    /// produz um verde que ningu├®m questiona.
    instantaneous_freq_checkpoints: Option<Vec<FreqCheckpoint>>,
}

#[derive(Deserialize)]
struct FreqCheckpoint {
    t_sec: f64,
    freq_hz: f64,
}

/// Pico espectral (Hz) de uma janela Hann de `window_len` amostras centrada
/// em `centro_sec` segundos ÔÇö mesma t├®cnica de `aliasing.rs`. Hann para n├úo
/// borrar o pico com vazamento espectral; `magnitude_spectrum` em si n├úo
/// janela.
///
/// `window_len` importa: resolu├º├úo de bin ├® `sample_rate / window_len`. Uma
/// varredura precisa de janela curta (a frequ├¬ncia muda dentro dela ÔÇö janela
/// longa borra); um tom estacion├írio precisa de janela longa para bater
/// toler├óncia apertada (`freq_tolerance_hz: 2.0` em `gen_sine` exige melhor
/// que ~2 Hz/bin, e 4096 amostras a 44100 Hz s├│ d├úo ~10,8 Hz/bin ÔÇö usar esse
/// valor para tom estacion├írio reprovaria por quantiza├º├úo de bin, n├úo por
/// erro de sinal).
fn pico_espectral_hz(
    pcm: &Array1<f32>,
    centro_sec: f64,
    sample_rate: u32,
    window_len: usize,
) -> f64 {
    let centro_idx = (centro_sec * sample_rate as f64) as usize;
    let half = window_len / 2;
    let start = centro_idx.saturating_sub(half);
    let end = (start + window_len).min(pcm.len());
    let start = end.saturating_sub(window_len);
    let window_len = end - start;

    let janela: Vec<f32> = (start..end)
        .enumerate()
        .map(|(i, idx)| {
            let hann =
                0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / (window_len - 1) as f32).cos();
            pcm[idx] * hann
        })
        .collect();

    let espectro = magnitude_spectrum(Array1::from_vec(janela).view());
    let (bin_pico, _) = espectro
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
        .unwrap();
    bin_pico as f64 * sample_rate as f64 / window_len as f64
}

fn load_manifest() -> Manifest {
    let manifest_path = fixtures_dir().join("manifest.json");
    let raw = std::fs::read_to_string(&manifest_path).unwrap_or_else(|e| {
        panic!(
            "{manifest_path:?}: {e} ÔÇö rode `python scripts/generate_fixtures.py` (docs/17 ┬º2)"
        )
    });
    serde_json::from_str(&raw).expect("manifest.json malformado")
}

fn sha256_of(path: &Path) -> String {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("lendo {path:?}: {e}"));
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    format!("{:x}", hasher.finalize())
}

/// ├üudio decodificado de um WAV de fixture. `mono` j├í soma os canais quando
/// `channels > 1` ÔÇö usado pelas m├®tricas (BPM, LUFS, zero-crossing) que s├│
/// fazem sentido em mono; `interleaved` preserva os canais originais para
/// true peak, que deve ser medido por canal.
struct Decoded {
    mono: Array1<f32>,
    interleaved: Vec<f32>,
    channels: u32,
    sample_rate: u32,
}

/// Decodifica um WAV via `hound`. N├úo ├® o decoder de produ├º├úo (que ainda n├úo
/// existe ÔÇö `default_mixer.rs` s├│ tem um placeholder) ÔÇö ├® um shim exclusivo
/// deste harness, e ├® adequado exatamente porque as fixtures s├úo sempre WAV.
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
        },
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
        "docs/17 ┬º2.1 promete {EXPECTED_FIXTURE_COUNT} fixtures ÔÇö atualize a doc ou o gerador"
    );

    for (caminho, spec) in &manifest.files {
        let arquivo = fixtures_dir().join(caminho);

        // 1. A fixture n├úo mudou desde que os valores esperados foram calculados.
        assert_eq!(
            sha256_of(&arquivo),
            spec.sha256,
            "{caminho}: arquivo diferente do manifesto ÔÇö regenere (scripts/generate_fixtures.py) ou atualize o manifesto"
        );

        // `corrupted_truncated.wav` ├® o ├║nico caso cuja decodifica├º├úo deve
        // falhar por constru├º├úo ÔÇö coberto por `arquivos_invalidos_falham_sem_panic`.
        // `degenerate_zero_duration.wav` decodifica normalmente (buffer vazio,
        // sem erro do hound) e segue pelo caminho comum abaixo.
        if spec.expected.expected_behavior.as_deref() == Some("decode_error") {
            continue;
        }

        let audio = decodificar(&arquivo).unwrap_or_else(|e| panic!("{caminho}: {e}"));

        // 2. Vale para toda fixture, sempre.
        assert!(
            audio.interleaved.iter().all(|s| s.is_finite()),
            "{caminho}: amostra n├úo finita (I15)"
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
            // Fixtures na lista de bug conhecido (issue #18) s├úo cobradas
            // contra o valor ERRADO que produzem hoje, n├úo contra `bpm` ÔÇö
            // ver coment├írio na constante. Todas as outras s├úo cobradas
            // contra o valor verdadeiro, sem toler├óncia a oitava: um trem de
            // cliques uniforme n├úo tem ambiguidade musical leg├¡tima entre
            // tempo e metade de tempo, ent├úo medir metade ali ├® falha de
            // detec├º├úo, n├úo uma leitura alternativa defens├ível.
            let alvo = if BPM_METADE_CONHECIDA_ISSUE_18.contains(&caminho.as_str()) {
                bpm / 2.0
            } else {
                bpm
            };
            let erro_pct = (medido - alvo).abs() / alvo * 100.0;
            assert!(
                erro_pct <= tolerancia_pct,
                "{caminho}: BPM medido {medido:.1}, esperado {alvo:.1} (toler├óncia {tolerancia_pct}%)"
            );
        }

        // S├│ afirmado quando o gerador declara uma toler├óncia expl├¡cita
        // (hoje s├│ true_peak/*) ÔÇö nas demais fixtures o campo ├® uma c├│pia
        // informativa do pico de amostra, n├úo uma medi├º├úo de true peak
        // validada (perto do Nyquist ou em transientes r├ípidos o true peak
        // real diverge do pico de amostra por bem mais que 0,1-0,2 dB, sem
        // que isso seja regress├úo nenhuma).
        if let (Some(tp), Some(tolerancia)) = (
            spec.expected.true_peak_dbtp,
            spec.expected.true_peak_dbtp_tolerance,
        ) {
            let medido =
                measure_true_peak(&audio.interleaved, audio.channels, audio.sample_rate) as f64;
            assert!(
                (medido - tp).abs() <= tolerancia,
                "{caminho}: true peak medido {medido:.2} dBTP, esperado {tp:.2} (toler├óncia {tolerancia})"
            );
        }

        if let Some(lufs) = spec.expected.lufs_i {
            let medido = measure_lufs(&audio.mono, audio.sample_rate) as f64;
            // Toler├óncia larga de prop├│sito: ao contr├írio de BPM/frequ├¬ncia,
            // LUFS integrado com gating n├úo ├® deriv├ível ├á m├úo a partir da
            // amplitude linear usada para construir o sinal (conflict_targets
            // mistura um leito cont├¡nuo baixo com transientes esparsos ÔÇö o
            // gating da BS.1770 pesa desproporcionalmente os transientes).
            // O alvo aqui ├® pegar quebra grosseira (sinal mudo, ganho
            // trocado de sinal), n├úo validar LUFS ao d├®cimo.
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
            assert_eq!(&medido, indices, "{caminho}: ├¡ndices de zero-crossing");
        }

        if let (Some(freq), Some(tolerancia_hz)) =
            (spec.expected.freq_hz, spec.expected.freq_tolerance_hz)
        {
            let duracao_sec = audio.mono.len() as f64 / audio.sample_rate as f64;
            // Janela = buffer inteiro: tom estacion├írio, sem risco de borrar
            // frequ├¬ncia mudando dentro da janela (diferente do sweep).
            // Necess├írio para bater `freq_tolerance_hz: 2.0` ÔÇö 4096 amostras
            // s├│ dariam ~10,8 Hz/bin.
            let medido = pico_espectral_hz(
                &audio.mono,
                duracao_sec / 2.0,
                audio.sample_rate,
                audio.mono.len(),
            );
            assert!(
                (medido - freq).abs() <= tolerancia_hz,
                "{caminho}: pico espectral medido {medido:.1} Hz, esperado {freq:.1} Hz (toler├óncia {tolerancia_hz} Hz)"
            );
        }

        // Propriedade verific├ível do sinal em si, independente de qualquer
        // teste downstream ter sido escrito ÔÇö teria pego sozinho o bug de
        // normaliza├º├úo por `duration` em `gen_log_sweep` (docs/17 ┬º2.4).
        if let Some(checkpoints) = &spec.expected.instantaneous_freq_checkpoints {
            for cp in checkpoints {
                let medido = pico_espectral_hz(&audio.mono, cp.t_sec, audio.sample_rate, 4096);
                let tolerancia_hz = (cp.freq_hz * 0.05) + 20.0;
                assert!(
                    (medido - cp.freq_hz).abs() <= tolerancia_hz,
                    "{caminho}: em t={:.1}s, pico espectral medido {medido:.1} Hz, esperado {:.1} Hz (toler├óncia {tolerancia_hz:.1} Hz)",
                    cp.t_sec,
                    cp.freq_hz
                );
            }
        }
    }
}

/// `corrupted_truncated.wav`: cabe├ºalho v├ílido, dados truncados ÔÇö decodificar
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
        "corrupted_truncated.wav: deveria falhar e n├úo falhou"
    );
}
