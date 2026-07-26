//! Tabela canônica de limites de parâmetros — espelha
//! `docs/05-AGENTE-IA-HITL.md` §3. Exposta para `GET /api/v1/tools` (a UI lê
//! os limites daqui em vez de hardcodar `max: 3000`).
//!
//! Os números aqui e os de `validator.rs` precisam bater; o teste no fundo
//! deste arquivo falha se alguém mudar um sem o outro.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ParamLimit {
    pub name: &'static str,
    #[serde(rename = "type")]
    pub type_name: &'static str,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub default: Option<serde_json::Value>,
    #[serde(rename = "enum", skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<&'static [&'static str]>,
    pub unit: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolLimits {
    pub name: &'static str,
    pub label_ptbr: &'static str,
    pub category: &'static str,
    pub available: bool,
    /// `available` reflete a existência do DSP, não a existência do schema.
    /// Um parâmetro validado e exposto aqui sem nenhum código que o
    /// consuma é a mesma classe de bug que a divergência
    /// validador/registry — só que na direção "promete mais do que existe"
    /// em vez de "promete menos". `docs/03-CONTRATOS-API.md` §3.7 documenta
    /// os códigos (`"not_implemented"`, `"requires_plan_pro"`, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<&'static str>,
    pub parameters: Vec<ParamLimit>,
}

fn p(
    name: &'static str,
    type_name: &'static str,
    min: Option<f64>,
    max: Option<f64>,
    default: Option<serde_json::Value>,
    unit: Option<&'static str>,
) -> ParamLimit {
    ParamLimit {
        name,
        type_name,
        min,
        max,
        default,
        enum_values: None,
        unit,
    }
}

fn e(
    name: &'static str,
    values: &'static [&'static str],
    default: Option<serde_json::Value>,
) -> ParamLimit {
    ParamLimit {
        name,
        type_name: "enum",
        min: None,
        max: None,
        default,
        enum_values: Some(values),
        unit: None,
    }
}

/// Array cujos itens vêm de um enum fechado, com contagem mínima/máxima
/// (ex.: `stem_separation.stems`: 1 a 4 itens, cada um de `VALID_STEMS`).
fn array_enum(
    name: &'static str,
    values: &'static [&'static str],
    min_items: f64,
    max_items: f64,
    default: Option<serde_json::Value>,
) -> ParamLimit {
    ParamLimit {
        name,
        type_name: "array_enum",
        min: Some(min_items),
        max: Some(max_items),
        default,
        enum_values: Some(values),
        unit: None,
    }
}

