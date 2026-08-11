//! Entrada e sa├¡da de ├íudio: decodifica├º├úo de arquivo para PCM.
//!
//! A escrita de WAV mora em `dsp::mastering::default_mixer`
//! (`DefaultMixer::encode_wav`), junto do resto da masteriza├º├úo ÔÇö n├úo aqui,
//! para n├úo separar a valida├º├úo de formato do est├ígio que a usa.

pub mod decode;

pub use decode::{
    decode_to_pcm, detectar_formato, downmix_to_mono, DecodedAudio, FormatoDetectado,
};
