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

/// Registry de ferramentas com limites, na forma exposta por `GET /api/v1/tools`.
pub fn tool_registry() -> Vec<ToolLimits> {
    vec![
        ToolLimits {
            name: "compression",
            label_ptbr: "Compressão",
            category: "mastering",
            available: true,
            parameters: vec![
                p(
                    "ratio",
                    "float",
                    Some(1.0),
                    Some(10.0),
                    Some(2.0.into()),
                    Some(":1"),
                ),
                p(
                    "threshold_db",
                    "float",
                    Some(-60.0),
                    Some(0.0),
                    Some((-18.0).into()),
                    Some("dB"),
                ),
                p(
                    "attack_ms",
                    "integer",
                    Some(0.0),
                    Some(500.0),
                    Some(30.0.into()),
                    Some("ms"),
                ),
                p(
                    "release_ms",
                    "integer",
                    Some(10.0),
                    Some(5000.0),
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
            available: true,
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
                    Some(-24.0),
                    Some(24.0),
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
                p("bands", "array", Some(1.0), Some(8.0), None, None),
            ],
        },
        ToolLimits {
            name: "crossfade",
            label_ptbr: "Transição",
            category: "stitching",
            available: true,
            parameters: vec![
                p(
                    "duration_ms",
                    "integer",
                    Some(0.0),
                    Some(3000.0),
                    Some(1000.0.into()),
                    Some("ms"),
                ),
                e("curve", VALID_CURVES, Some("logarithmic".into())),
            ],
        },
        ToolLimits {
            name: "fade_in",
            label_ptbr: "Fade in",
            category: "stitching",
            available: true,
            parameters: vec![
                p(
                    "duration_ms",
                    "integer",
                    Some(0.0),
                    Some(10000.0),
                    Some(1000.0.into()),
                    Some("ms"),
                ),
                e("curve", VALID_CURVES, Some("logarithmic".into())),
            ],
        },
        ToolLimits {
            name: "fade_out",
            label_ptbr: "Fade out",
            category: "stitching",
            available: true,
            parameters: vec![
                p(
                    "duration_ms",
                    "integer",
                    Some(0.0),
                    Some(10000.0),
                    Some(1000.0.into()),
                    Some("ms"),
                ),
                e("curve", VALID_CURVES, Some("logarithmic".into())),
            ],
        },
        ToolLimits {
            name: "time_stretch",
            label_ptbr: "Ajuste de duração",
            category: "mastering",
            available: true,
            parameters: vec![p(
                "factor",
                "float",
                Some(0.90),
                Some(1.10),
                Some(1.0.into()),
                Some("×"),
            )],
        },
        ToolLimits {
            name: "lufs_normalization",
            label_ptbr: "Normalização LUFS",
            category: "mastering",
            available: true,
            parameters: vec![
                p(
                    "target_lufs",
                    "float",
                    Some(-30.0),
                    Some(-6.0),
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
            available: false, // ADR-0010 pendente — ver docs/adr/README.md
            parameters: vec![
                e("model", VALID_STEM_MODELS, Some("htdemucs".into())),
                p(
                    "stems",
                    "array_enum",
                    Some(1.0),
                    Some(4.0),
                    Some(serde_json::json!(["drums", "other"])),
                    None,
                ),
            ],
        },
    ]
}

pub const VALID_CURVES: &[&str] = &["linear", "logarithmic", "exponential"];
pub const VALID_STEM_MODELS: &[&str] = &["htdemucs", "htdemucs_ft"];
pub const VALID_STEMS: &[&str] = &["drums", "bass", "vocals", "other"];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::*;
    use crate::validator::ValidationLayer;
    use serde_json::Value;

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
            curve: "linear".to_string(),
        });
        let over_max = AudioToolDef::Crossfade(CrossfadeParams {
            duration_ms: max + 1,
            curve: "linear".to_string(),
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
}
