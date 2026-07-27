//! Decodificação de áudio comprimido/container para PCM em memória.
//!
//! Fecha a lacuna apontada em `docs/13-ROADMAP-SPRINTS.md` (Sprint 2.1,
//! "`decode_to_pcm` com symphonia + validação de magic bytes"): o `symphonia`
//! era dependência desde o começo, mas nada chamava a API dele — só o tipo de
//! erro era usado (`crate::Error::Decode`). Os dois decodificadores que
//! existiam eram shims privados via `hound`, um no example da fatia vertical e
//! outro no teste de fixtures, ambos só WAV e ambos incoláveis de fora.
//!
//! **Os canais são preservados.** `decode_to_pcm` devolve as amostras
//! intercaladas com a contagem de canais; o downmix para mono é
//! [`downmix_to_mono`], função à parte. Um decoder que só devolvesse mono não
//! conseguiria alimentar o caminho estéreo nunca — e estéreo é o padrão de
//! saída do produto —, e mudar a assinatura depois significaria mexer em todo
//! call site.

use crate::Error;
use ndarray::Array1;
use std::io::Cursor;
use symphonia::core::codecs::CodecParameters;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;

/// PCM decodificado, com os canais preservados.
#[derive(Debug, Clone)]
pub struct DecodedAudio {
    /// Amostras intercaladas (`[L0, R0, L1, R1, …]` para estéreo),
    /// normalizadas para -1.0..=1.0.
    pub interleaved: Vec<f32>,
    pub channels: u16,
    pub sample_rate: u32,
}

impl DecodedAudio {
    /// Quadros, isto é, amostras por canal. `0` se `channels` for `0` — que
    /// `decode_to_pcm` já recusa, mas o getter não pode dividir por zero se
    /// alguém construir a struct na mão.
    pub fn frames(&self) -> usize {
        if self.channels == 0 {
            return 0;
        }
        self.interleaved.len() / self.channels as usize
    }

    pub fn duration_sec(&self) -> f32 {
        if self.sample_rate == 0 {
            return 0.0;
        }
        self.frames() as f32 / self.sample_rate as f32
    }
}

/// Formato reconhecido pelos magic bytes, antes de qualquer decodificação.
///
/// Serve para dar erro legível ("isto é um OGG, que não está habilitado") em
/// vez do erro genérico de probe do symphonia, e para recusar de cara o que
/// nem áudio é.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatoDetectado {
    Wav,
    Flac,
    Mp3,
    Aiff,
    /// Container ISO-BMFF (`.m4a`, `.mp4`) — o `ftyp` na posição 4.
    IsoMp4,
    Ogg,
    Desconhecido,
}

impl FormatoDetectado {
    fn extensao_hint(self) -> Option<&'static str> {
        match self {
            Self::Wav => Some("wav"),
            Self::Flac => Some("flac"),
            Self::Mp3 => Some("mp3"),
            Self::Aiff => Some("aiff"),
            Self::IsoMp4 => Some("m4a"),
            Self::Ogg => Some("ogg"),
            Self::Desconhecido => None,
        }
    }
}

/// Inspeciona os primeiros bytes. Não decodifica nada — só classifica.
pub fn detectar_formato(bytes: &[u8]) -> FormatoDetectado {
    if bytes.len() < 12 {
        return FormatoDetectado::Desconhecido;
    }
    let head = &bytes[0..4];
    let form = &bytes[8..12];

    if head == b"RIFF" && form == b"WAVE" {
        return FormatoDetectado::Wav;
    }
    // AIFF e AIFF-C: contêiner IFF big-endian, `FORM` + `AIFF`/`AIFC`.
    if head == b"FORM" && (form == b"AIFF" || form == b"AIFC") {
        return FormatoDetectado::Aiff;
    }
    if head == b"fLaC" {
        return FormatoDetectado::Flac;
    }
    if head == b"OggS" {
        return FormatoDetectado::Ogg;
    }
    if form == b"ftyp" {
        return FormatoDetectado::IsoMp4;
    }
    // MP3: ou tem tag ID3v2 na frente, ou começa direto num frame sync
    // (11 bits em 1). O segundo caso é o MP3 "cru", sem tag.
    if &bytes[0..3] == b"ID3" {
        return FormatoDetectado::Mp3;
    }
    if bytes[0] == 0xFF && (bytes[1] & 0xE0) == 0xE0 {
        return FormatoDetectado::Mp3;
    }

    FormatoDetectado::Desconhecido
}

