//! Rota de diagn├│stico: exp├Áe a fatia vertical do pipeline de forma s├¡ncrona,
//! mais uma p├ígina de escuta A/B servida pela pr├│pria API.
//!
//! **Descart├ível de prop├│sito.** Existe para resolver um problema concreto e
//! tempor├írio: o motor DSP funciona (ver
//! `crates/audio_core/examples/fatia_vertical.rs`), mas n├úo h├í fila, worker,
//! nem execu├º├úo de pipeline ligada ├á API ÔÇö e o React em `ui/` foi desenhado
//! para o produto (canvas de n├│s, propostas do agente, painel de racioc├¡nio),
//! nada do que existe no backend hoje. Sem isto, julgar o resultado exige
//! `docker cp` e um player de sistema. Com isto, sobe a faixa pelo navegador e
//! compara. Quando o produto existir, este m├│dulo sai inteiro ÔÇö e o estado
//! dele sai junto, porque mora aqui dentro e n├úo no `AppState`.
//!
//! **N├úo ├® o pipeline de produ├º├úo.** Sem fila, sem persist├¬ncia, sem job, sem
//! agente. O `react_kernel` ├® `unimplemented!()` e n├úo ├® chamado daqui.

use audio_core::domain::{AudioCodec, PipelineConfig};
use audio_core::dsp::analysis::beat_tracking::{estimate_bpm, onset_strength};
use audio_core::dsp::{
    apply_fade_in, apply_fade_out, apply_lufs_gain, brickwall_limiter, crossfade_buffers,
    find_zero_crossing, time_stretch, DefaultAnalyzer, DefaultMixer, LufsGainOutcome,
};
use audio_core::io::{decode_to_pcm, downmix_to_mono};
use audio_core::ndarray::Array1;
use audio_core::{AudioAnalyzer, BeatDetectionParams, FadeCurve, TimeStretchFactor};
use axum::{
    body::Bytes,
    extract::{Multipart, Path, Query},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Teto de corpo do upload. Casa com o `client_max_body_size` do nginx
/// (`docs/18-DEPLOY-PUBLICO-NGINX.md`) ÔÇö descasar os dois faz o upload morrer
/// com 413 do proxy, sem mensagem da aplica├º├úo.
pub const LIMITE_UPLOAD_BYTES: usize = 100 * 1024 * 1024;

/// Teto de dura├º├úo, conferido depois do decode e antes do DSP.
///
/// A VPS n├úo ├® dedicada: hospeda o nginx de ~15 dom├¡nios de produ├º├úo, mais
/// Postgres e MinIO. A sa├¡da ├® mono `f32`, ent├úo 4 min a 48 kHz s├úo ~46 MB
/// **por c├│pia** ÔÇö e o pipeline segura v├írias vivas ao mesmo tempo (`pcm`,
/// `montado`, `esticado`, `pcm_final`, mais o resultado guardado). O pico
/// transit├│rio fica na casa dos 200 MB. Recusar na entrada ├® melhor que
/// estourar o `proxy_read_timeout` no meio: o nginx cortaria com 504 e o
/// usu├írio n├úo receberia nem erro da aplica├º├úo nem resultado parcial.
pub const LIMITE_DURACAO_SEG: f32 = 240.0;

/// Por quanto tempo o resultado fica dispon├¡vel para o `GET`.
const TTL_RESULTADO: Duration = Duration::from_secs(600);

/// Slot ├║nico, n├úo fila: para uma p├ígina de escuta de um usu├írio s├│, guardar
/// mais de um resultado n├úo serve para nada ÔÇö e capacidade medida em
/// *contagem*, com itens de tamanho ilimitado, n├úo ├® limite nenhum.
///
/// Mora no m├│dulo, n├úo no `AppState`: apagar este arquivo apaga o estado.
static ULTIMO_RESULTADO: OnceLock<Mutex<Option<ResultadoGuardado>>> = OnceLock::new();

struct ResultadoGuardado {
    id: Uuid,
    gravado_em: Instant,
    wav: Vec<u8>,
}

fn slot() -> &'static Mutex<Option<ResultadoGuardado>> {
    ULTIMO_RESULTADO.get_or_init(|| Mutex::new(None))
}

#[derive(Debug, Deserialize)]
pub struct SliceParams {
    /// `wav` devolve os bytes direto, sem o segundo request ÔÇö para `curl`.
    format: Option<String>,
    /// Fator de estiramento. Ausente = **sem estiramento**.
    stretch: Option<f32>,
}

#[derive(Debug, Serialize)]
pub struct Emenda {
    indice: usize,
    bloco_inicio_seg: f32,
    bloco_fim_seg: f32,
    amostra_alvo: usize,
    zero_crossing_mais_proximo: usize,
    posicao_na_saida_seg: f32,
    sobreposicao_amostras: usize,
}

