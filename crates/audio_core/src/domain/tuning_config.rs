use serde::{Deserialize, Serialize};

/// Configuracao de afinacao (correcao de pitch).
/// Opt-in por padrao (enabled: false) — ver ADR-0012.
/// Model_path NAO fica aqui (e concern de AppConfig/infraestrutura).
///
/// Newtype para confianca minima de deteccao tonal (0.0..=1.0).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MinConfidence(f32);

impl MinConfidence {
    pub const MIN: f32 = 0.0;
    pub const MAX: f32 = 1.0;

    pub fn get(&self) -> f32 {
        self.0
    }
}

impl TryFrom<f32> for MinConfidence {
    type Error = String;

    fn try_from(v: f32) -> Result<Self, Self::Error> {
        if !(Self::MIN..=Self::MAX).contains(&v) {
            return Err(format!(
                "MinConfidence deve estar em {}..={}, got {}",
                Self::MIN,
                Self::MAX,
                v
            ));
        }
        Ok(Self(v))
    }
}

/// Newtype para correcao maxima em cents (-100..=100).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MaxCorrectionCents(f32);

impl MaxCorrectionCents {
    pub const MIN: f32 = -100.0;
    pub const MAX: f32 = 100.0;

    pub fn get(&self) -> f32 {
        self.0
    }
}

impl TryFrom<f32> for MaxCorrectionCents {
    type Error = String;

    fn try_from(v: f32) -> Result<Self, Self::Error> {
        if !(Self::MIN..=Self::MAX).contains(&v) {
            return Err(format!(
                "MaxCorrectionCents deve estar em {}..={}, got {}",
                Self::MIN,
                Self::MAX,
                v
            ));
        }
        Ok(Self(v))
    }
}

/// Modo de correcao de afinacao.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TuningMode {
    /// Nao corrige — ideal para comparacao A/B.
    Disabled,
    /// Analisa e reporta, mas nao modifica o audio.
    AnalyzeOnly,
    /// Correcao global: aplica uma unica correcao de drift.
    Global,
    /// Correcao seletiva por stem (futuro — B3).
    PerStem,
}

/// Configuracao completa de afinacao para o pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuningConfig {
    pub enabled: bool,
    pub mode: TuningMode,
    /// Correcao maxima global em cents. Default: 50.
    pub max_global_cents: MaxCorrectionCents,
    /// Confianca minima para aceitar deteccao de tonica. Default: 0.7.
    pub min_confidence: MinConfidence,
    /// Referencia de pitch (Hz). None = auto-detect. Default: None.
    pub force_tonic_hz: Option<f32>,
    /// Modo forcado (maior/menor). None = auto-detect. Default: None.
    pub force_mode: Option<String>,
}

impl Default for TuningConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: TuningMode::Disabled,
            max_global_cents: MaxCorrectionCents::try_from(50.0)
                .expect("50.0 esta dentro de MaxCorrectionCents::MIN..=MAX por construcao"),
            min_confidence: MinConfidence::try_from(0.7)
                .expect("0.7 esta dentro de MinConfidence::MIN..=MAX por construcao"),
            force_tonic_hz: None,
            force_mode: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_min_confidence_valid_range() {
        assert!(MinConfidence::try_from(0.0).is_ok());
        assert!(MinConfidence::try_from(1.0).is_ok());
        assert!(MinConfidence::try_from(-0.1).is_err());
        assert!(MinConfidence::try_from(1.1).is_err());
    }

    #[test]
    fn test_max_correction_cents_valid_range() {
        assert!(MaxCorrectionCents::try_from(-100.0).is_ok());
        assert!(MaxCorrectionCents::try_from(100.0).is_ok());
        assert!(MaxCorrectionCents::try_from(-101.0).is_err());
        assert!(MaxCorrectionCents::try_from(101.0).is_err());
    }

    #[test]
    fn test_tuning_config_default_is_disabled() {
        let cfg = TuningConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.mode, TuningMode::Disabled);
    }

    #[test]
    fn test_tuning_config_serialize_deserialize_roundtrip() {
        let cfg = TuningConfig {
            enabled: true,
            mode: TuningMode::Global,
            max_global_cents: MaxCorrectionCents::try_from(25.0).unwrap(),
            min_confidence: MinConfidence::try_from(0.85).unwrap(),
            force_tonic_hz: Some(440.0),
            force_mode: Some("maior".to_string()),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let deserialized: TuningConfig = serde_json::from_str(&json).unwrap();
        assert!(deserialized.enabled);
        assert_eq!(deserialized.mode, TuningMode::Global);
        assert!((deserialized.max_global_cents.get() - 25.0).abs() < f32::EPSILON);
        assert!((deserialized.min_confidence.get() - 0.85).abs() < f32::EPSILON);
        assert_eq!(deserialized.force_tonic_hz, Some(440.0));
        assert_eq!(deserialized.force_mode, Some("maior".to_string()));
    }
}