/// Decodifica um arquivo de áudio em memória para PCM `f32`.
///
/// Aceita o que as features do `symphonia` habilitam (ver `Cargo.toml`):
/// WAV, FLAC, AIFF, MP3, AAC e ISO-MP4/M4A. Formato não reconhecido pelos
/// magic bytes é recusado antes de chegar no probe, com o nome do que foi
/// detectado na mensagem — quem chama de uma rota HTTP transforma isso num
/// 415 legível em vez de um 500 genérico.
pub fn decode_to_pcm(bytes: &[u8]) -> Result<DecodedAudio, Error> {
    let formato = detectar_formato(bytes);
    if formato == FormatoDetectado::Desconhecido {
        return Err(Error::Validation(
            "formato de áudio não reconhecido pelos magic bytes; \
             aceitos: WAV, FLAC, AIFF, MP3, AAC, M4A/MP4"
                .to_string(),
        ));
    }

    let fonte = Cursor::new(bytes.to_vec());
    let mss = MediaSourceStream::new(Box::new(fonte), MediaSourceStreamOptions::default());

    let mut hint = Hint::new();
    if let Some(ext) = formato.extensao_hint() {
        hint.with_extension(ext);
    }

    let mut format = symphonia::default::get_probe().probe(
        &hint,
        mss,
        FormatOptions::default(),
        MetadataOptions::default(),
    )?;

    // Primeira trilha de áudio com parâmetros de codec. Um MP4 pode trazer
    // vídeo junto; pegar `tracks()[0]` cegamente escolheria a trilha errada.
    let (track_id, params) = format
        .tracks()
        .iter()
        .filter(|t| t.track_type() == Some(TrackType::Audio))
        .find_map(|t| match t.codec_params.as_ref() {
            Some(CodecParameters::Audio(p)) => Some((t.id, p.clone())),
            _ => None,
        })
        .ok_or_else(|| {
            Error::Validation("arquivo sem trilha de áudio decodificável".to_string())
        })?;

    let mut decoder =
        symphonia::default::get_codecs().make_audio_decoder(&params, &default_opts())?;

    let mut interleaved: Vec<f32> = Vec::new();
    let mut channels: u16 = 0;
    let mut sample_rate: u32 = 0;
    let mut buffer_temp: Vec<f32> = Vec::new();

    while let Some(packet) = format.next_packet()? {
        if packet.track_id != track_id {
            continue;
        }
        let decoded = decoder.decode(&packet)?;
        let spec = decoded.spec();
        // O spec vem do buffer decodificado, não dos parâmetros do container:
        // alguns formatos declaram o sample rate só no primeiro frame.
        sample_rate = spec.rate();
        channels = spec.channels().count() as u16;

        buffer_temp.clear();
        decoded.copy_to_vec_interleaved(&mut buffer_temp);
        interleaved.extend_from_slice(&buffer_temp);
    }

    if channels == 0 || sample_rate == 0 || interleaved.is_empty() {
        return Err(Error::Validation(
            "arquivo decodificou para zero amostras — truncado ou vazio".to_string(),
        ));
    }

    Ok(DecodedAudio {
        interleaved,
        channels,
        sample_rate,
    })
}

fn default_opts() -> symphonia::core::codecs::audio::AudioDecoderOptions {
    symphonia::core::codecs::audio::AudioDecoderOptions::default()
}

