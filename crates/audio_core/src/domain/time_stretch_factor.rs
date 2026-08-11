use crate::error::Error;
use serde::{Deserialize, Serialize};

/// Fator de ajuste de dura├º├úo por `time_stretch` (1.0 = sem mudan├ºa) ÔÇö
/// validado na desserializa├º├úo, n├úo numa camada por cima
/// (`docs/16-CORRECOES-DSP` T0.0, I14).
///
/// Mesmo padr├úo de [`crate::CrossfadeMs`] (ver o coment├írio l├í para o
/// racional completo). `MIN`/`MAX` s├úo a fonte can├┤nica: `audio_agent::limits`
/// e `audio_agent::validator` leem daqui em vez de redigitar `0.90`/`1.10`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "f32")]
pub struct TimeStretchFactor(f32);

impl TimeStretchFactor {
    pub const MIN: f32 = 0.90;
    pub const MAX: f32 = 1.10;

    pub fn get(self) -> f32 {
        self.0
    }
}

impl TryFrom<f32> for TimeStretchFactor {
    type Error = Error;

    fn try_from(v: f32) -> Result<Self, Self::Error> {
        (Self::MIN..=Self::MAX)
            .contains(&v)
            .then_some(Self(v))
            .ok_or_else(|| {
                Error::Validation(format!(
                    "time_stretch_factor deve estar entre {} e {}; recebido {v}",
                    Self::MIN,
                    Self::MAX
                ))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_accepts_min_and_max_boundaries() {
        assert!(TimeStretchFactor::try_from(TimeStretchFactor::MIN).is_ok());
        assert!(TimeStretchFactor::try_from(TimeStretchFactor::MAX).is_ok());
    }

    #[test]
    fn test_rejects_outside_bounds() {
        assert!(TimeStretchFactor::try_from(TimeStretchFactor::MIN - 0.01).is_err());
        assert!(TimeStretchFactor::try_from(TimeStretchFactor::MAX + 0.01).is_err());
    }

    #[test]
    fn test_rejects_nan() {
        assert!(TimeStretchFactor::try_from(f32::NAN).is_err());
    }

    #[test]
    fn test_get_roundtrips_the_value() {
        let v = TimeStretchFactor::try_from(1.0).unwrap();
        assert_eq!(v.get(), 1.0);
    }

    #[test]
    fn test_deserialize_rejects_out_of_range_value() {
        let err = serde_json::from_str::<TimeStretchFactor>("1.5").unwrap_err();
        assert!(err.to_string().contains("time_stretch_factor"));
    }

    #[test]
    fn test_deserialize_accepts_in_range_value() {
        let v: TimeStretchFactor = serde_json::from_str("1.0").unwrap();
        assert_eq!(v.get(), 1.0);
    }

    #[test]
    fn test_serialize_is_transparent_as_the_inner_value() {
        let v = TimeStretchFactor::try_from(1.0).unwrap();
        assert_eq!(serde_json::to_string(&v).unwrap(), "1.0");
    }
}
