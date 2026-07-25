//! Fatia vertical do pipeline de remix, ponta a ponta, sobre uma fixture
//! sintética — não depende das duas faixas reais para existir (docs/17,
//! "a fatia vertical não depende das faixas para ser construída").
//!
//! ```text
//! cargo run --example fatia_vertical -- fixtures/audio/rhythm/rhythm_120bpm_mono.wav /tmp/saida.wav
//! ```
//!
//! **O que isto prova, e o que não prova.** Material sintético não responde
//! "soa bem" — só ouvir as duas faixas reais responde isso. O que este
//! binário prova: o pipeline roda ponta a ponta ou trava em algum lugar
//! específico; `export_wav` escreve bytes de verdade (deixou de ser
//! placeholder, ver `default_mixer.rs`); decodificação, detecção, seleção,
//! emenda e masterização se encaixam sem erro de tipo/contrato entre uma
//! etapa e a próxima; os instantes de emenda saem no stdout. Quando as
//! faixas reais chegarem, trocar os dois argumentos é a única mudança.
//!
//! **Lacunas que este binário expõe, não resolve** (cada uma já rastreada
//! em outro lugar, não é novidade deste arquivo):
//! - Não existe decodificador de produção (`symphonia` é dependência mas
//!   nada chama a API de decodificação — só o erro é usado, ver
//!   `error.rs`). A função `decodificar` abaixo é um shim local via
//!   `hound`, do mesmo jeito que `tests/fixtures_manifest.rs` já fazia —
//!   não é o decoder real, só destrava este binário.
//! - Não existe algoritmo de seleção de blocos (`SelectionConfig` é lido em
//!   lugar nenhum do crate, `docs/04-DOMINIO-DSP.md` §5). Este binário usa
//!   todos os blocos detectados, em ordem — não é decisão de produto, é só
//!   o que deixa o resto da cadeia rodar.
//! - A cadeia de masterização usa `apply_lufs_gain` + `brickwall_limiter`
//!   como existem hoje — o limiter é normalizador de pico, sem look-ahead,
//!   e a ordem/conflito de alvos correta (`docs/16` T3.1–T3.4) é trabalho
//!   futuro rastreado à parte, não implementado aqui.
//! - `estimate_bpm` reporta metade do andamento real para algumas fixtures
//!   de `rhythm/`/`click_tracks/` (issue #18, não corrigida) — o BPM
//!   impresso abaixo pode estar sujeito a esse bug conhecido; não é um
//!   problema novo deste binário.
//!
//! **Achado por este binário, não pré-existente:** rodando sobre
//! `fixtures/audio/rhythm/rhythm_120bpm_mono.wav`, a etapa 2 encontra **zero**
//! batidas candidatas — `detect_beat_frames` (`beat_tracking.rs`) exige
//! `onset[i] > 0.1`, e o pico real de `onset_strength` nesta fixture é
//! ~0.071, nunca cruza o limiar. Issue #27. O fallback da etapa 5 ("nenhum
//! bloco → usa o PCM bruto") existe por causa disso — sem ele, a saída seria
//! um buffer vazio, não um travamento, mas o pipeline "de verdade" (emenda,
//! seleção) nunca chegaria a rodar com este detector no estado atual.

