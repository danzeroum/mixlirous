//! Entrada e saída de áudio: decodificação de arquivo para PCM.
//!
//! A escrita de WAV mora em `dsp::mastering::default_mixer`
//! (`DefaultMixer::encode_wav`), junto do resto da masterização — não aqui,
//! para não separar a validação de formato do estágio que a usa.

pub mod decode;

pub use decode::{
    decode_to_pcm, detectar_formato, downmix_to_mono, DecodedAudio, FormatoDetectado,
};
