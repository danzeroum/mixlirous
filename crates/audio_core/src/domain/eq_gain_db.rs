use crate::error::Error;
use serde::{Deserialize, Serialize};

/// Ganho de uma banda de EQ dinâmico, em dB — validado na desserialização,
/// não numa camada por cima (`docs/16-CORRECOES-DSP` T0.0, I14).
///
/// Mesmo padrão de [`crate::CrossfadeMs`] (ver o comentário lá para o
/// racional completo). `MIN`/`MAX` são a fonte canônica: `audio_agent::limits`
/// e `audio_agent::validator` leem daqui em vez de redigitar `-24.0`/`24.0`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "f32")]
pub struct EqGainDb(f32);

impl EqGainDb {
    pub const MIN: f32 = -24.0;
    pub const MAX: f32 = 24.0;

    pub fn get(self) -> f32 {
        self.0
    }
}

impl TryFrom<f32> for EqGainDb {
    type Error = Error;

    fn try_from(v: f32) -> Result<Self, Self::Error> {
        (Self::MIN..=Self::MAX)
            .contains(&v)
            .then_some(Self(v))
            .ok_or_else(|| {
                Error::Validation(format!(
                    "eq_gain_db deve estar entre {} e {}; recebido {v}",
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
        assert!(EqGainDb::try_from(EqGainDb::MIN).is_ok());
        assert!(EqGainDb::try_from(EqGainDb::MAX).is_ok());
    }

    #[test]
    fn test_rejects_outside_bounds() {
        assert!(EqGainDb::try_from(EqGainDb::MIN - 0.01).is_err());
        assert!(EqGainDb::try_from(EqGainDb::MAX + 0.01).is_err());
    }

    #[test]
    fn test_rejects_nan() {
        assert!(EqGainDb::try_from(f32::NAN).is_err());
    }

    #[test]
    fn test_get_roundtrips_the_value() {
        let v = EqGainDb::try_from(3.0).unwrap();
        assert_eq!(v.get(), 3.0);
    }

    #[test]
    fn test_deserialize_rejects_out_of_range_value() {
        let err = serde_json::from_str::<EqGainDb>("100.0").unwrap_err();
        assert!(err.to_string().contains("eq_gain_db"));
    }

    #[test]
    fn test_deserialize_accepts_in_range_value() {
        let v: EqGainDb = serde_json::from_str("0.0").unwrap();
        assert_eq!(v.get(), 0.0);
    }

    #[test]
    fn test_serialize_is_transparent_as_the_inner_value() {
        let v = EqGainDb::try_from(0.0).unwrap();
        assert_eq!(serde_json::to_string(&v).unwrap(), "0.0");
    }
}