/// Downmix por média aritmética dos canais. Mesma técnica dos shims que
/// existiam no example e no teste de fixtures — mantida idêntica para não
/// mudar, de lado, o resultado de nenhum teste acústico já calibrado.
pub fn downmix_to_mono(audio: &DecodedAudio) -> Array1<f32> {
    let canais = audio.channels as usize;
    if canais <= 1 {
        return Array1::from_vec(audio.interleaved.clone());
    }

    let frames = audio.frames();
    let mono: Vec<f32> = (0..frames)
        .map(|frame| {
            let inicio = frame * canais;
            audio.interleaved[inicio..inicio + canais]
                .iter()
                .sum::<f32>()
                / canais as f32
        })
        .collect();

    Array1::from_vec(mono)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AudioCodec, PipelineConfig};
    use crate::dsp::DefaultMixer;

    fn config_wav(sample_rate: u32, bit_depth: u8) -> PipelineConfig {
        let mut config = PipelineConfig::default();
        config.format.sample_rate = sample_rate;
        config.format.channels = 1;
        config.format.bit_depth = bit_depth;
        config.format.codec = AudioCodec::WAV;
        config
    }

    /// Gera um WAV mono de verdade via `encode_wav_to_vec` e decodifica de
    /// volta — fecha o ciclo encode→decode sem depender das fixtures
    /// geradas, que não são commitadas.
    fn wav_mono(amostras: &[f32], sample_rate: u32) -> Vec<u8> {
        DefaultMixer
            .encode_wav_to_vec(
                &Array1::from_vec(amostras.to_vec()),
                &config_wav(sample_rate, 32),
            )
            .expect("encode do WAV de teste")
    }

    #[test]
    fn detecta_wav_pelos_magic_bytes() {
        let bytes = wav_mono(&[0.0, 0.5, -0.5], 44100);
        assert_eq!(detectar_formato(&bytes), FormatoDetectado::Wav);
    }

    #[test]
    fn detecta_nao_audio_como_desconhecido() {
        assert_eq!(
            detectar_formato(b"# Um markdown qualquer, nao e audio"),
            FormatoDetectado::Desconhecido
        );
    }

    #[test]
    fn arquivo_curto_demais_nao_estoura_indice() {
        // Regressão: a detecção lê bytes[8..12]; entrada com menos de 12
        // bytes precisa sair antes, não entrar em panic de slice.
        assert_eq!(detectar_formato(b"RIF"), FormatoDetectado::Desconhecido);
        assert_eq!(detectar_formato(b""), FormatoDetectado::Desconhecido);
    }

    #[test]
    fn nao_audio_e_recusado_com_validation_nao_panic() {
        let err = decode_to_pcm(b"# Um markdown qualquer, nao e audio").unwrap_err();
        assert!(
            matches!(err, Error::Validation(_)),
            "esperava Validation, veio {err:?}"
        );
    }

    #[test]
    fn decodifica_wav_mono_preservando_amostras() {
        let original = vec![0.0f32, 0.5, -0.5, 0.25, -0.25];
        let bytes = wav_mono(&original, 44100);

        let decodificado = decode_to_pcm(&bytes).expect("decode do WAV mono");
        assert_eq!(decodificado.channels, 1);
        assert_eq!(decodificado.sample_rate, 44100);
        assert_eq!(decodificado.frames(), original.len());
        for (esperado, obtido) in original.iter().zip(decodificado.interleaved.iter()) {
            assert!((esperado - obtido).abs() < 1e-6, "{esperado} vs {obtido}");
        }
    }

    #[test]
    fn downmix_de_mono_e_identidade() {
        let audio = DecodedAudio {
            interleaved: vec![0.1, 0.2, 0.3],
            channels: 1,
            sample_rate: 44100,
        };
        assert_eq!(downmix_to_mono(&audio).to_vec(), vec![0.1, 0.2, 0.3]);
    }

    /// O ponto da correção de forma: os canais chegam preservados e o
    /// downmix é passo separado, não embutido no decode.
    #[test]
    fn downmix_de_estereo_tira_a_media_dos_canais() {
        let audio = DecodedAudio {
            interleaved: vec![1.0, 0.0, 0.5, -0.5, -1.0, 1.0],
            channels: 2,
            sample_rate: 48000,
        };
        assert_eq!(audio.frames(), 3);
        assert_eq!(downmix_to_mono(&audio).to_vec(), vec![0.5, 0.0, 0.0]);
    }

    #[test]
    fn duracao_vem_de_quadros_e_nao_de_amostras_intercaladas() {
        // Um estéreo de 2 quadros a 4 Hz dura 0,5 s — não 1,0 s, que é o que
        // sairia se alguém dividisse o comprimento intercalado pelo rate.
        let audio = DecodedAudio {
            interleaved: vec![0.0, 0.0, 0.0, 0.0],
            channels: 2,
            sample_rate: 4,
        };
        assert!((audio.duration_sec() - 0.5).abs() < 1e-6);
    }
}
