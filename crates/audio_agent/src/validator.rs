use crate::limits::{
    VALID_CROSSFADE_CURVES, VALID_EQ_FILTER_TYPES, VALID_FADE_CURVES, VALID_STEMS,
    VALID_STEM_MODELS,
};
use crate::tools::AudioToolDef;
use serde_json::Value;
use thiserror::Error;

/// Camada de validação estrita que tipa e limita parâmetros de ferramentas.
///
/// Os limites abaixo espelham a tabela canônica de `docs/05-AGENTE-IA-HITL.md`
/// §3. Mudar um limite aqui exige atualizar a tabela e o schema exposto à UI
/// no mesmo PR (ver `CONTRIBUTING.md`).
pub struct ValidationLayer {}

impl Default for ValidationLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl ValidationLayer {
    pub fn new() -> Self {
        Self {}
    }

    /// Valida uma tool call antes da execução
    pub fn validate_tool_call(
        &self,
        tool: &AudioToolDef,
        _context: &Value,
    ) -> Result<AudioToolDef, ValidationError> {
        match tool {
            AudioToolDef::Compression(params) => {
                // T0.0 (docs/16, I14): os quatro limites abaixo não são
                // redigitados aqui — cada newtype é a checagem, igual ao
                // braço de Crossfade logo adiante. makeup_gain_db/knee_db
                // ficam de fora do lote de 9 (docs/04 não os lista) e
                // continuam via `bound()`.
                audio_core::CompressionRatio::try_from(params.ratio)
                    .map_err(|e| ValidationError::Bound(e.to_string()))?;
                audio_core::ThresholdDb::try_from(params.threshold_db)
                    .map_err(|e| ValidationError::Bound(e.to_string()))?;
                audio_core::AttackMs::try_from(params.attack_ms)
                    .map_err(|e| ValidationError::Bound(e.to_string()))?;
                audio_core::ReleaseMs::try_from(params.release_ms)
                    .map_err(|e| ValidationError::Bound(e.to_string()))?;
                bound(
                    "compression.makeup_gain_db",
                    params.makeup_gain_db,
                    -12.0,
                    12.0,
                )?;
                bound("compression.knee_db", params.knee_db, 0.0, 12.0)?;

                // R1: ataque não pode ser maior que o release
                if params.attack_ms > params.release_ms {
                    return Err(ValidationError::Rule(
                        "ataque não pode ser maior que o release".to_string(),
                    ));
                }
                // R2: ratio alto com threshold raso é compressão destrutiva
                if params.ratio >= 8.0 && params.threshold_db > -10.0 {
                    return Err(ValidationError::Rule(
                        "compressão destrutiva: ratio alto com threshold raso".to_string(),
                    ));
                }

                Ok(AudioToolDef::Compression(params.clone()))
            }
            AudioToolDef::DynamicEq(params) => {
                if params.bands.is_empty() || params.bands.len() > 8 {
                    return Err(ValidationError::Bound(
                        "dynamic_eq.bands deve ter entre 1 e 8 bandas".to_string(),
                    ));
                }
                for band in &params.bands {
                    bound("dynamic_eq.bands[].freq_hz", band.freq_hz, 20.0, 20000.0)?;
                    // T0.0 (docs/16, I14): não redigita -24.0/24.0 — checagem
                    // é audio_core::EqGainDb.
                    audio_core::EqGainDb::try_from(band.gain_db)
                        .map_err(|e| ValidationError::Bound(e.to_string()))?;
                    bound("dynamic_eq.bands[].q", band.q, 0.1, 10.0)?;
                    enum_value(
                        "dynamic_eq.bands[].type_filter",
                        &band.type_filter,
                        VALID_EQ_FILTER_TYPES,
                    )?;
                }
                // R4: bandas com freq_hz duplicada (±5%)
                for i in 0..params.bands.len() {
                    for j in (i + 1)..params.bands.len() {
                        let (a, b) = (params.bands[i].freq_hz, params.bands[j].freq_hz);
                        if a > 0.0 && (a - b).abs() / a <= 0.05 {
                            return Err(ValidationError::Rule(
                                "bandas de EQ sobrepostas".to_string(),
                            ));
                        }
                    }
                }
                Ok(AudioToolDef::DynamicEq(params.clone()))
            }
            AudioToolDef::Crossfade(params) => {
                // T0.0 (docs/16, I14): o limite não é redigitado aqui — o
                // newtype é a checagem. `bound()` comparava contra 0.0/3000.0
                // hardcoded, uma cópia manual do que audio_core::CrossfadeMs
                // já garante; a checagem duplicada é exatamente o "terceiro
                // lugar" que divergia sem ninguém perceber.
                audio_core::CrossfadeMs::try_from(params.duration_ms)
                    .map_err(|e| ValidationError::Bound(e.to_string()))?;
                enum_value("crossfade.curve", &params.curve, VALID_CROSSFADE_CURVES)?;
                Ok(AudioToolDef::Crossfade(params.clone()))
            }
            AudioToolDef::FadeIn(params) => {
                bound(
                    "fade_in.duration_ms",
                    params.duration_ms as f32,
                    0.0,
                    10000.0,
                )?;
                enum_value("fade_in.curve", &params.curve, VALID_FADE_CURVES)?;
                Ok(AudioToolDef::FadeIn(params.clone()))
            }
            AudioToolDef::FadeOut(params) => {
                bound(
                    "fade_out.duration_ms",
                    params.duration_ms as f32,
                    0.0,
                    10000.0,
                )?;
                enum_value("fade_out.curve", &params.curve, VALID_FADE_CURVES)?;
                Ok(AudioToolDef::FadeOut(params.clone()))
            }
            AudioToolDef::TimeStretch(params) => {
                // T0.0 (docs/16, I14): não redigita 0.90/1.10 — checagem é
                // audio_core::TimeStretchFactor.
                audio_core::TimeStretchFactor::try_from(params.factor)
                    .map_err(|e| ValidationError::Bound(e.to_string()))?;
                Ok(AudioToolDef::TimeStretch(params.clone()))
            }
            AudioToolDef::LufsNormalization(params) => {
                // T0.0 (docs/16, I14): não redigita -30.0/-6.0 — checagem é
                // audio_core::LufsTarget. max_true_peak_db fica de fora do
                // lote de 9 (docs/04 não o lista) e continua via `bound()`.
                audio_core::LufsTarget::try_from(params.target_lufs)
                    .map_err(|e| ValidationError::Bound(e.to_string()))?;
                bound(
                    "lufs_normalization.max_true_peak_db",
                    params.max_true_peak_db,
                    -6.0,
                    0.0,
                )?;
                Ok(AudioToolDef::LufsNormalization(params.clone()))
            }
            AudioToolDef::StemSeparation(params) => {
                enum_value("stem_separation.model", &params.model, VALID_STEM_MODELS)?;
                if params.stems.is_empty() || params.stems.len() > 4 {
                    return Err(ValidationError::Bound(
                        "stem_separation.stems deve ter entre 1 e 4 itens".to_string(),
                    ));
                }
                for stem in &params.stems {
                    enum_value("stem_separation.stems[]", stem, VALID_STEMS)?;
                }
                Ok(AudioToolDef::StemSeparation(params.clone()))
            }
        }
    }
}

