use crate::domain::{AudioCodec, AudioFingerprint, BeatBlock, PipelineConfig};
use crate::ports::AudioMixer;
use ndarray::{s, Array1};
use std::io::{self, Seek, Write};
use std::path::Path;

fn io_err(e: impl std::fmt::Display) -> crate::Error {
    crate::Error::Io(io::Error::other(e.to_string()))
}

pub struct DefaultMixer;

impl DefaultMixer {
    /// Mesma coisa que `export_wav`, mas para qualquer `Write + Seek` em vez
    /// de um caminho no disco — é aqui que as validações e a conversão de
    /// amostras moram de fato; `export_wav` só abre o arquivo e delega.
    ///
    /// Existe porque quem serve WAV por HTTP precisa dos bytes em memória
    /// (`Cursor<Vec<u8>>`), e escrever num arquivo temporário só para lê-lo
    /// de volta seria I/O inventado. Método inerente, não do trait: um
    /// método genérico quebraria a object safety de `dyn AudioMixer`.
    ///
    /// As duas restrições deliberadas continuam valendo, cada uma retornando
    /// erro em vez de escrever algo diferente do pedido silenciosamente (a
    /// mesma regra de `apply_lufs_gain`/`LufsGainOutcome`: nunca falha calado):
    ///
    /// - **Só `AudioCodec::WAV`.** MP3/AAC/FLAC declarados em
    ///   `AudioFormat::codec` não têm encoder em lugar nenhum do crate.
    /// - **Só `channels == 1`.** Nenhum estágio do pipeline (crossfade,
    ///   time_stretch, mastering) opera em áudio estéreo hoje — todos
    ///   recebem `Array1<f32>`/`&[f32]` mono. Escrever um cabeçalho de 2
    ///   canais para dados mono seria um WAV tecnicamente válido e
    ///   semanticamente errado (canais trocados/duplicados sem ninguém
    ///   pedir). `PipelineConfig::default()` declara `channels: 2` — quem
    ///   chamar com o default precisa sobrescrever para 1 explicitamente.
    pub fn encode_wav<W: Write + Seek>(
        &self,
        pcm: &Array1<f32>,
        writer: W,
        config: &PipelineConfig,
    ) -> Result<(), crate::Error> {
        if !matches!(config.format.codec, AudioCodec::WAV) {
            return Err(crate::Error::Validation(format!(
                "encode_wav só escreve WAV; codec pedido foi {:?}",
                config.format.codec
            )));
        }
        if config.format.channels != 1 {
            return Err(crate::Error::Validation(format!(
                "encode_wav só escreve mono hoje — nenhum estágio do pipeline \
                 processa estéreo; channels pedido foi {}",
                config.format.channels
            )));
        }

        let (bits_per_sample, sample_format) = match config.format.bit_depth {
            16 => (16, hound::SampleFormat::Int),
            24 => (24, hound::SampleFormat::Int),
            32 => (32, hound::SampleFormat::Float),
            other => {
                return Err(crate::Error::Validation(format!(
                    "encode_wav só suporta bit_depth 16, 24 ou 32; pedido foi {other}"
                )))
            }
        };

        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: config.format.sample_rate,
            bits_per_sample,
            sample_format,
        };

        let mut writer = hound::WavWriter::new(writer, spec).map_err(io_err)?;

        for &sample in pcm.iter() {
            let clamped = sample.clamp(-1.0, 1.0);
            let write_result = match bits_per_sample {
                16 => writer.write_sample((clamped * i16::MAX as f32) as i16),
                24 => writer.write_sample((clamped * 8_388_607.0) as i32),
                32 => writer.write_sample(clamped),
                _ => unreachable!("bits_per_sample já validado acima"),
            };
            write_result.map_err(io_err)?;
        }

        writer.finalize().map_err(io_err)
    }

    /// Conveniência para quem quer os bytes de um WAV em memória sem montar
    /// o `Cursor` na mão — o caso da rota de diagnóstico.
    pub fn encode_wav_to_vec(
        &self,
        pcm: &Array1<f32>,
        config: &PipelineConfig,
    ) -> Result<Vec<u8>, crate::Error> {
        let mut buf = io::Cursor::new(Vec::new());
        self.encode_wav(pcm, &mut buf, config)?;
        Ok(buf.into_inner())
    }
}

impl AudioMixer for DefaultMixer {
    fn render_stitched(
        &self,
        blocks: &[BeatBlock],
        pcm_source: &Array1<f32>,
        _config: &PipelineConfig,
    ) -> Array1<f32> {
        // Placeholder: concatena os blocos sequencialmente
        // Na prática: aplica crossfade, fades, time-stretch
        let mut output = Vec::new();
        for block in blocks {
            if block.end_sample <= pcm_source.len() && block.start_sample < block.end_sample {
                let block_pcm = pcm_source.slice(s![block.start_sample..block.end_sample]);
                output.extend_from_slice(block_pcm.as_slice().unwrap_or(&[]));
            }
        }
        Array1::from_vec(output)
    }

    /// Escreve `pcm` (mono) como WAV em `path`. Abre o arquivo e delega para
    /// [`DefaultMixer::encode_wav`], onde ficam as validações e a conversão
    /// de amostras — as mensagens de erro citam `encode_wav` por isso.
    fn export_wav(
        &self,
        pcm: &Array1<f32>,
        path: &Path,
        config: &PipelineConfig,
    ) -> Result<(), crate::Error> {
        // `BufWriter` porque `encode_wav` escreve amostra a amostra; sem ele
        // uma faixa de minutos vira milhões de `write` de 4 bytes no disco.
        let file = io::BufWriter::new(std::fs::File::create(path)?);
        self.encode_wav(pcm, file, config)
    }

