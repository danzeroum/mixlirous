use crate::error::Error;
use serde::{Deserialize, Serialize};

/// Alvo de normalização de loudness, em LUFS (tipicamente negativo) —
/// validado na desserialização, não numa camada por cima
/// (`docs/16-CORRECOES-DSP` T0.0, I14).
///
/// Mesmo padrão de [`crate::CrossfadeMs`] (ver o comentário lá para o
/// racional completo). `MIN`/`MAX` são a fonte canônica: `audio_agent::limits`
/// e `audio_agent::validator` leem daqui em vez de redigitar `-30.0`/`-6.0`.
/// Usado também em [`crate::MasteringConfig::lufs_target`] — o mesmo
/// invariante vale para um `PipelineConfig` reconstruído do banco quanto para
/// uma chamada de ferramenta do agente.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "f32")]
pub struct LufsTarget(f32);

impl LufsTarget {
    pub const MIN: f32 = -30.0;
    pub const MAX: f32 = -6.0;

    pub fn get(self) -> f32 {
        self.0
    }
}

impl TryFrom<f32> for LufsTarget {
    type Error = Error;

    fn try_from(v: f32) -> Result<Self, Self::Error> {
        (Self::MIN..=Self::MAX)
            .contains(&v)
            .then_some(Self(v))
            .ok_or_else(|| {
                Error::Validation(format!(
                    "lufs_target deve estar entre {} e {}; recebido {v}",
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
        assert!(LufsTarget::try_from(LufsTarget::MIN).is_ok());
        assert!(LufsTarget::try_from(LufsTarget::MAX).is_ok());
    }

    #[test]
    fn test_rejects_outside_bounds() {
        assert!(LufsTarget::try_from(LufsTarget::MIN - 0.01).is_err());
        assert!(LufsTarget::try_from(LufsTarget::MAX + 0.01).is_err());
    }

    #[test]
    fn test_rejects_nan() {
        assert!(LufsTarget::try_from(f32::NAN).is_err());
    }

    #[test]
    fn test_get_roundtrips_the_value() {
        let v = LufsTarget::try_from(-14.0).unwrap();
        assert_eq!(v.get(), -14.0);
    }

    #[test]
    fn test_deserialize_rejects_out_of_range_value() {
        let err = serde_json::from_str::<LufsTarget>("0.0").unwrap_err();
        assert!(err.to_string().contains("lufs_target"));
    }

    #[test]
    fn test_deserialize_accepts_in_range_value() {
        let v: LufsTarget = serde_json::from_str("-14.0").unwrap();
        assert_eq!(v.get(), -14.0);
    }

    #[test]
    fn test_serialize_is_transparent_as_the_inner_value() {
        let v = LufsTarget::try_from(-14.0).unwrap();
        assert_eq!(serde_json::to_string(&v).unwrap(), "-14.0");
    }
}