fn bound(field: &str, value: f32, min: f32, max: f32) -> Result<(), ValidationError> {
    if value < min || value > max {
        Err(ValidationError::Bound(format!(
            "{field} deve estar entre {min} e {max}; recebido {value}"
        )))
    } else {
        Ok(())
    }
}

fn enum_value(field: &str, value: &str, allowed: &[&str]) -> Result<(), ValidationError> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(ValidationError::Bound(format!(
            "{field} deve ser um de {allowed:?}; recebido {value:?}"
        )))
    }
}

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Type mismatch: {0}")]
    Type(String),
    #[error("Bound violation: {0}")]
    Bound(String),
    #[error("Rule violation: {0}")]
    Rule(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::*;

    fn layer() -> ValidationLayer {
        ValidationLayer::new()
    }

    fn valid_compression() -> CompressionParams {
        CompressionParams {
            ratio: 2.0,
            threshold_db: -18.0,
            attack_ms: 30,
            release_ms: 250,
            makeup_gain_db: 0.0,
            knee_db: 6.0,
        }
    }

    #[test]
    fn test_compression_within_bounds_is_ok() {
        let tool = AudioToolDef::Compression(valid_compression());
        assert!(layer().validate_tool_call(&tool, &Value::Null).is_ok());
    }

    #[test]
    fn test_compression_ratio_out_of_bounds_is_rejected() {
        let mut params = valid_compression();
        params.ratio = 15.0;
        let tool = AudioToolDef::Compression(params);
        assert!(layer().validate_tool_call(&tool, &Value::Null).is_err());
    }

    #[test]
    fn test_compression_rule_r1_attack_gt_release_rejected() {
        let mut params = valid_compression();
        params.attack_ms = 300;
        params.release_ms = 100;
        let tool = AudioToolDef::Compression(params);
        let err = layer().validate_tool_call(&tool, &Value::Null).unwrap_err();
        assert!(matches!(err, ValidationError::Rule(_)));
    }

    #[test]
    fn test_compression_rule_r2_destructive_combo_rejected() {
        let mut params = valid_compression();
        params.ratio = 9.0;
        params.threshold_db = -5.0;
        let tool = AudioToolDef::Compression(params);
        let err = layer().validate_tool_call(&tool, &Value::Null).unwrap_err();
        assert!(matches!(err, ValidationError::Rule(_)));
    }

    #[test]
    fn test_dynamic_eq_overlapping_bands_rejected() {
        let tool = AudioToolDef::DynamicEq(DynamicEqParams {
            bands: vec![
                EqBand {
                    freq_hz: 1000.0,
                    gain_db: 3.0,
                    q: 0.7,
                    type_filter: "peak".to_string(),
                },
                EqBand {
                    freq_hz: 1010.0,
                    gain_db: -2.0,
                    q: 0.7,
                    type_filter: "peak".to_string(),
                },
            ],
        });
        assert!(layer().validate_tool_call(&tool, &Value::Null).is_err());
    }

    #[test]
    fn test_dynamic_eq_too_many_bands_rejected() {
        let bands = (0..9)
            .map(|i| EqBand {
                freq_hz: 100.0 * (i as f32 + 1.0),
                gain_db: 0.0,
                q: 0.7,
                type_filter: "peak".to_string(),
            })
            .collect();
        let tool = AudioToolDef::DynamicEq(DynamicEqParams { bands });
        assert!(layer().validate_tool_call(&tool, &Value::Null).is_err());
    }

    #[test]
    fn test_dynamic_eq_invalid_type_filter_rejected() {
        // type_filter era String livre — o validador nunca checava o valor.
        // Registry (limits.rs) já expõe o enum; o validador precisa
        // impor o mesmo, senão volta a divergência de sempre.
        let tool = AudioToolDef::DynamicEq(DynamicEqParams {
            bands: vec![EqBand {
                freq_hz: 1000.0,
                gain_db: 0.0,
                q: 0.7,
                type_filter: "notch".to_string(),
            }],
        });
        assert!(layer().validate_tool_call(&tool, &Value::Null).is_err());
    }

    #[test]
    fn test_crossfade_duration_over_canonical_max_rejected() {
        let tool = AudioToolDef::Crossfade(CrossfadeParams {
            duration_ms: 3001,
            curve: "constant_power".to_string(),
        });
        assert!(layer().validate_tool_call(&tool, &Value::Null).is_err());
    }

    #[test]
    fn test_crossfade_invalid_curve_rejected() {
        let tool = AudioToolDef::Crossfade(CrossfadeParams {
            duration_ms: 1000,
            curve: "bezier".to_string(),
        });
        assert!(layer().validate_tool_call(&tool, &Value::Null).is_err());
    }

    #[test]
    fn test_crossfade_rejects_fade_vocabulary() {
        // Adendo R2 §0: crossfade e fade_in/fade_out não compartilham mais
        // vocabulário de curva. "logarithmic" descreve fade, não crossfade.
        let tool = AudioToolDef::Crossfade(CrossfadeParams {
            duration_ms: 1000,
            curve: "logarithmic".to_string(),
        });
        assert!(layer().validate_tool_call(&tool, &Value::Null).is_err());
    }

    #[test]
    fn test_fade_in_rejects_crossfade_vocabulary() {
        let tool = AudioToolDef::FadeIn(FadeParams {
            duration_ms: 1000,
            curve: "constant_power".to_string(),
        });
        assert!(layer().validate_tool_call(&tool, &Value::Null).is_err());
    }

    #[test]
    fn test_fade_out_accepts_canonical_curves() {
        for curve in ["linear", "logarithmic", "exponential"] {
            let tool = AudioToolDef::FadeOut(FadeParams {
                duration_ms: 1000,
                curve: curve.to_string(),
            });
            assert!(
                layer().validate_tool_call(&tool, &Value::Null).is_ok(),
                "{curve} deveria ser aceito em fade_out"
            );
        }
    }

    #[test]
    fn test_crossfade_accepts_canonical_curves() {
        for curve in ["constant_power", "constant_gain"] {
            let tool = AudioToolDef::Crossfade(CrossfadeParams {
                duration_ms: 1000,
                curve: curve.to_string(),
            });
            assert!(
                layer().validate_tool_call(&tool, &Value::Null).is_ok(),
                "{curve} deveria ser aceito em crossfade"
            );
        }
    }

    #[test]
    fn test_time_stretch_within_canonical_bounds() {
        let tool = AudioToolDef::TimeStretch(TimeStretchParams { factor: 1.05 });
        assert!(layer().validate_tool_call(&tool, &Value::Null).is_ok());
    }

    #[test]
    fn test_time_stretch_outside_canonical_bounds_rejected() {
        // 1.5 passava no validador antigo (0.5..=2.0); o canônico é 0.90..=1.10
        let tool = AudioToolDef::TimeStretch(TimeStretchParams { factor: 1.5 });
        assert!(layer().validate_tool_call(&tool, &Value::Null).is_err());
    }

    #[test]
    fn test_stem_separation_unknown_model_rejected() {
        let tool = AudioToolDef::StemSeparation(StemSeparationParams {
            model: "spleeter".to_string(),
            stems: vec!["drums".to_string()],
        });
        assert!(layer().validate_tool_call(&tool, &Value::Null).is_err());
    }

    #[test]
    fn test_stem_separation_valid_is_ok() {
        let tool = AudioToolDef::StemSeparation(StemSeparationParams {
            model: "htdemucs".to_string(),
            stems: vec!["drums".to_string(), "other".to_string()],
        });
        assert!(layer().validate_tool_call(&tool, &Value::Null).is_ok());
    }

    #[test]
    fn test_lufs_normalization_within_bounds() {
        let tool = AudioToolDef::LufsNormalization(LufsNormalizationParams {
            target_lufs: -14.0,
            max_true_peak_db: -1.0,
        });
        assert!(layer().validate_tool_call(&tool, &Value::Null).is_ok());
    }

    #[test]
    fn test_lufs_normalization_target_out_of_bounds_rejected() {
        let tool = AudioToolDef::LufsNormalization(LufsNormalizationParams {
            target_lufs: -2.0,
            max_true_peak_db: -1.0,
        });
        assert!(layer().validate_tool_call(&tool, &Value::Null).is_err());
    }
}
