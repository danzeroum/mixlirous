use crate::error::Error;
use serde::{Deserialize, Serialize};

/// Tempo de ataque do compressor, em milissegundos — validado na
/// desserialização, não numa camada por cima (`docs/16-CORRECOES-DSP` T0.0,
/// I14).
///
/// Mesmo padrão de [`crate::CrossfadeMs`] (ver o comentário lá para o
/// racional completo). `MIN`/`MAX` são a fonte canônica: `audio_agent::limits`
/// e `audio_agent::validator` leem daqui em vez de redigitar `0`/`500`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u32")]
pub struct AttackMs(u32);

impl AttackMs {
    pub const MIN: u32 = 0;
    pub const MAX: u32 = 500;

    pub fn get(self) -> u32 {
        self.0
    }
}

impl TryFrom<u32> for AttackMs {
    type Error = Error;

    fn try_from(v: u32) -> Result<Self, Self::Error> {
        (Self::MIN..=Self::MAX)
            .contains(&v)
            .then_some(Self(v))
            .ok_or_else(|| {
                Error::Validation(format!(
                    "attack_ms deve estar entre {} e {}; recebido {v}",
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
        assert!(AttackMs::try_from(AttackMs::MIN).is_ok());
        assert!(AttackMs::try_from(AttackMs::MAX).is_ok());
    }

    #[test]
    fn test_rejects_above_max() {
        assert!(AttackMs::try_from(AttackMs::MAX + 1).is_err());
        assert!(AttackMs::try_from(u32::MAX).is_err());
    }

    #[test]
    fn test_get_roundtrips_the_value() {
        let v = AttackMs::try_from(30).unwrap();
        assert_eq!(v.get(), 30);
    }

    #[test]
    fn test_deserialize_rejects_out_of_range_value() {
        let err = serde_json::from_str::<AttackMs>("50000").unwrap_err();
        assert!(err.to_string().contains("attack_ms"));
    }

    #[test]
    fn test_deserialize_accepts_in_range_value() {
        let v: AttackMs = serde_json::from_str("30").unwrap();
        assert_eq!(v.get(), 30);
    }

    #[test]
    fn test_serialize_is_transparent_as_the_inner_value() {
        let v = AttackMs::try_from(30).unwrap();
        assert_eq!(serde_json::to_string(&v).unwrap(), "30");
    }
}
