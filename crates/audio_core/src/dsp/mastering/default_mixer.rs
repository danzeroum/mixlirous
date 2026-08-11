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
    /// de um caminho no disco ÔÇö ├® aqui que as valida├º├Áes e a convers├úo de
    /// amostras moram de fato; `export_wav` s├│ abre o arquivo e delega.
    ///
    /// Existe porque quem serve WAV por HTTP precisa dos bytes em mem├│ria
    /// (`Cursor<Vec<u8>>`), e escrever num arquivo tempor├írio s├│ para l├¬-lo
    /// de volta seria I/O inventado. M├®todo inerente, n├úo do trait: um
    /// m├®todo gen├®rico quebraria a object safety de `dyn AudioMixer`.
    ///
    /// As duas restri├º├Áes deliberadas continuam valendo, cada uma retornando
    /// erro em vez de escrever algo diferente do pedido silenciosamente (a
    /// mesma regra de `apply_lufs_gain`/`LufsGainOutcome`: nunca falha calado):
    ///
    /// - **S├│ `AudioCodec::WAV`.** MP3/AAC/FLAC declarados em
    ///   `AudioFormat::codec` n├úo t├¬m encoder em lugar nenhum do crate.
    /// - **S├│ `channels == 1`.** Nenhum est├ígio do pipeline (crossfade,
    ///   time_stretch, mastering) opera em ├íudio est├®reo hoje ÔÇö todos
    ///   recebem `Array1<f32>`/`&[f32]` mono. Escrever um cabe├ºalho de 2
    ///   canais para dados mono seria um WAV tecnicamente v├ílido e
    ///   semanticamente errado (canais trocados/duplicados sem ningu├®m
    ///   pedir). `PipelineConfig::default()` declara `channels: 2` ÔÇö quem
    ///   chamar com o default precisa sobrescrever para 1 explicitamente.
    pub fn encode_wav<W: Write + Seek>(
        &self,
        pcm: &Array1<f32>,
        writer: W,
        config: &PipelineConfig,
    ) -> Result<(), crate::Error> {
        if !matches!(config.format.codec, AudioCodec::WAV) {
            return Err(crate::Error::Validation(format!(
                "encode_wav s├│ escreve WAV; codec pedido foi {:?}",
                config.format.codec
            )));
        }
        if config.format.channels != 1 {
            return Err(crate::Error::Validation(format!(
                "encode_wav s├│ escreve mono hoje ÔÇö nenhum est├ígio do pipeline \
                 processa est├®reo; channels pedido foi {}",
                config.format.channels
            )));
        }

        let (bits_per_sample, sample_format) = match config.format.bit_depth {
            16 => (16, hound::SampleFormat::Int),
            24 => (24, hound::SampleFormat::Int),
            32 => (32, hound::SampleFormat::Float),
            other => {
                return Err(crate::Error::Validation(format!(
                    "encode_wav s├│ suporta bit_depth 16, 24 ou 32; pedido foi {other}"
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
            let write_result = match bits_per_sample {
                // PCM inteiro n├úo tem representa├º├úo acima de fundo de escala,
                // ent├úo limitar aqui ├® obrigat├│rio ÔÇö o problema ├® faz├¬-lo em
                // sil├¬ncio, e isso continua aberto (issue #37): o certo ├®
                // contar as amostras limitadas e devolver o n├║mero, para
                // virar aviso. Enquanto a assinatura n├úo carrega isso, ao
                // menos o limite fica expl├¡cito e n├úo confundido com o caso
                // do float.
                16 => writer.write_sample((sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16),
                24 => writer.write_sample((sample.clamp(-1.0, 1.0) * 8_388_607.0) as i32),
                // WAV float de 32 bits representa acima de ┬▒1,0 sem problema
                // ÔÇö ├® justamente para isso que se usa float como formato
                // intermedi├írio. Limitar aqui era perda pura de informa├º├úo, e
                // ironicamente fazia do ├║nico formato que preserva margem o
                // ├║nico onde ela era descartada calada.
                32 => writer.write_sample(sample),
                _ => unreachable!("bits_per_sample j├í validado acima"),
            };
            write_result.map_err(io_err)?;
        }

        writer.finalize().map_err(io_err)
    }

    /// Conveni├¬ncia para quem quer os bytes de um WAV em mem├│ria sem montar
    /// o `Cursor` na m├úo ÔÇö o caso da rota de diagn├│stico.
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
        // Na pr├ítica: aplica crossfade, fades, time-stretch
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
    /// [`DefaultMixer::encode_wav`], onde ficam as valida├º├Áes e a convers├úo
    /// de amostras ÔÇö as mensagens de erro citam `encode_wav` por isso.
    fn export_wav(
        &self,
        pcm: &Array1<f32>,
        path: &Path,
        config: &PipelineConfig,
    ) -> Result<(), crate::Error> {
        // `BufWriter` porque `encode_wav` escreve amostra a amostra; sem ele
        // uma faixa de minutos vira milh├Áes de `write` de 4 bytes no disco.
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

    /// `PipelineConfig::default()` declara `channels: 2` ÔÇö n├úo ├® o config que
    /// `export_wav` aceita, ├® o config para o resto do pipeline. Um teste que
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
            // Quantiza├º├úo de 16 bits: passo de 1/32767 Ôëê 3e-5; toler├óncia
            // generosa (1e-3) s├│ para n├úo prender o teste no arredondamento
            // exato de um bit espec├¡fico.
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
        // A restri├º├úo que existe hoje s├│ porque nenhum est├ígio do pipeline
        // processa est├®reo ÔÇö n├úo um limite arbitr├írio de `export_wav`.
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

    /// Float de 32 bits preserva margem acima de ┬▒1,0 ÔÇö ├® o motivo de existir
    /// como formato intermedi├írio. O `clamp` que havia aqui destru├¡a isso em
    /// sil├¬ncio (ver a discuss├úo no #37).
    #[test]
    fn export_wav_float32_preserva_amostras_acima_de_fundo_de_escala() {
        let dir = std::env::temp_dir().join(format!("mixlirous_test_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("saida.wav");

        // 6.5 ├® a ordem de grandeza real medida no buffer intermedi├írio do
        // pipeline depois de `apply_lufs_gain` em material percussivo (#37).
        let pcm = Array1::from_vec(vec![6.5f32, -6.5, 1.0, -1.0, 0.0]);
        DefaultMixer
            .export_wav(&pcm, &path, &mono_wav_config(44100, 32))
            .unwrap();

        let lido = ler_wav_mono_f32(&path);
        assert_eq!(lido, pcm.to_vec(), "float de 32 bits n├úo pode limitar");

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// PCM inteiro n├úo tem representa├º├úo acima de fundo de escala: limitar ├®
    /// obrigat├│rio. O que falta ├® contar e reportar (#37), n├úo deixar de
    /// limitar.
    #[test]
    fn export_wav_inteiro_ainda_limita_por_falta_de_representacao() {
        let dir = std::env::temp_dir().join(format!("mixlirous_test_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("saida.wav");

        let pcm = Array1::from_vec(vec![6.5f32, -6.5, 0.5]);
        DefaultMixer
            .export_wav(&pcm, &path, &mono_wav_config(44100, 16))
            .unwrap();

        let lido = ler_wav_mono_f32(&path);
        assert!(lido[0] > 0.99 && lido[0] <= 1.0, "saturou em {}", lido[0]);
        assert!(lido[1] < -0.99 && lido[1] >= -1.0, "saturou em {}", lido[1]);
        assert!(
            (lido[2] - 0.5).abs() < 1e-3,
            "n├úo deveria tocar: {}",
            lido[2]
        );

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
