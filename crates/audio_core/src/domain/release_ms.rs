use crate::error::Error;
use serde::{Deserialize, Serialize};

/// Tempo de release do compressor, em milissegundos ÔÇö validado na
/// desserializa├º├úo, n├úo numa camada por cima (`docs/16-CORRECOES-DSP` T0.0,
/// I14).
///
/// Mesmo padr├úo de [`crate::CrossfadeMs`] (ver o coment├írio l├í para o
/// racional completo). `MIN`/`MAX` s├úo a fonte can├┤nica: `audio_agent::limits`
/// e `audio_agent::validator` leem daqui em vez de redigitar `10`/`5000`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u32")]
pub struct ReleaseMs(u32);

impl ReleaseMs {
    pub const MIN: u32 = 10;
    pub const MAX: u32 = 5000;

    pub fn get(self) -> u32 {
        self.0
    }
}

impl TryFrom<u32> for ReleaseMs {
    type Error = Error;

    fn try_from(v: u32) -> Result<Self, Self::Error> {
        (Self::MIN..=Self::MAX)
            .contains(&v)
            .then_some(Self(v))
            .ok_or_else(|| {
                Error::Validation(format!(
                    "release_ms deve estar entre {} e {}; recebido {v}",
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
        assert!(ReleaseMs::try_from(ReleaseMs::MIN).is_ok());
        assert!(ReleaseMs::try_from(ReleaseMs::MAX).is_ok());
    }

    #[test]
    fn test_rejects_outside_bounds() {
        assert!(ReleaseMs::try_from(ReleaseMs::MIN - 1).is_err());
        assert!(ReleaseMs::try_from(ReleaseMs::MAX + 1).is_err());
    }

    #[test]
    fn test_get_roundtrips_the_value() {
        let v = ReleaseMs::try_from(250).unwrap();
        assert_eq!(v.get(), 250);
    }

    #[test]
    fn test_deserialize_rejects_out_of_range_value() {
        let err = serde_json::from_str::<ReleaseMs>("50000").unwrap_err();
        assert!(err.to_string().contains("release_ms"));
    }

    #[test]
    fn test_deserialize_accepts_in_range_value() {
        let v: ReleaseMs = serde_json::from_str("250").unwrap();
        assert_eq!(v.get(), 250);
    }

    #[test]
    fn test_serialize_is_transparent_as_the_inner_value() {
        let v = ReleaseMs::try_from(250).unwrap();
        assert_eq!(serde_json::to_string(&v).unwrap(), "250");
    }
}
