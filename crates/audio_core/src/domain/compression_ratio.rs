use crate::error::Error;
use serde::{Deserialize, Serialize};

/// Raz├úo de compress├úo (`N`:1) ÔÇö validada na desserializa├º├úo, n├úo numa
/// camada por cima (`docs/16-CORRECOES-DSP` T0.0, I14).
///
/// Mesmo padr├úo de [`crate::CrossfadeMs`] (ver o coment├írio l├í para o
/// racional completo de por que "sem `Default`, sem `From` infal├¡vel, sem
/// aritm├®tica" ├® o ponto, n├úo s├│ a valida├º├úo em si). `MIN`/`MAX` s├úo a fonte
/// can├┤nica: `audio_agent::limits` e `audio_agent::validator` leem daqui em
/// vez de redigitar `1.0`/`10.0`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "f32")]
pub struct CompressionRatio(f32);

impl CompressionRatio {
    pub const MIN: f32 = 1.0;
    pub const MAX: f32 = 10.0;

    pub fn get(self) -> f32 {
        self.0
    }
}

impl TryFrom<f32> for CompressionRatio {
    type Error = Error;

    fn try_from(v: f32) -> Result<Self, Self::Error> {
        (Self::MIN..=Self::MAX)
            .contains(&v)
            .then_some(Self(v))
            .ok_or_else(|| {
                Error::Validation(format!(
                    "compression_ratio deve estar entre {} e {}; recebido {v}",
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
        assert!(CompressionRatio::try_from(CompressionRatio::MIN).is_ok());
        assert!(CompressionRatio::try_from(CompressionRatio::MAX).is_ok());
    }

    #[test]
    fn test_rejects_outside_bounds() {
        assert!(CompressionRatio::try_from(CompressionRatio::MIN - 0.01).is_err());
        assert!(CompressionRatio::try_from(CompressionRatio::MAX + 0.01).is_err());
    }

    #[test]
    fn test_rejects_nan() {
        // (MIN..=MAX).contains(&NaN) ├® sempre false ÔÇö NaN nunca desserializa
        // com sucesso, sem checagem expl├¡cita ├á parte (I15, issue #22).
        assert!(CompressionRatio::try_from(f32::NAN).is_err());
    }

    #[test]
    fn test_get_roundtrips_the_value() {
        let v = CompressionRatio::try_from(4.0).unwrap();
        assert_eq!(v.get(), 4.0);
    }

    #[test]
    fn test_deserialize_rejects_out_of_range_value() {
        let err = serde_json::from_str::<CompressionRatio>("15.0").unwrap_err();
        assert!(err.to_string().contains("compression_ratio"));
    }

    #[test]
    fn test_deserialize_accepts_in_range_value() {
        let v: CompressionRatio = serde_json::from_str("2.0").unwrap();
        assert_eq!(v.get(), 2.0);
    }

    #[test]
    fn test_serialize_is_transparent_as_the_inner_value() {
        let v = CompressionRatio::try_from(2.0).unwrap();
        assert_eq!(serde_json::to_string(&v).unwrap(), "2.0");
    }
}
