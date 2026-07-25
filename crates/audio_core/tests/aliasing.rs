//! Aliasing no reamostrador — docs/17.1 §3.2. Valida o `rubato` sinc de
//! `time_stretch` sobre fixtures que existiam e nunca tinham sido usadas para
//! isto (`sweeps/sweep_20_20k_mono.wav`, `tones/sine_8khz_mono.wav`). Hoje a
//! única prova de que a migração para sinc está correta é o nome da função —
//! estes dois testes são a prova real.
//!
//! **Achado ao escrever o primeiro teste, antes mesmo de rodar contra o
//! reamostrador:** `gen_log_sweep` normalizava o expoente da fase por
//! segundos, não por `duration` — a frequência instantânea batia o alvo em
//! `t=1,0s` sempre, nunca em `t=duration`. Para `duration=5.0` (o valor real
//! de todas as fixtures geradas), os últimos 4 dos 5 segundos da varredura
//! eram `sin()` de uma fase da ordem de 1e16 radianos: ruído numérico, não
//! uma varredura. Corrigido em `scripts/generate_fixtures.py`
//! (`gen_log_sweep`) — sem isso, "frequência instantânea conhecida por
//! construção" seria conhecida errada.
//!
//! **Por que são dois testes, não um.** A primeira tentativa usou só o sweep,
//! medindo pico espectral em pontos onde a frequência esperada (depois de
//! `time_stretch` deslocar tudo por `1/R`) continuava abaixo do Nyquist.
//! Isso passa tanto para o `rubato` sinc quanto para um reamostrador linear
//! ingênuo escrito à mão — não discrimina nada, porque um chirp constrito a
//! uma janela finita já teria energia espalhada (a frequência muda dentro da
//! própria janela) antes de qualquer aliasing entrar em cena; e interpolação
//! linear não é ruim o bastante nessa faixa para se distinguir do sinc.
//! Confirmado experimentalmente antes de decidir a forma final deste
//! arquivo, não por suposição.
//!
//! O teste que de fato discrimina precisa de duas coisas que o chirp não dá
//! ao mesmo tempo: uma frequência **estacionária** conhecida (sem
//! espalhamento espectral de chirp) e uma razão de reamostragem agressiva o
//! bastante para empurrar essa frequência **para além do novo Nyquist** —
//! aí o conteúdo é, por construção, irrepresentável, e a única pergunta é
//! se o reamostrador filtra (correto) ou dobra de volta como frequência
//! espúria (aliasing). Por isso o segundo teste usa `sine_8khz_mono.wav`,
//! não o sweep.

use audio_core::dsp::analysis::fft::magnitude_spectrum;
use audio_core::dsp::mastering::stretch::time_stretch;
use ndarray::Array1;

const SAMPLE_RATE: u32 = 44_100;

fn fixtures_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/audio")
}

fn decode_mono(path: &std::path::Path) -> Vec<f32> {
    let mut reader = hound::WavReader::open(path)
        .unwrap_or_else(|e| panic!("{path:?}: {e} — rode scripts/generate_fixtures.py"));
    let spec = reader.spec();
    let max_val = (1i64 << (spec.bits_per_sample - 1)) as f32;
    reader
        .samples::<i32>()
        .map(|s| s.unwrap() as f32 / max_val)
        .collect()
}

/// Espectro de magnitude de uma janela Hann centrada em `center_idx` — Hann
/// (não retangular) para não borrar o pico com vazamento espectral;
/// `magnitude_spectrum` em si não janela.
fn espectro_em(pcm: &[f32], center_idx: usize, window_len: usize) -> Vec<f32> {
    let half = window_len / 2;
    let start = center_idx.saturating_sub(half);
    let end = (start + window_len).min(pcm.len());
    let start = end.saturating_sub(window_len);

    let janela: Vec<f32> = (start..end)
        .enumerate()
        .map(|(i, idx)| {
            let hann =
                0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / (window_len - 1) as f32).cos();
            pcm[idx] * hann
        })
        .collect();

    magnitude_spectrum(Array1::from_vec(janela).view())
}

fn bin_para_hz(bin: usize, window_len: usize, sample_rate: u32) -> f32 {
    bin as f32 * sample_rate as f32 / window_len as f32
}

