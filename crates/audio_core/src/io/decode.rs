//! Decodifica├º├úo de ├íudio comprimido/container para PCM em mem├│ria.
//!
//! Fecha a lacuna apontada em `docs/13-ROADMAP-SPRINTS.md` (Sprint 2.1,
//! "`decode_to_pcm` com symphonia + valida├º├úo de magic bytes"): o `symphonia`
//! era depend├¬ncia desde o come├ºo, mas nada chamava a API dele ÔÇö s├│ o tipo de
//! erro era usado (`crate::Error::Decode`). Os dois decodificadores que
//! existiam eram shims privados via `hound`, um no example da fatia vertical e
//! outro no teste de fixtures, ambos s├│ WAV e ambos incol├íveis de fora.
//!
//! **Os canais s├úo preservados.** `decode_to_pcm` devolve as amostras
//! intercaladas com a contagem de canais; o downmix para mono ├®
//! [`downmix_to_mono`], fun├º├úo ├á parte. Um decoder que s├│ devolvesse mono n├úo
//! conseguiria alimentar o caminho est├®reo nunca ÔÇö e est├®reo ├® o padr├úo de
//! sa├¡da do produto ÔÇö, e mudar a assinatura depois significaria mexer em todo
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
    /// Amostras intercaladas (`[L0, R0, L1, R1, ÔÇª]` para est├®reo),
    /// normalizadas para -1.0..=1.0.
    pub interleaved: Vec<f32>,
    pub channels: u16,
    pub sample_rate: u32,
}

impl DecodedAudio {
    /// Quadros, isto ├®, amostras por canal. `0` se `channels` for `0` ÔÇö que
    /// `decode_to_pcm` j├í recusa, mas o getter n├úo pode dividir por zero se
    /// algu├®m construir a struct na m├úo.
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

/// Formato reconhecido pelos magic bytes, antes de qualquer decodifica├º├úo.
///
/// Serve para dar erro leg├¡vel ("isto ├® um OGG, que n├úo est├í habilitado") em
/// vez do erro gen├®rico de probe do symphonia, e para recusar de cara o que
/// nem ├íudio ├®.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatoDetectado {
    Wav,
    Flac,
    Mp3,
    Aiff,
    /// Container ISO-BMFF (`.m4a`, `.mp4`) ÔÇö o `ftyp` na posi├º├úo 4.
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

/// Inspeciona os primeiros bytes. N├úo decodifica nada ÔÇö s├│ classifica.
pub fn detectar_formato(bytes: &[u8]) -> FormatoDetectado {
    if bytes.len() < 12 {
        return FormatoDetectado::Desconhecido;
    }
    let head = &bytes[0..4];
    let form = &bytes[8..12];

    if head == b"RIFF" && form == b"WAVE" {
        return FormatoDetectado::Wav;
    }
    // AIFF e AIFF-C: cont├¬iner IFF big-endian, `FORM` + `AIFF`/`AIFC`.
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
    // MP3: ou tem tag ID3v2 na frente, ou come├ºa direto num frame sync
    // (11 bits em 1). O segundo caso ├® o MP3 "cru", sem tag.
    if &bytes[0..3] == b"ID3" {
        return FormatoDetectado::Mp3;
    }
    if bytes[0] == 0xFF && (bytes[1] & 0xE0) == 0xE0 {
        return FormatoDetectado::Mp3;
    }

    FormatoDetectado::Desconhecido
}

/// Decodifica um arquivo de ├íudio em mem├│ria para PCM `f32`.
///
/// Aceita o que as features do `symphonia` habilitam (ver `Cargo.toml`):
/// WAV, FLAC, AIFF, MP3, AAC e ISO-MP4/M4A. Formato n├úo reconhecido pelos
/// magic bytes ├® recusado antes de chegar no probe, com o nome do que foi
/// detectado na mensagem ÔÇö quem chama de uma rota HTTP transforma isso num
/// 415 leg├¡vel em vez de um 500 gen├®rico.
pub fn decode_to_pcm(bytes: &[u8]) -> Result<DecodedAudio, Error> {
    let formato = detectar_formato(bytes);
    if formato == FormatoDetectado::Desconhecido {
        return Err(Error::Validation(
            "formato de ├íudio n├úo reconhecido pelos magic bytes; \
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

    // Primeira trilha de ├íudio com par├ómetros de codec. Um MP4 pode trazer
    // v├¡deo junto; pegar `tracks()[0]` cegamente escolheria a trilha errada.
    let (track_id, params) = format
        .tracks()
        .iter()
        .filter(|t| t.track_type() == Some(TrackType::Audio))
        .find_map(|t| match t.codec_params.as_ref() {
            Some(CodecParameters::Audio(p)) => Some((t.id, p.clone())),
            _ => None,
        })
        .ok_or_else(|| {
            Error::Validation("arquivo sem trilha de ├íudio decodific├ível".to_string())
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
        // O spec vem do buffer decodificado, n├úo dos par├ómetros do container:
        // alguns formatos declaram o sample rate s├│ no primeiro frame.
        sample_rate = spec.rate();
        channels = spec.channels().count() as u16;

        buffer_temp.clear();
        decoded.copy_to_vec_interleaved(&mut buffer_temp);
        interleaved.extend_from_slice(&buffer_temp);
    }

    if channels == 0 || sample_rate == 0 || interleaved.is_empty() {
        return Err(Error::Validation(
            "arquivo decodificou para zero amostras ÔÇö truncado ou vazio".to_string(),
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

/// Downmix por m├®dia aritm├®tica dos canais. Mesma t├®cnica dos shims que
/// existiam no example e no teste de fixtures ÔÇö mantida id├¬ntica para n├úo
/// mudar, de lado, o resultado de nenhum teste ac├║stico j├í calibrado.
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
    /// volta ÔÇö fecha o ciclo encodeÔåÆdecode sem depender das fixtures
    /// geradas, que n├úo s├úo commitadas.
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
        // Regress├úo: a detec├º├úo l├¬ bytes[8..12]; entrada com menos de 12
        // bytes precisa sair antes, n├úo entrar em panic de slice.
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

    /// O ponto da corre├º├úo de forma: os canais chegam preservados e o
    /// downmix ├® passo separado, n├úo embutido no decode.
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
        // Um est├®reo de 2 quadros a 4 Hz dura 0,5 s ÔÇö n├úo 1,0 s, que ├® o que
        // sairia se algu├®m dividisse o comprimento intercalado pelo rate.
        let audio = DecodedAudio {
            interleaved: vec![0.0, 0.0, 0.0, 0.0],
            channels: 2,
            sample_rate: 4,
        };
        assert!((audio.duration_sec() - 0.5).abs() < 1e-6);
    }
}
