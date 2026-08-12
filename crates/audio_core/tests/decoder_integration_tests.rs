//! Testes de integra├º├úo do decodificador symphonia + pipeline.
//!
//! Usa `encode_wav_to_vec` do `DefaultMixer` para gerar WAV real e
//! `decode_to_pcm` para decodificar de volta ÔÇö ciclo completo encodeÔåÆdecode
//! sem depender das fixtures geradas (que n├úo s├úo commitadas).

use audio_core::dsp::DefaultMixer;
use audio_core::io::{decode_to_pcm, downmix_to_mono, DecodedAudio};
use audio_core::{AudioCodec, PipelineConfig};
use ndarray::Array1;

fn wav_config(sr: u32, bd: u8) -> PipelineConfig {
    let mut c = PipelineConfig::default();
    c.format.sample_rate = sr;
    c.format.channels = 1;
    c.format.bit_depth = bd;
    c.format.codec = AudioCodec::WAV;
    c
}

fn encode_wav(samples: &[f32], sr: u32, bd: u8) -> Vec<u8> {
    DefaultMixer
        .encode_wav_to_vec(&Array1::from_vec(samples.to_vec()), &wav_config(sr, bd))
        .expect("encode")
}

#[test]
fn roundtrip_32bit_float_preserves_samples() {
    let original: Vec<f32> = (0..1000).map(|i| (i as f32 * 0.001).sin() * 0.8).collect();
    let bytes = encode_wav(&original, 44100, 32);
    let decoded = decode_to_pcm(&bytes).expect("decode");
    assert_eq!(decoded.channels, 1);
    assert_eq!(decoded.sample_rate, 44100);
    assert_eq!(decoded.frames(), original.len());
    for (exp, got) in original.iter().zip(decoded.interleaved.iter()) {
        assert!((exp - got).abs() < 1e-6, "{exp} vs {got}");
    }
}

#[test]
fn roundtrip_16bit_within_quantization() {
    let original: Vec<f32> = (0..1000).map(|i| (i as f32 * 0.001).sin() * 0.9).collect();
    let bytes = encode_wav(&original, 44100, 16);
    let decoded = decode_to_pcm(&bytes).expect("decode");
    assert_eq!(decoded.frames(), original.len());
    for (exp, got) in original.iter().zip(decoded.interleaved.iter()) {
        assert!(
            (exp - got).abs() < 1e-3,
            "quantiza├º├úo 16-bit: {exp} vs {got}"
        );
    }
}

#[test]
fn decode_rejects_non_audio() {
    let err = decode_to_pcm(b"this is not audio at all").unwrap_err();
    assert!(
        format!("{err}").contains("formato"),
        "esperava erro de formato, obteve: {err}"
    );
}

#[test]
fn decode_rejects_empty() {
    let err = decode_to_pcm(&[]).unwrap_err();
    // N├úo deve panic ÔÇö formato desconhecido para bytes vazios.
    assert!(format!("{err}").contains("formato") || format!("{err}").contains("n├úo reconhecido"));
}

#[test]
fn downmono_of_mono_is_identity() {
    let audio = DecodedAudio {
        interleaved: vec![0.1, 0.2, 0.3, -0.1],
        channels: 1,
        sample_rate: 44100,
    };
    assert_eq!(downmix_to_mono(&audio).to_vec(), vec![0.1, 0.2, 0.3, -0.1]);
}

#[test]
fn duration_sec_is_frames_divided_by_rate() {
    // 100 frames a 50 Hz = 2.0 s
    let audio = DecodedAudio {
        interleaved: vec![0.0f32; 100],
        channels: 1,
        sample_rate: 50,
    };
    assert!((audio.duration_sec() - 2.0).abs() < 1e-6);
}

#[test]
fn stereo_frames_count_is_correct() {
    let audio = DecodedAudio {
        interleaved: vec![0.0f32; 6], // 3 frames ├ù 2 canais
        channels: 2,
        sample_rate: 44100,
    };
    assert_eq!(audio.frames(), 3);
}