    fn measure_similarity(
        &self,
        fingerprint_a: &AudioFingerprint,
        fingerprint_b: &AudioFingerprint,
    ) -> f32 {
        fingerprint_a.distance(fingerprint_b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn block(start: usize, end: usize) -> BeatBlock {
        BeatBlock {
            id: Uuid::new_v4(),
            start_sample: start,
            end_sample: end,
            start_time: start as f32 / 44100.0,
            end_time: end as f32 / 44100.0,
            duration: (end - start) as f32 / 44100.0,
            rms_energy: 0.1,
            spectral_centroid: 0.0,
            chroma_vector: None,
            beat_index: 0,
            score: 0.1,
        }
    }

    #[test]
    fn test_render_stitched_concatenates_blocks() {
        let pcm = Array1::from_vec((0..1000).map(|i| i as f32).collect());
        let blocks = vec![block(0, 100), block(200, 300)];
        let mixer = DefaultMixer;
        let out = mixer.render_stitched(&blocks, &pcm, &PipelineConfig::default());
        assert_eq!(out.len(), 200);
    }

    /// `PipelineConfig::default()` declara `channels: 2` — não é o config que
    /// `export_wav` aceita, é o config para o resto do pipeline. Um teste que
    /// usasse o default direto estaria testando o caminho de erro sem
    /// perceber; monta explicitamente o config mono que os testes de escrita
    /// real precisam.
    fn mono_wav_config(sample_rate: u32, bit_depth: u8) -> PipelineConfig {
        let mut config = PipelineConfig::default();
        config.format.sample_rate = sample_rate;
        config.format.channels = 1;
        config.format.bit_depth = bit_depth;
        config.format.codec = AudioCodec::WAV;
        config
    }

    fn ler_wav_mono_f32(path: &Path) -> Vec<f32> {
        let mut reader = hound::WavReader::open(path).unwrap();
        let spec = reader.spec();
        match spec.sample_format {
            hound::SampleFormat::Float => reader.samples::<f32>().map(|s| s.unwrap()).collect(),
            hound::SampleFormat::Int => {
                let max_val = (1i64 << (spec.bits_per_sample - 1)) as f32;
                reader
                    .samples::<i32>()
                    .map(|s| s.unwrap() as f32 / max_val)
                    .collect()
            }
        }
    }

    #[test]
    fn test_export_wav_roundtrips_32bit_float() {
        let dir = std::env::temp_dir().join(format!("mixlirous_test_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("saida.wav");

        let pcm = Array1::from_vec(vec![0.0f32, 0.5, -0.5, 1.0, -1.0]);
        let mixer = DefaultMixer;
        mixer
            .export_wav(&pcm, &path, &mono_wav_config(44100, 32))
            .unwrap();

        let lido = ler_wav_mono_f32(&path);
        assert_eq!(lido, pcm.to_vec());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_export_wav_16bit_roundtrips_within_quantization_tolerance() {
        let dir = std::env::temp_dir().join(format!("mixlirous_test_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("saida.wav");

        let pcm = Array1::from_vec(vec![0.0f32, 0.5, -0.5, 0.999, -0.999]);
        let mixer = DefaultMixer;
        mixer
            .export_wav(&pcm, &path, &mono_wav_config(44100, 16))
            .unwrap();

        let lido = ler_wav_mono_f32(&path);
        assert_eq!(lido.len(), pcm.len());
        for (esperado, obtido) in pcm.iter().zip(lido.iter()) {
            // Quantização de 16 bits: passo de 1/32767 ≈ 3e-5; tolerância
            // generosa (1e-3) só para não prender o teste no arredondamento
            // exato de um bit específico.
            assert!(
                (esperado - obtido).abs() < 1e-3,
                "esperado {esperado}, obtido {obtido}"
            );
        }

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_export_wav_rejects_non_wav_codec() {
        let dir = std::env::temp_dir().join(format!("mixlirous_test_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("saida.wav");

        let mut config = mono_wav_config(44100, 32);
        config.format.codec = AudioCodec::MP3;
        let mixer = DefaultMixer;
        let err = mixer
            .export_wav(&Array1::from_vec(vec![0.0f32]), &path, &config)
            .unwrap_err();
        assert!(err.to_string().contains("WAV"));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_export_wav_rejects_stereo_channels() {
        // A restrição que existe hoje só porque nenhum estágio do pipeline
        // processa estéreo — não um limite arbitrário de `export_wav`.
        let dir = std::env::temp_dir().join(format!("mixlirous_test_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("saida.wav");

        let config = PipelineConfig::default(); // channels: 2, sem override
        let mixer = DefaultMixer;
        let err = mixer
            .export_wav(&Array1::from_vec(vec![0.0f32]), &path, &config)
            .unwrap_err();
        assert!(err.to_string().contains("mono"));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_export_wav_rejects_unsupported_bit_depth() {
        let dir = std::env::temp_dir().join(format!("mixlirous_test_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("saida.wav");

        let mixer = DefaultMixer;
        let err = mixer
            .export_wav(
                &Array1::from_vec(vec![0.0f32]),
                &path,
                &mono_wav_config(44100, 8),
            )
            .unwrap_err();
        assert!(err.to_string().contains("bit_depth"));

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