#[derive(Debug, Serialize)]
pub struct SliceResposta {
    id: Uuid,
    audio_url: String,
    sample_rate: u32,
    canais_entrada: u16,
    duracao_entrada_seg: f32,
    duracao_saida_seg: f32,
    bpm_estimado: f32,
    batidas_detectadas: usize,
    blocos: usize,
    estiramento: f32,
    emendas: Vec<Emenda>,
    avisos: Vec<&'static str>,
}

type Erro = (StatusCode, String);

fn erro(status: StatusCode, codigo: &str) -> Erro {
    (status, codigo.to_string())
}

/// `GET /api/v1/dev/slice` ÔÇö a p├ígina de escuta.
///
/// `include_str!` em vez de `ServeDir`/`rust-embed`: zero depend├¬ncia nova,
/// nada a acrescentar no `COPY` do Dockerfile, e sem a fragilidade de caminho
/// relativo ao CWD que `prompts.rs` j├í carrega.
pub async fn pagina() -> Html<&'static str> {
    Html(include_str!("dev_slice.html"))
}

/// `GET /api/v1/dev/slice/{id}.wav` ÔÇö o ├íudio do ├║ltimo resultado.
pub async fn audio(Path(id_com_extensao): Path<String>) -> Result<Response, Erro> {
    let id_str = id_com_extensao
        .strip_suffix(".wav")
        .unwrap_or(&id_com_extensao);
    let id = Uuid::parse_str(id_str).map_err(|_| erro(StatusCode::BAD_REQUEST, "id_invalido"))?;

    let guardado = slot().lock().map_err(|_| {
        erro(
            StatusCode::INTERNAL_SERVER_ERROR,
            "estado_interno_envenenado",
        )
    })?;

    match guardado.as_ref() {
        Some(r) if r.id == id && r.gravado_em.elapsed() < TTL_RESULTADO => Ok((
            StatusCode::OK,
            [(header::CONTENT_TYPE, "audio/wav")],
            r.wav.clone(),
        )
            .into_response()),
        // 410 e n├úo 404 de prop├│sito: 404 pareceria rota inexistente e
        // mandaria algu├®m depurar a coisa errada. O recurso existiu.
        _ => Err((
            StatusCode::GONE,
            "resultado_expirado: o slot guarda s├│ o ├║ltimo render, por 10 min. \
             Reenvie o arquivo em POST /api/v1/dev/slice."
                .to_string(),
        )),
    }
}

/// `POST /api/v1/dev/slice` ÔÇö roda a fatia vertical sobre o arquivo enviado.
pub async fn processar(
    Query(params): Query<SliceParams>,
    multipart: Multipart,
) -> Result<Response, Erro> {
    let bytes = extrair_arquivo(multipart).await?;

    let estiramento = match params.stretch {
        // Ausente = 1.0. O example usa 1.05 fixo, e o coment├írio dele diz o
        // que ├®: "um ajuste de tempo plaus├¡vel, n├úo um estiramento artificial
        // s├│ para exercitar a fun├º├úo". Numa p├ígina cujo fim ├® julgar emenda e
        // masteriza├º├úo de ouvido, isso injeta uma vari├ível alheia ├ás duas e a
        // sa├¡da deixa de ser compar├ível com a entrada.
        None => 1.0,
        Some(f) => TimeStretchFactor::try_from(f)
            .map_err(|_| {
                (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!(
                        "parameter_out_of_bounds: stretch precisa estar entre {} e {}; veio {f}",
                        TimeStretchFactor::MIN,
                        TimeStretchFactor::MAX
                    ),
                )
            })?
            .get(),
    };

    // O pipeline ├® s├¡ncrono e pesado. No executor async ele travaria o
    // runtime inteiro enquanto processa.
    let (resposta, wav) = tokio::task::spawn_blocking(move || rodar_pipeline(&bytes, estiramento))
        .await
        .map_err(|_| erro(StatusCode::INTERNAL_SERVER_ERROR, "pipeline_panicou"))??;

    if params.format.as_deref() == Some("wav") {
        return Ok((StatusCode::OK, [(header::CONTENT_TYPE, "audio/wav")], wav).into_response());
    }

    {
        let mut guardado = slot().lock().map_err(|_| {
            erro(
                StatusCode::INTERNAL_SERVER_ERROR,
                "estado_interno_envenenado",
            )
        })?;
        *guardado = Some(ResultadoGuardado {
            id: resposta.id,
            gravado_em: Instant::now(),
            wav,
        });
    }

    Ok(Json(resposta).into_response())
}