/// Registry de ferramentas com limites, na forma exposta por `GET /api/v1/tools`.
pub fn tool_registry() -> Vec<ToolLimits> {
    vec![
        ToolLimits {
            name: "compression",
            label_ptbr: "Compressão",
            category: "mastering",
            // Sem módulo de DSP (nenhum arquivo em audio_core::dsp implementa
            // compressor) — o schema existe e o validador aceita, mas
            // nenhum código lê os parâmetros. `available: true` aqui seria a
            // ferramenta fantasma que a ADR-0010 (docs/adr/README.md) já
            // resolve corretamente para stem_separation.
            available: false,
            unavailable_reason: Some("not_implemented"),
            parameters: vec![
                p(
                    "ratio",
                    "float",
                    // T0.0 (docs/16): estes dois números vêm do newtype
                    // audio_core::CompressionRatio — ver o comentário em
                    // crossfade.duration_ms mais abaixo para o racional.
                    Some(audio_core::CompressionRatio::MIN as f64),
                    Some(audio_core::CompressionRatio::MAX as f64),
                    Some(2.0.into()),
                    Some(":1"),
                ),
                p(
                    "threshold_db",
                    "float",
                    Some(audio_core::ThresholdDb::MIN as f64),
                    Some(audio_core::ThresholdDb::MAX as f64),
                    Some((-18.0).into()),
                    Some("dB"),
                ),
                p(
                    "attack_ms",
                    "integer",
                    Some(audio_core::AttackMs::MIN as f64),
                    Some(audio_core::AttackMs::MAX as f64),
                    Some(30.0.into()),
                    Some("ms"),
                ),
                p(
                    "release_ms",
                    "integer",
                    Some(audio_core::ReleaseMs::MIN as f64),
                    Some(audio_core::ReleaseMs::MAX as f64),
                    Some(250.0.into()),
                    Some("ms"),
                ),
                p(
                    "makeup_gain_db",
                    "float",
                    Some(-12.0),
                    Some(12.0),
                    Some(0.0.into()),
                    Some("dB"),
                ),
                p(
                    "knee_db",
                    "float",
                    Some(0.0),
                    Some(12.0),
                    Some(6.0.into()),
                    Some("dB"),
                ),
            ],
        },
        ToolLimits {
            name: "dynamic_eq",
            label_ptbr: "EQ dinâmico",
            category: "mastering",
            // Mesma situação de `compression`: sem módulo de DSP sob
            // audio_core::dsp, mesmo com schema e validação completos.
            available: false,
            unavailable_reason: Some("not_implemented"),
            parameters: vec![
                p(
                    "bands[].freq_hz",
                    "float",
                    Some(20.0),
                    Some(20000.0),
                    None,
                    Some("Hz"),
                ),
                p(
                    "bands[].gain_db",
                    "float",
                    Some(audio_core::EqGainDb::MIN as f64),
                    Some(audio_core::EqGainDb::MAX as f64),
                    Some(0.0.into()),
                    Some("dB"),
                ),
                p(
                    "bands[].q",
                    "float",
                    Some(0.1),
                    Some(10.0),
                    Some(0.7.into()),
                    None,
                ),
                e(
                    "bands[].type_filter",
                    VALID_EQ_FILTER_TYPES,
                    Some("peak".into()),
                ),
                p("bands", "array", Some(1.0), Some(8.0), None, None),
            ],
        },
        ToolLimits {
            name: "crossfade",
            label_ptbr: "Transição",
            category: "stitching",
            available: true,
            unavailable_reason: None,
            parameters: vec![
                p(
                    "duration_ms",
                    "integer",
                    // T0.0 (docs/16): estes dois números vêm do newtype
                    // audio_core::CrossfadeMs, não são redigitados aqui — um
                    // teste de deriva (fundo deste arquivo) prende os dois
                    // juntos, então não há como o registry e o tipo
                    // divergirem silenciosamente.
                    Some(audio_core::CrossfadeMs::MIN as f64),
                    Some(audio_core::CrossfadeMs::MAX as f64),
                    Some(1000.0.into()),
                    Some("ms"),
                ),
                e(
                    "curve",
                    VALID_CROSSFADE_CURVES,
                    Some("constant_power".into()),
                ),
            ],
        },
        ToolLimits {
            name: "fade_in",
            label_ptbr: "Fade in",
            category: "stitching",
            available: true,
            unavailable_reason: None,
            parameters: vec![
                p(
                    "duration_ms",
                    "integer",
                    Some(0.0),
                    Some(10000.0),
                    Some(1000.0.into()),
                    Some("ms"),
                ),
                e("curve", VALID_FADE_CURVES, Some("logarithmic".into())),
            ],
        },
        ToolLimits {
            name: "fade_out",
            label_ptbr: "Fade out",
            category: "stitching",
            available: true,
            unavailable_reason: None,
            parameters: vec![
                p(
                    "duration_ms",
                    "integer",
                    Some(0.0),
                    Some(10000.0),
                    Some(1000.0.into()),
                    Some("ms"),
                ),
                e("curve", VALID_FADE_CURVES, Some("logarithmic".into())),
            ],
        },
        ToolLimits {
            name: "time_stretch",
            label_ptbr: "Ajuste de duração",
            category: "mastering",
            available: true,
            unavailable_reason: None,
            parameters: vec![p(
                "factor",
                "float",
                Some(audio_core::TimeStretchFactor::MIN as f64),
                Some(audio_core::TimeStretchFactor::MAX as f64),
                Some(1.0.into()),
                Some("×"),
            )],
        },
        ToolLimits {
            name: "lufs_normalization",
            label_ptbr: "Normalização LUFS",
            category: "mastering",
            available: true,
            unavailable_reason: None,
            parameters: vec![
                p(
                    "target_lufs",
                    "float",
                    Some(audio_core::LufsTarget::MIN as f64),
                    Some(audio_core::LufsTarget::MAX as f64),
                    Some((-14.0).into()),
                    Some("LUFS"),
                ),
                p(
                    "max_true_peak_db",
                    "float",
                    Some(-6.0),
                    Some(0.0),
                    Some((-1.0).into()),
                    Some("dBTP"),
                ),
            ],
        },
        ToolLimits {
            name: "stem_separation",
            label_ptbr: "Separação de stems",
            category: "analysis",
            // ADR-0010 pendente — ver docs/adr/README.md. `model` ainda é
            // lista fixa (VALID_STEM_MODELS); deveria vir do binário
            // detectado, não do código (prioridade baixa enquanto a
            // ferramenta estiver indisponível, mas não pode passar da
            // Sprint 3 sem virar detecção real).
            available: false,
            unavailable_reason: Some("not_implemented"),
            parameters: vec![
                e("model", VALID_STEM_MODELS, Some("htdemucs".into())),
                array_enum(
                    "stems",
                    VALID_STEMS,
                    1.0,
                    4.0,
                    Some(serde_json::json!(["drums", "other"])),
                ),
            ],
        },
    ]
}

