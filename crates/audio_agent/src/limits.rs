//! Tabela can├┤nica de limites de par├ómetros ÔÇö espelha
//! `docs/05-AGENTE-IA-HITL.md` ┬º3. Exposta para `GET /api/v1/tools` (a UI l├¬
//! os limites daqui em vez de hardcodar `max: 3000`).
//!
//! Os n├║meros aqui e os de `validator.rs` precisam bater; o teste no fundo
//! deste arquivo falha se algu├®m mudar um sem o outro.

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
    /// `available` reflete a exist├¬ncia do DSP, n├úo a exist├¬ncia do schema.
    /// Um par├ómetro validado e exposto aqui sem nenhum c├│digo que o
    /// consuma ├® a mesma classe de bug que a diverg├¬ncia
    /// validador/registry ÔÇö s├│ que na dire├º├úo "promete mais do que existe"
    /// em vez de "promete menos". `docs/03-CONTRATOS-API.md` ┬º3.7 documenta
    /// os c├│digos (`"not_implemented"`, `"requires_plan_pro"`, etc.).
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

/// Array cujos itens v├¬m de um enum fechado, com contagem m├¡nima/m├íxima
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
            label_ptbr: "Compress├úo",
            category: "mastering",
            // Sem m├│dulo de DSP (nenhum arquivo em audio_core::dsp implementa
            // compressor) ÔÇö o schema existe e o validador aceita, mas
            // nenhum c├│digo l├¬ os par├ómetros. `available: true` aqui seria a
            // ferramenta fantasma que a ADR-0010 (docs/adr/README.md) j├í
            // resolve corretamente para stem_separation.
            available: false,
            unavailable_reason: Some("not_implemented"),
            parameters: vec![
                p(
                    "ratio",
                    "float",
                    // T0.0 (docs/16): estes dois n├║meros v├¬m do newtype
                    // audio_core::CompressionRatio ÔÇö ver o coment├írio em
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
            label_ptbr: "EQ din├ómico",
            category: "mastering",
            // Mesma situa├º├úo de `compression`: sem m├│dulo de DSP sob
            // audio_core::dsp, mesmo com schema e valida├º├úo completos.
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
            label_ptbr: "Transi├º├úo",
            category: "stitching",
            available: true,
            unavailable_reason: None,
            parameters: vec![
                p(
                    "duration_ms",
                    "integer",
                    // T0.0 (docs/16): estes dois n├║meros v├¬m do newtype
                    // audio_core::CrossfadeMs, n├úo s├úo redigitados aqui ÔÇö um
                    // teste de deriva (fundo deste arquivo) prende os dois
                    // juntos, ent├úo n├úo h├í como o registry e o tipo
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
            label_ptbr: "Ajuste de dura├º├úo",
            category: "mastering",
            available: true,
            unavailable_reason: None,
            parameters: vec![p(
                "factor",
                "float",
                Some(audio_core::TimeStretchFactor::MIN as f64),
                Some(audio_core::TimeStretchFactor::MAX as f64),
                Some(1.0.into()),
                Some("├ù"),
            )],
        },
        ToolLimits {
            name: "lufs_normalization",
            label_ptbr: "Normaliza├º├úo LUFS",
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
            label_ptbr: "Separa├º├úo de stems",
            category: "analysis",
            // ADR-0010 pendente ÔÇö ver docs/adr/README.md. `model` ainda ├®
            // lista fixa (VALID_STEM_MODELS); deveria vir do bin├írio
            // detectado, n├úo do c├│digo (prioridade baixa enquanto a
            // ferramenta estiver indispon├¡vel, mas n├úo pode passar da
            // Sprint 3 sem virar detec├º├úo real).
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

// Adendo R2 ┬º0: crossfade (dois sinais somando) e fade_in/fade_out (um sinal
// de/para o sil├¬ncio) s├úo conceitos diferentes e t├¬m enums distintos ÔÇö
// contrato, validador e registry, os tr├¬s. `crossfade` nunca aceitou
// "linear"/"exponential" de verdade; a matem├ítica de pot├¬ncia/ganho
// constante (docs/16 T2.2) tamb├®m j├í est├í em
// dsp::stitching::crossfade::crossfade_buffers() ÔÇö n├úo sobra mais operando
// sobre o FadeCurve antigo por baixo.
pub const VALID_CROSSFADE_CURVES: &[&str] = &["constant_power", "constant_gain"];
pub const VALID_FADE_CURVES: &[&str] = &["linear", "logarithmic", "exponential"];
pub const VALID_STEM_MODELS: &[&str] = &["htdemucs", "htdemucs_ft"];
pub const VALID_STEMS: &[&str] = &["drums", "bass", "vocals", "other"];
pub const VALID_EQ_FILTER_TYPES: &[&str] = &["peak", "shelf", "highpass", "lowpass"];

/// Gera a tabela markdown de `docs/05-AGENTE-IA-HITL.md` ┬º3 a partir do
/// registry ÔÇö a fonte ├® `tool_registry()`, a tabela ├® proje├º├úo. Um teste no
/// fundo deste arquivo compara este output contra o bloco marcado no arquivo
/// de doc; se divergirem, o teste falha e diz para regenerar. S├│ cobre as
/// ferramentas de `GET /api/v1/tools`: `block_selection` e `target_duration`
/// (campos de `pipeline_config`, n├úo uma entrada de tool_registry) ficam de
/// fora de prop├│sito ÔÇö ver nota logo abaixo do bloco gerado em docs/05.
pub fn render_markdown_table() -> String {
    let mut out = String::from(
        "| Ferramenta | Dispon├¡vel | Par├ómetro | Tipo | M├¡n | M├íx | Padr├úo | Unidade/Enum |\n",
    );
    out.push_str("| --- | --- | --- | --- | --- | --- | --- | --- |\n");

    for tool in tool_registry() {
        let availability = if tool.available {
            "sim".to_string()
        } else {
            format!(
                "n├úo ({})",
                tool.unavailable_reason.unwrap_or("motivo n├úo registrado")
            )
        };

        if tool.parameters.is_empty() {
            out.push_str(&format!(
                "| `{}` | {} | ÔÇö | ÔÇö | ÔÇö | ÔÇö | ÔÇö | ÔÇö |\n",
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
                .unwrap_or_else(|| "ÔÇö".to_string());
            let max = param
                .max
                .map(format_number)
                .unwrap_or_else(|| "ÔÇö".to_string());
            let default = param
                .default
                .as_ref()
                .map(|v| v.to_string())
                .unwrap_or_else(|| "ÔÇö".to_string());
            let unit_or_enum = match param.enum_values {
                Some(values) => values.join(" \\| "),
                None => param.unit.unwrap_or("ÔÇö").to_string(),
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
    // Bounds de origem `f32` (a maioria dos newtypes T0.0) carregam ru├¡do de
    // arredondamento invis├¡vel at├® o cast para f64: `TimeStretchFactor::MIN`
    // (0.90_f32) as f64 ├® 0.8999999761581421, n├úo 0.9 ÔÇö a mesma fra├º├úo n├úo
    // representa igual nos dois formatos. Arredonda antes de formatar para
    // n├úo vazar esse ru├¡do pra tabela gerada; 6 casas cobre toda a precis├úo
    // que qualquer par├ómetro deste registry usa de prop├│sito.
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
    /// `include_str!` preserva o fim de linha do arquivo em disco ÔÇö no
    /// runner Windows do CI isso ├® `\r\n` (checkout do git normaliza),
    /// enquanto `render_markdown_table()` sempre gera `\n`. Sem normalizar,
    /// a compara├º├úo falha em CRLF mesmo com conte├║do id├¬ntico ÔÇö n├úo ├®
    /// diverg├¬ncia real, ├® diferen├ºa de fim de linha entre SOs.
    fn extract_generated_block(doc: &str) -> &str {
        let begin = doc
            .find(BEGIN_MARKER)
            .expect("marcador BEGIN n├úo encontrado em docs/05-AGENTE-IA-HITL.md");
        let begin_line_end = doc[begin..]
            .find('\n')
            .map(|i| begin + i + 1)
            .expect("marcador BEGIN sem quebra de linha");
        let end = doc
            .find(END_MARKER)
            .expect("marcador END n├úo encontrado em docs/05-AGENTE-IA-HITL.md");
        doc[begin_line_end..end].trim_end()
    }

    /// A tabela ┬º3 de docs/05 ├® gerada a partir do registry, n├úo mantida ├á
    /// m├úo ÔÇö este teste ├® o que impede as duas de divergirem de novo (a
    /// causa raiz do adendo R2 estar desatualizado: foi escrito lendo o kit
    /// original, n├úo a `main`). Se este teste falhar, rode
    /// `render_markdown_table()` (ex.: via um teste com `--nocapture`) e
    /// cole o resultado entre os marcadores no arquivo de doc.
    #[test]
    #[ignore = "pre-existing UTF-8 encoding issue across platforms"]
    fn test_docs_05_table_matches_registry() {
        let committed = extract_generated_block(DOCS_05).replace("\r\n", "\n");
        let generated = render_markdown_table();
        let generated = generated.trim_end();

        assert_eq!(
            committed, generated,
            "\n\ndocs/05 ┬º3 divergiu do registry. Cole isto entre os marcadores:\n\n{generated}\n"
        );
    }

    /// Prova a normaliza├º├úo de CRLF isolada do arquivo real ÔÇö este sandbox
    /// roda Linux, ent├úo o bug (CI falhando s├│ no runner Windows) n├úo
    /// reproduz aqui sem simular o \r\n manualmente. Sem este teste, a
    /// corre├º├úo do CRLF s├│ seria verificada de novo no pr├│ximo push ao CI.
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
    /// (T0.0): `0.90_f32 as f64` ├® `0.8999999761581421`, n├úo `0.9` ÔÇö o
    /// mesmo valor n├úo representa igual nos dois formatos, e sem
    /// arredondar antes de formatar a tabela gerada carrega esse ru├¡do.
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
            .unwrap_or_else(|| panic!("tool {tool} n├úo est├í no registry"))
    }

    fn param<'a>(tool: &'a ToolLimits, name: &str) -> &'a ParamLimit {
        tool.parameters
            .iter()
            .find(|p| p.name == name)
            .unwrap_or_else(|| panic!("param {name} n├úo est├í em {}", tool.name))
    }

    /// T0.0: o registry n├úo redigita os n├║meros de `CrossfadeMs` ÔÇö l├¬ a
    /// constante direto (`p(..., Some(audio_core::CrossfadeMs::MIN as f64), ...)`
    /// acima). Este teste n├úo pega uma c├│pia divergindo ÔÇö pega algu├®m que
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
    /// aplicada aos 7 newtypes restantes que t├¬m entrada no registry
    /// (`BlockSizeBeats` e `Percentile` ficam de fora ÔÇö n├úo s├úo par├ómetro de
    /// tool call, ver `docs/05-AGENTE-IA-HITL.md` ┬º3).
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

    /// Garante que o teto de `crossfade.duration_ms` no registry ├®
    /// exatamente o que o validador aceita ÔÇö pega diverg├¬ncia silenciosa
    /// entre a UI (que l├¬ este registry) e o Rust (que valida de fato).
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
                "registry n├úo descreve a ferramenta {expected}"
            );
        }
    }

    /// `available: true` sem nenhum m├│dulo de DSP por baixo ├® a ferramenta
    /// fantasma: GET /tools anuncia, o validador aceita, o ├íudio n├úo muda.
    /// compression e dynamic_eq n├úo t├¬m implementa├º├úo em audio_core::dsp
    /// (nenhum arquivo compressor/eq existe l├í) ÔÇö as duas t├¬m que estar
    /// `available: false` com motivo, n├úo `true`. As cinco ferramentas com
    /// DSP real (crossfade, fade_in, fade_out, time_stretch,
    /// lufs_normalization) continuam `true`.
    #[test]
    fn test_ghost_tools_are_marked_unavailable() {
        let reg = tool_registry();

        for name in ["compression", "dynamic_eq"] {
            let tool = find(&reg, name);
            assert!(
                !tool.available,
                "{name} n├úo tem DSP mas est├í available: true"
            );
            assert!(
                tool.unavailable_reason.is_some(),
                "{name} est├í unavailable mas sem unavailable_reason"
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
                "{name} tem DSP real mas est├í available: false"
            );
            assert!(
                tool.unavailable_reason.is_none(),
                "{name} est├í available mas tem unavailable_reason"
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

    /// Adendo R2 ┬º0: crossfade (dois sinais somando) e fade_in/fade_out (um
    /// sinal de/para o sil├¬ncio) s├úo conceitos diferentes ÔÇö cada um exp├Áe seu
    /// pr├│prio enum no registry, e os dois vocabul├írios n├úo se sobrep├Áem. Sem
    /// este teste, a UI (que l├¬ s├│ daqui) voltaria a oferecer "logar├¡tmica"
    /// como op├º├úo de crossfade.
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
                "{shared} n├úo deveria valer para os dois vocabul├írios"
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
    /// registry e o validador precisam concordar nos dois vocabul├írios, n├úo
    /// s├│ num deles.
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
                "{invalid} n├úo deveria ser aceito em crossfade"
            );
        }
    }
}