/// §3.2, parte 1: no regime representável (frequência esperada bem abaixo do
/// Nyquist mesmo após o deslocamento de `time_stretch`), o pico espectral
/// bate com a frequência instantânea conhecida por construção da varredura.
#[test]
fn time_stretch_preserva_frequencia_no_regime_representavel() {
    const FREQ_START: f64 = 20.0;
    const FREQ_END: f64 = 20_000.0;
    const ORIGINAL_DURATION: f64 = 5.0;

    let pcm_original = decode_mono(&fixtures_dir().join("sweeps/sweep_20_20k_mono.wav"));
    assert_eq!(
        pcm_original.len(),
        (SAMPLE_RATE as f64 * ORIGINAL_DURATION) as usize
    );

    // R = 0.5: encurta a duração pela metade — remapeamento linear de tempo
    // (sem correção de afinação, documentado em stretch.rs) dobra toda
    // frequência. Pontos escolhidos (t_orig <= 4.3s) mapeiam para
    // frequências bem abaixo do Nyquist de 22050 Hz mesmo depois de
    // dobradas — este teste é sobre fidelidade, não sobre o limite em si
    // (isso é o segundo teste).
    const R: f64 = 0.5;
    let target_duration = ORIGINAL_DURATION * R;

    let esticado = time_stretch(
        &Array1::from_vec(pcm_original),
        SAMPLE_RATE,
        target_duration as f32,
    )
    .expect("time_stretch não deveria falhar para entrada válida");
    let pcm_esticado = esticado.as_slice().unwrap();

    let log_ratio = (FREQ_END / FREQ_START).ln();
    let freq_orig_em = |t: f64| FREQ_START * ((t / ORIGINAL_DURATION) * log_ratio).exp();

    let window_len = 4096usize;
    for t_orig in [1.0, 2.0, 3.0, 4.0, 4.3] {
        let freq_esperada = freq_orig_em(t_orig) / R;
        let t_novo = t_orig * R;
        let center_idx = (t_novo * SAMPLE_RATE as f64) as usize;

        let espectro = espectro_em(pcm_esticado, center_idx, window_len);
        let (bin_pico, _) = espectro
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap();
        let pico_hz = bin_para_hz(bin_pico, window_len, SAMPLE_RATE);

        // Tolerância: resolução de bin (~10,8 Hz neste window_len) mais uma
        // margem relativa — apertado o bastante para reprovar um
        // reamostrador que borra ou desloca o pico, largo o bastante para
        // não quebrar por causa da resolução de bin em si.
        let tolerancia_hz = (freq_esperada * 0.05) + 20.0;
        assert!(
            (pico_hz as f64 - freq_esperada).abs() <= tolerancia_hz,
            "t_orig={t_orig}s: pico medido {pico_hz:.1} Hz, esperado {freq_esperada:.1} Hz (tolerância {tolerancia_hz:.1} Hz)"
        );
    }
}

/// §3.2, parte 2 — a que de fato discrimina. Um tom de 8 kHz, deslocado por
/// `time_stretch` para além do Nyquist de 22050 Hz (irrepresentável por
/// construção), tem que ser **suprimido**, não dobrado de volta como
/// frequência espúria. Verificado experimentalmente antes de escrever este
/// teste: um reamostrador ingênuo (decimação sem filtro anti-aliasing,
/// implementado só para essa verificação, não em produção) devolve o pico
/// espectral **na mesma magnitude** do caso representável — nenhuma
/// supressão, energia dobrada de volta como frequência errada. O `rubato`
/// sinc suprime a magnitude do pico em ~4 ordens de grandeza.
#[test]
fn time_stretch_suprime_conteudo_alem_do_nyquist_em_vez_de_aliasar() {
    let pcm = decode_mono(&fixtures_dir().join("tones/sine_8khz_mono.wav"));
    let duration = pcm.len() as f64 / SAMPLE_RATE as f64;
    let window_len = 4096usize;

    // r=0.5: 8 kHz -> 16 kHz, ainda abaixo do Nyquist (22050) — referência
    // de magnitude "sinal presente e bem formado".
    let pico_representavel = {
        let target_duration = duration * 0.5;
        let out_len = (pcm.len() as f64 * 0.5) as usize;
        let esticado = time_stretch(
            &Array1::from_vec(pcm.clone()),
            SAMPLE_RATE,
            target_duration as f32,
        )
        .unwrap();
        let espectro = espectro_em(esticado.as_slice().unwrap(), out_len / 2, window_len);
        espectro.iter().cloned().fold(0.0f32, f32::max)
    };

    // r=0.3: 8 kHz -> ~26,7 kHz, acima do Nyquist — irrepresentável por
    // construção. Um bom reamostrador filtra; um ruim dobra de volta.
    let pico_irrepresentavel = {
        let r = 0.3;
        let target_duration = duration * r;
        let out_len = (pcm.len() as f64 * r) as usize;
        let esticado = time_stretch(
            &Array1::from_vec(pcm.clone()),
            SAMPLE_RATE,
            target_duration as f32,
        )
        .unwrap();
        let espectro = espectro_em(esticado.as_slice().unwrap(), out_len / 2, window_len);
        espectro.iter().cloned().fold(0.0f32, f32::max)
    };

    assert!(
        pico_irrepresentavel < pico_representavel * 0.05,
        "conteúdo além do Nyquist não foi suprimido — pico representável={pico_representavel:.2}, pico irrepresentável={pico_irrepresentavel:.2} (esperado < 5% do primeiro; sinal de aliasing, não de filtragem)"
    );
}
