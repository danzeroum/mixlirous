//! Aliasing no reamostrador ÔÇö docs/17.1 ┬º3.2. Valida o `rubato` sinc de
//! `time_stretch` sobre fixtures que existiam e nunca tinham sido usadas para
//! isto (`sweeps/sweep_20_20k_mono.wav`, `tones/sine_8khz_mono.wav`). Hoje a
//! ├║nica prova de que a migra├º├úo para sinc est├í correta ├® o nome da fun├º├úo ÔÇö
//! estes dois testes s├úo a prova real.
//!
//! **Escopo exato do segundo teste ÔÇö leia antes de marcar "aliasing" como
//! resolvido.** `time_stretch_rejeita_imagem_de_conteudo_irrepresentavel`
//! cobre **rejei├º├úo de imagem por decima├º├úo**: conte├║do que passa a exigir
//! frequ├¬ncia acima do novo Nyquist tem que ser suprimido, n├úo dobrado de
//! volta como frequ├¬ncia esp├║ria. Isso **n├úo** cobre ondula├º├úo em banda
//! passante (o reamostrador alterar amplitude/fase de conte├║do j├í
//! represent├ível) nem pr├®-eco do filtro sinc (energia vazando para antes de
//! um transiente) ÔÇö s├úo falhas de qualidade de reamostragem distintas,
//! nenhuma testada aqui.
//!
//! **Achado ao escrever o primeiro teste, antes mesmo de rodar contra o
//! reamostrador:** `gen_log_sweep` normalizava o expoente da fase por
//! segundos, n├úo por `duration` ÔÇö a frequ├¬ncia instant├ónea batia o alvo em
//! `t=1,0s` sempre, nunca em `t=duration`. Para `duration=5.0` (o valor real
//! de todas as fixtures geradas), os ├║ltimos 4 dos 5 segundos da varredura
//! eram `sin()` de uma fase da ordem de 1e16 radianos: ru├¡do num├®rico, n├úo
//! uma varredura. Corrigido em `scripts/generate_fixtures.py`
//! (`gen_log_sweep`) ÔÇö sem isso, "frequ├¬ncia instant├ónea conhecida por
//! constru├º├úo" seria conhecida errada.
//!
//! **Por que s├úo dois testes, n├úo um.** A primeira tentativa usou s├│ o sweep,
//! medindo pico espectral em pontos onde a frequ├¬ncia esperada (depois de
//! `time_stretch` deslocar tudo por `1/R`) continuava abaixo do Nyquist.
//! Isso passa tanto para o `rubato` sinc quanto para um reamostrador linear
//! ing├¬nuo escrito ├á m├úo ÔÇö n├úo discrimina nada, porque um chirp constrito a
//! uma janela finita j├í teria energia espalhada (a frequ├¬ncia muda dentro da
//! pr├│pria janela) antes de qualquer aliasing entrar em cena; e interpola├º├úo
//! linear n├úo ├® ruim o bastante nessa faixa para se distinguir do sinc.
//! Confirmado experimentalmente antes de decidir a forma final deste
//! arquivo, n├úo por suposi├º├úo.
//!
//! O teste que de fato discrimina precisa de duas coisas que o chirp n├úo d├í
//! ao mesmo tempo: uma frequ├¬ncia **estacion├íria** conhecida (sem
//! espalhamento espectral de chirp) e uma raz├úo de reamostragem agressiva o
//! bastante para empurrar essa frequ├¬ncia **para al├®m do novo Nyquist** ÔÇö
//! a├¡ o conte├║do ├®, por constru├º├úo, irrepresent├ível, e a ├║nica pergunta ├®
//! se o reamostrador filtra (correto) ou dobra de volta como frequ├¬ncia
//! esp├║ria (aliasing). Por isso o segundo teste usa `sine_8khz_mono.wav`,
//! n├úo o sweep.

use audio_core::dsp::analysis::fft::magnitude_spectrum;
use audio_core::dsp::mastering::stretch::time_stretch;
use ndarray::Array1;

const SAMPLE_RATE: u32 = 44_100;

fn fixtures_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/audio")
}

fn decode_mono(path: &std::path::Path) -> Vec<f32> {
    let mut reader = hound::WavReader::open(path)
        .unwrap_or_else(|e| panic!("{path:?}: {e} ÔÇö rode scripts/generate_fixtures.py"));
    let spec = reader.spec();
    let max_val = (1i64 << (spec.bits_per_sample - 1)) as f32;
    reader
        .samples::<i32>()
        .map(|s| s.unwrap() as f32 / max_val)
        .collect()
}