use audio_core::dsp::analysis::beat_tracking::{estimate_bpm, onset_strength};
use audio_core::dsp::{
    apply_fade_in, apply_fade_out, apply_lufs_gain, brickwall_limiter, crossfade_buffers,
    find_zero_crossing, time_stretch, DefaultAnalyzer, DefaultMixer, LufsGainOutcome,
};
use audio_core::{
    AudioAnalyzer, AudioCodec, AudioMixer, BeatDetectionParams, FadeCurve, PipelineConfig,
    TimeStretchFactor,
};
use ndarray::Array1;
use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Shim de decodificação local, não o decoder de produção (ver comentário do
/// módulo). Mesma técnica de `tests/fixtures_manifest.rs::decodificar`:
/// downmix por média para mono, PCM inteiro normalizado para -1.0..=1.0.
fn decodificar(path: &Path) -> Result<(Array1<f32>, u32), hound::Error> {
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

    let channels = spec.channels as usize;
    let mono = if channels <= 1 {
        interleaved
    } else {
        let frames = interleaved.len() / channels;
        (0..frames)
            .map(|frame| {
                let start = frame * channels;
                interleaved[start..start + channels].iter().sum::<f32>() / channels as f32
            })
            .collect()
    };

    Ok((Array1::from_vec(mono), spec.sample_rate))
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let (Some(entrada), Some(saida)) = (args.get(1), args.get(2)) else {
        eprintln!("uso: cargo run --example fatia_vertical -- <entrada.wav> <saida.wav>");
        return ExitCode::FAILURE;
    };
    let entrada = PathBuf::from(entrada);
    let saida = PathBuf::from(saida);

    println!("== 1. Decodificação ==");
    let (pcm, sample_rate) = match decodificar(&entrada) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("falha ao decodificar {entrada:?}: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!(
        "{} amostras, {sample_rate} Hz, downmix para mono",
        pcm.len()
    );

    println!("\n== 2. Detecção de batidas ==");
    let params = BeatDetectionParams {
        sample_rate,
        ..Default::default()
    };
    let analyzer = DefaultAnalyzer;
    let beats = analyzer.detect_beats(&pcm, &params);
    let onset = onset_strength(&pcm, params.frame_size, params.hop_size);
    let bpm = estimate_bpm(&onset, sample_rate, params.hop_size);
    println!(
        "{} batidas candidatas, bpm estimado: {bpm:.1} (issue #18: pode ser metade do andamento real nesta fixture)",
        beats.len()
    );

    println!("\n== 3. Construção de blocos ==");
    let config = PipelineConfig::default();
    let block_size_beats = config.selection.block_size_beats.get();
    let blocks = analyzer.build_blocks(&pcm, &beats, block_size_beats, sample_rate);
    println!("{} blocos de {block_size_beats} batidas cada", blocks.len());

    println!("\n== 4. Seleção ==");
    println!(
        "sem algoritmo de seleção real (SelectionConfig não é lido em lugar nenhum, \
         docs/04 §5) — usando todos os {} blocos, em ordem",
        blocks.len()
    );

    println!("\n== 5. Emenda (crossfade + zero-crossing) ==");
    let fade_ms = 20.0f32;
    let fade_samples_alvo = ((fade_ms / 1000.0) * sample_rate as f32) as usize;
    let mut montado: Vec<f32> = Vec::new();

    for (i, block) in blocks.iter().enumerate() {
        let trecho = &pcm.as_slice().unwrap_or(&[])[block.start_sample..block.end_sample];
        if i == 0 {
            montado.extend_from_slice(trecho);
            continue;
        }

        let zc = find_zero_crossing(&pcm, block.start_sample, 200);
        let fade_samples = fade_samples_alvo.min(montado.len()).min(trecho.len());
        let start_a = montado.len() - fade_samples;

        println!(
            "emenda {i}: bloco fonte {:.3}s..{:.3}s (zero-crossing mais próximo do início: \
             amostra {zc}, alvo original amostra {}); posição na saída montada: {:.3}s, \
             sobreposição de {fade_samples} amostras",
            block.start_time,
            block.end_time,
            block.start_sample,
            start_a as f32 / sample_rate as f32
        );

        montado.resize(start_a + trecho.len(), 0.0);
        crossfade_buffers(
            &mut montado,
            start_a,
            trecho,
            0,
            fade_samples,
            config.crossfade.curve,
        );
    }

    if montado.is_empty() {
        println!("nenhum bloco disponível para emendar — usando o PCM decodificado bruto");
        montado = pcm.to_vec();
    }
    println!(
        "saída montada: {} amostras ({:.3}s)",
        montado.len(),
        montado.len() as f32 / sample_rate as f32
    );

    println!("\n== 6. Fades de borda + ajuste de duração ==");
    let fade_borda_samples = ((0.02) * sample_rate as f32) as usize;
    let fim = montado.len();
    apply_fade_in(
        &mut montado,
        0,
        fade_borda_samples.min(fim),
        &FadeCurve::Logarithmic,
    );
    apply_fade_out(
        &mut montado,
        fim.saturating_sub(fade_borda_samples),
        fade_borda_samples.min(fim),
        &FadeCurve::Logarithmic,
    );

    // Fator dentro da faixa validada pelo agente (T0.0, docs/16) — 1.05 é um
    // ajuste de tempo plausível, não um estiramento artificial só para
    // exercitar a função.
    let fator = TimeStretchFactor::try_from(1.05)
        .expect("1.05 está dentro de TimeStretchFactor::MIN..=MAX por construção");
    let duracao_atual = montado.len() as f32 / sample_rate as f32;
    let duracao_alvo = duracao_atual * fator.get();
    let esticado = time_stretch(
        &Array1::from_vec(montado.clone()),
        sample_rate,
        duracao_alvo,
    )
    .unwrap_or_else(|| {
        eprintln!("time_stretch não pôde esticar (buffer vazio?) — seguindo sem esticar");
        Array1::from_vec(montado)
    });
    println!(
        "duração após ajuste (fator {:.2}x): {:.3}s",
        fator.get(),
        esticado.len() as f32 / sample_rate as f32
    );

    println!("\n== 7. Masterização ==");
    let mut pcm_final = esticado.to_vec();
    let target_lufs = config.mastering.lufs_target.get();
    match apply_lufs_gain(&mut pcm_final, sample_rate, target_lufs) {
        LufsGainOutcome::Applied { gain_db } => {
            println!("LUFS: ganho de {gain_db:.2} dB aplicado (alvo {target_lufs} LUFS)")
        }
        LufsGainOutcome::UnmeasurableLoudness => {
            println!("LUFS: não mensurável (buffer curto/silencioso demais) — buffer não alterado")
        }
    }
    brickwall_limiter(&mut pcm_final, config.mastering.peak_db);
    println!(
        "limiter: pico alvo {} dBFS (normalizador de pico simples, não look-ahead — docs/16 T3.2)",
        config.mastering.peak_db
    );

    println!("\n== 8. Exportação ==");
    let mut export_config = config;
    export_config.format.channels = 1;
    export_config.format.sample_rate = sample_rate;
    export_config.format.bit_depth = 32;
    export_config.format.codec = AudioCodec::WAV;

    let mixer = DefaultMixer;
    match mixer.export_wav(&Array1::from_vec(pcm_final), &saida, &export_config) {
        Ok(()) => {
            println!("escrito em {saida:?}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("falha ao exportar: {e}");
            ExitCode::FAILURE
        }
    }
}