async fn extrair_arquivo(mut multipart: Multipart) -> Result<Bytes, Erro> {
    while let Some(campo) = multipart
        .next_field()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("multipart_invalido: {e}")))?
    {
        let nome = campo.name().unwrap_or_default().to_string();
        if nome == "file" || nome == "arquivo" {
            return campo.bytes().await.map_err(|_| {
                (
                    StatusCode::PAYLOAD_TOO_LARGE,
                    format!(
                        "file_too_large: o limite ├® {} MB",
                        LIMITE_UPLOAD_BYTES / 1024 / 1024
                    ),
                )
            });
        }
    }
    Err(erro(
        StatusCode::BAD_REQUEST,
        "campo_ausente: envie o arquivo no campo `file`",
    ))
}

/// A fatia vertical em si ÔÇö mesma cadeia do example, na mesma ordem.
fn rodar_pipeline(bytes: &[u8], estiramento: f32) -> Result<(SliceResposta, Vec<u8>), Erro> {
    let mut avisos: Vec<&'static str> = Vec::new();

    let decodificado = decode_to_pcm(bytes).map_err(|e| {
        (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            format!("unsupported_media_type: {e}"),
        )
    })?;

    let duracao_entrada = decodificado.duration_sec();
    if duracao_entrada > LIMITE_DURACAO_SEG {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            format!(
                "file_too_large: o limite ├® {:.0} s ({:.1} min) e a faixa tem {:.0} s",
                LIMITE_DURACAO_SEG,
                LIMITE_DURACAO_SEG / 60.0,
                duracao_entrada
            ),
        ));
    }

    let sample_rate = decodificado.sample_rate;
    let pcm = downmix_to_mono(&decodificado);

    // 1. Detec├º├úo de batidas
    let params = BeatDetectionParams {
        sample_rate,
        ..Default::default()
    };
    let analyzer = DefaultAnalyzer;
    let beats = analyzer.detect_beats(&pcm, &params);
    let onset = onset_strength(&pcm, params.frame_size, params.hop_size);
    let bpm = estimate_bpm(&onset, sample_rate, params.hop_size);

    // 2. Blocos
    let config = PipelineConfig::default();
    let block_size_beats = config.selection.block_size_beats.get();
    let blocos = analyzer.build_blocks(&pcm, &beats, block_size_beats, sample_rate);

    // 3. Emenda. Sem algoritmo de sele├º├úo (SelectionConfig n├úo ├® lido em
    //    lugar nenhum do crate ainda, docs/04 ┬º5) ÔÇö usa todos, em ordem.
    let fade_alvo = ((20.0f32 / 1000.0) * sample_rate as f32) as usize;
    let mut montado: Vec<f32> = Vec::new();
    let mut emendas: Vec<Emenda> = Vec::new();

    for (i, bloco) in blocos.iter().enumerate() {
        let trecho = &pcm.as_slice().unwrap_or(&[])[bloco.start_sample..bloco.end_sample];
        if i == 0 {
            montado.extend_from_slice(trecho);
            continue;
        }

        let zc = find_zero_crossing(&pcm, bloco.start_sample, 200);
        let fade_samples = fade_alvo.min(montado.len()).min(trecho.len());
        let start_a = montado.len() - fade_samples;

        emendas.push(Emenda {
            indice: i,
            bloco_inicio_seg: bloco.start_time,
            bloco_fim_seg: bloco.end_time,
            amostra_alvo: bloco.start_sample,
            zero_crossing_mais_proximo: zc,
            posicao_na_saida_seg: start_a as f32 / sample_rate as f32,
            sobreposicao_amostras: fade_samples,
        });

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
        // Acontece de verdade nas fixtures de `rhythm/`: `detect_beat_frames`
        // exige `onset[i] > 0.1` e o pico real dessas fixtures ├® ~0.071
        // (issue #27). Sem este aviso a p├ígina mentiria ÔÇö sem blocos, s├│ a
        // masteriza├º├úo agiu, e o "remix" n├úo fez nada.
        avisos.push("no_beats_detected");
        montado = pcm.to_vec();
    }

    // 4. Fades de borda
    let fade_borda = (0.02 * sample_rate as f32) as usize;
    let fim = montado.len();
    apply_fade_in(
        &mut montado,
        0,
        fade_borda.min(fim),
        &FadeCurve::Logarithmic,
    );
    apply_fade_out(
        &mut montado,
        fim.saturating_sub(fade_borda),
        fade_borda.min(fim),
        &FadeCurve::Logarithmic,
    );

    // 5. Estiramento ÔÇö s├│ se pedido explicitamente.
    let pcm_esticado = if (estiramento - 1.0).abs() < f32::EPSILON {
        Array1::from_vec(montado)
    } else {
        let duracao_atual = montado.len() as f32 / sample_rate as f32;
        let alvo = duracao_atual * estiramento;
        let entrada = Array1::from_vec(montado);
        match time_stretch(&entrada, sample_rate, alvo) {
            Some(esticado) => esticado,
            None => {
                avisos.push("time_stretch_skipped");
                entrada
            },
        }
    };

    // 6. Masteriza├º├úo
    let mut pcm_final = pcm_esticado.to_vec();
    if let LufsGainOutcome::UnmeasurableLoudness = apply_lufs_gain(
        &mut pcm_final,
        sample_rate,
        config.mastering.lufs_target.get(),
    ) {
        avisos.push("unmeasurable_loudness");
    }
    brickwall_limiter(&mut pcm_final, config.mastering.peak_db);

    // 7. Exporta├º├úo em mem├│ria
    let mut export = config;
    export.format.channels = 1;
    export.format.sample_rate = sample_rate;
    export.format.bit_depth = 32;
    export.format.codec = AudioCodec::WAV;

    let duracao_saida = pcm_final.len() as f32 / sample_rate as f32;
    let wav = DefaultMixer
        .encode_wav_to_vec(&Array1::from_vec(pcm_final), &export)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("falha_ao_codificar_wav: {e}"),
            )
        })?;

    let id = Uuid::new_v4();
    Ok((
        SliceResposta {
            id,
            audio_url: format!("/api/v1/dev/slice/{id}.wav"),
            sample_rate,
            canais_entrada: decodificado.channels,
            duracao_entrada_seg: duracao_entrada,
            duracao_saida_seg: duracao_saida,
            bpm_estimado: bpm,
            batidas_detectadas: beats.len(),
            blocos: blocos.len(),
            estiramento,
            emendas,
            avisos,
        },
        wav,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use audio_core::dsp::DefaultMixer;

    fn wav_de_teste(amostras: &[f32], sample_rate: u32) -> Vec<u8> {
        let mut config = PipelineConfig::default();
        config.format.sample_rate = sample_rate;
        config.format.channels = 1;
        config.format.bit_depth = 32;
        config.format.codec = AudioCodec::WAV;
        DefaultMixer
            .encode_wav_to_vec(&Array1::from_vec(amostras.to_vec()), &config)
            .unwrap()
    }

    /// Uma senoide curta n├úo cruza o limiar de onset ÔÇö cai no fallback de PCM
    /// bruto, e o aviso precisa aparecer. ├ë o caso da issue #27.
    #[test]
    fn sem_batidas_avisa_em_vez_de_mentir() {
        let amostras: Vec<f32> = (0..44100).map(|i| (i as f32 * 0.05).sin() * 0.3).collect();
        let bytes = wav_de_teste(&amostras, 44100);

        let (resposta, wav) = rodar_pipeline(&bytes, 1.0).unwrap();
        assert!(
            resposta.avisos.contains(&"no_beats_detected"),
            "avisos: {:?}",
            resposta.avisos
        );
        assert!(!wav.is_empty());
    }

    /// O ponto da corre├º├úo: sem `?stretch`, a dura├º├úo de sa├¡da acompanha a de
    /// entrada. Com o 1.05 fixo do example, sairia ~5% mais longa e a
    /// compara├º├úo A/B estaria comparando materiais diferentes.
    #[test]
    fn sem_stretch_a_duracao_nao_muda() {
        let amostras: Vec<f32> = (0..44100).map(|i| (i as f32 * 0.05).sin() * 0.3).collect();
        let bytes = wav_de_teste(&amostras, 44100);

        let (resposta, _) = rodar_pipeline(&bytes, 1.0).unwrap();
        assert_eq!(resposta.estiramento, 1.0);
        assert!(
            (resposta.duracao_saida_seg - resposta.duracao_entrada_seg).abs() < 0.01,
            "entrada {} vs sa├¡da {}",
            resposta.duracao_entrada_seg,
            resposta.duracao_saida_seg
        );
    }

    #[test]
    fn arquivo_que_nao_e_audio_da_415_e_nao_500() {
        let (status, _) = rodar_pipeline(b"# isto nao e audio nenhum", 1.0).unwrap_err();
        assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }

    #[test]
    fn faixa_longa_demais_e_recusada_na_entrada() {
        // Sample rate baixo para fabricar dura├º├úo longa sem alocar mem├│ria:
        // 8 Hz ├ù 4000 amostras = 500 s, acima do teto de 240 s.
        let amostras = vec![0.1f32; 4000];
        let bytes = wav_de_teste(&amostras, 8);

        let (status, msg) = rodar_pipeline(&bytes, 1.0).unwrap_err();
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        assert!(msg.contains("file_too_large"), "msg: {msg}");
    }
}