/// Espectro de magnitude de uma janela Hann centrada em `center_idx` ÔÇö Hann
/// (n├úo retangular) para n├úo borrar o pico com vazamento espectral;
/// `magnitude_spectrum` em si n├úo janela.
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

/// ┬º3.2, parte 1: no regime represent├ível (frequ├¬ncia esperada bem abaixo do
/// Nyquist mesmo ap├│s o deslocamento de `time_stretch`), o pico espectral
/// bate com a frequ├¬ncia instant├ónea conhecida por constru├º├úo da varredura.
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

    // R = 0.5: encurta a dura├º├úo pela metade ÔÇö remapeamento linear de tempo
    // (sem corre├º├úo de afina├º├úo, documentado em stretch.rs) dobra toda
    // frequ├¬ncia. Pontos escolhidos (t_orig <= 4.3s) mapeiam para
    // frequ├¬ncias bem abaixo do Nyquist de 22050 Hz mesmo depois de
    // dobradas ÔÇö este teste ├® sobre fidelidade, n├úo sobre o limite em si
    // (isso ├® o segundo teste).
    const R: f64 = 0.5;
    let target_duration = ORIGINAL_DURATION * R;

    let esticado = time_stretch(
        &Array1::from_vec(pcm_original),
        SAMPLE_RATE,
        target_duration as f32,
    )
    .expect("time_stretch n├úo deveria falhar para entrada v├ílida");
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

        // Toler├óncia: resolu├º├úo de bin (~10,8 Hz neste window_len) mais uma
        // margem relativa ÔÇö apertado o bastante para reprovar um
        // reamostrador que borra ou desloca o pico, largo o bastante para
        // n├úo quebrar por causa da resolu├º├úo de bin em si.
        let tolerancia_hz = (freq_esperada * 0.05) + 20.0;
        assert!(
            (pico_hz as f64 - freq_esperada).abs() <= tolerancia_hz,
            "t_orig={t_orig}s: pico medido {pico_hz:.1} Hz, esperado {freq_esperada:.1} Hz (toler├óncia {tolerancia_hz:.1} Hz)"
        );
    }
}

/// ┬º3.2, parte 2 ÔÇö a que de fato discrimina, e s├│ sobre **rejei├º├úo de
/// imagem por decima├º├úo** (ver aviso de escopo no topo do arquivo, n├úo ├®
/// aliasing/qualidade de reamostragem em geral). Um tom de 8 kHz, deslocado
/// por `time_stretch` para al├®m do Nyquist de 22050 Hz (irrepresent├ível por
/// constru├º├úo), tem que ser **suprimido**, n├úo dobrado de volta como
/// frequ├¬ncia esp├║ria. Verificado experimentalmente antes de escrever este
/// teste: um reamostrador ing├¬nuo (decima├º├úo sem filtro anti-aliasing,
/// implementado s├│ para essa verifica├º├úo, n├úo em produ├º├úo) devolve o pico
/// espectral **na mesma magnitude** do caso represent├ível ÔÇö nenhuma
/// supress├úo, energia dobrada de volta como frequ├¬ncia errada. O `rubato`
/// sinc suprime a magnitude do pico em ~4 ordens de grandeza.
#[test]
fn time_stretch_rejeita_imagem_de_conteudo_irrepresentavel() {
    let pcm = decode_mono(&fixtures_dir().join("tones/sine_8khz_mono.wav"));
    let duration = pcm.len() as f64 / SAMPLE_RATE as f64;
    let window_len = 4096usize;

    // r=0.5: 8 kHz -> 16 kHz, ainda abaixo do Nyquist (22050) ÔÇö refer├¬ncia
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

    // r=0.3: 8 kHz -> ~26,7 kHz, acima do Nyquist ÔÇö irrepresent├ível por
    // constru├º├úo. Um bom reamostrador filtra; um ruim dobra de volta.
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
        "conte├║do al├®m do Nyquist n├úo foi suprimido ÔÇö pico represent├ível={pico_representavel:.2}, pico irrepresent├ível={pico_irrepresentavel:.2} (esperado < 5% do primeiro; sinal de aliasing, n├úo de filtragem)"
    );
}