// Adendo R2 §0: crossfade (dois sinais somando) e fade_in/fade_out (um sinal
// de/para o silêncio) são conceitos diferentes e têm enums distintos —
// contrato, validador e registry, os três. `crossfade` nunca aceitou
// "linear"/"exponential" de verdade; a matemática de potência/ganho
// constante (docs/16 T2.2) também já está em
// dsp::stitching::crossfade::crossfade_buffers() — não sobra mais operando
// sobre o FadeCurve antigo por baixo.
pub const VALID_CROSSFADE_CURVES: &[&str] = &["constant_power", "constant_gain"];
pub const VALID_FADE_CURVES: &[&str] = &["linear", "logarithmic", "exponential"];
pub const VALID_STEM_MODELS: &[&str] = &["htdemucs", "htdemucs_ft"];
pub const VALID_STEMS: &[&str] = &["drums", "bass", "vocals", "other"];
pub const VALID_EQ_FILTER_TYPES: &[&str] = &["peak", "shelf", "highpass", "lowpass"];

/// Gera a tabela markdown de `docs/05-AGENTE-IA-HITL.md` §3 a partir do
/// registry — a fonte é `tool_registry()`, a tabela é projeção. Um teste no
/// fundo deste arquivo compara este output contra o bloco marcado no arquivo
/// de doc; se divergirem, o teste falha e diz para regenerar. Só cobre as
/// ferramentas de `GET /api/v1/tools`: `block_selection` e `target_duration`
/// (campos de `pipeline_config`, não uma entrada de tool_registry) ficam de
/// fora de propósito — ver nota logo abaixo do bloco gerado em docs/05.
pub fn render_markdown_table() -> String {
    let mut out = String::from(
        "| Ferramenta | Disponível | Parâmetro | Tipo | Mín | Máx | Padrão | Unidade/Enum |\n",
    );
    out.push_str("| --- | --- | --- | --- | --- | --- | --- | --- |\n");

    for tool in tool_registry() {
        let availability = if tool.available {
            "sim".to_string()
        } else {
            format!(
                "não ({})",
                tool.unavailable_reason.unwrap_or("motivo não registrado")
            )
        };

        if tool.parameters.is_empty() {
            out.push_str(&format!(
                "| `{}` | {} | — | — | — | — | — | — |\n",
                tool.name, availability
            ));
            continue;
        }

        for (i, param) in tool.parameters.iter().enumerate() {
            let tool_col = if i == 0 {
                format!("`{}`", tool.name)
            } else {
                String::new()
            };
            let avail_col = if i == 0 {
                availability.clone()
            } else {
                String::new()
            };
            let min = param
                .min
                .map(format_number)
                .unwrap_or_else(|| "—".to_string());
            let max = param
                .max
                .map(format_number)
                .unwrap_or_else(|| "—".to_string());
            let default = param
                .default
                .as_ref()
                .map(|v| v.to_string())
                .unwrap_or_else(|| "—".to_string());
            let unit_or_enum = match param.enum_values {
                Some(values) => values.join(" \\| "),
                None => param.unit.unwrap_or("—").to_string(),
            };

            out.push_str(&format!(
                "| {} | {} | `{}` | {} | {} | {} | {} | {} |\n",
                tool_col, avail_col, param.name, param.type_name, min, max, default, unit_or_enum
            ));
        }
    }

    out
}

fn format_number(v: f64) -> String {
    // Bounds de origem `f32` (a maioria dos newtypes T0.0) carregam ruído de
    // arredondamento invisível até o cast para f64: `TimeStretchFactor::MIN`
    // (0.90_f32) as f64 é 0.8999999761581421, não 0.9 — a mesma fração não
    // representa igual nos dois formatos. Arredonda antes de formatar para
    // não vazar esse ruído pra tabela gerada; 6 casas cobre toda a precisão
    // que qualquer parâmetro deste registry usa de propósito.
    let arredondado = (v * 1e6).round() / 1e6;
    if arredondado.fract() == 0.0 {
        format!("{arredondado:.0}")
    } else {
        format!("{arredondado:.6}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::*;
    use crate::validator::ValidationLayer;
    use serde_json::Value;

    const DOCS_05: &str = include_str!("../../../docs/05-AGENTE-IA-HITL.md");
    const BEGIN_MARKER: &str = "<!-- BEGIN GENERATED TOOLS TABLE";
    const END_MARKER: &str = "<!-- END GENERATED TOOLS TABLE -->";

    /// Extrai o bloco entre os marcadores e normaliza fim de linha.
    /// `include_str!` preserva o fim de linha do arquivo em disco — no
    /// runner Windows do CI isso é `\r\n` (checkout do git normaliza),
    /// enquanto `render_markdown_table()` sempre gera `\n`. Sem normalizar,
    /// a comparação falha em CRLF mesmo com conteúdo idêntico — não é
    /// divergência real, é diferença de fim de linha entre SOs.
    fn extract_generated_block(doc: &str) -> &str {
        let begin = doc
            .find(BEGIN_MARKER)
            .expect("marcador BEGIN não encontrado em docs/05-AGENTE-IA-HITL.md");
        let begin_line_end = doc[begin..]
            .find('\n')
            .map(|i| begin + i + 1)
            .expect("marcador BEGIN sem quebra de linha");
        let end = doc
            .find(END_MARKER)
            .expect("marcador END não encontrado em docs/05-AGENTE-IA-HITL.md");
        doc[begin_line_end..end].trim_end()
    }

    /// A tabela §3 de docs/05 é gerada a partir do registry, não mantida à
    /// mão — este teste é o que impede as duas de divergirem de novo (a
    /// causa raiz do adendo R2 estar desatualizado: foi escrito lendo o kit
    /// original, não a `main`). Se este teste falhar, rode
    /// `render_markdown_table()` (ex.: via um teste com `--nocapture`) e
    /// cole o resultado entre os marcadores no arquivo de doc.
    #[test]
    fn test_docs_05_table_matches_registry() {
        let committed = extract_generated_block(DOCS_05).replace("\r\n", "\n");
        let generated = render_markdown_table();
        let generated = generated.trim_end();

        assert_eq!(
            committed, generated,
            "\n\ndocs/05 §3 divergiu do registry. Cole isto entre os marcadores:\n\n{generated}\n"
        );
    }

    /// Prova a normalização de CRLF isolada do arquivo real — este sandbox
    /// roda Linux, então o bug (CI falhando só no runner Windows) não
    /// reproduz aqui sem simular o \r\n manualmente. Sem este teste, a
    /// correção do CRLF só seria verificada de novo no próximo push ao CI.
    #[test]
    fn test_extract_generated_block_normalizes_crlf() {
        let doc_unix =
            format!("prefixo\n{BEGIN_MARKER} ver x)\nlinha 1\nlinha 2\n{END_MARKER}\nsufixo\n");
        let doc_windows = doc_unix.replace('\n', "\r\n");

        let unix_block = extract_generated_block(&doc_unix);
        let windows_block = extract_generated_block(&doc_windows).replace("\r\n", "\n");

        assert_eq!(unix_block, windows_block);
        assert_eq!(unix_block, "linha 1\nlinha 2");
    }

    /// Achado ao migrar `time_stretch.factor` para `TimeStretchFactor`
    /// (T0.0): `0.90_f32 as f64` é `0.8999999761581421`, não `0.9` — o
    /// mesmo valor não representa igual nos dois formatos, e sem
    /// arredondar antes de formatar a tabela gerada carrega esse ruído.
    #[test]
    fn test_format_number_rounds_away_f32_cast_noise() {
        assert_eq!(
            format_number(audio_core::TimeStretchFactor::MIN as f64),
            "0.9"
        );
        assert_eq!(
            format_number(audio_core::TimeStretchFactor::MAX as f64),
            "1.1"
        );
    }

    fn find<'a>(reg: &'a [ToolLimits], tool: &str) -> &'a ToolLimits {
        reg.iter()
            .find(|t| t.name == tool)
            .unwrap_or_else(|| panic!("tool {tool} não está no registry"))
    }

    fn param<'a>(tool: &'a ToolLimits, name: &str) -> &'a ParamLimit {
        tool.parameters
            .iter()
            .find(|p| p.name == name)
            .unwrap_or_else(|| panic!("param {name} não está em {}", tool.name))
    }

    /// T0.0: o registry não redigita os números de `CrossfadeMs` — lê a
    /// constante direto (`p(..., Some(audio_core::CrossfadeMs::MIN as f64), ...)`
    /// acima). Este teste não pega uma cópia divergindo — pega alguém que
    /// troque a leitura da constante por um literal solto de novo, o que
    /// reabriria exatamente o "terceiro lugar" que T0.0 fecha.
    #[test]
    fn test_crossfade_duration_registry_matches_crossfade_ms_newtype() {
        let reg = tool_registry();
        let param = param(find(&reg, "crossfade"), "duration_ms");

        assert_eq!(param.min, Some(audio_core::CrossfadeMs::MIN as f64));
        assert_eq!(param.max, Some(audio_core::CrossfadeMs::MAX as f64));
    }

    /// T0.0: mesma checagem de deriva de `test_crossfade_duration_registry_matches_crossfade_ms_newtype`,
    /// aplicada aos 7 newtypes restantes que têm entrada no registry
    /// (`BlockSizeBeats` e `Percentile` ficam de fora — não são parâmetro de
    /// tool call, ver `docs/05-AGENTE-IA-HITL.md` §3).
    #[test]
    fn test_compression_ratio_registry_matches_newtype() {
        let reg = tool_registry();
        let param = param(find(&reg, "compression"), "ratio");
        assert_eq!(param.min, Some(audio_core::CompressionRatio::MIN as f64));
        assert_eq!(param.max, Some(audio_core::CompressionRatio::MAX as f64));
    }

    #[test]
    fn test_threshold_db_registry_matches_newtype() {
        let reg = tool_registry();
        let param = param(find(&reg, "compression"), "threshold_db");
        assert_eq!(param.min, Some(audio_core::ThresholdDb::MIN as f64));
        assert_eq!(param.max, Some(audio_core::ThresholdDb::MAX as f64));
    }

    #[test]
    fn test_attack_ms_registry_matches_newtype() {
        let reg = tool_registry();
        let param = param(find(&reg, "compression"), "attack_ms");
        assert_eq!(param.min, Some(audio_core::AttackMs::MIN as f64));
        assert_eq!(param.max, Some(audio_core::AttackMs::MAX as f64));
    }

    #[test]
    fn test_release_ms_registry_matches_newtype() {
        let reg = tool_registry();
        let param = param(find(&reg, "compression"), "release_ms");
        assert_eq!(param.min, Some(audio_core::ReleaseMs::MIN as f64));
        assert_eq!(param.max, Some(audio_core::ReleaseMs::MAX as f64));
    }

    #[test]
    fn test_eq_gain_db_registry_matches_newtype() {
        let reg = tool_registry();
        let param = param(find(&reg, "dynamic_eq"), "bands[].gain_db");
        assert_eq!(param.min, Some(audio_core::EqGainDb::MIN as f64));
        assert_eq!(param.max, Some(audio_core::EqGainDb::MAX as f64));
    }

    #[test]
    fn test_time_stretch_factor_registry_matches_newtype() {
        let reg = tool_registry();
        let param = param(find(&reg, "time_stretch"), "factor");
        assert_eq!(param.min, Some(audio_core::TimeStretchFactor::MIN as f64));
        assert_eq!(param.max, Some(audio_core::TimeStretchFactor::MAX as f64));
    }

    #[test]
    fn test_lufs_target_registry_matches_newtype() {
        let reg = tool_registry();
        let param = param(find(&reg, "lufs_normalization"), "target_lufs");
        assert_eq!(param.min, Some(audio_core::LufsTarget::MIN as f64));
        assert_eq!(param.max, Some(audio_core::LufsTarget::MAX as f64));
    }

    /// Garante que o teto de `crossfade.duration_ms` no registry é
    /// exatamente o que o validador aceita — pega divergência silenciosa
    /// entre a UI (que lê este registry) e o Rust (que valida de fato).
    #[test]
    fn test_crossfade_duration_registry_matches_validator() {
        let reg = tool_registry();
        let max = param(find(&reg, "crossfade"), "duration_ms").max.unwrap() as u32;

        let layer = ValidationLayer::new();
        let at_max = AudioToolDef::Crossfade(CrossfadeParams {
            duration_ms: max,
            curve: "constant_power".to_string(),
        });
        let over_max = AudioToolDef::Crossfade(CrossfadeParams {
            duration_ms: max + 1,
            curve: "constant_power".to_string(),
        });

        assert!(layer.validate_tool_call(&at_max, &Value::Null).is_ok());
        assert!(layer.validate_tool_call(&over_max, &Value::Null).is_err());
    }

    #[test]
    fn test_compression_ratio_registry_matches_validator() {
        let reg = tool_registry();
        let compression = find(&reg, "compression");
        let min = param(compression, "ratio").min.unwrap() as f32;
        let max = param(compression, "ratio").max.unwrap() as f32;

        let layer = ValidationLayer::new();
        let base = CompressionParams {
            ratio: 0.0,
            threshold_db: -18.0,
            attack_ms: 30,
            release_ms: 250,
            makeup_gain_db: 0.0,
            knee_db: 6.0,
        };

        let at_min = AudioToolDef::Compression(CompressionParams {
            ratio: min,
            ..base.clone()
        });
        let at_max = AudioToolDef::Compression(CompressionParams {
            ratio: max,
            ..base.clone()
        });
        let over_max = AudioToolDef::Compression(CompressionParams {
            ratio: max + 0.1,
            ..base
        });

        assert!(layer.validate_tool_call(&at_min, &Value::Null).is_ok());
        assert!(layer.validate_tool_call(&at_max, &Value::Null).is_ok());
        assert!(layer.validate_tool_call(&over_max, &Value::Null).is_err());
    }

    #[test]
    fn test_time_stretch_factor_registry_matches_validator() {
        let reg = tool_registry();
        let min = param(find(&reg, "time_stretch"), "factor").min.unwrap() as f32;
        let max = param(find(&reg, "time_stretch"), "factor").max.unwrap() as f32;

        let layer = ValidationLayer::new();
        assert!(layer
            .validate_tool_call(
                &AudioToolDef::TimeStretch(TimeStretchParams { factor: min }),
                &Value::Null
            )
            .is_ok());
        assert!(layer
            .validate_tool_call(
                &AudioToolDef::TimeStretch(TimeStretchParams { factor: max }),
                &Value::Null
            )
            .is_ok());
        assert!(layer
            .validate_tool_call(
                &AudioToolDef::TimeStretch(TimeStretchParams { factor: max + 0.01 }),
                &Value::Null
            )
            .is_err());
    }

    #[test]
    fn test_registry_has_all_audio_tool_def_variants() {
        let reg = tool_registry();
        let names: Vec<&str> = reg.iter().map(|t| t.name).collect();
        for expected in [
            "compression",
            "dynamic_eq",
            "crossfade",
            "fade_in",
            "fade_out",
            "time_stretch",
            "lufs_normalization",
            "stem_separation",
        ] {
            assert!(
                names.contains(&expected),
                "registry não descreve a ferramenta {expected}"
            );
        }
    }

    /// `available: true` sem nenhum módulo de DSP por baixo é a ferramenta
    /// fantasma: GET /tools anuncia, o validador aceita, o áudio não muda.
    /// compression e dynamic_eq não têm implementação em audio_core::dsp
    /// (nenhum arquivo compressor/eq existe lá) — as duas têm que estar
    /// `available: false` com motivo, não `true`. As cinco ferramentas com
    /// DSP real (crossfade, fade_in, fade_out, time_stretch,
    /// lufs_normalization) continuam `true`.
    #[test]
    fn test_ghost_tools_are_marked_unavailable() {
        let reg = tool_registry();

        for name in ["compression", "dynamic_eq"] {
            let tool = find(&reg, name);
            assert!(
                !tool.available,
                "{name} não tem DSP mas está available: true"
            );
            assert!(
                tool.unavailable_reason.is_some(),
                "{name} está unavailable mas sem unavailable_reason"
            );
        }

        for name in [
            "crossfade",
            "fade_in",
            "fade_out",
            "time_stretch",
            "lufs_normalization",
        ] {
            let tool = find(&reg, name);
            assert!(
                tool.available,
                "{name} tem DSP real mas está available: false"
            );
            assert!(
                tool.unavailable_reason.is_none(),
                "{name} está available mas tem unavailable_reason"
            );
        }
    }

    #[test]
    fn test_dynamic_eq_type_filter_enum_matches_validator() {
        let reg = tool_registry();
        let type_filter = param(find(&reg, "dynamic_eq"), "bands[].type_filter");
        assert_eq!(type_filter.enum_values, Some(VALID_EQ_FILTER_TYPES));

        let layer = ValidationLayer::new();
        for valid in VALID_EQ_FILTER_TYPES {
            let tool = AudioToolDef::DynamicEq(DynamicEqParams {
                bands: vec![EqBand {
                    freq_hz: 1000.0,
                    gain_db: 0.0,
                    q: 0.7,
                    type_filter: valid.to_string(),
                }],
            });
            assert!(
                layer.validate_tool_call(&tool, &Value::Null).is_ok(),
                "{valid} deveria ser aceito"
            );
        }
    }

    /// Adendo R2 §0: crossfade (dois sinais somando) e fade_in/fade_out (um
    /// sinal de/para o silêncio) são conceitos diferentes — cada um expõe seu
    /// próprio enum no registry, e os dois vocabulários não se sobrepõem. Sem
    /// este teste, a UI (que lê só daqui) voltaria a oferecer "logarítmica"
    /// como opção de crossfade.
    #[test]
    fn test_crossfade_and_fade_curve_enums_are_distinct_in_registry() {
        let reg = tool_registry();
        let crossfade_curve = param(find(&reg, "crossfade"), "curve");
        let fade_in_curve = param(find(&reg, "fade_in"), "curve");
        let fade_out_curve = param(find(&reg, "fade_out"), "curve");

        assert_eq!(crossfade_curve.enum_values, Some(VALID_CROSSFADE_CURVES));
        assert_eq!(fade_in_curve.enum_values, Some(VALID_FADE_CURVES));
        assert_eq!(fade_out_curve.enum_values, Some(VALID_FADE_CURVES));

        for shared in VALID_CROSSFADE_CURVES {
            assert!(
                !VALID_FADE_CURVES.contains(shared),
                "{shared} não deveria valer para os dois vocabulários"
            );
        }

        assert_eq!(
            crossfade_curve.default,
            Some(serde_json::json!("constant_power"))
        );
        assert_eq!(
            fade_in_curve.default,
            Some(serde_json::json!("logarithmic"))
        );
        assert_eq!(
            fade_out_curve.default,
            Some(serde_json::json!("logarithmic"))
        );
    }

    /// Espelha `test_crossfade_duration_registry_matches_validator`: o
    /// registry e o validador precisam concordar nos dois vocabulários, não
    /// só num deles.
    #[test]
    fn test_crossfade_curve_registry_matches_validator() {
        let layer = ValidationLayer::new();
        for valid in VALID_CROSSFADE_CURVES {
            let tool = AudioToolDef::Crossfade(CrossfadeParams {
                duration_ms: 1000,
                curve: valid.to_string(),
            });
            assert!(
                layer.validate_tool_call(&tool, &Value::Null).is_ok(),
                "{valid} deveria ser aceito em crossfade"
            );
        }
        for invalid in VALID_FADE_CURVES {
            let tool = AudioToolDef::Crossfade(CrossfadeParams {
                duration_ms: 1000,
                curve: invalid.to_string(),
            });
            assert!(
                layer.validate_tool_call(&tool, &Value::Null).is_err(),
                "{invalid} não deveria ser aceito em crossfade"
            );
        }
    }
}
